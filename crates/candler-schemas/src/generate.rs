//! Generation of the committed JSON Schema files and the check that keeps them in step with the
//! structs they describe.
//!
//! Every wire root is generated under one configuration: JSON Schema draft 2020-12, optional
//! fields without a `null` type, `$id` set to `urn:candler:schemas:<name>`, shared definitions
//! under `$defs` of the same file, and no crate version anywhere in the output.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use schemars::generate::SchemaSettings;
use schemars::transform::{Transform, transform_subschemas};
use schemars::{JsonSchema, Schema};
use serde_json::{Map, Value, json};

/// The extension every generated schema file carries.
const FILE_SUFFIX: &str = ".schema.json";

/// The prefix of every schema's `$id`.
const ID_PREFIX: &str = "urn:candler:schemas:";

/// One generated root schema and the file it is committed to.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedSchema {
    /// The file name inside the schema directory: `<name>.schema.json`.
    pub file_name: String,
    /// The root schema, with its `$id` set.
    pub schema: Schema,
}

impl GeneratedSchema {
    /// Generates the root schema of `T` under the one generator configuration.
    ///
    /// The file is named `<name>.schema.json` and the schema's `$id` is
    /// `urn:candler:schemas:<name>`.
    #[must_use]
    pub fn of<T: JsonSchema>(name: &str) -> Self {
        let mut schema = schema_settings()
            .into_generator()
            .into_root_schema_for::<T>();
        // BORROW: .to_owned() makes the owned key `Schema::insert` takes
        schema.insert("$id".to_owned(), Value::from(format!("{ID_PREFIX}{name}")));
        Self {
            file_name: format!("{name}{FILE_SUFFIX}"),
            schema,
        }
    }

    /// The committed text of the schema: 2-space-indented JSON with keys in sorted order and a
    /// trailing newline.
    #[must_use]
    pub fn render(&self) -> String {
        format!("{:#}\n", self.schema.as_value())
    }
}

/// Every wire root schema, in file-name order.
#[must_use]
pub fn generated_schemas() -> Vec<GeneratedSchema> {
    in_file_name_order(Vec::new())
}

/// Sorts roots by file name, the order the committed files are listed in.
fn in_file_name_order(mut roots: Vec<GeneratedSchema>) -> Vec<GeneratedSchema> {
    roots.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    roots
}

/// The generator configuration shared by every root.
///
/// It targets JSON Schema draft 2020-12 and strips the `null` alternative that `Option` adds to a
/// schema, since an absent optional field is omitted and never written as `null`.
#[must_use]
pub fn schema_settings() -> SchemaSettings {
    SchemaSettings::draft2020_12().with_transform(OmitNull)
}

/// Removes `null` as an accepted type and `null` as a default.
#[derive(Debug, Clone, Copy)]
struct OmitNull;

impl Transform for OmitNull {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);
        if let Some(object) = schema.as_object_mut() {
            omit_null_default(object);
            omit_null_type(object);
            omit_null_alternative(object);
        }
    }
}

fn omit_null_default(object: &mut Map<String, Value>) {
    if object.get("default").is_some_and(Value::is_null) {
        object.remove("default");
    }
}

/// Turns `"type": ["string", "null"]` into `"type": "string"`.
fn omit_null_type(object: &mut Map<String, Value>) {
    let mut only = None;
    if let Some(Value::Array(types)) = object.get_mut("type")
        && types.len() > 1
    {
        types.retain(|t| t != "null");
        if types.len() == 1 {
            only = types.pop();
        }
    }
    if let Some(only) = only {
        // BORROW: .to_owned() makes the owned key `Map::insert` takes
        object.insert("type".to_owned(), only);
    }
}

/// Turns `"anyOf": [X, {"type": "null"}]` into `X`, keeping the keywords beside it.
fn omit_null_alternative(object: &mut Map<String, Value>) {
    let null_schema = json!({"type": "null"});
    let mut only = None;
    if let Some(Value::Array(alternatives)) = object.get_mut("anyOf") {
        alternatives.retain(|alternative| *alternative != null_schema);
        if alternatives.len() == 1 {
            only = alternatives.pop();
        }
    }
    if let Some(only) = only {
        object.remove("anyOf");
        if let Value::Object(fields) = only {
            for (key, value) in fields {
                object.entry(key).or_insert(value);
            }
        }
    }
}

/// What was being done to a schema file or its directory when it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaFileAction {
    /// Reading a committed file.
    Read,
    /// Creating the schema directory.
    Create,
    /// Writing a generated file.
    Write,
    /// Listing the schema directory.
    List,
}

impl fmt::Display for SchemaFileAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Create => "create",
            Self::Write => "write",
            Self::List => "list",
        })
    }
}

/// Why a committed schema file could not be checked or written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaFileError {
    /// The file or its directory could not be read, listed or written.
    #[error("failed to {action} {}: {source}", .path.display())]
    Io {
        /// What was attempted.
        action: SchemaFileAction,
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying failure.
        source: io::Error,
    },
    /// The committed file does not exist.
    #[error("schema file {} is missing (run with CANDLER_BLESS_SCHEMAS=1 to write it)", .path.display())]
    Missing {
        /// The expected location of the file.
        path: PathBuf,
    },
    /// The committed file differs from the generated text.
    #[error("schema file {} differs from its generated form (first difference at line {line}; run with CANDLER_BLESS_SCHEMAS=1 to rewrite it)", .path.display())]
    Drift {
        /// The committed file.
        path: PathBuf,
        /// The first line, counted from 1, that differs.
        line: usize,
    },
}

impl SchemaFileError {
    fn io(action: SchemaFileAction, path: &Path, source: io::Error) -> Self {
        Self::Io {
            action,
            // BORROW: .to_owned() makes the owned path the error holds
            path: path.to_owned(),
            source,
        }
    }
}

/// Compares the generated text of `root` with the file it is committed to in `dir`, or, when
/// `bless` is set, writes the generated text there instead.
///
/// Blessing creates `dir` if it is missing and leaves a file that already matches untouched.
///
/// # Errors
/// Returns [`SchemaFileError::Missing`] if the file does not exist.
/// Returns [`SchemaFileError::Drift`] if the file differs from the generated text.
/// Returns [`SchemaFileError::Io`] if the file cannot be read, or the directory or file cannot be
/// written.
pub fn check_or_bless(
    root: &GeneratedSchema,
    dir: &Path,
    bless: bool,
) -> Result<(), SchemaFileError> {
    let path = dir.join(&root.file_name);
    let generated = root.render();
    let committed = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(SchemaFileError::io(SchemaFileAction::Read, &path, e)),
    };
    if bless {
        // BORROW: explicit .as_str() instead of Deref coercion
        if committed.as_deref() == Some(generated.as_str()) {
            return Ok(());
        }
        fs::create_dir_all(dir)
            .map_err(|e| SchemaFileError::io(SchemaFileAction::Create, dir, e))?;
        return fs::write(&path, generated)
            .map_err(|e| SchemaFileError::io(SchemaFileAction::Write, &path, e));
    }
    match committed {
        None => Err(SchemaFileError::Missing { path }),
        Some(text) if text == generated => Ok(()),
        Some(text) => Err(SchemaFileError::Drift {
            line: first_difference(&text, &generated),
            path,
        }),
    }
}

/// Lists the schema files in `dir` that no root generates, in name order.
///
/// A missing directory holds none. Files that do not end in `.schema.json` are not schema files.
///
/// # Errors
/// Returns [`SchemaFileError::Io`] if the directory cannot be listed.
pub fn unexpected_files(
    roots: &[GeneratedSchema],
    dir: &Path,
) -> Result<Vec<String>, SchemaFileError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(SchemaFileError::io(SchemaFileAction::List, dir, e)),
    };
    let mut names = entries
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<String>, io::Error>>()
        .map_err(|e| SchemaFileError::io(SchemaFileAction::List, dir, e))?;
    names.retain(|name| {
        name.ends_with(FILE_SUFFIX) && !roots.iter().any(|root| root.file_name == *name)
    });
    names.sort_unstable();
    Ok(names)
}

/// The 1-based number of the first line at which two texts differ.
fn first_difference(left: &str, right: &str) -> usize {
    left.lines()
        .zip(right.lines())
        .position(|(l, r)| l != r)
        .unwrap_or_else(|| left.lines().count().min(right.lines().count()))
        + 1
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::primitives::{Digest, RunKey};

    /// A root used only by the tests.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    #[schemars(deny_unknown_fields)]
    struct Probe {
        /// Always present.
        digest: Digest,
        /// Present only when known.
        note: Option<String>,
        /// Present only when the run is known.
        run: Option<RunKey>,
        /// Present only when the digest is known.
        #[serde(default)]
        extra: Option<Digest>,
    }

    fn probe() -> GeneratedSchema {
        GeneratedSchema::of::<Probe>("probe")
    }

    /// Reports whether a schema still allows `null`: a `null` type, a `null` default or constant.
    fn mentions_null(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().any(|(key, inner)| {
                let here = match key.as_str() {
                    "type" => {
                        inner == "null"
                            || inner
                                .as_array()
                                .is_some_and(|types| types.iter().any(|t| t == "null"))
                    }
                    "default" | "const" => inner.is_null(),
                    _ => false,
                };
                here || mentions_null(inner)
            }),
            Value::Array(items) => items.iter().any(mentions_null),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
        }
    }

    #[test]
    fn null_detection_sees_every_shape_the_generator_removes() {
        assert!(mentions_null(&json!({"type": ["string", "null"]})));
        assert!(mentions_null(
            &json!({"anyOf": [{"$ref": "#/$defs/A"}, {"type": "null"}]})
        ));
        assert!(mentions_null(
            &json!({"properties": {"a": {"default": null}}})
        ));
        assert!(!mentions_null(
            &json!({"type": "string", "properties": {"type": {}}})
        ));
    }

    #[test]
    fn generated_root_names_its_file_and_id() {
        let root = probe();
        assert_eq!(root.file_name, "probe.schema.json");
        assert_eq!(
            root.schema.get("$id"),
            Some(&json!("urn:candler:schemas:probe"))
        );
        assert_eq!(
            root.schema.get("$schema"),
            Some(&json!("https://json-schema.org/draft/2020-12/schema"))
        );
    }

    #[test]
    fn generated_schema_has_no_null_type_and_leaves_options_optional() {
        let root = probe().schema.to_value();
        assert!(!mentions_null(&root), "a null remains in:\n{root:#}");
        assert_eq!(root["required"], json!(["digest"]));
        assert_eq!(root["properties"]["note"]["type"], json!("string"));
        assert_eq!(root["properties"]["run"]["$ref"], json!("#/$defs/RunKey"));
        assert_eq!(root["properties"]["extra"]["$ref"], json!("#/$defs/Digest"));
        assert_eq!(root["additionalProperties"], json!(false));
        assert!(root["$defs"]["Digest"]["pattern"].is_string());
    }

    #[test]
    fn generated_schema_validates_what_a_writer_emits() {
        let root = probe().schema.to_value();
        let schema = jsonschema::validator_for(&root).expect("the schema compiles");
        let digest = format!("sha256:{}", "ab".repeat(32));
        assert!(schema.is_valid(&json!({"digest": digest})));
        assert!(schema.is_valid(&json!({"digest": digest, "note": "x"})));
        assert!(!schema.is_valid(&json!({"digest": digest, "note": null})));
        assert!(!schema.is_valid(&json!({"digest": digest, "unknown": 1})));
        assert!(!schema.is_valid(&json!({})));
    }

    #[test]
    fn rendered_text_is_two_space_indented_sorted_and_newline_terminated() {
        let text = probe().render();
        assert!(text.ends_with("}\n"));
        assert!(text.starts_with("{\n  \"$defs\": {\n    \"Digest\": {\n"));
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.strip_prefix("  \""))
            .filter_map(|rest| rest.split_once('"').map(|(key, _)| key))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
        let again = GeneratedSchema::of::<Probe>("probe").render();
        assert_eq!(text, again);
    }

    #[test]
    fn rendered_text_carries_no_crate_version() {
        let text = probe().render();
        assert!(!text.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn drift_check_passes_after_a_bless_fails_after_a_mutation_and_passes_after_another_bless() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let root = probe();
        let file = dir.path().join(&root.file_name);

        assert!(matches!(
            check_or_bless(&root, dir.path(), false),
            Err(SchemaFileError::Missing { .. })
        ));

        check_or_bless(&root, dir.path(), true).expect("fresh bless writes the file");
        assert_eq!(
            fs::read_to_string(&file).expect("file exists"),
            root.render()
        );
        check_or_bless(&root, dir.path(), false).expect("a blessed file has no drift");

        let mut text = fs::read_to_string(&file).expect("file exists");
        text = text.replacen("\"digest\"", "\"digests\"", 1);
        fs::write(&file, text).expect("mutate the file");
        let error = check_or_bless(&root, dir.path(), false).expect_err("a mutated file drifts");
        assert!(matches!(error, SchemaFileError::Drift { line, .. } if line > 1));

        check_or_bless(&root, dir.path(), true).expect("second bless rewrites the file");
        check_or_bless(&root, dir.path(), false).expect("no drift after the second bless");
        assert_eq!(
            fs::read_to_string(&file).expect("file exists"),
            root.render()
        );
    }

    #[test]
    fn drift_check_detects_a_changed_struct_through_the_committed_text() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Probe {
            /// Always present.
            digest: Digest,
        }
        let dir = tempfile::tempdir().expect("temporary directory");
        check_or_bless(&probe(), dir.path(), true).expect("bless");
        let narrower = GeneratedSchema::of::<Probe>("probe");
        assert!(matches!(
            check_or_bless(&narrower, dir.path(), false),
            Err(SchemaFileError::Drift { .. })
        ));
    }

    #[test]
    fn drift_errors_say_how_to_regenerate() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let missing = check_or_bless(&probe(), dir.path(), false).expect_err("no file yet");
        assert!(missing.to_string().contains("CANDLER_BLESS_SCHEMAS=1"));
        check_or_bless(&probe(), dir.path(), true).expect("bless");
        fs::write(dir.path().join("probe.schema.json"), "{}\n").expect("mutate");
        let drift = check_or_bless(&probe(), dir.path(), false).expect_err("drifted");
        assert!(drift.to_string().contains("CANDLER_BLESS_SCHEMAS=1"));
    }

    #[test]
    fn blessing_leaves_a_matching_file_untouched() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let root = probe();
        check_or_bless(&root, dir.path(), true).expect("bless");
        let file = dir.path().join(&root.file_name);
        let before = fs::metadata(&file)
            .and_then(|m| m.modified())
            .expect("mtime");
        check_or_bless(&root, dir.path(), true).expect("bless again");
        let after = fs::metadata(&file)
            .and_then(|m| m.modified())
            .expect("mtime");
        assert_eq!(before, after);
    }

    #[test]
    fn drift_check_reports_a_directory_it_cannot_use() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let blocker = dir.path().join("file");
        fs::write(&blocker, "").expect("file");
        let result = check_or_bless(&probe(), &blocker.join("schema"), true);
        assert!(matches!(
            result,
            Err(SchemaFileError::Io {
                action: SchemaFileAction::Read,
                ..
            })
        ));
    }

    #[test]
    fn unexpected_files_lists_schema_files_no_root_generates() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let roots = [probe()];
        let none: Vec<String> = Vec::new();
        assert_eq!(unexpected_files(&roots, dir.path()).expect("list"), none);
        check_or_bless(&roots[0], dir.path(), true).expect("bless");
        fs::write(dir.path().join(".gitkeep"), "").expect("keep file");
        fs::write(dir.path().join("README.md"), "").expect("other file");
        assert_eq!(unexpected_files(&roots, dir.path()).expect("list"), none);
        fs::write(dir.path().join("stale.schema.json"), "{}\n").expect("stale file");
        fs::write(dir.path().join("another.schema.json"), "{}\n").expect("stale file");
        assert_eq!(
            unexpected_files(&roots, dir.path()).expect("list"),
            ["another.schema.json", "stale.schema.json"]
        );
        let missing = dir.path().join("absent");
        assert_eq!(unexpected_files(&roots, &missing).expect("list"), none);
    }

    #[test]
    fn serde_json_keeps_keys_sorted() {
        assert_eq!(json!({"b": 1, "a": 2}).to_string(), r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn roots_are_listed_in_file_name_order() {
        let unsorted = vec![
            GeneratedSchema::of::<Probe>("zeta"),
            GeneratedSchema::of::<Probe>("alpha"),
            GeneratedSchema::of::<Probe>("mid"),
        ];
        let names: Vec<String> = in_file_name_order(unsorted)
            .into_iter()
            .map(|root| root.file_name)
            .collect();
        assert_eq!(
            names,
            ["alpha.schema.json", "mid.schema.json", "zeta.schema.json"]
        );
    }

    #[test]
    fn the_shared_list_is_in_order_with_unique_file_names() {
        let names: Vec<String> = generated_schemas()
            .into_iter()
            .map(|r| r.file_name)
            .collect();
        let mut ordered = names.clone();
        ordered.sort_unstable();
        assert_eq!(names, ordered);
        ordered.dedup();
        assert_eq!(names.len(), ordered.len());
        assert!(names.iter().all(|n| n.ends_with(FILE_SUFFIX)));
    }
}
