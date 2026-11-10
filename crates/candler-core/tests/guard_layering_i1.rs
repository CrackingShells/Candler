//! Guards for the layering of the core: the instrument's domain stays out of the core, the
//! harness crate and the published schemas (I1), and the import takes every domain name from its
//! rules. The deny-list below is a literal of this file, written from the names of the legacy
//! domain; a test that rewrites those names in a copy of the mini-corpus proves that the import
//! does not need them.
#![cfg(test)]

#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_core::import::assemble::import;
use candler_core::import::rules::{
    HarnessRule, ImportRules, ImportRulesBuilder, MarkerCode, MarkerRule, SnapshotRule,
    TreatmentRule,
};
use candler_core::store::Store;
use candler_schemas::primitives::{FieldPath, Name, NameToken, ReasonCode};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use legacy_import::{mini, project, reader};
use oracle::{expected_rows, expected_runs};
use regex::Regex;
use support::assert_clean;

/// The names of the legacy domain that no layer below the rules may hold.
const DENY_LIST: [&str; 11] = [
    "htn",
    "HTN",
    "gtpyhop",
    "GTPyhop",
    "rikyu",
    "Rikyu",
    "RIKYU",
    "tier_d",
    "TIER_D",
    "D-E",
    "mcp-python-ingestion",
];

/// What the domain names become in the renamed copy of the corpus, in the order they are applied.
const RENAMES: [(&str, &str); 11] = [
    ("mcp-python-ingestion", "mcp-ingest"),
    ("gtpyhop", "planlib"),
    ("GTPyhop", "Planlib"),
    ("rikyu", "zenith"),
    ("Rikyu", "Zenith"),
    ("RIKYU", "ZENITH"),
    ("tier_d", "tier_x"),
    ("TIER_D", "TIER_X"),
    ("htn", "acme"),
    ("HTN", "ACME"),
    ("D-E", "X-E"),
];

/// The folder `crates/<krate>/<inside>` of the workspace.
fn crate_dir(krate: &str, inside: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(krate)
        .join(inside)
}

/// The folder `python/<package>/src` of the workspace.
fn python_src(package: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("python")
        .join(package)
        .join("src")
}

/// Guards: I1 — none of the deny-listed domain names appears in the core's sources, the harness
/// crate's sources, the published schemas, the `candler` binary's sources or the Python evaluator
/// helper's sources (the schemas crate's Rust sources are not scanned).
/// Oracle: the deny-list literal of this file.
#[test]
fn the_core_the_harness_the_schemas_the_binary_and_the_helper_hold_no_domain_name() {
    let folders = [
        crate_dir("candler-core", "src"),
        crate_dir("candler-harness", "src"),
        crate_dir("candler", "src"),
        crate_dir("candler-schemas", "schema"),
        python_src("candler-evaluator"),
    ];
    for folder in &folders {
        for needle in DENY_LIST {
            assert_clean(folder, needle);
        }
    }
}

/// The bytes with every domain name renamed.
fn renamed(bytes: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    for (from, to) in RENAMES {
        text = text.replace(from, to);
    }
    text.into_bytes()
}

/// Copies `from` to `to` with every directory name, file name and file renamed, a gzip file
/// decompressed, rewritten and compressed again.
fn copy_renamed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the target directory is created");
    for entry in fs::read_dir(from).expect("the directory lists") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(String::from_utf8(renamed(name.as_bytes())).expect("a UTF-8 name"));
        if entry.file_type().expect("a type").is_dir() {
            copy_renamed(&entry.path(), &target);
            continue;
        }
        let bytes = fs::read(entry.path()).expect("the file reads");
        let written = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut plain = Vec::new();
            // BORROW: the decoder reads the slice of `bytes` in place
            MultiGzDecoder::new(bytes.as_slice())
                .read_to_end(&mut plain)
                .expect("the stream gunzips");
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&renamed(&plain))
                .expect("the stream compresses");
            encoder.finish().expect("the stream finishes")
        } else {
            renamed(&bytes)
        };
        fs::write(&target, written).expect("the file is written");
    }
}

/// A rules builder with everything but the markers: the instrument and the legacy format are
/// the caller's.
fn rules_without_markers(instrument: &str, format: &str) -> ImportRulesBuilder {
    let name = |text: &str| Name::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let path = |text: &str| FieldPath::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    ImportRules::builder()
        .format(NameToken::from_str(format).unwrap_or_else(|e| panic!("{format}: {e}")))
        .harness(HarnessRule::new(
            name("claude-code"),
            "/claude_code_version".into(),
        ))
        .treatment(TreatmentRule::new(
            "/plan/skill_version".into(),
            path("resolved_skill_version"),
        ))
        .instrument(name(instrument))
        .snapshot(SnapshotRule::new(name("catalog"), "/catalog/sha256".into()))
        .suite_kind(name("legacy-check"))
        .entry_params(vec![name("scale"), name("text"), name("tier")])
        .metadata(path("plan.model"))
}

/// The status counts of the oracle's rows over the mini-corpus.
fn oracle_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in expected_rows(&mini()) {
        *counts.entry(row.status).or_default() += 1;
    }
    counts
}

/// The projected rows and the run count of an import of `folder` with `rules` into a fresh
/// store; the store folder is returned so that the caller can scan it.
fn import_rows(
    folder: &Path,
    rules: &ImportRules,
    store_dir: &Path,
) -> (usize, Vec<oracle::ExpectedRow>) {
    let store = Store::init(store_dir).expect("the store initialises");
    let report = import(folder, rules, &reader, &store).expect("the folder imports");
    let mut rows = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store
            .outcome_files(&entry.key)
            .expect("the outcome files list")
        {
            for record in store.read_outcomes(&file).expect("the file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                rows.push(project("", entry.key.replicate(), &value));
            }
        }
    }
    (usize::try_from(report.runs).expect("a run count"), rows)
}

/// Guards: I1 — a copy of the mini-corpus whose every file, directory and stream has the domain
/// names rewritten, imported with rules that name an `acme-eval` instrument, a renamed legacy
/// format and a renamed marker, gives the oracle's run count and the oracle's count of each
/// status, and the store holds none of the deny-listed names. Oracle: `oracle::expected_runs`
/// and `oracle::expected_rows` over the original mini-corpus, and the deny-list literal.
#[test]
fn a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store() {
    let copy = tempfile::tempdir().expect("a temporary directory");
    copy_renamed(&mini(), copy.path());
    for needle in DENY_LIST {
        assert_clean(copy.path(), needle);
    }

    let pattern = Regex::new(r"TIER_X_REPLAY:[^\n]*reason=(?P<code>[A-Za-z0-9_]+)")
        .expect("the pattern compiles");
    let zero = Regex::new(r"declares zero 'mappings' entries").expect("the pattern compiles");
    let rules = rules_without_markers("acme-eval", "acme-results@1")
        .marker(MarkerRule::new(pattern, MarkerCode::Capture("code".into())))
        .marker(MarkerRule::new(
            zero,
            MarkerCode::Fixed(ReasonCode::from_str("zero_mappings").expect("a reason code")),
        ))
        .build()
        .expect("the renamed rules build");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, rows) = import_rows(copy.path(), &rules, store_dir.path());
    assert_eq!(runs, expected_runs(&mini()).len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows {
        // BORROW: `.clone()` copies the status
        *counts.entry(row.status.clone()).or_default() += 1;
    }
    assert_eq!(counts, oracle_counts());
    for needle in DENY_LIST {
        assert_clean(store_dir.path(), needle);
    }
}

/// Guards: I1 and D6 — a reason code comes from the rules and not from the core: with rules that
/// hold no marker, every `not_measured` record of the mini-corpus has the cause
/// `{legacy: unrecorded}`, while the oracle's rows show that the rules with markers give some
/// `not_measured` record another cause. Oracle: `oracle::expected_rows` over the mini-corpus.
#[test]
fn without_markers_every_unmeasured_record_is_unrecorded() {
    let marked: Vec<_> = expected_rows(&mini())
        .into_iter()
        .filter(|row| row.status == "not_measured")
        .filter(|row| row.cause.as_deref() != Some("legacy: unrecorded"))
        .collect();
    assert!(
        !marked.is_empty(),
        "the mini-corpus has no `not_measured` record with a marker code"
    );

    let rules = rules_without_markers("htn-creator-eval", "htn-results@1")
        .build()
        .expect("the rules without markers build");
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, rows) = import_rows(&mini(), &rules, store_dir.path());
    assert_eq!(runs, 6);
    let unmeasured: Vec<_> = rows
        .iter()
        .filter(|row| row.status == "not_measured")
        .collect();
    assert!(
        !unmeasured.is_empty(),
        "the mini-corpus has no `not_measured` record"
    );
    for row in unmeasured {
        assert_eq!(row.cause.as_deref(), Some("legacy: unrecorded"), "{row:?}");
    }
}
