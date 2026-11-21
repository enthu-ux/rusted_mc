//! Prints canonical 1.8.9 `ChunkProviderGenerate.provideChunk` samples with caves/ravines toggled.
//!
//! Usage: cave_probe <seed> <modes> <cx> <cz> [<cx> <cz> ...]
//!   <modes> is a comma-separated list, e.g. `0,1,2,3`
//!     mode 0 = caves only, 1 = ravines only, 2 = both, 3 = neither (base terrain)
//! Format per chunk: "seed <seed> mode <mode> chunk <cx> <cz>", then one line per (x, z) column:
//! "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.
//!
//! The Java oracle (`oracle/java/src/CaveOracle.java`) prints the identical format;
//! `tools/test_caves_parity.py` compares them line-by-line. Modules are always off (structures are
//! not ported yet), matching the oracle.

use mc_worldgen::terrain::{GenOptions, TerrainGen};

fn options(mode: u32) -> GenOptions {
    GenOptions {
        caves: mode == 0 || mode == 2,
        ravines: mode == 1 || mode == 2,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args[1].parse().expect("seed");
    let modes: Vec<u32> = args[2]
        .split(',')
        .map(|m| m.parse().expect("mode"))
        .collect();
    let mut chunks: Vec<(i32, i32)> = Vec::new();
    let mut i = 3;
    while i + 1 < args.len() {
        chunks.push((
            args[i].parse().expect("cx"),
            args[i + 1].parse().expect("cz"),
        ));
        i += 2;
    }

    let mut gen = TerrainGen::new(seed);
    let mut out = String::new();
    for &mode in &modes {
        for &(cx, cz) in &chunks {
            let primer = gen.generate_chunk(cx, cz, options(mode)).blocks;
            out.push_str(&format!("seed {seed} mode {mode} chunk {cx} {cz}\n"));
            for x in 0..16usize {
                for z in 0..16usize {
                    out.push_str(&format!("{x} {z}"));
                    for y in 0..256usize {
                        out.push_str(&format!(" {}", primer[(x << 12) | (z << 8) | y]));
                    }
                    out.push('\n');
                }
            }
        }
    }

    print!("{out}");
}
