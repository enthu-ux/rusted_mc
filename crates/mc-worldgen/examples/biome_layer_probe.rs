//! Prints canonical 1.8.9 biome-layer output (biome ids) for several fixed seeds.
//!
//! The Java oracle (`oracle/java/src/BiomeLayerOracle.java`) prints the identical format;
//! `tools/test_biome_layer_parity.py` compares them line-by-line.
//! Layer 0 = `BiomeLayers::biome` (genBiomes); layer 1 = `BiomeLayers::voronoi`.

use mc_worldgen::layer::BiomeLayers;

fn main() {
    let seeds: [i64; 3] = [0, 12345, -7];
    let regions: [[i32; 4]; 3] = [[0, 0, 16, 16], [-37, 210, 16, 16], [512, 512, 16, 16]];
    let mut out = String::new();

    for &s in &seeds {
        let mut layers = BiomeLayers::new(s);
        for li in 0..2 {
            for r in &regions {
                let ids = if li == 0 {
                    layers.biome.get_ints(r[0], r[1], r[2], r[3])
                } else {
                    layers.voronoi.get_ints(r[0], r[1], r[2], r[3])
                };
                out.push_str(&format!(
                    "seed {} layer {} {} {} {} {}\n",
                    s, li, r[0], r[1], r[2], r[3]
                ));
                for v in &ids {
                    out.push_str(&format!("{v}\n"));
                }
            }
        }
    }

    print!("{out}");
}
