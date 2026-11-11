//! Holm's step-down adjustment of p-values.

use super::ALPHA;
use super::error::StatsError;

/// The Holm-adjusted p-values, in the input order.
///
/// The values are sorted ascending by `(p, index)`. The adjusted value at sorted rank `i`
/// (from 1, among `m` values) is `min(1, max over j <= i of (m − j + 1) · p_j)`. Equal p-values
/// get equal adjusted values. An empty list gives an empty list.
///
/// # Errors
/// Returns [`StatsError::NotAProbability`] with the index of the first value that is `NaN` or
/// outside `[0, 1]`.
pub fn holm(p: &[f64]) -> Result<Vec<f64>, StatsError> {
    if let Some(index) = p.iter().position(|value| !(0.0..=1.0).contains(value)) {
        return Err(StatsError::NotAProbability { index });
    }
    let mut ranked: Vec<(usize, f64)> = p.iter().copied().enumerate().collect();
    ranked.sort_by(|(i, x), (j, y)| x.total_cmp(y).then(i.cmp(j)));
    let mut multiplier = size(ranked.len());
    let mut running = 0.0_f64;
    let mut adjusted: Vec<(usize, f64)> = ranked
        .into_iter()
        .map(|(index, value)| {
            running = running.max(multiplier * value);
            multiplier -= 1.0;
            (index, running.min(1.0))
        })
        .collect();
    adjusted.sort_by_key(|(index, _)| *index);
    Ok(adjusted.into_iter().map(|(_, value)| value).collect())
}

/// Which adjusted values reject the null: those below [`ALPHA`], strictly.
#[must_use]
pub fn rejected(adjusted: &[f64]) -> Vec<bool> {
    adjusted.iter().map(|value| *value < ALPHA).collect()
}

/// The number of tests as a float.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "CAST: a slice length is far below 2^53"
)]
const fn size(len: usize) -> f64 {
    len as f64 // CAST: usize → f64, a slice length is far below 2^53
}
