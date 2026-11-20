//! Prints canonical 1.8.9 *batch* noise samples for several fixed seeds.
//!
//! Companion to `noise_probe.rs`: covers the array paths
//! (`Perlin::generate_default`/`generate` = `func_151599_a`/`func_151600_a`,
//! `Simplex::populate_noise_array` = `func_151606_a`).
//! The Java oracle (`oracle/java/src/NoiseBatchOracle.java`) prints the identical
//! format; `tools/test_noise_batch_parity.py` compares them byte-for-byte.
//! Doubles are printed as raw bits so no rounding is introduced.

use mc_core::random::JavaRandom;
use mc_worldgen::noise::{Perlin, Simplex};

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn dump(out: &mut String, tag: &str, values: &[f64]) {
    out.push_str(tag);
    out.push('\n');
    for v in values {
        out.push_str(&hex(*v));
        out.push('\n');
    }
}

fn main() {
    let seeds: [i64; 3] = [0, 12345, -1];
    let mut out = String::new();

    // Simplex 2D batch (accumulates into a fresh zeroed buffer).
    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Simplex::new(&mut r);

        let mut a = Vec::new();
        g.populate_noise_array(&mut a, 1.5, -2.25, 5, 4, 1.0, 1.0, 1.0);
        dump(&mut out, &format!("simplex_batch seed {s}"), &a);

        let mut b = Vec::new();
        g.populate_noise_array(&mut b, 0.0, 0.0, 8, 3, 0.5, 0.75, 2.0);
        dump(&mut out, &format!("simplex_batch_scaled seed {s}"), &b);
    }

    // Perlin batch with explicit freq/amp decay.
    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Perlin::new(&mut r, 4);

        let mut a = Vec::new();
        g.generate(&mut a, 0.0, 0.0, 5, 5, 1.0, 1.0, 0.5, 0.5);
        dump(&mut out, &format!("perlin_batch seed {s}"), &a);

        let mut b = Vec::new();
        g.generate(&mut b, -3.0, 2.0, 4, 6, 0.25, 0.5, 0.6, 0.75);
        dump(&mut out, &format!("perlin_batch_scaled seed {s}"), &b);
    }

    // Perlin batch with the default amp decay.
    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Perlin::new(&mut r, 8);

        let mut a = Vec::new();
        g.generate_default(&mut a, 0.0, 0.0, 5, 5, 1.0, 1.0, 0.5);
        dump(&mut out, &format!("perlin_default seed {s}"), &a);
    }

    print!("{out}");
}
