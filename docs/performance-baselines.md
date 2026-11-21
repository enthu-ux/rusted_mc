# Performance baselines (recorded)

Reproducible references for `docs/performance-contract.md`. Recorded on the dev machine; re-record
after any change to the thing measured or the machine.

- Machine: **AMD Ryzen 3 PRO 2300U (4 cores)**, Windows 10 Pro.
- Toolchain: **rustc 1.100.0**, Java **Temurin 1.8.0_492**, `server-mcp.jar` 1.8.9.
- Runner: `py tools/perf_baseline.py` (3 runs, keeps the best median; JSON under `.local/perf/`).

## Base terrain, per chunk — 2026-11-21

In-process timings, 75 samples per side (3 seeds × 5 chunks × 5 iterations), release build, JIT
warm-up for Java. No caves/ravines/structures.

| side | samples | min (ms) | median (ms) | p95 (ms) | max (ms) |
|---|---:|---:|---:|---:|---:|
| Rust (`mc-worldgen::terrain`) | 75 | 1.2057 | 1.3469 | 1.9925 | 2.0269 |
| Java 1.8.9 (`ChunkProviderGenerate`) | 75 | 2.6954 | 3.6295 | 6.3294 | 8.4815 |

Rust/Java median ratio: **0.371×** (≈2.7× faster).

**Caveat:** the Rust side is *base terrain only*; the Java side is `provideChunk`, which also builds
the biome array and the skylight map. So this is a starting reference, **not** a fair M1
comparison. Re-measure once the Rust side reaches feature parity (`worldgen_seed`).