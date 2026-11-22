//! Prints the mineshaft-placed chunk blocks (`provideChunk` + the `populate` half), matching
//! `oracle/java/src/MineshaftPlacementOracle.java`:
//!
//! Usage: mineshaft_probe <seed> <cx> <cz> [<cx> <cz> ...]
//! Per chunk: "seed <seed> chunk <cx> <cz>", then 256 lines "x z v0 ... v255"
//! (`vi = blockId * 16 + metadata`), indexed like the other probes.
//!
//! `TerrainGen::populate_mineshaft_chunk` generates the surrounding 3x3 (whose generation also
//! registers every start the four covering populate passes can meet) and applies them.

use mc_worldgen::terrain::TerrainGen;

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
        let blocks = gen.populate_mineshaft_chunk(cx, cz);
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
