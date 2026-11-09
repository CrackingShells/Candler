//! Planning: which entries of a checked suite go to an evaluator for one run, which do not apply
//! to it, and how the sent ones are grouped into requests.
//!
//! The plan is made before any process starts and depends only on the suite and the run's
//! manifest. What it cannot know is the outcome of a dependency; the engine settles that while it
//! runs the batches in stage order.

use std::collections::{BTreeMap, BTreeSet};

use candler_schemas::evaluation::SuiteEntry;
use candler_schemas::execution::RunManifest;
use candler_schemas::primitives::{ExpectationId, Name, RunKey};

use crate::evaluation::registry::{CheckedEntry, CheckedSuite};

/// The entries of one request: one evaluator's entries that depend on one another, in an order
/// in which every dependency comes before its dependents.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Batch {
    /// The evaluator that grades every entry.
    pub evaluator: Name,
    /// The place of the batch in the run: a batch runs after every batch of a lower stage, because
    /// one of its entries depends on an entry graded there.
    pub stage: usize,
    /// The entries, dependencies first; suite order breaks ties.
    pub entries: Vec<CheckedEntry>,
}

/// What the engine does for one run.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RunPlan {
    /// The run.
    pub run: RunKey,
    /// The entries that do not apply to the run, in suite order. They are never sent.
    pub not_applicable: Vec<CheckedEntry>,
    /// The batches, ordered by stage, then by evaluator name, then by the suite place of their
    /// first entry.
    pub batches: Vec<Batch>,
}

/// Why a run cannot be planned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// Entries depend on each other in a loop.
    #[error("the suite has a dependency cycle: {}", join(cycle))]
    Cycle {
        /// The entries of the cycle, each depending on the next and the last on the first.
        cycle: Vec<ExpectationId>,
    },
    /// An entry depends on an id the suite does not hold.
    #[error("entry {entry} depends on {dependency}, which the suite does not hold")]
    UnknownDependency {
        /// The entry.
        entry: ExpectationId,
        /// The id that names no entry.
        dependency: ExpectationId,
    },
    /// Entries of one evaluator that depend on each other, and entries of another evaluator that
    /// depend on them in turn, would need one request to run both before and after the other.
    #[error(
        "entries {} cannot be split into requests: their evaluators depend on each other",
        join(entries)
    )]
    Interleaved {
        /// The entries that no order of requests can serve.
        entries: Vec<ExpectationId>,
    },
}

/// The ids, comma separated.
fn join(ids: &[ExpectationId]) -> String {
    ids.iter()
        .map(ExpectationId::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether `entry` applies to `run`: every condition of its `applies_to` reads a value the run
/// manifest records and finds it among the values the condition lists. An entry with no
/// `applies_to` applies to every run.
#[must_use]
pub fn applies(run: &RunManifest, entry: &SuiteEntry) -> bool {
    let Some(conditions) = entry.applies_to() else {
        return true;
    };
    conditions.iter().all(|condition| {
        run.comparable()
            .values()
            .iter()
            .find(|value| value.path() == condition.path())
            .and_then(|value| value.value())
            .is_some_and(|found| {
                condition
                    .any_of()
                    .iter()
                    .any(|wanted| wanted.as_value() == found.as_value())
            })
    })
}

/// Plans `suite` for the run whose manifest is `run`.
///
/// An entry that does not apply becomes a `not_applicable` entry of the plan. The entries that
/// apply are grouped so that entries of one evaluator joined by a `depends_on` chain share a
/// batch; a dependency on another evaluator's entry puts the dependent's batch in a later stage.
///
/// # Errors
/// Returns [`PlanError::UnknownDependency`] for a dependency that names no entry,
/// [`PlanError::Cycle`] for entries that depend on each other in a loop and
/// [`PlanError::Interleaved`] when the batches would have to wait for each other.
pub fn plan(run: &RunManifest, suite: &CheckedSuite) -> Result<RunPlan, PlanError> {
    let entries = suite.entries();
    let by_id: BTreeMap<&ExpectationId, usize> = entries
        .iter()
        .enumerate()
        .map(|(at, checked)| (checked.entry.id(), at))
        .collect();
    let mut deps: Vec<Vec<usize>> = Vec::with_capacity(entries.len());
    for checked in entries {
        let mut row = Vec::new();
        for dependency in checked.entry.depends_on() {
            let Some(at) = by_id.get(dependency) else {
                return Err(PlanError::UnknownDependency {
                    // BORROW: .clone() keeps the ids in the error
                    entry: checked.entry.id().clone(),
                    dependency: dependency.clone(),
                });
            };
            row.push(*at);
        }
        deps.push(row);
    }
    check_acyclic(entries, &deps)?;

    let applicable: Vec<bool> = entries
        .iter()
        .map(|checked| applies(run, &checked.entry))
        .collect();
    let not_applicable = entries
        .iter()
        .zip(&applicable)
        .filter(|(_, applies)| !**applies)
        // BORROW: .clone() gives the plan its own entries
        .map(|(checked, _)| checked.clone())
        .collect();

    // Entries of one evaluator that depend on each other share a component.
    let mut parent: Vec<usize> = (0..entries.len()).collect();
    for (at, row) in deps.iter().enumerate() {
        for &dep in row {
            if applicable.get(at) == Some(&true)
                && applicable.get(dep) == Some(&true)
                && evaluator_of(entries, at) == evaluator_of(entries, dep)
            {
                let (a, b) = (root(&parent, at), root(&parent, dep));
                if let Some(slot) = parent.get_mut(a.max(b)) {
                    *slot = a.min(b);
                }
            }
        }
    }
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for at in 0..entries.len() {
        if applicable.get(at) == Some(&true) {
            components.entry(root(&parent, at)).or_default().push(at);
        }
    }
    let stage_of = stages(entries, &deps, &parent, &components)?;
    let mut batches: Vec<(usize, Batch)> = components
        .iter()
        .filter_map(|(head, members)| {
            let stage = stage_of.get(head).copied().unwrap_or_default();
            let ordered = order(members, &deps);
            let evaluator = ordered
                .first()
                .and_then(|at| evaluator_of(entries, *at))
                .cloned()?; // BORROW: the batch owns the name
            let first = ordered.iter().copied().min().unwrap_or_default();
            let checked = ordered
                .iter()
                .filter_map(|at| entries.get(*at))
                .cloned() // BORROW: the batch owns its entries
                .collect();
            Some((
                first,
                Batch {
                    evaluator,
                    stage,
                    entries: checked,
                },
            ))
        })
        .collect();
    batches.sort_by(|(a_first, a), (b_first, b)| {
        (a.stage, &a.evaluator, a_first).cmp(&(b.stage, &b.evaluator, b_first))
    });
    Ok(RunPlan {
        run: run.run().clone(), // BORROW: the plan owns the key
        not_applicable,
        batches: batches.into_iter().map(|(_, batch)| batch).collect(),
    })
}

/// The evaluator name of the entry at `at`.
fn evaluator_of<'a>(entries: &'a [CheckedEntry], at: usize) -> Option<&'a Name> {
    entries
        .get(at)
        .map(|checked| &checked.entry.evaluator().name)
}

/// The head of the component of `at`.
fn root(parent: &[usize], mut at: usize) -> usize {
    while let Some(next) = parent.get(at).copied()
        && next != at
    {
        at = next;
    }
    at
}

/// Orders `members` so that every dependency comes before its dependents, suite order first.
fn order(members: &[usize], deps: &[Vec<usize>]) -> Vec<usize> {
    let mut pending: Vec<usize> = members.to_vec();
    pending.sort_unstable();
    let mut done: Vec<usize> = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let ready = pending.iter().position(|at| {
            deps.get(*at)
                .into_iter()
                .flatten()
                .all(|dep| !members.contains(dep) || done.contains(dep))
        });
        // The graph is acyclic, so some member is always ready; the fallback keeps the loop total.
        done.push(pending.remove(ready.unwrap_or_default()));
    }
    done
}

/// The stage of each component, keyed by its head: one more than the highest stage of a component
/// it depends on.
fn stages(
    entries: &[CheckedEntry],
    deps: &[Vec<usize>],
    parent: &[usize],
    components: &BTreeMap<usize, Vec<usize>>,
) -> Result<BTreeMap<usize, usize>, PlanError> {
    let mut stage: BTreeMap<usize, usize> = BTreeMap::new();
    let mut waiting: BTreeSet<usize> = components.keys().copied().collect();
    while !waiting.is_empty() {
        let mut progressed = false;
        for head in waiting.clone() {
            let mut needs: Vec<usize> = Vec::new();
            let mut ready = true;
            for member in components.get(&head).into_iter().flatten() {
                for dep in deps.get(*member).into_iter().flatten() {
                    let other = root(parent, *dep);
                    // A dependency that does not apply is in no component and gates nothing here.
                    if other != head && components.contains_key(&other) {
                        match stage.get(&other) {
                            Some(found) => needs.push(*found),
                            None => ready = false,
                        }
                    }
                }
            }
            if ready {
                stage.insert(head, needs.into_iter().max().map_or(0, |top| top + 1));
                waiting.remove(&head);
                progressed = true;
            }
        }
        if !progressed {
            let entries_left = waiting
                .iter()
                .flat_map(|head| components.get(head).into_iter().flatten())
                .filter_map(|at| entries.get(*at))
                .map(|checked| checked.entry.id().clone()) // BORROW: the error owns the ids
                .collect();
            return Err(PlanError::Interleaved {
                entries: entries_left,
            });
        }
    }
    Ok(stage)
}

/// Fails with a cycle of `deps` when there is one.
fn check_acyclic(entries: &[CheckedEntry], deps: &[Vec<usize>]) -> Result<(), PlanError> {
    // 0 unseen, 1 on the current path, 2 finished
    let mut mark = vec![0_u8; entries.len()];
    for start in 0..entries.len() {
        let mut path: Vec<usize> = Vec::new();
        if let Some(cycle) = walk(start, deps, &mut mark, &mut path) {
            return Err(PlanError::Cycle {
                cycle: cycle
                    .iter()
                    .filter_map(|at| entries.get(*at))
                    .map(|checked| checked.entry.id().clone()) // BORROW: the error owns the ids
                    .collect(),
            });
        }
    }
    Ok(())
}

/// Depth-first walk from `at`; returns the nodes of the first cycle met.
fn walk(
    at: usize,
    deps: &[Vec<usize>],
    mark: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    match mark.get(at).copied() {
        Some(2) | None => return None,
        Some(1) => {
            let from = path.iter().position(|node| *node == at).unwrap_or_default();
            return path.get(from..).map(<[usize]>::to_vec);
        }
        Some(_) => {}
    }
    if let Some(slot) = mark.get_mut(at) {
        *slot = 1;
    }
    path.push(at);
    for dep in deps.get(at).into_iter().flatten() {
        if let Some(cycle) = walk(*dep, deps, mark, path) {
            return Some(cycle);
        }
    }
    path.pop();
    if let Some(slot) = mark.get_mut(at) {
        *slot = 2;
    }
    None
}
