//! The runs that still lack a complete set of outcomes: the selection `Pending`.

use std::collections::{BTreeMap, BTreeSet};

use candler_schemas::primitives::{ExpectationId, Name, NameVersion};

use super::{EvaluateError, RunPlan};
use crate::store::Store;

/// The plans of `plans` that are not complete, in their order. A run is complete when, for every
/// entry of its plan, an outcome file of that entry's evaluator identity (`name@version`, from
/// `identities`) holds a record for the entry's id. An outcome file of another version of the
/// evaluator does not count, and neither does one of another evaluator. An entry whose evaluator
/// has no identity is never complete.
///
/// # Errors
///
/// Returns [`EvaluateError::Store`] when an outcome file of a run cannot be listed or read.
pub fn pending_runs(
    store: &Store,
    plans: Vec<RunPlan>,
    identities: &BTreeMap<Name, NameVersion>,
) -> Result<Vec<RunPlan>, EvaluateError> {
    let mut pending = Vec::new();
    for plan in plans {
        // The ids each evaluator identity already holds a record for.
        let mut held: BTreeMap<NameVersion, BTreeSet<ExpectationId>> = BTreeMap::new();
        for file in store.outcome_files(&plan.run)? {
            let ids = held.entry(file.evaluator.clone()).or_default(); // BORROW: the map owns its identities
            for record in store.read_outcomes(&file)? {
                ids.insert(record.expectation_id().clone()); // BORROW: the set owns its ids
            }
        }
        let entries = plan
            .not_applicable
            .iter()
            .chain(plan.batches.iter().flat_map(|batch| batch.entries.iter()));
        let mut complete = true;
        for checked in entries {
            let covered = identities
                .get(&checked.entry.evaluator().name)
                .and_then(|identity| held.get(identity))
                .is_some_and(|ids| ids.contains(checked.entry.id()));
            if !covered {
                complete = false;
                break;
            }
        }
        if !complete {
            pending.push(plan);
        }
    }
    Ok(pending)
}
