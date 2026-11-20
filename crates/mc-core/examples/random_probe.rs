//! Prints a canonical `java.util.Random` sequence for a fixed set of seeds.
//!
//! The Java oracle (`oracle/java/src/RandomOracle.java`) prints the identical
//! format; `tools/test_random_parity.py` compares them byte-for-byte.

use mc_core::random::JavaRandom;

fn main() {
    let seeds: [i64; 8] = [
        0,
        1,
        12345,
        -1,
        2_147_483_647,
        -2_147_483_648,
        i64::MAX,
        i64::MIN,
    ];
    let bounds: [i32; 10] = [1, 2, 3, 6, 7, 16, 255, 256, 1000, 123_456_789];

    let mut out = String::new();
    for &s in &seeds {
        out.push_str(&format!("seed {s}\n"));
        let mut r = JavaRandom::new(s);
        for _ in 0..64 {
            out.push_str(&format!("nextInt {}\n", r.next_int()));
        }
        for &b in &bounds {
            for _ in 0..16 {
                out.push_str(&format!("nextInt({b}) {}\n", r.next_int_bound(b)));
            }
        }
        for _ in 0..64 {
            out.push_str(&format!("nextLong {}\n", r.next_long()));
        }
        for _ in 0..64 {
            out.push_str(&format!("nextBoolean {}\n", r.next_boolean() as i32));
        }
        for _ in 0..64 {
            out.push_str(&format!("nextFloat {:08x}\n", r.next_float().to_bits()));
        }
        for _ in 0..64 {
            out.push_str(&format!("nextDouble {:016x}\n", r.next_double().to_bits()));
        }
    }
    print!("{out}");
}
