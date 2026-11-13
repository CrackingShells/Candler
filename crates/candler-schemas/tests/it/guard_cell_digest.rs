#![cfg(test)]
//! The cell digest: every path of the manifest is classified as hashed or outside the hash, the
//! digests equal those an independent implementation computed, and the lists of a cell are
//! sorted and free of duplicates on the wire.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use candler_schemas::catalog::CatalogSnapshot;
use candler_schemas::execution::{
    Comparable, Deviation, DigestRef, EnvironmentRef, GitCommit, HarnessIdentity, ImportRecord,
    Integrity, RunManifest, SecretPresence, TreatmentRef,
};
use candler_schemas::primitives::{
    Digest, FieldPath, JobId, Name, NameToken, NameVersion, RunKey, Timestamp,
};
use candler_schemas::values::{JsonValue, MeasuredValue, ResolvedValue, SourceEntry, ValueSource};
use schemars::schema_for;
use serde::Deserialize;
use serde_json::{Value, json};

/// The text of a vector file of the independent oracle.
fn oracle(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-core/tests/oracle/vectors")
        .join(name);
    std::fs::read_to_string(path).expect("oracle vector file")
}

#[derive(Deserialize)]
struct VectorFile {
    vector: Vec<Vector>,
}

/// One row of `cell_digest.toml`: JSON texts and the digest an independent implementation gave.
#[derive(Deserialize)]
struct Vector {
    cell: String,
    comparable: String,
    digest: String,
    shuffled: String,
    outside: String,
    outside_twin: String,
}

fn vectors() -> Vec<Vector> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file: VectorFile = toml::from_str(oracle("cell_digest.toml").as_str()).expect("toml");
    file.vector
}

/// The parts of a cell, read one by one from JSON text.
struct Parts {
    values: Vec<ResolvedValue>,
    harness: HarnessIdentity,
    treatment: TreatmentRef,
    snapshots: Vec<DigestRef>,
    inputs: Vec<DigestRef>,
    environments: Vec<EnvironmentRef>,
}

fn field<T: for<'de> Deserialize<'de>>(doc: &Value, name: &str) -> T {
    // BORROW: .cloned() hands the part to the reader by value
    serde_json::from_value(doc.get(name).cloned().expect("part present")).expect("part reads")
}

fn parts(text: &str) -> Parts {
    let doc: Value = serde_json::from_str(text).expect("json");
    Parts {
        values: field(&doc, "values"),
        harness: field(&doc, "harness"),
        treatment: field(&doc, "treatment"),
        snapshots: field(&doc, "snapshots"),
        inputs: field(&doc, "inputs"),
        environments: field(&doc, "environments"),
    }
}

/// Builds a cell from its parts through the builder, which sorts every list.
fn rebuild(parts: Parts) -> Comparable {
    let mut builder = Comparable::builder()
        .harness(parts.harness)
        .treatment(parts.treatment);
    for value in parts.values {
        builder = builder.value(value);
    }
    for entry in parts.snapshots {
        builder = builder.snapshot(entry);
    }
    for entry in parts.inputs {
        builder = builder.input(entry);
    }
    for entry in parts.environments {
        builder = builder.environment(entry);
    }
    builder.build().expect("the parts build")
}

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

fn digest(c: char) -> Digest {
    Digest::from_str(digest_text(c).as_str()).expect("digest") // BORROW: explicit .as_str()
}

/// The typed roots `comparable` holds that need a source: the harness, the treatment and each non-empty list.
fn root_paths(comparable: &Comparable) -> Vec<&'static str> {
    let mut roots = vec!["harness", "treatment"];
    if !comparable.snapshots().is_empty() {
        roots.push("snapshots");
    }
    if !comparable.inputs().is_empty() {
        roots.push("inputs");
    }
    if !comparable.environments().is_empty() {
        roots.push("environments");
    }
    roots
}

/// A manifest of `comparable` whose every value and typed root has the source `source`.
fn manifest(comparable: Comparable, source: &ValueSource) -> RunManifest {
    let mut builder = RunManifest::builder()
        .run(RunKey::new(comparable.digest(), 1, 1).expect("run key"))
        .experiment(digest('0'))
        .integrity(Integrity::Valid);
    for value in comparable.values() {
        // BORROW: .clone() gives each entry its own path and source
        builder = builder.source(SourceEntry::new(value.path().clone(), source.clone()));
    }
    for root in root_paths(&comparable) {
        // BORROW: .clone() gives each entry its own source
        builder = builder.source(SourceEntry::new(
            FieldPath::from_str(root).expect("root path"),
            source.clone(),
        ));
    }
    builder.comparable(comparable).build().expect("manifest")
}

/// Every property path of a schema, joined by `.`, with `[]` for the items of an array.
fn property_paths(schema: &Value) -> BTreeSet<String> {
    fn walk(root: &Value, schema: &Value, prefix: &str, depth: u32, out: &mut BTreeSet<String>) {
        assert!(depth < 40, "the schema recurses without end");
        let target = schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix("#/$defs/"))
            .and_then(|name| root.get("$defs").and_then(|defs| defs.get(name)));
        if let Some(target) = target {
            walk(root, target, prefix, depth + 1, out);
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, inner) in properties {
                let path = if prefix.is_empty() {
                    // BORROW: .clone() makes the owned path
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                // BORROW: .clone() keeps the path for the walk below it
                out.insert(path.clone());
                // BORROW: explicit .as_str() instead of Deref coercion
                walk(root, inner, path.as_str(), depth + 1, out);
            }
        }
        if let Some(items) = schema.get("items") {
            // BORROW: explicit .as_str() instead of Deref coercion
            walk(root, items, format!("{prefix}[]").as_str(), depth + 1, out);
        }
        for keyword in ["oneOf", "anyOf", "allOf"] {
            for inner in schema
                .get(keyword)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                walk(root, inner, prefix, depth + 1, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(schema, schema, "", 0, &mut out);
    out
}

#[derive(Deserialize)]
struct Table {
    path: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    class: String,
}

/// Reports whether the table entry `name` covers `path`: it is the path or lies above it.
fn covers(name: &str, path: &str) -> bool {
    path == name
        || path
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with("[]"))
}

/// Guards: Cond 1 — every path of the manifest schema is classified `hashed` or `nuisance` in a table written by hand from R01, and a change to each hashed part changes the cell digest. Oracle: `tests/vectors/manifest/hashed_paths.toml`, from R01 §Run manifest.
#[test]
fn every_manifest_path_is_classified_and_hashed_parts_move_the_digest() {
    let table_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/manifest/hashed_paths.toml");
    let text = std::fs::read_to_string(table_path).expect("table");
    // BORROW: explicit .as_str() instead of Deref coercion
    let table: Table = toml::from_str(text.as_str()).expect("table reads");
    for entry in &table.path {
        assert!(
            // BORROW: explicit .as_str() instead of Deref coercion
            matches!(entry.class.as_str(), "hashed" | "nuisance"),
            "class of {}",
            entry.name
        );
    }
    let schema = serde_json::to_value(schema_for!(RunManifest)).expect("schema");
    let paths = property_paths(&schema);
    assert!(
        paths.contains("comparable.values[].path"),
        "walker reaches the values"
    );
    assert!(
        paths.contains("integrity.reason"),
        "walker reaches a oneOf branch"
    );
    let unclassified: Vec<&String> = paths
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .filter(|path| {
            // a container is classified by the entries below it
            let below = format!("{path}.");
            !table.path.iter().any(|e| {
                // BORROW: explicit .as_str() instead of Deref coercion
                covers(e.name.as_str(), path.as_str()) || e.name.starts_with(below.as_str())
            })
        })
        .collect();
    assert!(unclassified.is_empty(), "unclassified: {unclassified:?}");
    for entry in &table.path {
        // BORROW: explicit .as_str() instead of Deref coercion
        let live = paths
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .any(|path| covers(entry.name.as_str(), path.as_str()));
        assert!(
            live,
            "table entry {} names no path of the schema",
            entry.name
        );
    }

    // Each hashed entry, changed in a cell, changes the digest; the table lists exactly these.
    let first = vectors().into_iter().next().expect("a vector");
    let base = rebuild(parts(first.comparable.as_str())).digest(); // BORROW: explicit .as_str()
    let hashed: BTreeSet<&str> = table
        .path
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .filter(|e| e.class == "hashed")
        .map(|e| e.name.as_str())
        .collect();
    let mut changed = BTreeSet::new();
    for name in &hashed {
        let mut cell = parts(first.comparable.as_str()); // BORROW: explicit .as_str()
        match *name {
            "comparable.values" => cell.values.push(ResolvedValue::recorded(
                FieldPath::from_str("plan.zz_extra").expect("path"),
                JsonValue::new(json!(1)).expect("value"),
            )),
            "comparable.harness" => {
                cell.harness.name = Name::from_str("another-harness").expect("name");
            }
            "comparable.treatment" => cell.treatment.tag = Some(String::from("changed")),
            "comparable.snapshots" => cell.snapshots.push(DigestRef::new(
                Name::from_str("catalog").expect("name"),
                digest('e'),
            )),
            "comparable.inputs" => cell.inputs.push(DigestRef::new(
                Name::from_str("script").expect("name"),
                digest('e'),
            )),
            "comparable.environments" => cell.environments.push(EnvironmentRef::new(
                NameVersion::from_str("env-plugin@1.0.0").expect("plugin"),
                digest('e'),
            )),
            other => panic!("hashed entry {other} has no perturbation in this test"),
        }
        let moved = rebuild(cell).digest();
        assert_ne!(moved, base, "changing {name} must change the digest");
        changed.insert(*name);
    }
    assert_eq!(changed, hashed);
}

/// Guards: Cond 1 — the cell digest equals `sha256` of the RFC 8785 form that an independent implementation computed, whether the cell is read, built or taken from a manifest. Oracle: `cell_digest.toml` (Python `rfc8785`, engine named in its header).
#[test]
fn digests_equal_the_oracle() {
    let all = vectors();
    assert_eq!(all.len(), 6, "three corpus cells and three invented ones");
    for vector in &all {
        // BORROW: explicit .as_str() instead of Deref coercion
        let read: Comparable = serde_json::from_str(vector.comparable.as_str()).expect("reads");
        assert_eq!(
            // BORROW: explicit .as_str() instead of Deref coercion
            read.digest().as_str(),
            vector.digest,
            "read, {}",
            vector.cell
        );
        // BORROW: explicit .as_str() instead of Deref coercion
        let built = rebuild(parts(vector.comparable.as_str()));
        assert_eq!(
            // BORROW: explicit .as_str() instead of Deref coercion
            built.digest().as_str(),
            vector.digest,
            "built, {}",
            vector.cell
        );
        assert_eq!(
            built, read,
            "the builder and the reader agree, {}",
            vector.cell
        );
        let held = manifest(read, &ValueSource::ExperimentFile);
        assert_eq!(
            // BORROW: explicit .as_str() instead of Deref coercion
            held.cell_digest().as_str(),
            vector.digest,
            "manifest, {}",
            vector.cell
        );
    }
}

/// Guards: Cond 1 — a cell whose lists are written in another order gives the same digest once built. Oracle: the `shuffled` twin of each vector in `cell_digest.toml`, shuffled by the generator.
#[test]
fn shuffled_twins_give_the_same_digest() {
    let mut differing = 0_usize;
    for vector in vectors() {
        if vector.shuffled != vector.comparable {
            differing += 1;
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        let built = rebuild(parts(vector.shuffled.as_str()));
        // BORROW: explicit .as_str() instead of Deref coercion
        assert_eq!(built.digest().as_str(), vector.digest, "{}", vector.cell);
    }
    assert!(differing >= 4, "the twins must really be shuffled");
}

/// What lies outside the hash in an oracle vector.
struct Outside {
    source: ValueSource,
    secrets: Vec<SecretPresence>,
    measured: Vec<MeasuredValue>,
    started_at: Timestamp,
    ended_at: Timestamp,
    host: String,
}

fn outside(text: &str) -> Outside {
    let doc: Value = serde_json::from_str(text).expect("json");
    let sources: Vec<Value> = field(&doc, "sources");
    let first = sources.first().expect("a source");
    Outside {
        source: field(first, "source"),
        secrets: field(&doc, "secrets"),
        measured: field(&doc, "measured"),
        started_at: field(&doc, "started_at"),
        ended_at: field(&doc, "ended_at"),
        host: field(&doc, "host"),
    }
}

/// Builds a manifest of `comparable` with the fields outside the hash that `outside` gives.
fn with_outside(comparable: Comparable, outside: Outside, extras: bool) -> RunManifest {
    let mut builder = RunManifest::builder()
        .run(
            RunKey::new(
                comparable.digest(),
                if extras { 2 } else { 1 },
                if extras { 3 } else { 1 },
            )
            .expect("run key"),
        )
        .experiment(digest(if extras { '1' } else { '0' }))
        .integrity(if extras {
            serde_json::from_value(json!({"kind": "invalid", "reason": "killed"}))
                .expect("integrity")
        } else {
            Integrity::Valid
        })
        .started_at(outside.started_at)
        .ended_at(outside.ended_at)
        .host(outside.host);
    for value in comparable.values() {
        // BORROW: .clone() gives each entry its own path and source
        builder = builder.source(SourceEntry::new(
            value.path().clone(),
            outside.source.clone(),
        ));
    }
    for root in root_paths(&comparable) {
        // BORROW: .clone() gives each entry its own source
        builder = builder.source(SourceEntry::new(
            FieldPath::from_str(root).expect("root path"),
            outside.source.clone(),
        ));
    }
    for secret in outside.secrets {
        builder = builder.secret(secret);
    }
    for measured in outside.measured {
        builder = builder.measured(measured);
    }
    if extras {
        builder = builder
            .job(JobId::from_str("018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d").expect("job"))
            .factor(FieldPath::from_str("plan.model").expect("path"))
            .deviation(Deviation::new(
                FieldPath::from_str("plan.runs").expect("path"),
                Some(JsonValue::new(json!(3)).expect("value")),
                None,
            ))
            .import(ImportRecord::new(
                NameToken::from_str("campaign-import@v1").expect("format"),
                GitCommit::from_str("0123456789abcdef0123456789abcdef01234567").expect("commit"),
                JsonValue::new(json!({"valid": true})).expect("value"),
            ));
    }
    builder.comparable(comparable).build().expect("manifest")
}

/// Guards: Cond 1 — two manifests that differ in every field outside the hash (sources, secrets, measured values, times, host, job, run key, experiment, integrity, deviations, import) have the same cell digest. Oracle: the `outside` and `outside_twin` fields of `cell_digest.toml`, which carry the same digest.
#[test]
fn nuisance_twins_give_the_same_digest() {
    let mut secrets_differ = 0_usize;
    for vector in vectors() {
        // BORROW: explicit .as_str() instead of Deref coercion
        let cell = rebuild(parts(vector.comparable.as_str()));
        // BORROW: .clone() gives the twin its own cell
        let plain = with_outside(cell.clone(), outside(vector.outside.as_str()), false);
        // BORROW: explicit .as_str() instead of Deref coercion
        let twin = with_outside(cell, outside(vector.outside_twin.as_str()), true);
        assert_ne!(plain, twin, "the manifests differ, {}", vector.cell);
        if plain.sources().is_empty() {
            assert_eq!(plain.sources(), twin.sources(), "{}", vector.cell);
        } else {
            assert_ne!(plain.sources(), twin.sources(), "{}", vector.cell);
        }
        secrets_differ += usize::from(plain.secrets() != twin.secrets());
        assert_eq!(
            // BORROW: explicit .as_str() instead of Deref coercion
            plain.cell_digest().as_str(),
            vector.digest,
            "{}",
            vector.cell
        );
        assert_eq!(
            // BORROW: explicit .as_str() instead of Deref coercion
            twin.cell_digest().as_str(),
            vector.digest,
            "{}",
            vector.cell
        );
    }
    assert!(secrets_differ >= 1, "some twin must differ in its secrets");
}

/// A hand-written cell: two entries in every list, sorted as R01 sorts them.
fn base_comparable() -> Value {
    json!({
        "values": [{"path": "plan.a", "value": 1}, {"path": "plan.b"}],
        "harness": {"name": "claude-code", "version": "2.1.0"},
        "treatment": {"name": "htn-creator", "tag": "v0.2.3"},
        "snapshots": [
            {"kind": "catalog", "digest": digest_text('a')},
            {"kind": "catalog", "digest": digest_text('b')}
        ],
        "inputs": [
            {"kind": "prompt", "digest": digest_text('c')},
            {"kind": "script", "digest": digest_text('a')}
        ],
        "environments": [
            {"plugin": "alpha-eval@1.0.0", "lock_digest": digest_text('d')},
            {"plugin": "beta-eval@2.0.0", "lock_digest": digest_text('a')}
        ]
    })
}

/// A hand-written manifest of the base cell.
fn base_manifest() -> Value {
    json!({
        "schema": "run-manifest/1",
        "run": {"cell": digest_text('f'), "replicate": 1, "attempt": 1},
        "experiment": digest_text('0'),
        "factors": [],
        "comparable": base_comparable(),
        "sources": [
            {"path": "plan.a", "source": {"kind": "experiment_file"}},
            {"path": "plan.b", "source": {"kind": "unrecorded"}},
            {"path": "harness", "source": {"kind": "experiment_file"}},
            {"path": "treatment", "source": {"kind": "experiment_file"}},
            {"path": "snapshots", "source": {"kind": "experiment_file"}},
            {"path": "inputs", "source": {"kind": "experiment_file"}},
            {"path": "environments", "source": {"kind": "experiment_file"}}
        ],
        "secrets": [],
        "measured": [],
        "deviations": [],
        "integrity": {"kind": "valid"}
    })
}

/// Reorders or repeats an entry of the list at `pointer` in `doc`.
fn edit_list(doc: &mut Value, pointer: &str, how: &str) {
    let list = doc
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .expect("the list is there");
    match how {
        "unsorted" => list.swap(0, 1),
        "duplicate" => {
            let first = list.first().cloned().expect("an entry"); // BORROW: .cloned() copies the entry
            list.insert(1, first);
        }
        other => panic!("unknown edit {other}"),
    }
}

fn message<T>(result: Result<T, serde_json::Error>) -> String {
    match result {
        Ok(_) => String::from("accepted"),
        Err(error) => error.to_string(),
    }
}

/// Guards: Cond 1 — a reader refuses a cell whose values, snapshots, inputs or environments are unsorted or hold a duplicate, and a manifest whose sources do not cover its values and typed roots or name another path; the sorted base and a source under a root are accepted. Oracle: R01 §Run manifest ("every list is sorted and holds no duplicate; a reader refuses one that is not" and "Outside the hash"), worked as a table of hand-written edits.
#[test]
fn unsorted_or_duplicated_lists_are_refused() {
    // The positive controls: the sorted bases read.
    assert_eq!(
        message(serde_json::from_value::<Comparable>(base_comparable())),
        "accepted"
    );
    assert_eq!(
        message(serde_json::from_value::<RunManifest>(base_manifest())),
        "accepted"
    );

    for list in ["values", "snapshots", "inputs", "environments"] {
        for (how, needle) in [("unsorted", "not sorted"), ("duplicate", "repeats")] {
            let mut cell = base_comparable();
            edit_list(&mut cell, format!("/{list}").as_str(), how); // BORROW: explicit .as_str()
            let text = message(serde_json::from_value::<Comparable>(cell));
            assert!(text.contains(needle), "Comparable {list} {how}: {text}");

            let mut held = base_manifest();
            edit_list(&mut held, format!("/comparable/{list}").as_str(), how); // BORROW: explicit .as_str()
            if list == "values" {
                // keep the sources equal to the values so that only the order is wrong
                edit_list(
                    &mut held,
                    "/sources",
                    if how == "unsorted" {
                        "unsorted"
                    } else {
                        "duplicate"
                    },
                );
            }
            let text = message(serde_json::from_value::<RunManifest>(held));
            assert!(text.contains(needle), "RunManifest {list} {how}: {text}");
        }
    }

    // Sources: a value without one, a stray one, one twice, a typed root without one, a source
    // under a typed root (the control), and a source of a path that is neither.
    let drop_source = |path: &str| {
        let mut doc = base_manifest();
        doc.pointer_mut("/sources")
            .and_then(Value::as_array_mut)
            .expect("sources")
            // INDEX: a source entry is an object with a `path`
            .retain(|entry| entry["path"] != path);
        message(serde_json::from_value::<RunManifest>(doc))
    };
    let add_source = |path: &str| {
        let mut doc = base_manifest();
        doc.pointer_mut("/sources")
            .and_then(Value::as_array_mut)
            .expect("sources")
            .push(json!({"path": path, "source": {"kind": "core_default"}}));
        message(serde_json::from_value::<RunManifest>(doc))
    };
    let text = drop_source("plan.b");
    assert!(
        text.contains("is missing"),
        "value without a source: {text}"
    );
    for root in [
        "harness",
        "treatment",
        "snapshots",
        "inputs",
        "environments",
    ] {
        let text = drop_source(root);
        assert!(
            text.contains(root) && text.contains("is missing"),
            "{root} without a source: {text}"
        );
    }
    let text = add_source("plan.c");
    assert!(text.contains("names no value"), "stray source: {text}");
    let text = add_source("settings.model");
    assert!(text.contains("names no value"), "settings source: {text}");
    let text = add_source("harnesses.version");
    assert!(text.contains("names no value"), "near-root source: {text}");

    // Control: a source under a root stands in for the root's own entry.
    let mut under = base_manifest();
    let list = under
        .pointer_mut("/sources")
        .and_then(Value::as_array_mut)
        .expect("sources");
    // INDEX: a source entry is an object with a `path`
    list.retain(|entry| entry["path"] != "harness");
    list.push(json!({"path": "harness.version", "source": {"kind": "core_default"}}));
    assert_eq!(
        message(serde_json::from_value::<RunManifest>(under)),
        "accepted"
    );

    let mut twice = base_manifest();
    edit_list(&mut twice, "/sources", "duplicate");
    let text = message(serde_json::from_value::<RunManifest>(twice));
    assert!(text.contains("repeats"), "{text}");
}

/// A resolved value at `path`, recorded as `value`.
fn resolved(path: &str, value: &Value) -> ResolvedValue {
    ResolvedValue::recorded(
        FieldPath::from_str(path).expect("path"),
        // BORROW: .clone() gives the value to the resolved value
        JsonValue::new(value.clone()).expect("value"),
    )
}

/// Guards: I17 — a cell built from resolved values takes the reserved roots into the typed fields, and a string value that is an absolute path is refused while a relative one is the control. Oracle: R01 §Run manifest ("a file enters a cell as an `inputs[]` digest"), with the expected cell written by hand.
#[test]
fn reserved_roots_fill_the_typed_fields_and_absolute_paths_are_refused() {
    let resolved_values = [
        resolved("plan.b", &json!("relative/dir/file.txt")),
        resolved("plan.a", &json!(1)),
        resolved(
            "harness",
            &json!({"name": "claude-code", "version": "2.1.0", "driver": "pty-driver@1.0.0"}),
        ),
        resolved(
            "treatment",
            &json!({"name": "htn-creator", "tag": "v0.2.3"}),
        ),
        resolved(
            "snapshots",
            &json!([
                {"kind": "catalog", "digest": digest_text('b')},
                {"kind": "catalog", "digest": digest_text('a')}
            ]),
        ),
        resolved("inputs", &json!([])),
    ];
    let mut harness = HarnessIdentity::new(
        Name::from_str("claude-code").expect("name"),
        semver::Version::parse("2.1.0").expect("version"),
    );
    harness.driver = Some(NameVersion::from_str("pty-driver@1.0.0").expect("driver"));
    let mut treatment = TreatmentRef::new(Name::from_str("htn-creator").expect("name"));
    treatment.tag = Some(String::from("v0.2.3"));
    let expected = Comparable::builder()
        .harness(harness)
        .treatment(treatment)
        .value(resolved("plan.a", &json!(1)))
        .value(resolved("plan.b", &json!("relative/dir/file.txt")))
        .snapshot(DigestRef::new(
            Name::from_str("catalog").expect("name"),
            digest('a'),
        ))
        .snapshot(DigestRef::new(
            Name::from_str("catalog").expect("name"),
            digest('b'),
        ))
        .build()
        .expect("expected cell");
    let built = Comparable::from_resolved(&resolved_values).expect("from resolved");
    assert_eq!(built, expected);

    for absolute in [
        "/etc/passwd",
        "\\\\server\\share",
        "C:\\Users\\x",
        "c:/work",
    ] {
        let mut list = resolved_values.to_vec();
        list.push(resolved("plan.where", &json!(absolute)));
        let error = Comparable::from_resolved(&list).expect_err("absolute path");
        assert!(
            error.to_string().contains("absolute path"),
            "{absolute}: {error}"
        );
    }
    let mut control = resolved_values.to_vec();
    control.push(resolved("plan.where", &json!("work/x")));
    assert!(Comparable::from_resolved(&control).is_ok());

    // An absolute path nested in an array or an object is refused at every depth; the relative
    // nested twin is the control.
    for nested in [
        json!(["ok", "/etc/passwd"]),
        json!({"dir": {"file": "C:\\Users\\x"}}),
        json!([[{"deep": ["a", "/root"]}]]),
    ] {
        let mut list = resolved_values.to_vec();
        list.push(resolved("plan.where", &nested));
        let error = Comparable::from_resolved(&list).expect_err("nested absolute path");
        assert!(
            error.to_string().contains("absolute path"),
            "{nested}: {error}"
        );
    }
    let mut relative = resolved_values.to_vec();
    relative.push(resolved(
        "plan.where",
        &json!({"dir": ["work/x", {"file": "a/b"}], "n": null}),
    ));
    assert!(Comparable::from_resolved(&relative).is_ok());

    // A path below a reserved root, and a missing harness, are refused.
    let mut below = resolved_values.to_vec();
    below.push(resolved("harness.version", &json!("9.9.9")));
    assert!(Comparable::from_resolved(&below).is_err());
    // INDEX: the list holds six values
    assert!(Comparable::from_resolved(&resolved_values[..2]).is_err());
}

#[derive(Deserialize)]
struct CatalogFile {
    vector: Vec<CatalogVector>,
}

#[derive(Deserialize)]
struct CatalogVector {
    document_a: String,
    document_b: String,
    document_c: String,
    digest_ab: String,
    digest_c: String,
}

/// Guards: Cond 1 — the digest of a catalog snapshot equals the independent one and does not change when only `captured_at` does, so a capture time never splits a cell. Oracle: `catalog_content_digest.toml` (Python `rfc8785`; A and B differ only in `captured_at`, C in a tool description).
#[test]
fn catalog_content_digest_ignores_the_capture_time() {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file: CatalogFile =
        toml::from_str(oracle("catalog_content_digest.toml").as_str()).expect("toml");
    let vector = file.vector.first().expect("a vector");
    // BORROW: explicit .as_str() instead of Deref coercion
    let read = |text: &str| serde_json::from_str::<CatalogSnapshot>(text).expect("snapshot");
    // BORROW: explicit .as_str() instead of Deref coercion
    let a = read(vector.document_a.as_str());
    // BORROW: explicit .as_str() instead of Deref coercion
    let b = read(vector.document_b.as_str());
    // BORROW: explicit .as_str() instead of Deref coercion
    let c = read(vector.document_c.as_str());
    assert_ne!(
        a.captured_at(),
        b.captured_at(),
        "the captures differ in time"
    );
    assert_eq!(
        // BORROW: explicit .as_str() instead of Deref coercion
        a.content_digest().expect("digest").as_str(),
        vector.digest_ab
    );
    assert_eq!(
        // BORROW: explicit .as_str() instead of Deref coercion
        b.content_digest().expect("digest").as_str(),
        vector.digest_ab
    );
    assert_eq!(
        // BORROW: explicit .as_str() instead of Deref coercion
        c.content_digest().expect("digest").as_str(),
        vector.digest_c
    );
    assert_ne!(vector.digest_ab, vector.digest_c);
}

/// Guards: Wire rule — an import record keeps the legacy integrity block it was given as a JSON value and reads it back, and `null` is refused where the record is read; the three reader causes are invalid reasons by their snake-case names. Oracle: README §Gotchas wire rules ("`null` is never written"), with the expected texts written by hand.
#[test]
fn import_record_round_trips_its_legacy_integrity_and_reasons_are_named() {
    let format = NameToken::from_str("campaign-import@v1").expect("format");
    let commit = GitCommit::from_str("0123456789abcdef0123456789abcdef01234567").expect("commit");
    for legacy in [
        json!({"valid": true, "n": null}),
        json!("ok"),
        json!(0),
        json!([]),
    ] {
        // BORROW: .clone() gives the record its own copy of the block
        let record = ImportRecord::new(
            format.clone(), // BORROW: .clone() the record owns its format
            commit.clone(), // BORROW: .clone() the record owns its commit
            JsonValue::new(legacy.clone()).expect("value"),
        );
        let written = serde_json::to_value(&record).expect("written");
        assert_eq!(written["legacy_integrity"], legacy);
        let read: ImportRecord = serde_json::from_value(written).expect("read back");
        assert_eq!(read, record);
    }
    let rules_digest = digest('a');
    let plain = ImportRecord::new(
        format.clone(), // BORROW: .clone() the record owns its format
        commit.clone(), // BORROW: .clone() the record owns its commit
        JsonValue::new(json!("ok")).expect("value"),
    );
    assert!(serde_json::to_value(&plain).expect("plain")["rules"].is_null());
    let ruled = plain.rules(rules_digest);
    let written = serde_json::to_value(&ruled).expect("ruled");
    assert_eq!(
        written["rules"],
        json!(format!("sha256:{}", "a".repeat(64)))
    );
    let read: ImportRecord = serde_json::from_value(written).expect("read back");
    assert_eq!(read, ruled);
    let null = json!({
        "format": "campaign-import@v1",
        "base_commit": "0123456789abcdef0123456789abcdef01234567",
        "legacy_integrity": null,
        "sources": [],
        "requests": []
    });
    assert!(serde_json::from_value::<ImportRecord>(null).is_err());

    for reason in [
        "collection_error",
        "reader_error",
        "format_unknown",
        "gateway",
    ] {
        let integrity: Integrity =
            serde_json::from_value(json!({"kind": "invalid", "reason": reason}))
                .expect("a named reason");
        assert_eq!(
            serde_json::to_value(&integrity).expect("written"),
            json!({"kind": "invalid", "reason": reason})
        );
    }
    let unknown = json!({"kind": "invalid", "reason": "collection-error"});
    assert!(serde_json::from_value::<Integrity>(unknown).is_err());
}

/// Guards: I12 — a resolution that keeps its settings builds a manifest once the settings are filtered out: the sources of settings are dropped, those of values and typed roots stay, and the unfiltered list is refused. Oracle: R01 §Run manifest ("Outside the hash": settings never enter the manifest), with the expected sources written by hand.
#[test]
fn a_resolution_with_settings_builds_a_manifest() {
    let resolved_values = [
        resolved("plan.a", &json!(1)),
        // A sibling whose name only starts with the setting `runner`: it is a value, not a setting.
        resolved("runner2.x", &json!(2)),
        resolved("harness", &json!({"name": "h", "version": "1.0.0"})),
        resolved("treatment", &json!({"name": "t"})),
    ];
    let source = ValueSource::ExperimentFile;
    let entry =
        |path: &str| SourceEntry::new(FieldPath::from_str(path).expect("path"), source.clone()); // BORROW: .clone() gives each entry its own source
    let sources = [
        entry("plan.a"),
        entry("runner2.x"),
        entry("harness.version"),
        entry("treatment"),
        entry("runner.parallel"),
        entry("runner.cache.dir"),
    ];
    let settings = [FieldPath::from_str("runner").expect("path")];
    let (cell, kept) =
        Comparable::from_resolution(&resolved_values, &sources, &settings).expect("resolution");
    let paths: Vec<&str> = kept.iter().map(|entry| entry.path().as_str()).collect(); // BORROW: .as_str() lends the path text
    assert_eq!(
        paths,
        ["plan.a", "runner2.x", "harness.version", "treatment"]
    );

    let mut builder = RunManifest::builder()
        .run(RunKey::new(cell.digest(), 1, 1).expect("run key"))
        .experiment(digest('0'))
        .integrity(Integrity::Valid);
    for kept_source in kept {
        builder = builder.source(kept_source);
    }
    builder.comparable(cell).build().expect("manifest");

    // The unfiltered sources are refused by the builder, and a source that is no setting, value or root by the filter.
    let (cell, _) =
        Comparable::from_resolution(&resolved_values, &sources, &settings).expect("resolution");
    let mut whole = RunManifest::builder()
        .run(RunKey::new(cell.digest(), 1, 1).expect("run key"))
        .experiment(digest('0'))
        .integrity(Integrity::Valid);
    for each in &sources {
        whole = whole.source(each.clone()); // BORROW: .clone() gives the builder its own entry
    }
    assert!(whole.comparable(cell).build().is_err());
    assert!(Comparable::from_resolution(&resolved_values, &sources, &[]).is_err());
}
