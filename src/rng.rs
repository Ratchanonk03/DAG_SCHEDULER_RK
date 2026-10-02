//! Deterministic pseudo-random number generation (PROVIDED — do not modify).
//!
//! This assignment uses **no external crates**, so we ship a small
//! SplitMix64 generator instead of depending on `rand`.
//!
//! Two consequences you should understand:
//!
//! 1. **Reproducible benchmarks.** Input tensors are generated from a fixed
//!    seed, so the same DAG produces the same numbers on every run. Your
//!    timing results are therefore comparable across runs and machines.
//!
//! 2. **Reproducible steal patterns.** Each worker seeds its own generator
//!    from its worker ID, so victim selection is deterministic per worker.
//!    This makes steal-rate numbers stable enough to reason about, and makes
//!    scheduling bugs far easier to reproduce than with a global RNG.

/// A small, fast, deterministic PRNG (SplitMix64).
///
/// Not cryptographically secure — it is for benchmarking and victim
/// selection only.
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Create a generator from a seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Produce the next 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Produce a value in `0..n`.
    ///
    /// # Panics
    /// Panics if `n == 0`.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "SplitMix64::below requires n > 0");
        (self.next_u64() % n as u64) as usize
    }

    /// Produce an `f32` in `[-1.0, 1.0)`.
    pub fn next_f32_signed(&mut self) -> f32 {
        // Take the top 24 bits for a uniform value in [0, 1).
        let unit = (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32;
        unit * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn test_below_in_range() {
        let mut r = SplitMix64::new(7);
        for _ in 0..1000 {
            let v = r.below(4);
            assert!(v < 4);
        }
    }

    #[test]
    fn test_f32_in_range() {
        let mut r = SplitMix64::new(9);
        for _ in 0..1000 {
            let v = r.next_f32_signed();
            assert!((-1.0..1.0).contains(&v), "value out of range: {}", v);
        }
    }
}
