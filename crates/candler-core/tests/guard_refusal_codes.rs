//! Guards for the registry of refusal codes: every error type of the workspace is coded or
//! listed as Candler's own, and every row of the catalogue equals its row in
//! `docs/contract/codes.toml`.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::refusal::{CatalogRow, Side};
use regex::Regex;

/// The fallback code of an error that is not an author's.
const INTERNAL_CODE: &str = "candler.internal";

/// The sections of `codes.toml` that hold no row of a code.
const NO_CODE_SECTIONS: [&str; 2] = ["internal", "helper"];

/// The sections whose rows the core and the harness crates own: their rows are in the catalogue.
const CATALOGUED_SECTIONS: [&str; 8] = [
    "resolve",
    "experiment",
    "runner",
    "isolation",
    "store",
    "supervise",
    "frames",
    "harness",
];

/// The error types the oracle gives to a later leaf, as `<file>::<name>`; that leaf empties this
/// list. An entry that is not a type of the source, is in the catalogue, or is not a type the
/// oracle gives to that leaf fails the guard, so the list cannot hide a type of this leaf.
const PENDING: &[&str] = &[
    "crates/candler-core/src/evaluation/client/mod.rs::ClientError",
    "crates/candler-core/src/evaluation/context/mod.rs::ContextError",
    "crates/candler-core/src/evaluation/engine/mod.rs::EvaluateError",
    "crates/candler-core/src/evaluation/engine/plan.rs::PlanError",
    "crates/candler-core/src/evaluation/registry/mod.rs::RegistryError",
    "crates/candler-core/src/evaluation/registry/needs.rs::NeedError",
    "crates/candler-core/src/evaluation/selection/diff.rs::EquivalenceError",
    "crates/candler-core/src/import/mod.rs::ImportError",
    "crates/candler-core/src/import/raw.rs::RawIngestError",
    "crates/candler-core/src/jobs/dir.rs::JobError",
    "crates/candler-core/src/jobs/lost.rs::RecoverError",
    "crates/candler-core/src/jobs/orchestrate/session.rs::OrchestrateError",
    "crates/candler-core/src/jobs/runtime/mod.rs::RuntimeError",
    "crates/candler-core/src/jobs/slots.rs::SlotError",
    "crates/candler-core/src/jobs/view.rs::JobViewError",
    "crates/candler-core/src/manifest/compare.rs::VerdictError",
    "crates/candler-core/src/trace/mod.rs::NormaliseError",
];

/// The workspace root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One row of `refusal_scope.tsv`.
struct Scope {
    path: String,
    file: String,
    author: String,
    leaf: String,
}

/// The rows of the scope oracle, read by the names of its header row.
fn scope() -> Vec<Scope> {
    let text =
        fs::read_to_string(root().join("__roadmap__/m3-lean-cli/refusal_scope.tsv")).unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let at = |name: &str| header.iter().position(|column| *column == name).unwrap();
    let (enum_at, file_at, author_at, leaf_at) = (at("enum"), at("file"), at("author"), at("leaf"));
    lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            Scope {
                path: cells[enum_at].to_owned(),
                file: cells[file_at].to_owned(),
                author: cells[author_at].to_owned(),
                leaf: cells[leaf_at].to_owned(),
            }
        })
        .collect()
}

/// The last segment of a Rust path.
fn name_of(path: &str) -> &str {
    path.rsplit("::").next().unwrap()
}

/// Every type that derives `thiserror::Error` under `crates/*/src`, as `<file>::<name>`.
fn error_types() -> Vec<String> {
    let derive = Regex::new(
        r"(?s)#\[derive\(([^)]*)\)\]((?:\s*#\[[^\]]*\])*)\s*(?:pub(?:\([^)]*\))?\s+)?(?:enum|struct)\s+(\w+)",
    )
    .unwrap();
    let error = Regex::new(r"\bError\b").unwrap();
    let mut found = Vec::new();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                if name != "target" && name != "tests" && name != "benches" {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path.components().any(|c| c.as_os_str() == "src")
            {
                let text = fs::read_to_string(&path).unwrap();
                let file = path
                    .strip_prefix(root().join(""))
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let file = file.trim_start_matches("../../").to_owned();
                for capture in derive.captures_iter(&text) {
                    if error.is_match(&capture[1]) {
                        found.push(format!("{file}::{}", &capture[3]));
                    }
                }
            }
        }
    }
    found.sort();
    found
}

/// The parsed `codes.toml`.
fn codes() -> toml::Table {
    let text = fs::read_to_string(root().join("docs/contract/codes.toml")).unwrap();
    text.parse::<toml::Table>().unwrap()
}

/// A row of a section of codes.
struct CodeRow {
    code: String,
    section: String,
    side: String,
    enum_name: String,
    variant: String,
}

/// The rows of every section that holds codes.
fn code_rows(table: &toml::Table) -> Vec<CodeRow> {
    let mut rows = Vec::new();
    for (section, body) in table {
        if NO_CODE_SECTIONS.contains(&section.as_str()) {
            continue;
        }
        for (code, row) in body.as_table().unwrap() {
            let field = |name: &str| row[name].as_str().unwrap().to_owned();
            rows.push(CodeRow {
                code: code.clone(),
                section: section.clone(),
                side: field("side"),
                enum_name: field("enum"),
                variant: field("variant"),
            });
        }
    }
    rows
}

/// The `[internal]` rows as `(enum, variant)`; a row with no variant covers the whole enum.
fn internal_rows(table: &toml::Table) -> Vec<(String, Option<String>)> {
    table["internal"]
        .as_table()
        .unwrap()
        .values()
        .map(|row| {
            assert!(
                row["reason"]
                    .as_str()
                    .is_some_and(|reason| !reason.is_empty()),
                "an [internal] row gives its reason: {row:?}"
            );
            (
                row["enum"].as_str().unwrap().to_owned(),
                row.get("variant").map(|v| v.as_str().unwrap().to_owned()),
            )
        })
        .collect()
}

/// The catalogue of the core, the harness crates and the schemas.
fn catalogue() -> Vec<CatalogRow> {
    let mut rows = candler_core::refusal::catalog();
    rows.extend(candler_harness_claude_code::refusal::catalog());
    rows
}

/// Guards: R1 — every error type of the workspace is coded, or listed as Candler's own in
/// `[internal]`, or only wraps coded errors, or is on the pending list of a later leaf; an error of this leaf's scope
/// implements `Refusal` and is in the catalogue; the oracle's classification (an author's enum
/// has no fallback variant, an enum that is not an author's has nothing else) holds.
///
/// Oracle: the `thiserror` types found by a scan of `crates/*/src`, `codes.toml` and
/// `__roadmap__/m3-lean-cli/refusal_scope.tsv`, none of them generated from the code under test.
///
/// Bite: a new `thiserror` enum named in neither `codes.toml` nor the pending list; a variant of
/// an enum the oracle gives to the author whose row carries the fallback code; a variant's side
/// set to `candler` where the oracle says author.
#[test]
fn r1_every_error_enum_is_coded_or_internal() {
    let table = codes();
    let coded: BTreeSet<String> = code_rows(&table)
        .into_iter()
        .map(|row| row.enum_name)
        .collect();
    let internal: BTreeSet<String> = internal_rows(&table)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let types = error_types();
    let scope = scope();
    let catalogue = catalogue();
    let catalogued: BTreeSet<&str> = catalogue.iter().map(|row| row.enum_name).collect();

    for pending in PENDING {
        assert!(
            types.contains(&(*pending).to_owned()),
            "{pending} is on the pending list and is not a type of the source"
        );
        let (file, name) = pending.rsplit_once("::").unwrap();
        let row = scope
            .iter()
            .find(|row| row.file == file && name_of(&row.path) == name)
            .unwrap_or_else(|| panic!("{pending} is on the pending list and not in the oracle"));
        assert_eq!(
            row.leaf, "refusal_late",
            "{pending} is on the pending list and the oracle gives it to {}",
            row.leaf
        );
        let shared = types
            .iter()
            .filter(|t| t.rsplit("::").next() == Some(name))
            .count()
            > 1;
        assert!(
            shared || !catalogued.contains(name),
            "{pending} is on the pending list and is in the catalogue: remove it"
        );
    }

    for entry in &types {
        if PENDING.contains(&entry.as_str()) {
            continue;
        }
        let name = entry.rsplit("::").next().unwrap();
        let delegates_only = {
            let mut rows = catalogue
                .iter()
                .filter(|row| row.enum_name == name)
                .peekable();
            rows.peek().is_some() && rows.all(|row| row.delegates)
        };
        assert!(
            coded.contains(name) || internal.contains(name) || delegates_only,
            "{entry} is in neither codes.toml nor [internal] nor the pending list"
        );
    }

    for row in scope.iter().filter(|row| row.leaf == "refusal_core") {
        let name = name_of(&row.path);
        if name == "CliError" {
            continue;
        }
        let rows: Vec<&CatalogRow> = catalogue.iter().filter(|r| r.enum_name == name).collect();
        assert!(
            !rows.is_empty(),
            "{} is in this leaf's scope and has no row in the catalogue",
            row.path
        );
        match row.author.as_str() {
            "yes" => {
                for r in &rows {
                    assert!(
                        r.code != INTERNAL_CODE && r.side != Side::Candler,
                        "{}::{} is an author's error by the oracle and carries code {:?}, side {}",
                        row.path,
                        r.variant,
                        r.code,
                        r.side
                    );
                }
            }
            "no" => {
                for r in &rows {
                    assert!(
                        r.code == INTERNAL_CODE && r.side == Side::Candler,
                        "{}::{} is not an author's error by the oracle and carries code {:?}, side {}",
                        row.path,
                        r.variant,
                        r.code,
                        r.side
                    );
                }
            }
            "mixed" => {}
            other => panic!("{}: the oracle's author column holds {other:?}", row.path),
        }
    }
}

/// Guards: R1 — every row of the catalogue equals its row in `codes.toml` (code, side, enum and
/// variant), a variant with the fallback code has its `[internal]` row, a delegating variant has
/// no code, every row of this leaf's sections is in the catalogue, codes are unique and have the
/// form `^[a-z]+(\.[a-z_]+)+$`, and each row says when it arises and what its path points at.
///
/// Oracle: `docs/contract/codes.toml`, which is read and never generated from the code.
///
/// Bite: a code of the catalogue that `codes.toml` lacks; a variant whose side differs from its
/// row; a row of `codes.toml` that no variant raises.
#[test]
fn r1_every_variant_code_equals_its_row() {
    let table = codes();
    let rows = code_rows(&table);
    let internal = internal_rows(&table);
    let catalogue = catalogue();
    let form = Regex::new(r"^[a-z]+(\.[a-z_]+)+$").unwrap();

    let by_code: BTreeMap<&str, &CodeRow> =
        rows.iter().map(|row| (row.code.as_str(), row)).collect();
    assert_eq!(
        by_code.len(),
        rows.len(),
        "a code is written twice in codes.toml"
    );

    let mut raised: BTreeSet<&str> = BTreeSet::new();
    for row in &catalogue {
        let who = format!("{}::{}", row.enum_name, row.variant);
        assert!(
            !row.when.is_empty() && row.when.ends_with('.') && !row.when.contains('\n'),
            "{who}: `when` is one sentence"
        );
        assert!(
            !row.at.is_empty() && !row.at.contains('\n'),
            "{who}: `at` is one line"
        );
        if row.delegates {
            assert!(row.code.is_empty(), "{who}: a delegating row owns no code");
            continue;
        }
        assert!(
            form.is_match(row.code),
            "{who}: code {:?} has the wrong form",
            row.code
        );
        if row.code == INTERNAL_CODE {
            assert_eq!(
                row.side,
                Side::Candler,
                "{who}: an internal row blames Candler"
            );
            assert!(
                internal.iter().any(|(name, variant)| name == row.enum_name
                    && variant.as_deref().is_none_or(|v| v == row.variant)),
                "{who} has the fallback code and no [internal] row"
            );
            continue;
        }
        let Some(registered) = by_code.get(row.code) else {
            panic!("{who}: code {} is not in codes.toml", row.code);
        };
        assert_eq!(
            registered.side,
            row.side.as_str(),
            "{who}: side of {}",
            row.code
        );
        assert_eq!(
            registered.enum_name, row.enum_name,
            "{who}: enum of {}",
            row.code
        );
        assert_eq!(
            registered.variant, row.variant,
            "{who}: variant of {}",
            row.code
        );
        assert!(
            raised.insert(row.code),
            "{who}: code {} is raised by two variants",
            row.code
        );
    }

    for row in &rows {
        if CATALOGUED_SECTIONS.contains(&row.section.as_str()) {
            assert!(
                raised.contains(row.code.as_str()),
                "codes.toml row {} ([{}]) is raised by no variant of the catalogue",
                row.code,
                row.section
            );
        }
    }
}
