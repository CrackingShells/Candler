#![cfg(test)]
//! The refusal wire: the `refusal/1` record, the stable codes, the JSON pointers of parse errors
//! and the class of every field of the contract surfaces. Every expectation is read from a
//! hand-written file under `tests/vectors` or from `docs/contract/codes.toml`, never from the code
//! it checks.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use candler_schemas::evaluation::{PluginManifest, ProtocolVersion, SuiteFile};
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::Name;
use candler_schemas::refusal::{Refusal, RefusalCode, RefusalRecord, Side, catalog};
use serde_json::{Value, json};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vectors() -> PathBuf {
    crate_dir().join("tests").join("vectors")
}

fn schema_file(name: &str) -> Value {
    let path = crate_dir()
        .join("schema")
        .join(format!("{name}.schema.json"));
    let text = fs::read_to_string(path).expect("the committed schema exists");
    serde_json::from_str(&text).expect("the schema is JSON")
}

fn read_toml_file(path: &Path) -> toml::Table {
    let text = fs::read_to_string(path).expect("a readable file");
    toml::from_str(&text).expect("a TOML file")
}

fn codes_registry() -> toml::Table {
    read_toml_file(&crate_dir().join("../../docs/contract/codes.toml"))
}

/// Guards: C1 — a `refusal/1` record is the document an agent reads from `--error-format json`.
/// It round-trips to the same JSON value, validates against its committed schema, and the reader
/// and the schema both refuse every invalid record: an unknown field, a code that is a number, a
/// side outside the six, `causes` missing, a path and a file together, a path that is no pointer.
/// Oracle: `tests/vectors/refusals/records.json`, written by hand.
/// Bite: `code` serialised as an index (the round trip of every valid record goes red); a `side`
/// renamed in the type only (the side test below and the records go red).
#[test]
fn c1_a_refusal_record_round_trips_and_is_validated_by_its_schema() {
    let text = fs::read_to_string(vectors().join("refusals/records.json")).expect("the vectors");
    let all: Value = serde_json::from_str(&text).expect("the vectors are JSON");
    let validator = jsonschema::validator_for(&schema_file("refusal-record"))
        .expect("the refusal schema compiles");
    for case in all["valid"].as_array().expect("valid records") {
        let name = case["name"].as_str().expect("a name");
        let record = case["record"].clone();
        let read: RefusalRecord = serde_json::from_value(record.clone())
            .unwrap_or_else(|e| panic!("valid record `{name}` is refused: {e}"));
        assert_eq!(
            serde_json::to_value(&read).expect("serialises"),
            record,
            "valid record `{name}` does not round-trip"
        );
        assert!(validator.is_valid(&record), "the schema refuses `{name}`");
    }
    for case in all["invalid"].as_array().expect("invalid records") {
        let name = case["name"].as_str().expect("a name");
        let record = case["record"].clone();
        assert!(
            serde_json::from_value::<RefusalRecord>(record.clone()).is_err(),
            "the reader accepts invalid record `{name}`"
        );
        assert!(
            !validator.is_valid(&record),
            "the schema accepts invalid record `{name}`"
        );
    }
}

/// Guards: C1 — the six sides are written with the spellings the registry uses, and the code is
/// written as its text.
/// Oracle: the hand-written list below, which `codes.toml` and the README of the contract use.
/// Bite: a `side` name drifting from its registry spelling (`Environment` written `env`); `code`
/// serialised as an index.
#[test]
fn c1_sides_and_codes_are_written_as_the_registry_spells_them() {
    let spellings = [
        (Side::Plugin, "plugin"),
        (Side::Suite, "suite"),
        (Side::Experiment, "experiment"),
        (Side::Config, "config"),
        (Side::Environment, "environment"),
        (Side::Candler, "candler"),
    ];
    for (side, text) in spellings {
        assert_eq!(serde_json::to_value(side).expect("serialises"), json!(text));
        assert_eq!(side.as_str(), text);
        assert_eq!(side.to_string(), text);
    }
    let code = RefusalCode::new("experiment.path_overlap").expect("a code");
    assert_eq!(
        serde_json::to_value(&code).expect("serialises"),
        json!("experiment.path_overlap")
    );
    for bad in [
        "overlap",
        "Experiment.overlap",
        "experiment.",
        ".a",
        "a..b",
        "a.b-c",
        "a.b1",
        "1a.b",
        "a._",
    ] {
        let verdict = if bad == "a._" {
            "accepted by design"
        } else {
            "refused"
        };
        let accepted = RefusalCode::new(bad).is_ok();
        assert_eq!(accepted, bad == "a._", "`{bad}` must be {verdict}");
    }
}

/// A refusal is built from an error by the three methods of the trait and by `RefusalRecord::of`.
/// Guards: C1 — the record of an error carries its code, side and path, its message and the
/// messages of its causes, and a file or a path never both.
/// Oracle: the hand-written expectations below.
/// Bite: the pointer dropped from the record; the causes left empty.
#[test]
fn c1_the_record_of_an_error_carries_its_code_side_path_and_message() {
    let error = ExperimentFile::from_toml_str("schema = \"experiment/1\"\nreplicates = \"two\"\n")
        .expect_err("a replicates that is text");
    let record = RefusalRecord::of(&error);
    assert_eq!(record.code().as_str(), "experiment.document_invalid");
    assert_eq!(record.side(), Side::Experiment);
    assert_eq!(record.path(), Some("/replicates"));
    assert_eq!(record.file(), None);
    assert_eq!(record.message(), error.to_string());
    assert!(
        record
            .clone()
            .in_file(String::from("experiments/a.toml"))
            .is_err(),
        "a record with a path takes no file"
    );
    let moved = RefusalRecord::new(record.code().clone(), Side::Experiment, String::from("m"))
        .in_file(String::from("experiments/a.toml"))
        .expect("a file");
    assert!(moved.at_path(String::from("/a")).is_err());
}

/// The hand list of the codes whose side is `candler`: errors only Candler can raise. No other row
/// may be `candler`; an author's error is never blamed on Candler (A5).
const INTERNAL_CODES: [&str; 24] = [
    "manifest.git_commit_invalid",
    "manifest.field_unset",
    "manifest.list_unsorted",
    "manifest.list_empty",
    "manifest.sources_mismatch",
    "manifest.event_kind_known",
    "manifest.tree_entry_under_file",
    "manifest.tree_dir_not_empty",
    "protocol.field_unset",
    "outcome.field_unset",
    "outcome.number_not_finite",
    "format.digest_invalid",
    "format.instrument_digest_invalid",
    "format.name_token_invalid",
    "format.timestamp_invalid",
    "format.timestamp_instant_invalid",
    "format.run_key_invalid",
    "format.request_id_invalid",
    "format.job_id_invalid",
    "format.boot_id_invalid",
    "format.container_id_invalid",
    "catalog.field_unset",
    "catalog.extra_key_reserved",
    "catalog.field_null",
];

/// The variants of each error type the registry covers, counted by hand from the enums: each
/// variant has a row, and the wrapping ones are marked as delegating.
const VARIANT_COUNTS: [(&str, usize); 9] = [
    ("ExecutionError", 25),
    ("EvaluationError", 13),
    ("OutcomeError", 16),
    ("FormatError", 19),
    ("CanonicalError", 5),
    ("CatalogError", 6),
    ("ValuesError", 2),
    ("SchemaTagError", 1),
    ("VersionMismatch", 1),
];

/// The code pattern `^[a-z]+(\.[a-z_]+)+$`, written out without the code under test.
fn fits_code_pattern(text: &str) -> bool {
    let mut segments = text.split('.');
    let first = segments.next().unwrap_or_default();
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_lowercase()) {
        return false;
    }
    let rest: Vec<&str> = segments.collect();
    !rest.is_empty()
        && rest
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
}

/// Guards: C2 — every variant of the schemas' author-facing enums has a code that fits
/// `^[a-z]+(\.[a-z_]+)+$` and is equal, with its side, enum and variant, to its row in
/// `codes.toml`; every row of the registry is a variant of the catalogue; codes are unique; the side
/// is `candler` only for the hand list of internal codes; the wrapping variants own no row.
/// Oracle: `docs/contract/codes.toml`, written by hand, and the counts above.
/// Bite: two variants sharing a code; a code renamed in the table; a `codes.toml` row deleted; a
/// side changed in the table only.
#[test]
fn c2_codes_stay_stable_and_equal_their_registry_rows() {
    let registry = codes_registry();
    let rows = registry["schemas"]
        .as_table()
        .expect("the [schemas] section");
    let rows_in_catalog = catalog();

    let mut seen = BTreeSet::new();
    for row in rows_in_catalog.iter().filter(|r| !r.delegates) {
        assert!(fits_code_pattern(row.code), "`{}` is not a code", row.code);
        assert!(seen.insert(row.code), "code `{}` is used twice", row.code);
        let entry = rows
            .get(row.code)
            .and_then(toml::Value::as_table)
            .unwrap_or_else(|| panic!("`{}` has no row in codes.toml", row.code));
        assert_eq!(entry["enum"].as_str(), Some(row.enum_name), "{}", row.code);
        assert_eq!(entry["variant"].as_str(), Some(row.variant), "{}", row.code);
        assert_eq!(
            entry["side"].as_str(),
            Some(row.side.as_str()),
            "{}",
            row.code
        );
        assert!(!row.when.is_empty() && !row.at.is_empty(), "{}", row.code);
    }
    for code in rows.keys() {
        assert!(
            seen.contains(code.as_str()),
            "registry row `{code}` is no variant of the catalogue"
        );
    }
    let internal: BTreeSet<&str> = rows
        .iter()
        .filter(|(_, row)| row["side"].as_str() == Some("candler"))
        .map(|(code, _)| code.as_str())
        .collect();
    let expected: BTreeSet<&str> = INTERNAL_CODES.iter().copied().collect();
    assert_eq!(
        internal, expected,
        "only the hand list of internal codes is `candler`"
    );

    for row in rows_in_catalog.iter().filter(|r| r.delegates) {
        assert!(
            row.code.is_empty(),
            "{}::{} owns a code",
            row.enum_name,
            row.variant
        );
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &rows_in_catalog {
        *counts.entry(row.enum_name).or_default() += 1;
    }
    for (name, count) in VARIANT_COUNTS {
        assert_eq!(counts.get(name), Some(&count), "variants of {name}");
    }
    let delegating = rows_in_catalog.iter().filter(|r| r.delegates).count();
    assert_eq!(delegating, 6, "the wrapping variants, by hand");
}

/// Guards: C2 — the code and side an error reports are the registry's, through the public ways to
/// raise an error: the wrappers return the inner code and take the document's side.
/// Oracle: the hand-written (code, side) pairs below, each a row of `codes.toml`.
/// Bite: a wrapping variant that returns its own code; a side taken from the inner error.
#[test]
fn c2_errors_raised_through_the_public_api_report_their_registered_code() {
    let registry = codes_registry();
    let rows = registry["schemas"]
        .as_table()
        .expect("the [schemas] section");
    let check = |error: &dyn Refusal, code: &str, side: Side| {
        assert_eq!(error.code().as_str(), code);
        assert_eq!(error.side(), side);
        let row = rows
            .get(code)
            .and_then(toml::Value::as_table)
            .expect("a row");
        assert_eq!(row["side"].as_str(), Some(side.as_str()), "{code}");
    };
    check(
        &ExperimentFile::from_toml_str("= not toml").expect_err("not TOML"),
        "experiment.toml_invalid",
        Side::Experiment,
    );
    check(
        &ExperimentFile::from_toml_str("schema = \"experiment/2\"").expect_err("a tag"),
        "experiment.document_invalid",
        Side::Experiment,
    );
    check(
        &PluginManifest::from_toml_str(
            "schema = \"plugin-manifest/1\"\nname = \"p\"\nversion = \"1.0.0\"\nprotocol = { major = 1, minor = 0 }\ncommand = []\nenvironment = { kind = \"none\" }\n",
        )
        .expect_err("no command"),
        "plugin.command_empty",
        Side::Plugin,
    );
    check(
        &PluginManifest::from_toml_str("not = ").expect_err("not TOML"),
        "plugin.manifest_invalid",
        Side::Plugin,
    );
    check(
        &SuiteFile::from_toml_str("= not toml").expect_err("not TOML"),
        "suite.document_invalid",
        Side::Suite,
    );
    check(
        &SuiteFile::from_json_str("{").expect_err("not JSON"),
        "suite.json_invalid",
        Side::Suite,
    );
    let name = Name::from_str("two words").expect_err("a name with a space");
    check(&name, "format.name_invalid", Side::Config);
    let mismatch = ProtocolVersion::new(1, 0)
        .accepts(&ProtocolVersion::new(2, 0))
        .expect_err("a newer major");
    check(&mismatch, "protocol.version_mismatch", Side::Plugin);
}

#[derive(Debug)]
struct PointerCase {
    name: String,
    file: String,
    format: String,
    pointer: Option<String>,
    missing: Option<String>,
    unknown: bool,
    text: String,
}

fn pointer_cases() -> Vec<PointerCase> {
    let table = read_toml_file(&vectors().join("refusals/pointers.toml"));
    table["case"]
        .as_array()
        .expect("the cases")
        .iter()
        .map(|case| {
            let text = |key: &str| {
                case.get(key)
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            };
            PointerCase {
                name: text("name").expect("a name"),
                file: text("file").expect("a file kind"),
                format: text("format").expect("a format"),
                pointer: text("pointer"),
                missing: text("missing"),
                unknown: case
                    .get("unknown")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false),
                text: text("text").expect("a text"),
            }
        })
        .collect()
}

/// Guards: C3 — a malformed experiment file, suite or plug-in manifest is refused with the JSON
/// pointer of the field at fault: a wrong type, a wrong tag, an item of an array, a key holding `/`
/// or `~`; and with the pointer of the parent plus the field's name in the message for a missing
/// required field. The root has no pointer.
/// An unknown key is refused with the pointer of the key and the code `<surface>.unknown_field`,
/// in each of the three documents, at the top and in every closed table; a plug-in blames the
/// plug-in, a suite the suite.
/// Oracle: `tests/vectors/refusals/pointers.toml`, written by hand.
/// Bite: serde's message with no path (the pointer dropped, as before this change); a pointer that
/// names the parent of the field at fault; the attribute removed from a nested type.
#[test]
fn c3_parse_errors_carry_the_json_pointer_of_the_field_at_fault() {
    let cases = pointer_cases();
    assert!(cases.len() >= 20, "the oracle holds its cases");
    for case in cases {
        let error: Box<dyn Refusal> = match (case.file.as_str(), case.format.as_str()) {
            ("experiment", "toml") => {
                Box::new(ExperimentFile::from_toml_str(&case.text).expect_err(&case.name))
            }
            ("suite", "toml") => {
                Box::new(SuiteFile::from_toml_str(&case.text).expect_err(&case.name))
            }
            ("suite", "json") => {
                Box::new(SuiteFile::from_json_str(&case.text).expect_err(&case.name))
            }
            ("manifest", "toml") => {
                Box::new(PluginManifest::from_toml_str(&case.text).expect_err(&case.name))
            }
            other => panic!("`{}`: no reader for {other:?}", case.name),
        };
        let side = match case.file.as_str() {
            "experiment" => Side::Experiment,
            "suite" => Side::Suite,
            _ => Side::Plugin,
        };
        assert_eq!(
            error.side(),
            side,
            "{}: the side of `{}`",
            case.file,
            case.name
        );
        assert_eq!(
            error.code().as_str().ends_with(".unknown_field"),
            case.unknown,
            "{} {}: the code of `{}` is {}",
            case.file,
            case.format,
            case.name,
            error.code()
        );
        assert_eq!(
            error.path(),
            case.pointer,
            "{} {}: the pointer of `{}`",
            case.file,
            case.format,
            case.name
        );
        if let Some(missing) = &case.missing {
            let record = match (case.file.as_str(), case.format.as_str()) {
                ("experiment", _) => {
                    RefusalRecord::of(&ExperimentFile::from_toml_str(&case.text).expect_err("e"))
                }
                ("suite", "toml") => {
                    RefusalRecord::of(&SuiteFile::from_toml_str(&case.text).expect_err("e"))
                }
                ("suite", _) => {
                    RefusalRecord::of(&SuiteFile::from_json_str(&case.text).expect_err("e"))
                }
                _ => RefusalRecord::of(&PluginManifest::from_toml_str(&case.text).expect_err("e")),
            };
            assert!(
                record.message().contains(missing.as_str()),
                "`{}`: the message names `{missing}`: {}",
                case.name,
                record.message()
            );
        }
    }
}

/// The property tree of a schema file: every dotted path (`[]` is an array item) with whether its
/// parent requires it and the class it declares. A `$ref` is followed with its sibling keywords
/// merged; `oneOf`, `anyOf` and `allOf` are followed without a step in the path; a map-valued
/// property has no `properties` and is one field.
fn walk(
    node: &Value,
    root: &Value,
    prefix: &str,
    stack: &mut Vec<String>,
    out: &mut BTreeMap<String, (bool, Option<String>)>,
) {
    let mut node = node.clone();
    let mut pushed = false;
    if let Some(target) = node.get("$ref").and_then(Value::as_str) {
        let name = target.rsplit('/').next().expect("a ref name").to_owned();
        if stack.contains(&name) {
            return;
        }
        let mut merged = root["$defs"][name.as_str()].clone();
        if let (Some(into), Some(from)) = (merged.as_object_mut(), node.as_object()) {
            for (key, value) in from {
                if key != "$ref" {
                    into.insert(key.clone(), value.clone());
                }
            }
        }
        stack.push(name);
        pushed = true;
        node = merged;
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        for branch in node
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            walk(branch, root, prefix, stack, out);
        }
    }
    let required: BTreeSet<&str> = node
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if let Some(properties) = node.get("properties").and_then(Value::as_object) {
        for (name, sub) in properties {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            let declared = declared_class(sub, root);
            let entry = (required.contains(name.as_str()), declared);
            if let Some(previous) = out.insert(path.clone(), entry.clone()) {
                assert_eq!(previous, entry, "`{path}` is read two ways by two branches");
            }
            walk(sub, root, &path, stack, out);
        }
    }
    if let Some(items) = node.get("items").filter(|items| items.is_object()) {
        walk(items, root, &format!("{prefix}[]"), stack, out);
    }
    if pushed {
        stack.pop();
    }
}

/// The class a property declares with `x-candler-class`, beside its `$ref` or in it.
fn declared_class(property: &Value, root: &Value) -> Option<String> {
    let own = property.get("x-candler-class").and_then(Value::as_str);
    let referred = property
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|target| root["$defs"][target.rsplit('/').next()?].get("x-candler-class"))
        .and_then(Value::as_str);
    own.or(referred).map(str::to_owned)
}

const CLASS_FILES: [&str; 9] = [
    "plugin-manifest",
    "suite",
    "experiment",
    "outcome-record",
    "evaluator-describe-result",
    "evaluator-evaluate-params",
    "evaluator-message",
    "evaluator-notification",
    "evaluator-request-side",
];

/// Guards: C4 — every property of the nine schema files of the six surfaces has exactly one class:
/// `candler` or `free` when the schema declares it with `x-candler-class`, otherwise `required`
/// when its parent lists it and `optional` when it does not. The class equals the hand table's, and
/// no property lacks a row or has a row of its own that names no property.
/// Oracle: `tests/vectors/field_classes.toml`, classified by hand against the frame of the pages.
/// Bite: a declared class that contradicts the hand table (a `free` on a `candler` field); a
/// property of the nine files with no row in the table.
#[test]
fn c4_every_property_has_exactly_one_class_equal_to_the_hand_table() {
    let table = read_toml_file(&vectors().join("field_classes.toml"));
    for file in CLASS_FILES {
        let root = schema_file(file);
        let mut found = BTreeMap::new();
        walk(&root, &root, "", &mut Vec::new(), &mut found);
        let rows = table
            .get(file)
            .and_then(toml::Value::as_table)
            .unwrap_or_else(|| panic!("`{file}` has no table of classes"));
        for (path, (required, declared)) in &found {
            let class = match declared.as_deref() {
                Some(class @ ("candler" | "free")) => class,
                Some(other) => panic!("{file}: `{path}` declares the class `{other}`"),
                None if *required => "required",
                None => "optional",
            };
            let expected = rows
                .get(path.as_str())
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| panic!("{file}: property `{path}` has no row in the table"));
            assert_eq!(class, expected, "{file}: the class of `{path}`");
        }
        for path in rows.keys() {
            assert!(
                found.contains_key(path.as_str()),
                "{file}: row `{path}` names no property"
            );
        }
    }
}

/// Guards: C4 — the sample rows the contract names are in the table with their class: the schema
/// tag of the three author-written documents is `required`, a budget cap is `optional`, a treatment
/// name is `required`, a field Candler writes is `candler` and a stored label is `free`.
/// Oracle: the hand-written (file, path, class) triples below.
/// Bite: a sample row edited in the table; `schema` classed `candler`.
#[test]
fn c4_the_sample_rows_hold() {
    let table = read_toml_file(&vectors().join("field_classes.toml"));
    let class = |file: &str, path: &str| {
        table[file][path]
            .as_str()
            .unwrap_or_else(|| panic!("{file}: `{path}` has no row"))
            .to_owned()
    };
    for file in ["plugin-manifest", "suite", "experiment"] {
        assert_eq!(class(file, "schema"), "required", "{file}");
    }
    assert_eq!(class("experiment", "budget.max_parallel"), "optional");
    assert_eq!(class("experiment", "treatment.name"), "required");
    assert_eq!(class("evaluator-evaluate-params", "scratch"), "candler");
    assert_eq!(class("suite", "expectations[].group"), "free");
}

/// Guards: C3 — the tables an author fills freely stay open: a suite's `source`, an entry's
/// `params`, and a factor's levels and `fixed` accept any key, while the closed tables around them
/// refuse one.
/// Oracle: the hand-written suite below, which holds unknown keys only inside open tables.
/// Bite: `source` removed from the suite's properties (the suite is refused at `/source`).
#[test]
fn c3_open_tables_accept_any_key() {
    let text = "schema = \"suite/1\"\n\n[source]\nref = \"x@1\"\nanything = { nested = [1, 2] }\n\n[[expectations]]\nid = \"A1\"\nrole = \"gate\"\nkind = \"run_script\"\ndepends_on = []\n[expectations.evaluator]\nname = \"rubric\"\nrange = \"^0.4\"\n[expectations.params]\nwhatever = true\n";
    let suite = SuiteFile::from_toml_str(text).expect("open tables accept any key");
    assert_eq!(
        suite.source().and_then(|table| table.get("ref")),
        Some(&json!("x@1"))
    );
}

/// Plants `zzz_unknown` in every table of `value` that is not open, one table at a time, and
/// returns the document and the pointer the refusal must give for each.
fn plantings(
    value: &toml::Value,
    pointer: &str,
    open: &[&str],
    out: &mut Vec<(toml::Value, String)>,
    root: &toml::Value,
    steps: &[String],
) {
    match value {
        toml::Value::Table(table) => {
            let mut planted = root.clone();
            let mut cursor = &mut planted;
            for step in steps {
                cursor = match cursor {
                    toml::Value::Table(t) => t.get_mut(step.as_str()).expect("a table"),
                    toml::Value::Array(a) => a
                        .get_mut(step.parse::<usize>().expect("an index"))
                        .expect("an item"),
                    scalar @ (toml::Value::String(_)
                    | toml::Value::Integer(_)
                    | toml::Value::Float(_)
                    | toml::Value::Boolean(_)
                    | toml::Value::Datetime(_)) => scalar,
                };
            }
            if let toml::Value::Table(t) = cursor {
                t.insert(String::from("zzz_unknown"), toml::Value::Integer(1));
            }
            out.push((planted, format!("{pointer}/zzz_unknown")));
            for (key, inner) in table {
                if open.contains(&key.as_str()) {
                    continue;
                }
                let mut next = steps.to_vec();
                next.push(key.clone());
                plantings(inner, &format!("{pointer}/{key}"), open, out, root, &next);
            }
        }
        toml::Value::Array(items) => {
            for (index, inner) in items.iter().enumerate() {
                let mut next = steps.to_vec();
                next.push(index.to_string());
                plantings(inner, &format!("{pointer}/{index}"), open, out, root, &next);
            }
        }
        toml::Value::String(_)
        | toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
}

/// Boxes a refusal.
fn boxed<E: Refusal + 'static>(error: E) -> Box<dyn Refusal> {
    Box::new(error)
}

const FULL_EXPERIMENT: &str = r#"schema = "experiment/1"
replicates = 2
secrets = ["API_KEY"]

[fixed]
"plan.model" = "m"

[[factors]]
path = "plan.skill"
levels = ["a", "b"]

[budget]
max_cost_usd = 5.0
max_parallel = 2
max_runs = 10
max_tokens = 1000
wall_s = 600

[budget.per_run]
max_cost_usd = 1.0
turns = 20
wall_s = 60

[harness]
name = "claude-code"
version = "1.0.0"

[treatment]
name = "t"
tag = "v1"
commit = "0123456789abcdef0123456789abcdef01234567"
source = "skills/t"

[[inputs]]
kind = "files"
path = "data"
at = "data"
audience = "agent"
"#;

const FULL_SUITE: &str = r#"schema = "suite/1"

[source]
ref = "x@1"

[[expectations]]
id = "A1"
group = "g"
role = "gate"
kind = "run_script"
depends_on = []
deadline_ms = 1000

[[expectations.applies_to]]
path = "plan.model"
any_of = ["m"]

[expectations.evaluator]
name = "rubric"
range = "^0.4"

[expectations.params]
script = "check.py"
"#;

const FULL_MANIFEST_UV: &str = r#"schema = "plugin-manifest/1"
name = "rubric"
version = "0.4.2"
command = ["python", "run.py"]

[protocol]
major = 1
minor = 0

[environment]
kind = "uv"
project = "py"
lock = "py/uv.lock"
"#;

const FULL_MANIFEST_NONE: &str = r#"schema = "plugin-manifest/1"
name = "rubric"
version = "0.4.2"
command = ["python", "run.py"]

[protocol]
major = 1
minor = 0

[environment]
kind = "none"
"#;

/// Guards: C3 — the differential over whole documents: a full valid experiment file, suite and
/// manifest (both environments) is read; then one unknown key is planted in every table that is
/// not open, one at a time, and each plant is refused with the code `<surface>.unknown_field` and
/// the pointer of the planted key. The open tables (`fixed`, a factor's levels, a suite's `source`
/// and an entry's `params`) are skipped.
/// Oracle: the hand-written documents above; the pointers are built from the table paths.
/// Bite: the attribute removed from any nested type; the key check removed from the environment.
#[test]
fn c3_an_unknown_key_in_every_closed_table_is_refused_at_its_pointer() {
    let documents = [
        ("experiment", FULL_EXPERIMENT, vec!["fixed", "levels"]),
        ("suite", FULL_SUITE, vec!["source", "params"]),
        ("manifest", FULL_MANIFEST_UV, vec![]),
        ("manifest", FULL_MANIFEST_NONE, vec![]),
    ];
    for (kind, text, open) in documents {
        let read = |text: &str| -> Result<(), Box<dyn Refusal>> {
            match kind {
                "experiment" => ExperimentFile::from_toml_str(text)
                    .map(|_| ())
                    .map_err(boxed),
                "suite" => SuiteFile::from_toml_str(text).map(|_| ()).map_err(boxed),
                _ => PluginManifest::from_toml_str(text)
                    .map(|_| ())
                    .map_err(boxed),
            }
        };
        assert!(read(text).is_ok(), "the full {kind} is valid");
        let root: toml::Value = toml::from_str(text).expect("TOML");
        let mut planted = Vec::new();
        plantings(&root, "", &open, &mut planted, &root, &[]);
        assert!(planted.len() >= 3, "{kind}: tables to plant in");
        for (document, pointer) in planted {
            let changed = toml::to_string(&document).expect("TOML text");
            let Err(error) = read(changed.as_str()) else {
                panic!("{kind}: the unknown key at `{pointer}` is accepted:\n{changed}");
            };
            assert!(
                error.code().as_str().ends_with(".unknown_field"),
                "{kind} `{pointer}`: {}",
                error.code()
            );
            assert_eq!(error.path().as_deref(), Some(pointer.as_str()), "{kind}");
        }
    }
}
