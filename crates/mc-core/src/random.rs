//! Exact port of `java.util.Random` (a 48-bit linear congruential generator).
//!
//! Vanilla 1.8.9 draws from this class everywhere (worldgen, mobs, block ticks),
//! so matching it bit-for-bit is a prerequisite for 1:1 parity.

const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

/// A bit-exact reimplementation of `java.util.Random`.
#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    /// `new Random(seed)`.
    pub fn new(seed: i64) -> Self {
        let mut r = JavaRandom { seed: 0 };
        r.set_seed(seed);
        r
    }

    /// `Random.setSeed(seed)`.
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ MULTIPLIER) & MASK;
    }

    /// `Random.next(bits)` — the protected primitive.
    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `Random.nextInt()`.
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `Random.nextInt(bound)` including its rejection sampling.
    ///
    /// # Panics
    /// Panics if `bound <= 0`, as Java throws `IllegalArgumentException`.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound - 1;
        if (bound & m) == 0 {
            // bound is a power of two.
            ((bound as i64 * r as i64) >> 31) as i32
        } else {
            let mut u = r;
            loop {
                let r2 = u % bound;
                if u.wrapping_sub(r2).wrapping_add(m) >= 0 {
                    r = r2;
                    break;
                }
                u = self.next(31);
            }
            r
        }
    }

    /// `Random.nextLong()`.
    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    /// `Random.nextBoolean()`.
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// `Random.nextFloat()` — exact (only integer ops and a power-of-two division).
    pub fn next_float(&mut self) -> f32 {
        (self.next(24) as f32) / ((1i32 << 24) as f32)
    }

    /// `Random.nextDouble()` — exact (only integer ops and a power-of-two division).
    pub fn next_double(&mut self) -> f64 {
        let hi = (self.next(26) as i64) << 27;
        let lo = self.next(27) as i64;
        ((hi.wrapping_add(lo)) as f64) / 9007199254740992.0 // 2^53
    }

    // TODO: `nextGaussian` needs Java `StrictMath.log` (fdlibm) for last-bit parity.
}

#[cfg(test)]
mod tests {
    use super::JavaRandom;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = JavaRandom::new(12345);
        let mut b = JavaRandom::new(12345);
        for _ in 0..100 {
            assert_eq!(a.next_int(), b.next_int());
        }
    }

    #[test]
    fn next_int_bound_power_of_two_is_in_range() {
        let mut r = JavaRandom::new(42);
        for _ in 0..10_000 {
            let v = r.next_int_bound(256);
            assert!((0..256).contains(&v));
        }
    }

    #[test]
    fn next_int_bound_non_power_of_two_is_in_range() {
        let mut r = JavaRandom::new(7);
        for _ in 0..10_000 {
            let v = r.next_int_bound(1000);
            assert!((0..1000).contains(&v));
        }
    }

    /// First draws of a fresh `Random(seed)`, taken from the JDK oracle.
    #[test]
    fn golden_first_draws() {
        assert_eq!(JavaRandom::new(0).next_int(), -1155484576);
        assert_eq!(JavaRandom::new(0).next_long(), -4962768465676381896);
        assert!(JavaRandom::new(0).next_boolean());
        assert_eq!(JavaRandom::new(0).next_float().to_bits(), 0x3f3b20b4);
        assert_eq!(
            JavaRandom::new(0).next_double().to_bits(),
            0x3fe764168ea6ca89
        );

        assert_eq!(JavaRandom::new(12345).next_int(), 1553932502);
        assert_eq!(JavaRandom::new(42).next_int(), -1170105035);
        assert_eq!(JavaRandom::new(42).next_int_bound(1000), 130);
    }
}
