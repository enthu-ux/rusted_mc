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

## Canonical chunk (blocks + biome + height map), per chunk — 2026-11-22

`cargo run -q --release -p mc-worldgen --example carve_bench` (8 chunks × 3 seeds × 5 iterations,
release, in-process; same machine). Every row is `TerrainGen::generate_chunk`, which now also builds
the canonical artefacts (biome array + height map); `terrain_bench` / `generate_base_chunk` remains
blocks-only, so the base-terrain baseline above is unchanged. No Java counterpart yet, no ratio.

| pass | median (ms) | p95 (ms) | max (ms) |
|---|---:|---:|---:|
| base terrain + artefacts (`GenOptions::none`) | 1.5828 | 3.0664 | 7.1551 |
| + caves | 1.8111 | 3.6016 | 5.4359 |
| + caves + ravines | 1.9490 | 3.5028 | 6.8443 |
| + mineshaft registration (`GenOptions::vanilla`) | not recorded | - | - |

`mc-perf` spans (mean per call, the same run): `worldgen.chunk` 1.7577, `worldgen.shape` 1.3162,
`worldgen.surface` 0.2776, `worldgen.caves` 0.2474, `worldgen.ravines` 0.0623,
`worldgen.artifacts` 0.0561 ms.

Reading: carving ≈ **+0.37 ms/chunk**, artefact build ≈ **0.06 ms/chunk** (`worldgen.artifacts`).
Run-to-run spread on this dev box is roughly ±20% (a loaded machine moved the base row by 60% with no
code change, so those runs are discarded per `docs/performance-contract.md` §1), so treat the deltas
as indicative until the reference-server benchmark exists; the full pass stays well under the
recorded Java `provideChunk` median (3.63 ms, which includes caves). Not a gate yet.

**Mineshaft registration** adds `worldgen.mineshafts`: 0.0500–0.0794 ms/chunk across the two loaded
runs above - it walks the same 17x17 neighbourhood but only builds piece trees for chunks that pass
`canSpawnStructureAtCoords` (p ≈ 0.004 once `max(|x|,|z|) >= 80`). Re-record the whole table on a
quiet machine when the next feature lands.