//! Quality and gate rates per cell.

use std::collections::BTreeMap;
use std::fmt;

use candler_schemas::outcome::{Role, Status};
use candler_schemas::primitives::{Digest, NameVersion};
use serde::{Serialize, Serializer};

use super::select::Selection;

/// A cell: the digest of the cell of a run, written as its bare hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct CellKey(Digest);

impl CellKey {
    /// The cell with this digest.
    #[must_use]
    pub const fn new(cell: Digest) -> Self {
        Self(cell)
    }

    /// The digest of the cell.
    #[must_use]
    pub const fn digest<'a>(&'a self) -> &'a Digest {
        &self.0
    }
}

impl fmt::Display for CellKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.hex())
    }
}

impl Serialize for CellKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The gate and quality rates of one cell, each as a numerator and a denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Rates {
    gate: [u64; 2],
    quality: [u64; 2],
}

impl Rates {
    /// The gate sums as `(numerator, denominator)`.
    #[must_use]
    pub const fn gate(&self) -> (u64, u64) {
        let [num, den] = self.gate;
        (num, den)
    }

    /// The quality sums as `(numerator, denominator)`.
    #[must_use]
    pub const fn quality(&self) -> (u64, u64) {
        let [num, den] = self.quality;
        (num, den)
    }

    /// Adds a weight to the sums of a role; a descriptive role adds nothing.
    fn add(&mut self, role: Role, num: u64, den: u64) {
        let sums = match role {
            Role::Gate => &mut self.gate,
            Role::Quality => &mut self.quality,
            Role::Descriptive => return,
        };
        let [sum_num, sum_den] = sums;
        *sum_num = sum_num.saturating_add(num);
        *sum_den = sum_den.saturating_add(den);
    }
}

/// The rates per cell of one evaluator version over its outcomes ([`Selection::outcomes`]).
///
/// Per cell, the numerators and denominators of the effective weights are summed over the records
/// whose own role is `gate` (into the gate pair) or `quality` (into the quality pair) and whose
/// status is `pass`, `fail` or `blocked`; a `blocked` weighs with numerator 0. `not_measured` and
/// `not_applicable` records add nothing, and neither does a 0/0 `pass`, whose weight is 0/0. A cell
/// with any outcome of the evaluator has an entry, with zero sums when nothing counts.
#[must_use]
pub fn rates(selection: &Selection, evaluator: &NameVersion) -> BTreeMap<CellKey, Rates> {
    let mut cells: BTreeMap<CellKey, Rates> = BTreeMap::new();
    for record in selection
        .outcomes()
        .filter(|record| record.evaluator() == evaluator)
    {
        // BORROW: the key owns the cell digest
        let cell = cells
            .entry(CellKey::new(record.run().cell().clone()))
            .or_default();
        match record.status() {
            Status::Pass | Status::Fail | Status::Blocked => {
                if let Some(weight) = record.effective_weight() {
                    cell.add(record.role(), weight.num(), weight.den());
                }
            }
            Status::NotMeasured | Status::NotApplicable => {}
        }
    }
    cells
}

/// The rates as the TOML document `show --rates` prints: one `[cell."<cell hex>"]` table per
/// cell in key order, each with `gate = [num, den]` and `quality = [num, den]`, one blank line
/// between tables and one trailing newline. An empty map gives an empty document.
#[must_use]
pub fn rates_toml(cells: &BTreeMap<CellKey, Rates>) -> String {
    let tables: Vec<String> = cells
        .iter()
        .map(|(cell, rate)| {
            let (gate_num, gate_den) = rate.gate();
            let (quality_num, quality_den) = rate.quality();
            format!(
                "[cell.\"{cell}\"]\ngate = [{gate_num}, {gate_den}]\nquality = [{quality_num}, {quality_den}]\n"
            )
        })
        .collect();
    tables.join("\n")
}
