//! The decision summary of a report: what each tested expectation came to, and what was left out.

use candler_schemas::outcome::Role;
use candler_schemas::primitives::{ExpectationId, FieldPath};
use serde_json::Value;

use super::report::{Comparison, Entry, OmitReason};

/// One decision: an expectation tested between the two arms of a comparison.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Decision {
    /// The experiment digest of the comparability class of the comparison.
    pub experiment: String,
    /// The declared factor the arms differ in.
    pub factor: FieldPath,
    /// The levels of [`Self::factor`] that label arm A and arm B.
    pub levels: Option<(Value, Value)>,
    /// The expectation.
    pub id: ExpectationId,
    /// Its role.
    pub role: Role,
    /// The difference, arm B minus arm A, when the test gives one.
    pub diff: Option<f64>,
    /// The lower limit of the interval of [`Self::diff`].
    pub diff_lo: Option<f64>,
    /// The upper limit of the interval of [`Self::diff`].
    pub diff_hi: Option<f64>,
    /// The Holm-adjusted p over the family of the role.
    pub p_holm: f64,
    /// Whether the adjusted p is below alpha.
    pub reject: bool,
    /// The number of tests in the Holm family: the tested expectations of the comparison that
    /// have this role.
    pub family: usize,
}

/// One expectation of a comparison that was not tested.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Omitted {
    /// The experiment digest of the comparability class of the comparison.
    pub experiment: String,
    /// The declared factor the arms differ in.
    pub factor: FieldPath,
    /// The levels of [`Self::factor`] that label arm A and arm B.
    pub levels: Option<(Value, Value)>,
    /// The expectation.
    pub id: ExpectationId,
    /// Its role.
    pub role: Role,
    /// Why it was left out.
    pub reason: OmitReason,
}

/// What a report decided: one [`Decision`] per tested entry and one [`Omitted`] per untested
/// entry, in the order of the comparisons and of their entries.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Summary {
    /// The tested entries.
    pub decisions: Vec<Decision>,
    /// The entries left out, with their reasons.
    pub omitted: Vec<Omitted>,
}

impl Summary {
    /// Derives the summary from the compared pairs of a report.
    #[must_use]
    pub fn of<'a>(comparisons: &'a [Comparison]) -> Self {
        let mut summary = Self::default();
        for comparison in comparisons {
            for entry in &comparison.entries {
                match entry {
                    Entry::Tested(tested) => {
                        let family = comparison
                            .entries
                            .iter()
                            .filter(
                                |held| matches!(held, Entry::Tested(t) if t.role == tested.role),
                            )
                            .count();
                        summary.decisions.push(Decision {
                            experiment: comparison.experiment.clone(), // BORROW: the summary owns the text
                            factor: comparison.factor.clone(), // BORROW: the summary owns the factor
                            levels: comparison.levels.clone(), // BORROW: the summary owns the levels
                            id: tested.id.clone(),             // BORROW: the summary owns the id
                            role: tested.role,
                            diff: tested.diff,
                            diff_lo: tested.diff_lo,
                            diff_hi: tested.diff_hi,
                            p_holm: tested.p_holm,
                            reject: tested.reject,
                            family,
                        });
                    }
                    Entry::Omitted { id, role, reason } => summary.omitted.push(Omitted {
                        experiment: comparison.experiment.clone(), // BORROW: the summary owns the text
                        factor: comparison.factor.clone(), // BORROW: the summary owns the factor
                        levels: comparison.levels.clone(), // BORROW: the summary owns the levels
                        id: id.clone(),                    // BORROW: the summary owns the id
                        role: *role,
                        reason: *reason,
                    }),
                }
            }
        }
        summary
    }
}
