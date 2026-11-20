//! Prints canonical 1.8.9 base-terrain samples (stone/water + surface blocks) for fixed
//! seeds/chunks.
//!
//! The Java oracle (`oracle/java/src/TerrainOracle.java`) prints the identical format;
//! `tools/test_terrain_parity.py` compares them line-by-line.
//! One line per (x, z) column: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.

use mc_worldgen::terrain::TerrainGen;

fn main() {
    let seeds: [i64; 3] = [0, 12345, -7];
    let chunks: [[i32; 2]; 21] = [
        [-48, -48],
        [-22, -22],
        [-48, -43],
        [-6, 47],
        [-42, 15],
        [-22, 46],
        [-33, 39],
        [-47, 47],
        [-169, 30],
        [88, -81],
        [126, -14],
        [-184, -135],
        [-40, -126],
        [-111, 6],
        [-38, -192],
        [-124, -128],
        [27, -110],
        [-48, 185],
        [36, -163],
        [100, -52],
        [-4096, -2048],
    ];
    let mut out = String::new();

    for &seed in &seeds {
        let mut gen = TerrainGen::new(seed);
        for c in &chunks {
            let primer = gen.generate_chunk(c[0], c[1]);
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
