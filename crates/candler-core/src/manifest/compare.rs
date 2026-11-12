//! Whether two runs may be compared: their comparable records and the verdict per stratum.
//!
//! Paths are compared by whole segments, never by characters: `model.id` covers `model.id.x` and
//! never `model.idx`. A value's path is its own dotted path; the harness contributes
//! `harness.<field>`, and the treatment, the snapshots, the inputs and the environments each
//! contribute one leaf at their root name.

use std::collections::BTreeMap;
use std::str::FromStr;

use candler_schemas::canonical;
use candler_schemas::execution::{Comparable, Deviation, RunManifest};
use candler_schemas::primitives::{Digest, FieldPath};
use candler_schemas::values::JsonValue;
use serde_json::Value;

/// The manifest path of the tool list the harness reported at start.
const INIT_TOOLS_PATH: &str = "census.init_tools";

/// The manifest path of who observed the tool list, recorded when it was not the harness itself.
/// An absent source is the harness's own report.
const INIT_SOURCE_PATH: &str = "census.source";

/// The measured path of the commit the evaluation infrastructure was built from. A verdict needs
/// it recorded, and equal across the runs of a stratum.
const EVAL_INFRA_COMMIT_PATH: &str = "provenance.eval_infra.commit";

/// The measured path that says whether the evaluation infrastructure had uncommitted changes. A
/// verdict needs it recorded, and false in every run.
const EVAL_INFRA_DIRTY_PATH: &str = "provenance.eval_infra.dirty";

/// The measured path of the sandbox contract, which must agree across the runs of a stratum.
const SANDBOX_CONTRACT_PATH: &str = "provenance.sandbox_contract";

/// The path an imported stratum is refused at when it mixes imported and live arms.
const IMPORT_PATH: &str = "import";

/// The path an imported stratum is refused at when the arms were not read with one set of import
/// rules, or an arm does not record them.
const IMPORT_RULES_PATH: &str = "import.rules";

/// The first segments of the declared factors that change the tool list the harness reports, and
/// so exempt `census.init_tools`.
const TOOL_FACTOR_ROOTS: &[&str] = &["tools", "mcp"];

/// The roots a value of `comparable` never takes: the typed fields own them.
const TYPED_ROOTS: &[&str] = &[
    "harness",
    "treatment",
    "snapshots",
    "inputs",
    "environments",
];

/// Why a verdict could not be reached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VerdictError {
    /// An arm whose record cannot be compared; the stratum has no verdict.
    #[error("malformed arm: {problem} ({path})")]
    Malformed {
        /// What is wrong.
        problem: &'static str,
        /// The list, path or manifest field it concerns; empty when it concerns the stratum.
        path: String,
    },
}

/// Whether the arms of a stratum are comparable.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// The arms differ only in the declared factors.
    Comparable,
    /// The arms differ elsewhere, at these paths, sorted and without duplicates.
    Refused(Vec<FieldPath>),
    /// An arm could not be read; no verdict was computed for the stratum.
    Malformed(Vec<VerdictError>),
}

/// What the comparison reads of one arm.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmView {
    comparable: Comparable,
    claimed_factors: Vec<FieldPath>,
    init_tools: Option<JsonValue>,
    init_source: Option<JsonValue>,
    provenance: BTreeMap<String, JsonValue>,
    imported: bool,
    import_rules: Option<Digest>,
}

/// Whether a measured path is one a verdict reads from the provenance block.
///
/// These three say whether two runs came from the same build of the same contract, so they gate.
/// Every other `provenance.*` path describes a run and gates nothing: `provenance.eval_infra.tag`
/// and `provenance.eval_infra.commits_ahead` (a git build and a packaged build of one commit
/// differ in them and still pool) and `provenance.packages.*` (the analysis reads it).
fn gates_verdict(path: &str) -> bool {
    path == EVAL_INFRA_COMMIT_PATH || path == EVAL_INFRA_DIRTY_PATH || path == SANDBOX_CONTRACT_PATH
}

impl ArmView {
    /// Reads what a verdict needs from a manifest: `comparable`, `factors`, the measured
    /// `census.init_tools` (absent is `unrecorded`) and, for an imported run, the digest of the
    /// import rules.
    ///
    /// # Errors
    /// [`VerdictError::Malformed`] when a value sits at a root the typed fields own, or when
    /// `census.init_tools` is measured twice. The order and uniqueness of the `comparable` lists
    /// are the schema reader's to refuse: a [`Comparable`] cannot be built without it.
    pub fn from_manifest(manifest: &RunManifest) -> Result<Self, VerdictError> {
        let comparable = manifest.comparable();
        for value in comparable.values() {
            let root = value.path().segments().next().unwrap_or_default();
            if TYPED_ROOTS.contains(&root) {
                return Err(VerdictError::Malformed {
                    problem: "a value sits at a root the typed fields own",
                    path: value.path().to_string(),
                });
            }
        }
        let mut measured = manifest
            .measured()
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .filter(|m| m.path().as_str() == INIT_TOOLS_PATH);
        let init_tools = match (measured.next(), measured.next()) {
            (Some(_), Some(_)) => {
                return Err(VerdictError::Malformed {
                    problem: "a measured path appears twice",
                    path: INIT_TOOLS_PATH.to_owned(), // BORROW: .to_owned() makes the owned path text
                });
            }
            // BORROW: .clone() gives the view its own value
            (Some(only), None) => Some(only.value().clone()),
            (None, _) => None,
        };
        let init_source = manifest
            .measured()
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .find(|m| m.path().as_str() == INIT_SOURCE_PATH)
            .map(|m| m.value().clone()); // BORROW: .clone() gives the view its own value
        let mut provenance = BTreeMap::new();
        // EXPLICIT: the loop fills `provenance` and refuses a repeated path; .map() would hide the state update
        for entry in manifest.measured() {
            // BORROW: explicit .as_str() instead of Deref coercion
            let path = entry.path().as_str();
            if !gates_verdict(path) {
                continue;
            }
            // BORROW: .to_owned() and .clone() give the view its own path and value
            if provenance
                .insert(path.to_owned(), entry.value().clone())
                .is_some()
            {
                return Err(VerdictError::Malformed {
                    problem: "a measured path appears twice",
                    path: path.to_owned(), // BORROW: .to_owned() makes the owned path text
                });
            }
        }
        Ok(Self {
            // BORROW: .clone() gives the view its own cell
            comparable: comparable.clone(),
            // BORROW: .to_vec() gives the view its own list
            claimed_factors: manifest.factors().to_vec(),
            init_tools,
            init_source,
            provenance,
            imported: manifest.import().is_some(),
            // BORROW: .cloned() gives the view its own digest
            import_rules: manifest.import().and_then(|import| import.rules.clone()),
        })
    }

    /// The comparable record of the arm.
    #[must_use]
    pub const fn comparable<'a>(&'a self) -> &'a Comparable {
        &self.comparable
    }

    /// The factors the arm's own manifest claims. A verdict never reads them: a manifest cannot
    /// vouch for itself.
    #[must_use]
    pub fn claimed_factors<'a>(&'a self) -> &'a [FieldPath] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.claimed_factors.as_slice()
    }

    /// Whether the run was imported: its manifest holds an import record.
    #[must_use]
    pub const fn is_imported(&self) -> bool {
        self.imported
    }

    /// The sha256 of the import rules the run was read with, or `None` for a live run and for an
    /// import that did not record them.
    #[must_use]
    pub const fn import_rules<'a>(&'a self) -> Option<&'a Digest> {
        self.import_rules.as_ref()
    }

    /// The tool list the harness reported at start, or `None` when it was not recorded.
    #[must_use]
    pub const fn init_tools<'a>(&'a self) -> Option<&'a JsonValue> {
        self.init_tools.as_ref()
    }
}

/// The top-level manifest fields outside the cell hash: the set `hashed_paths.toml` calls
/// `nuisance`. No `comparable.*` path is in it. The verdict never reads it: it diffs
/// `comparable` alone, and a test pins this set to the hand-written one.
pub const NUISANCE_PATHS: &[&str] = &[
    "schema",
    "run",
    "experiment",
    "factors",
    "sources",
    "secrets",
    "measured",
    "deviations",
    "started_at",
    "ended_at",
    "host",
    "job",
    "evaluation_inputs",
    "integrity",
    "import",
];

/// Whether `declared` names `path` or an ancestor of it, comparing whole segments.
#[must_use]
pub fn covers(declared: &[&str], path: &[&str]) -> bool {
    let mut own = path.iter();
    declared.iter().all(|segment| own.next() == Some(segment))
}

/// The leaves of one arm by path segments; a path two parts both claim holds both leaves.
type Leaves = BTreeMap<Vec<String>, Vec<Option<Value>>>;

/// Serialises a part of a cell. The parts are plain data, so this cannot fail.
fn part_json<T: serde::Serialize>(part: &T) -> Value {
    match serde_json::to_value(part) {
        Ok(value) => value,
        Err(error) => unreachable!("a part of a cell is plain data: {error}"),
    }
}

/// Splits a dotted path into segments.
fn segments_of(path: &str) -> Vec<String> {
    // BORROW: .to_owned() makes the owned segment
    path.split('.').map(str::to_owned).collect()
}

/// Flattens a cell to its leaves: every value at its own path, each field of the harness at
/// `harness.<field>`, and the treatment, snapshots, inputs and environments each at their root.
fn flatten(cell: &Comparable) -> Leaves {
    let mut leaves = Leaves::new();
    let mut put = |path: Vec<String>, leaf: Option<Value>| {
        leaves.entry(path).or_default().push(leaf);
    };
    for value in cell.values() {
        put(
            // BORROW: explicit .as_str() instead of Deref coercion
            segments_of(value.path().as_str()),
            // BORROW: .clone() gives the leaf its own value
            value.value().map(|json| json.as_value().clone()),
        );
    }
    if let Value::Object(fields) = part_json(cell.harness()) {
        for (field, leaf) in fields {
            put(vec!["harness".to_owned(), field], Some(leaf)); // BORROW: .to_owned() makes the owned segment
        }
    }
    let wholes = [
        ("treatment", part_json(cell.treatment())),
        ("snapshots", part_json(&cell.snapshots())),
        ("inputs", part_json(&cell.inputs())),
        ("environments", part_json(&cell.environments())),
    ];
    for (root, leaf) in wholes {
        put(vec![root.to_owned()], Some(leaf)); // BORROW: .to_owned() makes the owned segment
    }
    leaves
}

/// The text two leaves are equal by: their RFC 8785 form, as the factors of a manifest are found.
fn leaf_key(leaf: Option<&Value>) -> Option<String> {
    leaf.map(|value| canonical::to_jcs(value).unwrap_or_else(|_| value.to_string()))
}

/// The leaves of a path as one value: the leaf itself, or the array of them when two parts claim
/// the path.
fn as_json(held: &[Option<Value>]) -> Option<JsonValue> {
    let value = match held {
        [] => return None,
        // BORROW: .clone() gives the deviation its own value
        [only] => only.clone()?,
        several => Value::Array(
            several
                .iter()
                // BORROW: .clone() gives the deviation its own values
                .map(|leaf| leaf.clone().unwrap_or(Value::Null))
                .collect(),
        ),
    };
    JsonValue::new(value).ok()
}

/// The paths where two cells differ, sorted by path and without duplicates.
///
/// A path held by one side only differs, and so does a value recorded on one side and
/// `unrecorded` on the other. Lists are compared in their stored order.
#[must_use]
pub fn compare(stored: &Comparable, observed: &Comparable) -> Vec<Deviation> {
    let left = flatten(stored);
    let right = flatten(observed);
    let mut paths: Vec<&Vec<String>> = left.keys().chain(right.keys()).collect();
    paths.sort();
    paths.dedup();
    let mut out = Vec::new();
    for path in paths {
        let before = left.get(path).map(Vec::as_slice).unwrap_or_default();
        let after = right.get(path).map(Vec::as_slice).unwrap_or_default();
        let keys = |held: &[Option<Value>]| -> Vec<Option<String>> {
            held.iter().map(|leaf| leaf_key(leaf.as_ref())).collect()
        };
        if keys(before) == keys(after) {
            continue;
        }
        let Ok(field) = FieldPath::from_str(&path.join(".")) else {
            unreachable!("the segments of a flattened path are field segments")
        };
        out.push(Deviation::new(field, as_json(before), as_json(after)));
    }
    out
}

/// Whether some declared factor covers the path.
fn declared_covers(declared: &[FieldPath], path: &FieldPath) -> bool {
    let own: Vec<&str> = path.segments().collect();
    declared.iter().any(|factor| {
        let prefix: Vec<&str> = factor.segments().collect();
        // BORROW: `.as_slice()` views each vector as a slice
        covers(prefix.as_slice(), own.as_slice())
    })
}

/// The paths where `arm` differs from `first` outside the declared factors.
fn residual(first: &ArmView, arm: &ArmView, declared: &[FieldPath]) -> Vec<FieldPath> {
    compare(first.comparable(), arm.comparable())
        .into_iter()
        .map(|d| d.path)
        .filter(|path| !declared_covers(declared, path))
        .collect()
}

/// Whether two arms differ in `census.init_tools` without a declared `tools.*` or `mcp.*` factor
/// that differs between them to account for it.
fn tools_unexplained(one: &ArmView, other: &ArmView, declared: &[FieldPath]) -> bool {
    // A tool list is the same observation only from the same source: a list the gateway saw is
    // never the harness's own, nor an unrecorded one.
    let key = |arm: &ArmView| {
        (
            leaf_key(arm.init_tools().map(JsonValue::as_value)),
            leaf_key(arm.init_source.as_ref().map(JsonValue::as_value)),
        )
    };
    let differ = key(one) != key(other);
    let factor_varies = compare(one.comparable(), other.comparable())
        .iter()
        .any(|d| {
            declared_covers(declared, &d.path)
                && d.path
                    .segments()
                    .next()
                    .is_some_and(|root| TOOL_FACTOR_ROOTS.contains(&root))
        });
    differ && !factor_varies
}

/// The provenance paths that keep a stratum from a verdict, each named by the path that refused.
/// A gated path never counts as equal when it is unrecorded: an arm that did not record it
/// identifies nothing, so it matches no other arm, itself included.
/// - `provenance.eval_infra.commit` when any arm did not record it (an empty string or a value that
///   is not a string is not a commit), or the arms record different ones;
/// - `provenance.eval_infra.dirty` when any arm did not record it, or any arm records anything
///   but false;
/// - `provenance.sandbox_contract` when any arm did not record it, or the arms record different
///   ones.
///
/// Nothing else of the provenance block is read: see [`gates_verdict`].
fn provenance_refused(views: &[&ArmView]) -> Vec<FieldPath> {
    fn held<'a>(arm: &'a ArmView, path: &str) -> Option<&'a Value> {
        arm.provenance.get(path).map(JsonValue::as_value)
    }
    // The verdict reads whether a value is there, never its format: the recorder owns that. A
    // commit must still be a non-empty string to identify a build.
    fn is_recorded(path: &str, value: Option<&Value>) -> bool {
        match value {
            None => false,
            Some(held) if path == EVAL_INFRA_COMMIT_PATH => {
                held.as_str().is_some_and(|text| !text.is_empty())
            }
            Some(_) => true,
        }
    }
    let mut paths: Vec<&str> = Vec::new();
    if views
        .iter()
        .any(|arm| held(arm, EVAL_INFRA_DIRTY_PATH) != Some(&Value::Bool(false)))
    {
        paths.push(EVAL_INFRA_DIRTY_PATH);
    }
    for path in [EVAL_INFRA_COMMIT_PATH, SANDBOX_CONTRACT_PATH] {
        let recorded = views.iter().all(|arm| is_recorded(path, held(arm, path)));
        let keys: Vec<Option<String>> = views.iter().map(|arm| leaf_key(held(arm, path))).collect();
        if !recorded || keys.windows(2).any(|pair| pair.first() != pair.get(1)) {
            paths.push(path);
        }
    }
    paths
        .into_iter()
        .filter_map(|path| FieldPath::from_str(path).ok())
        .collect()
}

/// The paths that keep a stratum of imported arms from a verdict, each named by the path that
/// refused. The import recorded its own identity, so the stratum is gated on that instead of on
/// the measured provenance:
/// - `import.rules` when any arm did not record the digest of its import rules, or the arms
///   record different ones;
/// - `provenance.eval_infra.commit`, `provenance.eval_infra.dirty` and
///   `provenance.sandbox_contract` when any arm did not record the path in `comparable.values`
///   (the commit must be a non-empty string), or the arms record different values. The gate
///   compares them itself, so no declared factor can pool arms of two legacy builds.
///
/// A dirty legacy build is not refused: one legacy launch is one checkout, so arms that record
/// the same commit, flag, contract and rules came from one tree.
fn imported_refused(views: &[&ArmView]) -> Vec<FieldPath> {
    fn held<'a>(arm: &'a ArmView, path: &str) -> Option<&'a Value> {
        arm.comparable()
            .values()
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .find(|value| value.path().as_str() == path)
            .and_then(|value| value.value())
            .map(JsonValue::as_value)
    }
    fn recorded(arm: &ArmView, path: &str) -> bool {
        match held(arm, path) {
            None => false,
            Some(json) if path == EVAL_INFRA_COMMIT_PATH => {
                json.as_str().is_some_and(|text| !text.is_empty())
            }
            Some(_) => true,
        }
    }
    let mut paths: Vec<&str> = Vec::new();
    let rules_agree = views.iter().all(|arm| {
        arm.import_rules().is_some()
            && arm.import_rules() == views.first().and_then(|first| first.import_rules())
    });
    if !rules_agree {
        paths.push(IMPORT_RULES_PATH);
    }
    for path in [
        EVAL_INFRA_COMMIT_PATH,
        EVAL_INFRA_DIRTY_PATH,
        SANDBOX_CONTRACT_PATH,
    ] {
        // The gate compares these itself: a declared factor never pools arms of two builds.
        let keys: Vec<Option<String>> = views.iter().map(|arm| leaf_key(held(arm, path))).collect();
        let agree = !keys.windows(2).any(|pair| pair.first() != pair.get(1));
        if !views.iter().all(|arm| recorded(arm, path)) || !agree {
            paths.push(path);
        }
    }
    paths
        .into_iter()
        .filter_map(|path| FieldPath::from_str(path).ok())
        .collect()
}

/// Whether the arms of a stratum are comparable, given the factors the experiment declares.
///
/// Every arm is diffed against the first. A deviation is dropped when a declared factor covers
/// it; the arms' own `factors[]` is never read. `census.init_tools` differing between any two
/// arms is a deviation unless a declared `tools.*` or `mcp.*` factor itself differs between those
/// two arms, so two arms that agree on every such factor must agree on the tool list. The check
/// always runs. A stratum of live arms is also refused on the provenance that says whether the
/// runs came from the same Candler: see `provenance_refused`. An arm that could not be read gives
/// no verdict, whatever the other arms hold, and a stratum with no arm has none either.
///
/// An arm is imported if and only if its manifest holds an import record. A stratum that mixes
/// imported and live arms is refused, naming `import`. A stratum of imported arms is gated by
/// `imported_refused` in place of the provenance gate: the import rules recorded and equal, and
/// the legacy commit, dirty flag and sandbox contract recorded. The dirty flag of a legacy build
/// does not refuse: arms that agree on commit, flag, contract and rules pool, on the assumption
/// that one legacy launch is one checkout; no declared factor pools arms of two legacy builds.
#[must_use]
pub fn stratum_verdict(arms: &[Result<ArmView, VerdictError>], declared: &[FieldPath]) -> Verdict {
    let errors: Vec<VerdictError> = arms
        .iter()
        .filter_map(|arm| arm.as_ref().err())
        .cloned() // BORROW: .cloned() gives the verdict its own errors
        .collect();
    if !errors.is_empty() {
        return Verdict::Malformed(errors);
    }
    let views: Vec<&ArmView> = arms.iter().filter_map(|arm| arm.as_ref().ok()).collect();
    let Some((first, rest)) = views.split_first() else {
        return Verdict::Malformed(vec![VerdictError::Malformed {
            problem: "the stratum has no arm",
            path: String::new(),
        }]);
    };
    let mut refused: Vec<FieldPath> = rest
        .iter()
        .flat_map(|arm| residual(first, arm, declared))
        .collect();
    let unexplained = views.iter().enumerate().any(|(at, one)| {
        views
            .iter()
            .skip(at + 1)
            .any(|other| tools_unexplained(one, other, declared))
    });
    let imported = views.iter().filter(|arm| arm.is_imported()).count();
    if imported == 0 {
        refused.extend(provenance_refused(&views));
    } else if imported == views.len() {
        refused.extend(imported_refused(&views));
    } else {
        let Ok(field) = FieldPath::from_str(IMPORT_PATH) else {
            unreachable!("`import` is a field path")
        };
        refused.push(field);
    }
    if unexplained {
        let Ok(field) = FieldPath::from_str(INIT_TOOLS_PATH) else {
            unreachable!("`census.init_tools` is a field path")
        };
        refused.push(field);
    }
    refused.sort();
    refused.dedup();
    if refused.is_empty() {
        Verdict::Comparable
    } else {
        Verdict::Refused(refused)
    }
}
