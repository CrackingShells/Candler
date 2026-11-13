//! The diff of two evaluator versions on `(status, value, weight, cause)`, with an equivalence file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{ExpectationId, FormatError, RunKey};
use serde::{Deserialize, Serialize, Serializer};
use sha2::{Digest as _, Sha256};

use super::fields::FourFields;
use super::select::Selection;

/// A row of the join: a run and an expectation.
type RowKey = (RunKey, ExpectationId);

/// Why a change was reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChangeClass {
    /// The status differs.
    Status,
    /// The value differs, a number's kind included.
    Value,
    /// The weight differs after normalisation.
    Weight,
    /// The cause differs, in kind or in content.
    Cause,
    /// The row is in `from` and has no match in `to`.
    Missing,
    /// The row is in `to` and has no match in `from`.
    Extra,
    /// A row the equivalence file lists is absent on a side.
    EquivalenceMissing,
    /// A row the equivalence file lists does not show the rule's exact pair.
    EquivalenceMismatch,
}

/// One row that is neither unchanged nor excused.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Change {
    /// Why the row is reported. A row that differs in several fields gets the first class in the
    /// order status, value, weight, cause; both sides are in the change.
    pub class: ChangeClass,
    /// The run of the row, in its path form.
    #[serde(serialize_with = "run_text")]
    pub run: RunKey,
    /// The expectation of the row.
    pub expectation: ExpectationId,
    /// The fields in `from`, or `null` when the row is absent there.
    pub from: Option<FourFields>,
    /// The fields in `to`, or `null` when the row is absent there.
    pub to: Option<FourFields>,
}

/// Writes a run key in its path form.
fn run_text<S: Serializer>(run: &RunKey, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(run)
}

/// An expectation that is in one suite and not the other, and how many rows it leaves uncompared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Uncompared {
    /// The expectation.
    pub expectation: ExpectationId,
    /// The count of `(run, expectation)` rows left out, on either side.
    pub count: u64,
}

/// The equivalence file a diff used: its sha256 and how many rows it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct EquivalenceRecord {
    /// The sha256 of the file's bytes, as lowercase hex.
    pub sha256: String,
    /// The count of rows the file lists.
    pub rows: u64,
}

/// The result of a diff: `unchanged + excused + changed == compared`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct DiffReport {
    /// The rows present on either side for an expectation both suites hold.
    pub compared: u64,
    /// The compared rows whose four fields are equal.
    pub unchanged: u64,
    /// The listed rows that show exactly the rule's pair.
    pub excused: u64,
    /// The compared rows in `changes`.
    pub changed: u64,
    /// One entry per changed row, in `(run, expectation)` order.
    pub changes: Vec<Change>,
    /// The expectations only one suite holds, with their row counts, in expectation order.
    pub uncompared: Vec<Uncompared>,
    /// The equivalence file used, or `null` when none was given.
    pub equivalence: Option<EquivalenceRecord>,
}

/// Why an equivalence file was refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EquivalenceError {
    /// The file cannot be read.
    #[error("cannot read the equivalence file {}: {source}", path.display())]
    Io {
        /// The path that was read.
        path: PathBuf,
        /// The error of the read.
        source: std::io::Error,
    },
    /// The file is not the equivalence format, an unknown key included.
    #[error("the equivalence file is not valid: {detail}")]
    Parse {
        /// What the parser reported.
        detail: String,
    },
    /// A row names a run that is not in the path form.
    #[error("the equivalence row names the run {value:?}, which is not a run key: {source}")]
    Run {
        /// The text of the run.
        value: String,
        /// Why it is not a run key.
        source: FormatError,
    },
    /// A row is listed twice, in one rule or in two.
    #[error("the equivalence file lists the row ({run}, {expectation}) twice")]
    DuplicateRow {
        /// The run of the row, in its path form.
        run: String,
        /// The expectation of the row.
        expectation: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    // The generator is provenance only; a reader never uses it, and the file may name it.
    #[serde(default, rename = "generator")]
    _generator: Option<String>,
    rules: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    name: String,
    from: FourFields,
    to: FourFields,
    rows: Vec<RawRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    run: String,
    expectation: ExpectationId,
}

/// One rule: the only change it allows, from the first pair member to the second.
#[derive(Debug, Clone)]
struct Rule {
    from: FourFields,
    to: FourFields,
}

/// The rows a diff may excuse, each with the one change it may show, and the sha256 of the file
/// they came from.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Equivalence {
    rules: Vec<Rule>,
    rows: BTreeMap<RowKey, usize>,
    sha256: String,
    names: Vec<String>,
}

/// The lowercase hex of `bytes`.
fn hex(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut text = String::new();
    for byte in bytes {
        text.extend(char::from_digit(u32::from(byte >> 4), 16));
        text.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    text
}

impl Equivalence {
    /// Reads an equivalence file.
    ///
    /// # Errors
    /// Returns [`EquivalenceError`] if the file cannot be read, holds an unknown key anywhere,
    /// names a run that is not a run key, or lists a row twice.
    pub fn load(path: &Path) -> Result<Self, EquivalenceError> {
        let bytes = fs::read(path).map_err(|source| EquivalenceError::Io {
            path: path.to_path_buf(), // BORROW: the error owns the path
            source,
        })?;
        Self::from_bytes(&bytes)
    }

    /// Reads the bytes of an equivalence file, keeping their sha256.
    ///
    /// # Errors
    /// Returns the same errors as [`Equivalence::load`], except the read failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EquivalenceError> {
        let file: RawFile = serde_json::from_slice(bytes).map_err(|e| EquivalenceError::Parse {
            detail: e.to_string(),
        })?;
        let mut rules = Vec::new();
        let mut names = Vec::new();
        let mut rows = BTreeMap::new();
        for (index, rule) in file.rules.into_iter().enumerate() {
            for row in rule.rows {
                // BORROW: explicit .as_str() instead of Deref coercion
                let run =
                    row.run
                        .as_str()
                        .parse::<RunKey>()
                        .map_err(|source| EquivalenceError::Run {
                            value: row.run.clone(), // BORROW: the error owns the text
                            source,
                        })?;
                let key = (run, row.expectation);
                if rows.contains_key(&key) {
                    return Err(EquivalenceError::DuplicateRow {
                        run: key.0.to_string(),
                        expectation: key.1.to_string(),
                    });
                }
                rows.insert(key, index);
            }
            names.push(rule.name);
            rules.push(Rule {
                from: rule.from.normalised(),
                to: rule.to.normalised(),
            });
        }
        Ok(Self {
            rules,
            rows,
            sha256: hex(Sha256::digest(bytes)),
            names,
        })
    }

    /// The sha256 of the file's bytes, as lowercase hex.
    #[must_use]
    pub fn sha256<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.sha256.as_str()
    }

    /// The count of rows the file lists.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        u64::try_from(self.rows.len()).unwrap_or(u64::MAX)
    }

    /// The names of the rules, in file order.
    #[must_use]
    pub const fn rule_names<'a>(&'a self) -> &'a [String] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.names.as_slice()
    }

    /// The rule that lists `key`, if one does.
    fn rule_for<'a>(&'a self, key: &RowKey) -> Option<&'a Rule> {
        self.rows.get(key).and_then(|index| self.rules.get(*index))
    }
}

/// The class of the first field in which two unequal rows differ.
fn class_of(from: &FourFields, to: &FourFields) -> ChangeClass {
    if from.status != to.status {
        ChangeClass::Status
    } else if !from.value_equals(to) {
        ChangeClass::Value
    } else if from.weight != to.weight {
        ChangeClass::Weight
    } else {
        ChangeClass::Cause
    }
}

/// The row of each outcome of a one-evaluator selection, keyed by `(run, expectation)`.
fn latest_rows(selection: &Selection) -> BTreeMap<RowKey, FourFields> {
    selection
        .outcomes()
        .map(|record| (row_key(record), FourFields::of(record)))
        .collect()
}

/// The join key of a record.
fn row_key(record: &OutcomeRecord) -> RowKey {
    // BORROW: the key owns copies of the run and the expectation
    (record.run().clone(), record.expectation_id().clone())
}

/// Diffs two evaluator versions on `(status, value, weight, cause)`.
///
/// Rows are joined on `(run, expectation)`, with the latest current record of each side; hand in
/// one evaluator version per side (see [`Selection::for_evaluator`]). Weights equal to their
/// status default are absent on both sides, evidence is never compared, and a number keeps its
/// kind. A version's suite is the expectations it has a record for. A row of an expectation both
/// suites hold is compared: a row on one side only is `missing` (in `from`) or `extra` (in `to`),
/// and an expectation only one suite holds is left out and listed in `uncompared` with its row
/// count. With an equivalence file, a listed row is excused only when both sides exist and equal
/// the rule's `from` and `to` exactly; any other listed row is an `equivalence_missing` (a side is
/// absent) or `equivalence_mismatch` change, and an unlisted row is never excused. A listed row
/// that is on neither side is not a row of the join and is not counted. Nothing compared is
/// reported as `compared == 0`; the caller decides what that means.
#[must_use]
pub fn diff(from: &Selection, to: &Selection, equivalence: Option<&Equivalence>) -> DiffReport {
    let rows_from = latest_rows(from);
    let rows_to = latest_rows(to);
    let suite_from: BTreeSet<&ExpectationId> = rows_from.keys().map(|(_, e)| e).collect();
    let suite_to: BTreeSet<&ExpectationId> = rows_to.keys().map(|(_, e)| e).collect();
    let keys: BTreeSet<&RowKey> = rows_from.keys().chain(rows_to.keys()).collect();

    let mut report = DiffReport {
        compared: 0,
        unchanged: 0,
        excused: 0,
        changed: 0,
        changes: Vec::new(),
        uncompared: Vec::new(),
        equivalence: equivalence.map(|e| EquivalenceRecord {
            sha256: e.sha256.clone(), // BORROW: the report owns the digest text
            rows: e.row_count(),
        }),
    };
    let mut left_out: BTreeMap<&ExpectationId, u64> = BTreeMap::new();
    for key in keys {
        let expectation = &key.1;
        if !(suite_from.contains(expectation) && suite_to.contains(expectation)) {
            let count = left_out.entry(expectation).or_default();
            *count = count.saturating_add(1);
            continue;
        }
        report.compared = report.compared.saturating_add(1);
        let left = rows_from.get(key);
        let right = rows_to.get(key);
        let rule = equivalence.and_then(|e| e.rule_for(key));
        let verdict = judge(left, right, rule);
        match verdict {
            Verdict::Unchanged => report.unchanged = report.unchanged.saturating_add(1),
            Verdict::Excused => report.excused = report.excused.saturating_add(1),
            Verdict::Changed(class) => {
                report.changed = report.changed.saturating_add(1);
                report.changes.push(Change {
                    class,
                    run: key.0.clone(),         // BORROW: the change owns the run
                    expectation: key.1.clone(), // BORROW: the change owns the expectation
                    from: left.cloned(),
                    to: right.cloned(),
                });
            }
        }
    }
    report.uncompared = left_out
        .into_iter()
        .map(|(expectation, count)| Uncompared {
            expectation: expectation.clone(), // BORROW: the entry owns the expectation
            count,
        })
        .collect();
    report
}

/// What the diff decided about one compared row.
enum Verdict {
    Unchanged,
    Excused,
    Changed(ChangeClass),
}

/// Decides one row from its two sides and the rule that lists it, if any.
fn judge(left: Option<&FourFields>, right: Option<&FourFields>, rule: Option<&Rule>) -> Verdict {
    match (left, right, rule) {
        (Some(a), Some(b), Some(rule)) => {
            if a == &rule.from && b == &rule.to {
                Verdict::Excused
            } else {
                Verdict::Changed(ChangeClass::EquivalenceMismatch)
            }
        }
        (Some(a), Some(b), None) => {
            if a == b {
                Verdict::Unchanged
            } else {
                Verdict::Changed(class_of(a, b))
            }
        }
        (Some(_), None, Some(_)) | (None, Some(_), Some(_)) => {
            Verdict::Changed(ChangeClass::EquivalenceMissing)
        }
        (Some(_), None, None) => Verdict::Changed(ChangeClass::Missing),
        (None, Some(_), None) => Verdict::Changed(ChangeClass::Extra),
        // A key comes from one of the two sides, so a row absent on both does not occur.
        (None, None, Some(_) | None) => Verdict::Unchanged,
    }
}
