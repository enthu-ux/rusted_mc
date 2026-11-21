//! In-process chunk-generation benchmark (base terrain: caves/ravines off, matching
//! `TerrainBench.java`) for the performance contract.
//!
//! Prints one JSON line; `tools/perf_baseline.py` pairs it with the Java 1.8.9 baseline.

use mc_perf::Stats;
use mc_worldgen::terrain::TerrainGen;
use std::time::Instant;

fn main() {
    let seeds: [i64; 3] = [0, 12345, -7];
    let chunks: [[i32; 2]; 5] = [[0, 0], [1, -1], [-48, -48], [-37, 210], [512, 512]];
    let iters = 5usize;

    // Warm up (noise/palette/scratch + JIT-free but cache-warm).
    for &seed in &seeds {
        let mut gen = TerrainGen::new(seed);
        for c in &chunks {
            let _ = gen.generate_base_chunk(c[0], c[1]);
        }
    }

    let mut stats = Stats::new();
    for _ in 0..iters {
        for &seed in &seeds {
            let mut gen = TerrainGen::new(seed);
            for c in &chunks {
                let start = Instant::now();
                let _ = gen.generate_base_chunk(c[0], c[1]);
                stats.record(start.elapsed());
            }
        }
    }

    let ms = |d: Option<std::time::Duration>| d.map(|x| x.as_secs_f64() * 1000.0).unwrap_or(0.0);
    println!(
        "{{\"bench\":\"terrain_rust\",\"samples\":{},\"per_chunk_ms\":{{\"min\":{:.4},\"median\":{:.4},\"p95\":{:.4},\"max\":{:.4},\"mean\":{:.4}}}}}",
        stats.len(),
        ms(stats.min()),
        ms(stats.median()),
        ms(stats.percentile(95.0)),
        ms(stats.max()),
        ms(stats.mean())
    );
}
