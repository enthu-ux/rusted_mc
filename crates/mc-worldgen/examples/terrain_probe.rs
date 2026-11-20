//! Prints canonical 1.8.9 base-terrain *shape* samples (stone/water/air) for fixed seeds/chunks.
//!
//! The Java oracle (`oracle/java/src/TerrainOracle.java`) prints the identical format;
//! `tools/test_terrain_parity.py` compares them line-by-line.
//! One line per (x, z) column: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.

use mc_worldgen::terrain::TerrainGen;

fn main() {
    let seeds: [i64; 3] = [0, 12345, -7];
    let chunks: [[i32; 2]; 2] = [[0, 0], [1, -1]];
    let mut out = String::new();

    for &seed in &seeds {
        let mut gen = TerrainGen::new(seed);
        for c in &chunks {
            let primer = gen.set_blocks_in_chunk(c[0], c[1]);
            out.push_str(&format!("seed {} chunk {} {}\n", seed, c[0], c[1]));
            for x in 0..16usize {
                for z in 0..16usize {
                    out.push_str(&format!("{x} {z}"));
                    for y in 0..256usize {
                        let idx = (x << 12) | (z << 8) | y;
                        out.push_str(&format!(" {}", primer[idx]));
                    }
                    out.push('\n');
                }
            }
        }
    }

    print!("{out}");
}
