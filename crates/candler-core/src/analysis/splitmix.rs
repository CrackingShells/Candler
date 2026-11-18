//! `SplitMix64`, the seeded generator of every resampled statistic and of the run order.

use core::num::NonZeroU64;

/// The `SplitMix64` generator (Steele, Lea & Flood 2014), one 64-bit word of state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMix64 {
    /// The state, advanced by the golden-ratio increment at each draw.
    state: u64,
}

impl SplitMix64 {
    /// A generator whose first draw follows from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit output.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw uniform on `0..n`, without modulo bias.
    ///
    /// An output below `2^64 mod n` is rejected and drawn again, so every residue has the same
    /// number of accepted outputs.
    pub const fn below(&mut self, n: NonZeroU64) -> u64 {
        let n = n.get();
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.next_u64();
            if x >= threshold {
                return x % n;
            }
        }
    }
}
