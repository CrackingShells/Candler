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
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::JsonValue;
use serde_json::Value;

/// The manifest path of the tool list the harness reported at start.
const INIT_TOOLS_PATH: &str = "census.init_tools";

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
}

impl ArmView {
    /// Reads the three things a verdict needs from a manifest: `comparable`, `factors` and the
    /// measured `census.init_tools` (absent is `unrecorded`).
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
        Ok(Self {
            // BORROW: .clone() gives the view its own cell
            comparable: comparable.clone(),
            // BORROW: .to_vec() gives the view its own list
            claimed_factors: manifest.factors().to_vec(),
            init_tools,
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
    let differ = leaf_key(one.init_tools().map(JsonValue::as_value))
        != leaf_key(other.init_tools().map(JsonValue::as_value));
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

/// Whether the arms of a stratum are comparable, given the factors the experiment declares.
///
/// Every arm is diffed against the first. A deviation is dropped when a declared factor covers
/// it; the arms' own `factors[]` is never read. `census.init_tools` differing between any two
/// arms is a deviation unless a declared `tools.*` or `mcp.*` factor itself differs between those
/// two arms, so two arms that agree on every such factor must agree on the tool list. The check
/// always runs.
/// An arm that could not be read gives no verdict, whatever the other arms hold, and a stratum
/// with no arm has none either.
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
