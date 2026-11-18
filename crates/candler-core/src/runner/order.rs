//! The order in which a plan starts its runs.
//!
//! Each replicate round holds every cell once, in an order shuffled from the experiment's digest,
//! so the independence of runs is argued from the design and not from the order the file lists
//! its cells in.

use core::num::NonZeroU64;

use candler_schemas::primitives::Digest;

use crate::analysis::splitmix::SplitMix64;

/// The seed of the run order of the experiment `experiment`.
///
/// A reproducibility contract: the input is the experiment digest, which `plan_runs` holds (the
/// stored group's digest when a past experiment answers, else the digest of the cells' digests).
/// The seed is the first 8 bytes of that digest, read as a little-endian number. Changing this
/// derivation changes the run order of every replay of every stored experiment.
#[must_use]
pub fn seed_of(experiment: &Digest) -> u64 {
    let mut bytes = [0_u8; 8];
    for (slot, pair) in bytes.iter_mut().zip(experiment.hex().as_bytes().chunks(2)) {
        // EXPLICIT: a digest is 64 lowercase hex digits, so every pair parses and the default
        // is not reached
        *slot = core::str::from_utf8(pair)
            .ok()
            .and_then(|text| u8::from_str_radix(text, 16).ok())
            .unwrap_or_default();
    }
    u64::from_le_bytes(bytes)
}

/// The `(cell index, replicate)` of every run, in the order the runs start.
///
/// Replicates come in order, from 1 to `replicates`. Within one replicate round every cell index
/// of `0..cells` appears once, in a Fisher–Yates shuffle drawn from [`SplitMix64`] seeded with
/// `seed` (see [`seed_of`]). The rounds continue one generator, so a seed fixes the whole order.
#[must_use]
pub fn run_order(cells: usize, replicates: u32, seed: u64) -> Vec<(usize, u32)> {
    let mut rng = SplitMix64::new(seed);
    let mut order = Vec::new();
    for replicate in 1..=replicates {
        let mut round: Vec<usize> = (0..cells).collect();
        for end in (1..cells).rev() {
            let Some(bound) = u64::try_from(end)
                .ok()
                .and_then(|last| last.checked_add(1))
                .and_then(NonZeroU64::new)
            else {
                continue;
            };
            // EXPLICIT: the draw is below `end + 1`, so the conversion back succeeds; `end` is
            // the no-op swap
            let pick = usize::try_from(rng.below(bound)).unwrap_or(end);
            round.swap(end, pick);
        }
        order.extend(round.into_iter().map(|cell| (cell, replicate)));
    }
    order
}
