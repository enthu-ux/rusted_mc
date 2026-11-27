//! Prints a nether-fortress-placed chunk (`provideChunk` layout + the fortress `populate` half),
//! matching `oracle/java/src/NetherPlacementOracle.java`:
//!
//! Usage: nether_probe <seed> <cx> <cz> [<cx> <cz> ...]
//! Per chunk: "seed <seed> chunk <cx> <cz>", then 256 lines "x z v0 ... v255"
//! (`vi = blockId * 16 + metadata`).
//!
//! The nether terrain is not ported, so the harness supplies a **synthetic** base chunk
//! (bedrock at y=0, netherrack y=1..=39, air above) to both sides. Fortress placement only reads the
//! base through `replaceAirAndLiquidDownwards` (the nether bodies never check liquids), so this
//! isolates placement from nether worldgen. `NetherBridge::populate_chunk` uses the per-pass
//! `hellRNG` replay (see `netherbridge::hell_populate_rand`).

use mc_worldgen::structure::netherbridge::NetherBridge;
use mc_worldgen::structure::placement::ChunkWorld;

const BEDROCK: u16 = 7 << 4;
const NETHERRACK: u16 = 87 << 4;
const SURFACE: i32 = 39;

/// The synthetic base (indexed `x << 12 | z << 8 | y`, like `ChunkWorld`).
fn base_chunk() -> Vec<u16> {
    let mut blocks = vec![0u16; 16 * 16 * 256];
    for x in 0..16 {
        for z in 0..16 {
            let column = (x << 12) | (z << 8);
            blocks[column] = BEDROCK;
            for y in 1..=SURFACE {
                blocks[column | y as usize] = NETHERRACK;
            }
        }
    }
    blocks
}

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

    let mut out = String::new();
    for &(cx, cz) in &chunks {
        // Registration (the `provideChunk` half) for the 3x3; layout state is per target chunk.
        let mut bridge = NetherBridge::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                bridge.generate(seed, cx + dx, cz + dz);
            }
        }

        let mut world = ChunkWorld::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.insert_chunk(cx + dx, cz + dz, base_chunk());
            }
        }
        bridge.populate_chunk(&mut world, cx, cz);

        let blocks = world.chunk_blocks(cx, cz);
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
