//! In-process carve-pass benchmark (base vs caves vs caves+ravines) plus an `mc-perf` span
//! breakdown, over the `caves_parity` fixture chunks.
//!
//! Not wired into `tools/perf_baseline.py` (that tool pairs `terrain_bench` with the Java
//! baseline). Run manually:
//!
//! ```sh
//! cargo run -q --release -p mc-worldgen --example carve_bench
//! ```

use mc_perf::Stats;
use mc_worldgen::terrain::{GenOptions, TerrainGen};
use std::collections::HashMap;
use std::time::Instant;

fn main() {
    mc_perf::set_spans_enabled(true);
    let seeds = [0i64, 12345, -7];
    let chunks = [
        [0, 0],
        [1, -1],
        [-48, -48],
        [-37, 210],
        [512, 512],
        [-3, 0],
        [-5, 1],
        [6, 5],
    ];
    let opts = [
        ("base  ", GenOptions::none()),
        (
            "caves ",
            GenOptions {
                caves: true,
                ..GenOptions::none()
            },
        ),
        (
            "+ravin",
            GenOptions {
                caves: true,
                ravines: true,
                ..GenOptions::none()
            },
        ),
        // Structures only *register* during `provideChunk`; their blocks come from `populate`.
        // `vanilla()` enables caves, ravines, mineshafts and scattered features (temples).
        ("+struct", GenOptions::vanilla()),
    ];
    let iters = 5;

    for &seed in &seeds {
        let mut g = TerrainGen::new(seed);
        for c in &chunks {
            for (_, o) in &opts {
                let _ = g.generate_chunk(c[0], c[1], *o);
            }
        }
    }
    let _ = mc_perf::take_spans();

    for &(name, o) in &opts {
        let mut stats = Stats::new();
        for _ in 0..iters {
            for &seed in &seeds {
                let mut g = TerrainGen::new(seed);
                for c in &chunks {
                    let t = Instant::now();
                    let _ = g.generate_chunk(c[0], c[1], o);
                    stats.record(t.elapsed());
                }
            }
        }
        println!(
            "{name}: median {:.4} ms  p95 {:.4} ms  max {:.4} ms  n={}",
            stats.median().unwrap().as_secs_f64() * 1000.0,
            stats.percentile(95.0).unwrap().as_secs_f64() * 1000.0,
            stats.max().unwrap().as_secs_f64() * 1000.0,
            stats.len()
        );
    }

    let spans = mc_perf::take_spans();
    let mut agg: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in spans {
        agg.entry(s.name)
            .or_default()
            .push(s.duration.as_secs_f64() * 1000.0);
    }
    let mut names: Vec<&str> = agg.keys().copied().collect();
    names.sort();
    for n in names {
        let v = &agg[n];
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        println!("span {n}: mean {mean:.4} ms over {} calls", v.len());
    }
}
