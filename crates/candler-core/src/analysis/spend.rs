//! The spend of a set of runs: the costs their manifests recorded.

use candler_schemas::execution::RunManifest;

/// The measured path that holds the cost of a run, in dollars.
const COST_PATH: &str = "run.cost_usd";

/// What a set of sealed runs cost, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct Spend {
    /// The sum of the recorded costs, in dollars.
    pub cost_usd: f64,
    /// The runs whose cost is in the sum.
    pub runs_counted: u64,
    /// The runs that recorded no cost, which the sum does not cover.
    pub runs_without_total: u64,
}

/// Sums the cost each manifest recorded under `run.cost_usd`. A manifest without a numeric cost
/// is counted in [`Spend::runs_without_total`], never as zero. Pass the manifests of sealed runs
/// only.
#[must_use]
pub fn spend_of<'a, I>(manifests: I) -> Spend
where
    I: IntoIterator<Item = &'a RunManifest>,
{
    let mut spend = Spend::default();
    for manifest in manifests {
        let cost = manifest
            .measured()
            .iter()
            .find(|entry| entry.path().to_string() == COST_PATH)
            .and_then(|entry| entry.value().as_value().as_f64());
        match cost {
            Some(dollars) => {
                spend.cost_usd += dollars;
                spend.runs_counted = spend.runs_counted.saturating_add(1);
            }
            None => spend.runs_without_total = spend.runs_without_total.saturating_add(1),
        }
    }
    spend
}
