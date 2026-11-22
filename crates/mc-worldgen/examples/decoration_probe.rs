//! Prints a 1.8.9 chunk after `ChunkProviderGenerate.populate`'s decoration passes.
//!
//! Usage: decoration_probe <seed> <mode> <cx> <cz> [<cx> <cz> ...]
//!   mode = ores | surface | ore_surface
//! Format per chunk: "seed <seed> chunk <cx> <cz>", then 256 lines "x z v0 ... v255"
//! (`vi = blockId * 16 + metadata`).
//!
//! The Java oracle (`oracle/java/src/DecorationOracle.java`) prints the identical format;
//! `tools/test_decoration_parity.py` compares them line-by-line. Only the layers named by the mode
//! run (vanilla always runs all of `BiomeDecorator.genDecorations`).

use mc_worldgen::decorate::DecorateOptions;
use mc_worldgen::terrain::TerrainGen;

fn options(mode: &str) -> DecorateOptions {
    DecorateOptions {
        ores: mode.contains("ore"),
        surface: mode.contains("surface"),
        plants: mode.contains("plant"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args[1].parse().expect("seed");
    let mode = &args[2];
    let opts = options(mode);
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
    for &(cx, cz) in &chunks {
        let blocks = gen.decorate_chunk(cx, cz, opts);
        out.push_str(&format!("seed {seed} chunk {cx} {cz}\n"));
        for x in 0..16usize {
            for z in 0..16usize {
                out.push_str(&format!("{x} {z}"));
                for y in 0..256usize {
                    out.push_str(&format!(" {}", blocks[(x << 12) | (z << 8) | y]));
                }
                out.push('\n');
            }
        }
    }

    print!("{out}");
}
