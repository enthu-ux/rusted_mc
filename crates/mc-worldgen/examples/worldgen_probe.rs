//! Prints the canonical 1.8.9 chunk (`provideChunk`: blocks + metadata, biome array, height map)
//! for fixed seeds/chunks.
//!
//! Usage: worldgen_probe <seed> <cx> <cz> [<cx> <cz> ...]
//! Format per chunk: "seed <seed> chunk <cx> <cz>", then 256 lines "x z v0 ... v255"
//! (`vi = blockId * 16 + metadata`), then "biome <256 ids>" (index `x + z * 16`) and
//! "height <256 values>" (index `z * 16 + x`).
//!
//! The Java oracle (`oracle/java/src/WorldgenOracle.java`) prints the identical format;
//! `tools/test_worldgen_parity.py` compares them line-by-line. Structures/decoration are not
//! ported yet, so this is the pre-decoration chunk (matching the oracle's `mapFeaturesEnabled =
//! false`).

use mc_worldgen::terrain::{GenOptions, TerrainGen};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args[1].parse().expect("seed");
    let mut chunks: Vec<(i32, i32)> = Vec::new();
    let mut i = 2;
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
        let chunk = gen.generate_chunk(cx, cz, GenOptions::vanilla());
        out.push_str(&format!("seed {seed} chunk {cx} {cz}\n"));
        for x in 0..16usize {
            for z in 0..16usize {
                out.push_str(&format!("{x} {z}"));
                for y in 0..256usize {
                    out.push_str(&format!(" {}", chunk.blocks[(x << 12) | (z << 8) | y]));
                }
                out.push('\n');
            }
        }

        out.push_str("biome");
        for id in &chunk.biomes {
            out.push_str(&format!(" {id}"));
        }
        out.push('\n');

        out.push_str("height");
        for h in &chunk.height_map {
            out.push_str(&format!(" {h}"));
        }
        out.push('\n');
    }

    print!("{out}");
}
