//! Prints the 8 draws after the `hellRNG` replay for the given chunks, matching
//! `NetherPlacementOracle rng`:
//!
//! Usage: nether_rng_probe <cx> <cz> [<cx> <cz> ...]
//! Per chunk: "rng <cx> <cz> i0 ... i7".
//!
//! The block dump cannot see this stream (only the corridor chest *contents* draw from it), so this
//! pins the `ChunkProviderHell.provideChunk` replay directly against vanilla.

use mc_worldgen::structure::netherbridge::hell_populate_rand;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut out = String::new();
    let mut i = 1;
    while i + 1 < args.len() {
        let cx: i32 = args[i].parse().expect("cx");
        let cz: i32 = args[i + 1].parse().expect("cz");
        let mut rand = hell_populate_rand(cx, cz);
        out.push_str(&format!("rng {cx} {cz}"));
        for _ in 0..8 {
            out.push_str(&format!(" {}", rand.next_int()));
        }
        out.push('\n');
        i += 2;
    }
    print!("{out}");
}
