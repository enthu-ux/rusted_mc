//! Prints canonical 1.8.9 noise samples for several fixed seeds.
//!
//! The Java oracle (`oracle/java/src/NoiseOracle.java`) prints the identical format;
//! `tools/test_noise_parity.py` compares them byte-for-byte. Doubles are printed as
//! raw bits so no rounding is introduced.

use mc_core::random::JavaRandom;
use mc_worldgen::noise::{Improved, Octaves, Perlin, Simplex};

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn main() {
    let seeds: [i64; 3] = [0, 12345, -1];
    let mut out = String::new();

    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Improved::new(&mut r);

        let mut buf = Vec::new();
        g.populate_noise_array(&mut buf, 0.0, 0.0, 0.0, 5, 5, 5, 1.0, 1.0, 1.0, 1.0);
        out.push_str(&format!("improved3d seed {s}\n"));
        for v in &buf {
            out.push_str(&hex(*v));
            out.push('\n');
        }

        let mut buf2 = Vec::new();
        g.populate_noise_array(&mut buf2, 1.5, 2.5, 3.5, 4, 1, 4, 0.5, 0.5, 0.5, 2.0);
        out.push_str(&format!("improved2d seed {s}\n"));
        for v in &buf2 {
            out.push_str(&hex(*v));
            out.push('\n');
        }
    }

    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Simplex::new(&mut r);
        out.push_str(&format!("simplex seed {s}\n"));
        for xi in 0..8 {
            for yi in 0..8 {
                let x = xi as f64 * 0.7 - 2.0;
                let y = yi as f64 * 0.7 - 2.0;
                out.push_str(&hex(g.noise_2d(x, y)));
                out.push('\n');
            }
        }
    }

    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Perlin::new(&mut r, 4);
        out.push_str(&format!("perlin4 seed {s}\n"));
        for xi in 0..8 {
            for yi in 0..8 {
                let x = xi as f64 * 0.7 - 2.0;
                let y = yi as f64 * 0.7 - 2.0;
                out.push_str(&hex(g.sample_2d(x, y)));
                out.push('\n');
            }
        }
    }

    for &s in &seeds {
        let mut r = JavaRandom::new(s);
        let g = Octaves::new(&mut r, 16);
        let mut buf = Vec::new();
        g.generate(&mut buf, 0, 0, 0, 5, 3, 5, 1.0, 1.0, 1.0);
        out.push_str(&format!("octaves16 seed {s}\n"));
        for v in &buf {
            out.push_str(&hex(*v));
            out.push('\n');
        }
    }

    print!("{out}");
}
