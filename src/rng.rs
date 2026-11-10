//! Fast, deterministic pseudo-random number generator (PRNG) in pure Rust std.
//!
//! Implements Xoshiro256** initialized via SplitMix64 for high-quality,
//! 100% reproducible random sequences given a 64-bit seed.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

/// Helper function to advance a SplitMix64 state and return the next pseudorandom u64.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

impl Rng {
    /// Create a new deterministic PRNG with the specified 64-bit seed.
    pub fn with_seed(seed: u64) -> Self {
        // Guarantee a non-zero SplitMix64 initial state even if seed == 0
        let mut sm_state = if seed == 0 {
            0x517cc1b727220a95
        } else {
            seed
        };
        let s0 = splitmix64(&mut sm_state);
        let s1 = splitmix64(&mut sm_state);
        let s2 = splitmix64(&mut sm_state);
        let s3 = splitmix64(&mut sm_state);
        let s = [s0, s1, s2, s3];
        // Ensure state is not all zeros
        let s = if s == [0, 0, 0, 0] {
            [
                0x8a5cd789635d2dff,
                0x121fd2155c472f96,
                0x6985548eeac4790f,
                0x20155a4808290f0f,
            ]
        } else {
            s
        };
        Self { s }
    }

    /// Create a PRNG seeded from system wall-clock nanoseconds.
    pub fn from_time() -> Self {
        use std::time::SystemTime;
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42);
        Self::with_seed(nanos)
    }

    #[inline(always)]
    fn rotl(x: u64, k: u32) -> u64 {
        (x << k) | (x >> (64 - k))
    }

    /// Return the next pseudo-random 64-bit unsigned integer.
    pub fn next_u64(&mut self) -> u64 {
        let result = Self::rotl(self.s[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.s[1] << 17;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;
        self.s[3] = Self::rotl(self.s[3], 45);

        result
    }

    /// Return the next pseudo-random 32-bit unsigned integer.
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Return the next pseudo-random float in `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Return the next pseudo-random boolean.
    pub fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }

    /// Return a pseudo-random integer in `0..bound` with zero modulo bias.
    /// Uses Daniel Lemire's nearly divisionless unbiased range algorithm.
    pub fn next_usize(&mut self, bound: usize) -> usize {
        if bound <= 1 {
            return 0;
        }
        let bound = bound as u64;
        let mut x = self.next_u64();
        let mut m = (x as u128) * (bound as u128);
        let mut l = m as u64;
        if l < bound {
            let t = (!bound + 1) % bound;
            while l < t {
                x = self.next_u64();
                m = (x as u128) * (bound as u128);
                l = m as u64;
            }
        }
        (m >> 64) as usize
    }

    /// Randomly shuffle a slice in-place using Fisher-Yates algorithm.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        if slice.len() <= 1 {
            return;
        }
        for i in (1..slice.len()).rev() {
            let j = self.next_usize(i + 1);
            slice.swap(i, j);
        }
    }

    /// Pick a random element reference from a non-empty slice.
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            let idx = self.next_usize(slice.len());
            Some(&slice[idx])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_reproducibility() {
        let mut rng1 = Rng::with_seed(12345);
        let mut rng2 = Rng::with_seed(12345);

        for _ in 0..1000 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_rng_different_seeds() {
        let mut rng1 = Rng::with_seed(1);
        let mut rng2 = Rng::with_seed(2);
        let seq1: Vec<u64> = (0..10).map(|_| rng1.next_u64()).collect();
        let seq2: Vec<u64> = (0..10).map(|_| rng2.next_u64()).collect();
        assert_ne!(seq1, seq2);
    }

    #[test]
    fn test_next_usize_bounds() {
        let mut rng = Rng::with_seed(999);
        for bound in 1..=20 {
            for _ in 0..100 {
                let v = rng.next_usize(bound);
                assert!(v < bound, "value {} out of bounds for bound {}", v, bound);
            }
        }
    }

    #[test]
    fn test_shuffle() {
        let mut rng = Rng::with_seed(42);
        let mut items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let original = items.clone();
        rng.shuffle(&mut items);
        assert_eq!(items.len(), original.len());
        assert_ne!(items, original);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, original);
    }

    #[test]
    fn test_choose() {
        let mut rng = Rng::with_seed(777);
        let items = [10, 20, 30, 40];
        let chosen = rng.choose(&items);
        assert!(chosen.is_some());
        assert!(items.contains(chosen.unwrap()));

        let empty: [i32; 0] = [];
        assert_eq!(rng.choose(&empty), None);
    }
}
