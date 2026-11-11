//! Guards for the registry of refusal codes: every error type of the workspace is coded or
//! listed as Candler's own, and every row of the catalogue equals its row in
//! `docs/contract/codes.toml`.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::refusal::{CatalogRow, Refusal, Side};
use regex::Regex;

/// The fallback code of an error that is not an author's.
const INTERNAL_CODE: &str = "candler.internal";

/// The sections of `codes.toml` that hold no row of a code.
const NO_CODE_SECTIONS: [&str; 2] = ["internal", "helper"];

/// The sections whose rows the core and the harness crates own: their rows are in the catalogue.
const CATALOGUED_SECTIONS: [&str; 13] = [
    "resolve",
    "experiment",
    "runner",
    "isolation",
    "store",
    "supervise",
    "frames",
    "harness",
    "jobs",
    "evaluation",
    "import",
    "analysis",
    "manifest",
];

/// The workspace root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One row of `refusal_scope.tsv`.
struct Scope {
    path: String,
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
    let (enum_at, author_at, leaf_at) = (at("enum"), at("author"), at("leaf"));
    lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            Scope {
                path: cells[enum_at].to_owned(),
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

/// Every source file under `crates/*/src`, as `(<file>, <text>)`. A scan of text: a derive that a
/// `macro_rules!` expands into is not seen.
fn source_files() -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                // `tests` and `benches` of a crate hold no source of the crate; the same names
                // deeper down are modules of it.
                let at_crate_root =
                    path.parent().and_then(Path::parent) == Some(root().join("crates").as_path());
                let is_test_dir = at_crate_root && (name == "tests" || name == "benches");
                if name != "target" && !is_test_dir {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path.components().any(|c| c.as_os_str() == "src")
            {
                let file = path
                    .strip_prefix(root().join(""))
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let file = file.trim_start_matches("../../").to_owned();
                files.push((file, fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// Every error type under `crates/*/src`, as `<file>::<name>`: a type with any `derive(..)` of
/// the attributes above it that names `Error`, however many `derive` lines it has, and a type
/// with a hand-written `impl Error for`.
fn error_types() -> Vec<String> {
    let header = Regex::new(
        r"(?s)((?:\s*(?:#\[[^\]]*\]|///[^\n]*))+)\s*(?:pub(?:\([^)]*\))?\s+)?(?:enum|struct)\s+(\w+)",
    )
    .unwrap();
    let derive = Regex::new(r"derive\(([^)]*)\)").unwrap();
    let error = Regex::new(r"\bError\b").unwrap();
    let manual =
        Regex::new(r"impl(?:<[^>]*>)?\s+(?:std::error::|core::error::)?Error\s+for\s+(\w+)")
            .unwrap();
    let mut found = Vec::new();
    for (file, text) in source_files() {
        for capture in header.captures_iter(&text) {
            let derives_error = derive
                .captures_iter(&capture[1])
                .any(|group| error.is_match(&group[1]));
            if derives_error {
                found.push(format!("{file}::{}", &capture[2]));
            }
        }
        for capture in manual.captures_iter(&text) {
            found.push(format!("{file}::{}", &capture[1]));
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The module path of a source file, as `<crate>::<module>`: `crates/candler-core/src/a/b.rs`
/// and `crates/candler-core/src/a/b/mod.rs` are `candler_core::a::b`.
fn module_path(file: &str) -> String {
    let rest = file.strip_prefix("crates/").unwrap();
    let (krate, inside) = rest.split_once("/src/").unwrap();
    let inside = inside.strip_suffix(".rs").unwrap();
    let inside = inside.strip_suffix("/mod").unwrap_or(inside);
    let mut path = krate.replace('-', "_");
    if inside != "lib" && inside != "main" {
        for segment in inside.split('/') {
            path.push_str("::");
            path.push_str(segment);
        }
    }
    path
}

/// The error types that have a `refusal_table!`, as `<file>::<name>`: the type a table names is the
/// one defined in the same file, or the one whose module the table's file imports it from, or the
/// only type of that name.
fn tabled_types(types: &[String]) -> BTreeSet<String> {
    let table = Regex::new(
        r"(?:(?:refusal|wrapper)_table!\s*[{(]\s*(?:struct\s+)?|impl\s+(?:candler_schemas::refusal::)?Refusal\s+for\s+)(\w+)\s*[,{]",
    )
    .unwrap();
    let uses = Regex::new(r"(?m)^\s*use\s+([^;]+);").unwrap();
    let mut tabled = BTreeSet::new();
    for (file, text) in source_files() {
        let krate = module_path(&file);
        let krate = krate.split("::").next().unwrap().to_owned();
        let mut imports: Vec<String> = Vec::new();
        for statement in uses.captures_iter(&text) {
            let statement = statement[1].replace('\n', " ");
            if let Some((prefix, group)) = statement.split_once("::{") {
                for item in group.trim_end_matches('}').split(',') {
                    imports.push(format!("{}::{}", prefix.trim(), item.trim()));
                }
            } else {
                imports.push(statement.trim().to_owned());
            }
        }
        let imports: Vec<String> = imports
            .into_iter()
            .map(|path| path.replacen("crate::", &format!("{krate}::"), 1))
            .collect();
        for capture in table.captures_iter(&text) {
            let name = &capture[1];
            let candidates: Vec<&String> = types
                .iter()
                .filter(|entry| name_of(entry) == name)
                .collect();
            if candidates.is_empty() {
                // A refusal that is no error type, such as an adapter.
                continue;
            }
            let chosen: Vec<&&String> = if candidates.len() == 1 {
                candidates.iter().collect()
            } else {
                candidates
                    .iter()
                    .filter(|entry| {
                        let (defined_in, _) = entry.rsplit_once("::").unwrap();
                        let module = module_path(defined_in);
                        defined_in == file
                            || imports.iter().any(|import| {
                                import.rsplit_once("::").is_some_and(|(from, last)| {
                                    last == name
                                        && (module == from
                                            || module.starts_with(&format!("{from}::")))
                                })
                            })
                    })
                    .collect()
            };
            assert!(
                chosen.len() == 1,
                "{file}: the table of `{name}` names {} types of that name ({candidates:?}): import it by its module",
                chosen.len()
            );
            tabled.insert((*chosen[0]).clone());
        }
    }
    tabled
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
/// `[internal]`, or only wraps coded errors, with no list of types waiting for a later leaf; an
/// error of the scope of `refusal_core` or `refusal_late` implements `Refusal` and is in the
/// catalogue; the oracle's classification (an enum that is an author's has a variant coded for
/// the author, one that is not has only the fallback code, a mixed one has both) holds, and the
/// fallback code goes with the side `candler`.
///
/// Oracle: the `thiserror` types found by a scan of `crates/*/src`, `codes.toml` and
/// `__roadmap__/m3-lean-cli/refusal_scope.tsv`, none of them generated from the code under test.
///
/// Bite: a new `thiserror` enum named in neither `codes.toml` nor `[internal]`; a variant of
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

    let tabled = tabled_types(&types);
    let whole_enum_internal: BTreeSet<String> = table["internal"]
        .as_table()
        .unwrap()
        .iter()
        .filter(|(_, row)| row.get("variant").is_none())
        .map(|(key, _)| key.clone())
        .collect();
    for entry in &types {
        let name = name_of(entry);
        let (file, _) = entry.rsplit_once("::").unwrap();
        let qualified = format!("{}::{name}", module_path(file));
        let delegates_only = {
            let mut rows = catalogue
                .iter()
                .filter(|row| row.enum_name == name)
                .peekable();
            rows.peek().is_some() && rows.all(|row| row.delegates)
        };
        let registered = coded.contains(name) || internal.contains(name) || delegates_only;
        assert!(
            (tabled.contains(entry) && registered) || whole_enum_internal.contains(&qualified),
            "{entry} has no table of codes in its own module's `refusal_table!` and no whole-enum [internal] row keyed `{qualified}`: code it, or list it as Candler's own"
        );
    }

    for row in scope.iter().filter(|row| row.leaf == "refusal_late") {
        let name = name_of(&row.path);
        let rows: Vec<&CatalogRow> = catalogue.iter().filter(|r| r.enum_name == name).collect();
        assert!(
            !rows.is_empty(),
            "{} is in this leaf's scope and has no row in the catalogue",
            row.path
        );
        let own = |r: &&&CatalogRow| !r.delegates;
        let authored = rows
            .iter()
            .filter(own)
            .filter(|r| r.code != INTERNAL_CODE)
            .count();
        let internal = rows
            .iter()
            .filter(own)
            .filter(|r| r.code == INTERNAL_CODE)
            .count();
        for r in rows.iter().filter(own) {
            assert_eq!(
                r.code == INTERNAL_CODE,
                r.side == Side::Candler,
                "{}::{} has code {:?} and side {}: the fallback code and the side `candler` go together",
                row.path,
                r.variant,
                r.code,
                r.side
            );
        }
        match row.author.as_str() {
            "yes" => assert!(
                authored > 0,
                "{} is an author's enum by the oracle and codes no variant",
                row.path
            ),
            "no" => assert!(
                authored == 0 && internal > 0,
                "{} is not an author's enum by the oracle and codes a variant for the author",
                row.path
            ),
            "mixed" => assert!(
                authored > 0 && internal > 0,
                "{} is a mixed enum by the oracle and has {authored} author and {internal} internal variants",
                row.path
            ),
            other => panic!("{}: the oracle's author column holds {other:?}", row.path),
        }
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

/// Guards: R1 — a variant that only wraps a coded error answers, through the walk of causes,
/// with the wrapped error's own code, side and path, not with the side its wrapper was written
/// with: an unknown kind found by the registry is the suite's, whoever wraps it, and a lock held
/// on the experiment is the environment's, though the orchestration wraps it as the
/// experiment's.
///
/// Oracle: the sides written by hand from the contract (`suite.kind_unknown` is the suite's,
/// `runner.experiment_locked` the environment's); the errors are built by hand.
///
/// Bite: make `coded` of the module return the wrapper (`Some(wrapper)`) instead of what it
/// wraps.
#[cfg(unix)]
#[test]
fn r1_a_wrapper_answers_with_the_side_of_what_it_wraps() {
    use candler_core::evaluation::engine::EvaluateError;
    use candler_core::evaluation::registry::RegistryError;
    use candler_core::jobs::orchestrate::session::OrchestrateError;
    use candler_core::runner::plan::PlanError;

    let kind = RegistryError::UnknownKind {
        entry: "R1".parse().unwrap(),
        evaluator: "seam".parse().unwrap(),
        kind: "check".parse().unwrap(),
    };
    let wrapped = EvaluateError::Registry(kind);
    let found = candler_core::refusal::find(&wrapped).expect("a coded error");
    assert_eq!(found.code().as_str(), "suite.kind_unknown");
    assert_eq!(found.side(), Side::Suite);

    // The core's wrappers: `ExecError::Driver` is written with the side `experiment`, and a
    // driver that cannot read the harness version is the environment's.
    let version = candler_core::runner::exec::ExecError::Driver(
        candler_harness::driver::DriverError::VersionOutput {
            detail: "no output".to_owned(),
        },
    );
    let found = candler_core::refusal::find(&version).expect("a coded error");
    assert_eq!(found.code().as_str(), "harness.version_unreadable");
    assert_eq!(found.side(), Side::Environment);

    let locked = OrchestrateError::Plan(PlanError::ExperimentLocked);
    let found = candler_core::refusal::find(&locked).expect("a coded error");
    assert_eq!(found.code().as_str(), "runner.experiment_locked");
    assert_eq!(found.side(), Side::Environment);
}

/// An error that is a refusal.
#[cfg(unix)]
trait Both: std::error::Error + Refusal + 'static {}

#[cfg(unix)]
impl<T: std::error::Error + Refusal + 'static> Both for T {}

/// One wrapper variant built by hand, with the code the walk of causes must give for it.
#[cfg(unix)]
struct Wrapped {
    /// The enum of the wrapper.
    enum_name: &'static str,
    /// The variant of the wrapper.
    variant: &'static str,
    /// The wrapper, holding the error it wraps.
    error: Box<dyn Both>,
    /// The pair `(code, side)` that the wrapper's own `Refusal` impl gives.
    direct: (String, String),
    /// The code the record carries: the wrapped error's, or the wrapper's own when the cause is a
    /// format error.
    code: &'static str,
}

/// The wrappers of every `delegate` row of the catalogue, and of every wrapper of a format
/// error, each built over a cause.
#[cfg(unix)]
fn wrapped_cases() -> Vec<Wrapped> {
    use candler_core::evaluation::context::ContextError;
    use candler_core::evaluation::engine::{EvaluateError, PlanError as EvalPlanError};
    use candler_core::evaluation::registry::{NeedError, RegistryError};
    use candler_core::experiment::surface::SurfaceError;
    use candler_core::import::ImportError;
    use candler_core::isolation::IsolationError;
    use candler_core::isolation::container::mounts::MountError;
    use candler_core::isolation::container::run::RunArgvError;
    use candler_core::isolation::container::sweeper::SweepError;
    use candler_core::isolation::gateway::{GatewayError, NotReadyCause, SecretError};
    use candler_core::jobs::dir::JobError;
    use candler_core::jobs::lost::RecoverError;
    use candler_core::jobs::orchestrate::session::OrchestrateError;
    use candler_core::jobs::slots::SlotError;
    use candler_core::resolve::preflight::ResolveError;
    use candler_core::runner::exec::ExecError;
    use candler_core::runner::plan::PlanError;
    use candler_core::store::StoreError;
    use candler_core::supervise::SuperviseError;
    use candler_harness::driver::DriverError;
    use candler_harness::spec::CommandSpecError;
    use candler_schemas::canonical::CanonicalError;
    use candler_schemas::evaluation::EvaluationError;
    use candler_schemas::execution::ExecutionError;
    use candler_schemas::outcome::OutcomeError;
    use candler_schemas::primitives::{FormatError, Name, RunKey};
    use candler_schemas::values::ValuesError;

    let format = || -> FormatError { "BAD NAME".parse::<Name>().unwrap_err() };
    let finite = || CanonicalError::NotFinite;
    let absolute = || ExecutionError::AbsolutePath {
        path: "model.effort".to_owned(),
    };
    let git = || ExecutionError::GitCommit {
        problem: "is short",
        value: "abc".to_owned(),
    };
    let sealed = || StoreError::RunSealed {
        run: "r".to_owned(),
    };
    let case = |enum_name, variant, error: Box<dyn Both>, code| Wrapped {
        enum_name,
        variant,
        direct: (
            error.code().as_str().to_owned(),
            error.side().as_str().to_owned(),
        ),
        error,
        code,
    };
    let candler_format = || -> FormatError { "x".parse::<RunKey>().unwrap_err() };
    let evaluate_registry = || RegistryError::UnknownKind {
        entry: "R1".parse().unwrap(),
        evaluator: "e".parse().unwrap(),
        kind: "k".parse().unwrap(),
    };
    vec![
        // The schemas'.
        case(
            "ExecutionError",
            "Format",
            Box::new(ExecutionError::Format(format())),
            "experiment.value_invalid",
        ),
        case(
            "ExecutionError",
            "Values",
            Box::new(ExecutionError::Values(ValuesError::Null)),
            "values.null",
        ),
        case(
            "ExecutionError",
            "Canonical",
            Box::new(ExecutionError::Canonical(finite())),
            "canonical.number_not_finite",
        ),
        case(
            "EvaluationError",
            "Canonical",
            Box::new(EvaluationError::Canonical(finite())),
            "canonical.number_not_finite",
        ),
        case(
            "OutcomeError",
            "Format",
            Box::new(OutcomeError::Format(format())),
            "outcome.value_invalid",
        ),
        case(
            "ValuesError",
            "Canonical",
            Box::new(ValuesError::Canonical(finite())),
            "canonical.number_not_finite",
        ),
        // The core's.
        case(
            "SurfaceError",
            "Format",
            Box::new(SurfaceError::Format(format())),
            "experiment.setting_value_invalid",
        ),
        case(
            "ResolveError",
            "Value",
            Box::new(ResolveError::Value(ValuesError::Null)),
            "values.null",
        ),
        case(
            "ResolveError",
            "Format",
            Box::new(ResolveError::Format(format())),
            "resolve.value_invalid",
        ),
        case(
            "StoreError",
            "Format",
            Box::new(StoreError::Format(format())),
            "store.value_invalid",
        ),
        case(
            "ExecError",
            "Isolation",
            Box::new(ExecError::Isolation(IsolationError::Docker {
                detail: "d".to_owned(),
            })),
            "isolation.docker_step_failed",
        ),
        case(
            "ExecError",
            "Store",
            Box::new(ExecError::Store(sealed())),
            "store.run_sealed",
        ),
        case(
            "ExecError",
            "Driver",
            Box::new(ExecError::Driver(DriverError::VersionOutput {
                detail: "d".to_owned(),
            })),
            "harness.version_unreadable",
        ),
        case(
            "ExecError",
            "Manifest",
            Box::new(ExecError::Manifest(git())),
            "manifest.git_commit_invalid",
        ),
        case(
            "ExecError",
            "Supervise",
            Box::new(ExecError::Supervise(SuperviseError::Spawn {
                program: "p".to_owned(),
                source: std::io::Error::other("x"),
            })),
            "supervise.spawn_failed",
        ),
        case(
            "GatewayError",
            "Secret",
            Box::new(GatewayError::Secret(SecretError::Missing {
                name: "n".to_owned(),
            })),
            "isolation.secret_missing",
        ),
        case(
            "GatewayError",
            "NotReady",
            Box::new(GatewayError::NotReady {
                cause: NotReadyCause::Capped,
            }),
            "isolation.gateway_capped",
        ),
        case(
            "RunArgvError",
            "Mount",
            Box::new(RunArgvError::Mount(MountError::OutsideRoots {
                path: "/p".into(),
            })),
            "isolation.mount_outside_roots",
        ),
        case(
            "DriverError",
            "Command",
            Box::new(DriverError::Command(CommandSpecError::EmptyArgv)),
            "candler.internal",
        ),
        case(
            "PlanError",
            "Cell",
            Box::new(PlanError::Cell { source: absolute() }),
            "experiment.absolute_path",
        ),
        case(
            "PlanError",
            "Cell",
            Box::new(PlanError::Cell { source: git() }),
            "manifest.git_commit_invalid",
        ),
        case(
            "PlanError",
            "Cell",
            Box::new(PlanError::Cell {
                source: ExecutionError::Format(format()),
            }),
            "experiment.value_invalid",
        ),
        // A format error that blames Candler answers as it is, under every document.
        case(
            "ExecutionError",
            "Format",
            Box::new(ExecutionError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        case(
            "OutcomeError",
            "Format",
            Box::new(OutcomeError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        case(
            "SurfaceError",
            "Format",
            Box::new(SurfaceError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        case(
            "ResolveError",
            "Format",
            Box::new(ResolveError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        case(
            "StoreError",
            "Format",
            Box::new(StoreError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        case(
            "ImportError",
            "Format",
            Box::new(ImportError::Format(candler_format())),
            "format.run_key_invalid",
        ),
        // The evaluation's.
        case(
            "EvaluateError",
            "Registry",
            Box::new(EvaluateError::Registry(evaluate_registry())),
            "suite.kind_unknown",
        ),
        case(
            "EvaluateError",
            "Suite",
            Box::new(EvaluateError::Suite {
                path: "s.toml".into(),
                source: EvaluationError::EmptyList {
                    field: "expectations",
                },
            }),
            "suite.list_empty",
        ),
        case(
            "EvaluateError",
            "Store",
            Box::new(EvaluateError::Store(sealed())),
            "store.run_sealed",
        ),
        case(
            "EvaluateError",
            "Plan",
            Box::new(EvaluateError::Plan(EvalPlanError::Cycle { cycle: vec![] })),
            "evaluation.dependency_cycle",
        ),
        case(
            "EvaluateError",
            "Describe",
            Box::new(EvaluateError::Describe {
                plugin: "p".parse().unwrap(),
                source: candler_core::evaluation::client::ClientError::Cancelled,
            }),
            "evaluation.cancelled",
        ),
        case(
            "EvaluateError",
            "Slots",
            Box::new(EvaluateError::Slots(SlotError::Io {
                op: "open",
                path: "/p".into(),
                detail: "d".to_owned(),
            })),
            "jobs.slot_io_failed",
        ),
        case(
            "RegistryError",
            "Manifest",
            Box::new(RegistryError::Manifest {
                path: "m.toml".into(),
                source: EvaluationError::EmptyList { field: "f" },
            }),
            "suite.list_empty",
        ),
        case(
            "RegistryError",
            "Need",
            Box::new(RegistryError::Need {
                entry: "R1".parse().unwrap(),
                kind: "k".parse().unwrap(),
                source: NeedError::BadGlob {
                    glob: "[".to_owned(),
                    detail: "d".to_owned(),
                },
            }),
            "plugin.need_glob_invalid",
        ),
        case(
            "ContextError",
            "Store",
            Box::new(ContextError::Store(sealed())),
            "store.run_sealed",
        ),
        // The jobs'.
        case(
            "RecoverError",
            "Job",
            Box::new(RecoverError::Job(JobError::Held)),
            "jobs.job_held",
        ),
        case(
            "RecoverError",
            "Sweep",
            Box::new(RecoverError::Sweep(SweepError::List {
                detail: "d".to_owned(),
            })),
            "isolation.sweep_list_failed",
        ),
        case(
            "OrchestrateError",
            "Plan",
            Box::new(OrchestrateError::Plan(PlanError::ExperimentLocked)),
            "runner.experiment_locked",
        ),
        case(
            "OrchestrateError",
            "Plan",
            Box::new(OrchestrateError::Plan(PlanError::Cell { source: git() })),
            "manifest.git_commit_invalid",
        ),
        // The import's.
        case(
            "ImportError",
            "Format",
            Box::new(ImportError::Format(format())),
            "import.value_invalid",
        ),
    ]
}

/// Every `(code, side)` pair `codes.toml` registers, with the fallback code and the side
/// `candler`.
fn registered_pairs() -> BTreeSet<(String, String)> {
    let mut pairs: BTreeSet<(String, String)> = code_rows(&codes())
        .into_iter()
        .map(|row| (row.code, row.side))
        .collect();
    pairs.insert((INTERNAL_CODE.to_owned(), "candler".to_owned()));
    pairs
}

/// Guards: R1 — the invariant of every record: the `(code, side)` pair a record carries is a row
/// of `codes.toml`, so a consumer that maps a code to its side gets one answer. Every wrapper
/// variant of the workspace (every `delegate` row of the catalogue) answers through the walk of
/// causes with the code and side of the error it wraps, and a wrapper of a format error, which
/// has no document of its own, answers with its own code and the side of its document; a cause
/// that blames Candler answers with its own code and the side `candler`.
///
/// Oracle: the pairs of `codes.toml`, which is read and never generated, and the codes written by
/// hand for each case; the wrappers and their causes are built by hand. A `delegate` row with no
/// case here fails the guard.
///
/// Bite: `through` returning the wrapper; the `through` calls of the schemas' wrappers removed;
/// the round-two rule restored, where a wrapper of a format error answers with the cause's code
/// and its own side.
#[cfg(unix)]
#[test]
fn r1_every_wrapper_answers_with_a_registered_code_and_side() {
    let pairs = registered_pairs();
    let mut catalogue = catalogue();
    catalogue.extend(candler_harness::refusal::catalog());
    let cases = wrapped_cases();
    for row in catalogue.iter().filter(|row| row.delegates) {
        assert!(
            cases
                .iter()
                .any(|case| case.enum_name == row.enum_name && case.variant == row.variant),
            "the delegate row {}::{} has no case in this guard",
            row.enum_name,
            row.variant
        );
    }
    for case in &cases {
        let who = format!("{}::{} over {}", case.enum_name, case.variant, case.code);
        let wrapper: &(dyn std::error::Error + 'static) = &*case.error;
        let found = candler_core::refusal::find(wrapper)
            .unwrap_or_else(|| panic!("{who}: the walk finds no refusal"));
        assert_eq!(found.code().as_str(), case.code, "{who}: code");
        let pair = (
            found.code().as_str().to_owned(),
            found.side().as_str().to_owned(),
        );
        assert!(
            pairs.contains(&pair),
            "{who}: the record ({}, {}) is no row of codes.toml",
            pair.0,
            pair.1
        );
        // The wrapper's own impl answers with a row too, though the schemas crate's wrappers
        // cannot be changed from here and keep the code of their cause with their own side.
        let schemas_owned = [
            "ExecutionError",
            "EvaluationError",
            "OutcomeError",
            "ValuesError",
        ];
        if !schemas_owned.contains(&case.enum_name) {
            assert!(
                pairs.contains(&case.direct),
                "{who}: the wrapper's own impl gives ({}, {}), which is no row of codes.toml",
                case.direct.0,
                case.direct.1
            );
        }
    }
}
