# Horizon performance reuse (tracked ledger)

Purpose: make sure we **actually** benefit from Horizon (beyond culling), and do it before the
renderer/netcode freeze assumptions in. This is the performance companion to
[`horizon-reuse.md`](horizon-reuse.md) (what the sparse checkout contains).

- Upstream: https://github.com/horizon-rust/horizon (LGPL-3.0; see `vendor/horizon/LICENSE`).
- Working copy: `vendor/horizon/` (gitignored), currently commit `0e5055d` (2026-11-20).
- **Re-sync before copying code** (`git -C vendor/horizon fetch --depth 1 origin main && git -C
  vendor/horizon reset --hard FETCH_HEAD`) and record the new commit. The tree moves fast.

## Current reality (be honest)

We copy **no Horizon runtime code today**. Our `crates/mc-*` depend only on each other. What we
use is Horizon's *method*: vanilla-parity harnesses, port-ledger thinking, and its docs. Horizon's
headline speed (3.5x start, 38% over Sodium, 47.8 ms/tick @ 50 bots) comes from infrastructure we
have not imported. Closing that gap is the point of this ledger.

Two rules that constrain "max reuse":

- **Parity-critical math we own.** Worldgen/RNG/physics must be bit-exact 1.8.9. Horizon's
  `horizon-core/{density,carving,noise,biome}` and `horizon-world/worldgen` are 26.3 semantics —
  take the *architecture*, never the math.
- **LGPL isolation.** Any copied/adapted code is LGPL-3.0. Keep it in clearly-marked modules,
  retain notices, publish modifications (plan §5). Prefer *adapting* small, version-agnostic files
  over importing whole 26.3 crates.

## Reuse ledger

Legend: **Take** = adapt the code, **Pattern** = reimplement the design, **Rewrite** = 1.8.9 spec.

### 1) Allocator, memory, startup, profiling — `horizon-threads`

| Horizon | What it gives | Use | When |
|---|---|---|---|
| `Allocator` / `GameAllocator` (mimalloc) | faster alloc, release behavior | Take (global allocator) | now |
| `allocator_stats.rs`, `process_memory.rs` | RSS/commit/peak | Take | now |
| `alloc_count.rs` | per-thread allocation counters, ~1 ns | Take (perf builds) | perf harness |
| `heap_census.rs` | tagged live-heap census | Take (opt-in) | M3+ |
| `boot_paint.rs` (Windows) | paint before first frame (launch feel) | Take | M3 |
| `work.rs` | `worker_count`, `par_chunks_mut`, serial reference | Take | M1+ |
| `jobs.rs`, `capped.rs`, `sizing.rs` | pool job classes/ranking | Take/Pattern | M2+ |
| `callgrind.rs`, `sampler.rs`, `profile.rs`, `stack_dump.rs`, `unwind.rs` | instruction/alloc + profiler hooks | Take | perf harness |

Also `horizon-system` (`Machine`, GPU/VRAM) and `horizon-platform` (`system` re-export). Cheap,
version-agnostic, and every benchmark/crash report needs it.

### 2) Measurement (the actual lever) — `horizon-perf`, `horizon-perf-format`, `horizon-bench`

`.hzperf` captures + query (`horizon-perf`), the capture format/thread (`horizon-perf-format`),
hardware + compare (`horizon-bench`), and tools `perf_bench.py`, `bench.py`, `bench_stats.py`,
`server_bench.py`, `pacing_bench.py`, `hot_paths.py`, `build_times.py`,
`entity_occlusion_bench.py`, `horizon-hot-paths-game`. Docs: `benchmark.md`,
`benchmark-report.md`, `hot-path-trends.md`, `profiling.md`, `perf-platform.md`.

Adopt as the **perf-gate backbone**; set budgets from measured 1.8.9 baselines (vanilla +
OptiFine), not from Horizon's 26.3 numbers.

### 3) Chunk streaming / world join / chunk build — `horizon-client-render`

| Horizon | What it gives | Use | When |
|---|---|---|---|
| `src/streaming.rs` | pool: generate → decorate → finish → light → mesh; `Arc` snapshots; sequence numbers drop stale work; idle scratch release | Pattern (adapt 1.8.9 stages) | M3/M4 |
| `pregeneration.md` + `streaming::Band` | strip/band pregen, ordered decoration (`DecorationOrder`), bounded memory | Pattern | M4 |
| `src/chunk.rs`, `chunk_map.rs`, `section_mask.rs`, `visbuffer.rs`, `poi_sections.rs` | chunk/section bookkeeping, visibility masks | Take/Pattern | M3 |
| `src/mesher.rs` + `mesher/` | parallel meshing architecture (scratch reuse) | Pattern (1.8.9 mesher ours) | M3 |
| `src/level_loading.rs`, `world/*` | load order / readiness | Pattern | M3 |
| `src/autotune.rs`, `dynamic_resolution.rs`, `perf_capture.rs`, `perf_gpu.rs`, `census.rs` | runtime tuning + capture | Take/Pattern | M4 |

`docs/pregeneration.md` is the concrete "world join / chunk build speed" blueprint: jobs handed
between ticks (not once per tick), bounded in-flight band, deterministic decoration order.

### 4) Rendering — `horizon-client-render/src/gfx/`

Version-agnostic plumbing to **Take**: `device.rs`, `arena.rs` + `arena/`, `cull.rs`,
`occlusion.rs`, `frustum.rs`, `gpu_culling.rs` (+ `.wgsl`), `view_sections.rs`, `visbuffer.rs`,
`upload.rs`, `timing.rs`, `frame_policy.rs`, `in_flight.rs`, `msaa.rs`, `translucency.rs`,
`render_geometry.rs`, `readback.rs`, `screenshot.rs`, `world_handover.rs`.

**Not ours:** HDR/shaders/OIT/26.3 block models/datapacks/mods (Rewrite). We keep Vulkan/wgpu
(plan §13).

### 5) World storage / load (join time) — `horizon-world`

`region.rs`, `compression.rs` (gzip/zlib/lz4 block framing), `nbt.rs` (in `horizon-core`),
`heightmap.rs`, `fs_sync.rs` → **Take** the Anvil plumbing. 1.8.9 chunk *format* (block id +
metadata + light nibbles, no palettes) and datafixers are ours (`Rewrite`); Horizon's `datafix`
goes 1.8→26.3, the wrong direction for us.

### 6) Launch / asset preparation — `horizon-prepare`, `horizon-assets`

`horizon-prepare/src/{launch_api,pool,worker,artifacts,check,java,files,inventory}.rs` and
`horizon-assets/src/{bake,json,zip,resource_source,model,...}.rs`; docs `build-times.md`,
`launcher-preparation.md`, `launcher-data-location.md`. `horizon-core/src/prepared_files.rs`.

Adopt the **prepare-once / parallel-decode / atlas-bake / disk-cache** pipeline for our own 1.8.9
assets. This is the launch-time lever (not the game loop).

### 7) Protocol / bots — `horizon-protocol`, `horizon-bots`, `horizon-java-properties`

`frame.rs`, `crypto.rs`, `rsa.rs`, `connection.rs`, `buf.rs`, `compression` → **Take/adapt**;
rewrite the packet set for protocol 47. `horizon-bots` → adapt for M2 join tests.
`horizon-java-properties` → `server.properties`.

### 8) Lighting — rewrite, not reuse

Horizon's `horizon-client-render/src/light.rs` is 26.3 (different light model and propagation). We
**do not take it**. 1.8.9's own lighting is a known hot spot, so we rewrite the engine: packed
nibble SoA planes, incremental queue, cached heightmaps, parallel relight (propose/commit). Light
values are parity-gated by `light_parity`. Only the data-oriented *patterns* are borrowed.

### 9) Do **not** take (26.3-specific)

Worldgen math (`horizon-core/{density,carving,noise,biome}`, `horizon-world/worldgen/`), 26.3
lighting, block models/datapacks/mods, protocol 777 packets, 26.3 redstone/AI/physics semantics,
26.3 datafixers, HDR/shader/OIT passes.

## Phased adoption

- **P0 (now, before M2/M3):** re-sync `vendor/horizon`; write the performance architecture
  contract with budgets; stand up the measurement harness + Java baseline; adopt `horizon-threads`
  allocator/profiling + `horizon-system`. Cheap, version-agnostic, makes everything after it
  measurable.
- **P1 (M1 worldgen):** adapt `horizon-world` region/compression/heightmap/NBT for Anvil read/write;
  worldgen stays ours.
- **P2 (M2 protocol):** adapt `horizon-protocol` framing/crypto/buf; `horizon-bots` for 47.
- **P3 (M3 first frames):** adapt `gfx` device/arena/culling + `streaming`/`mesher` architecture;
  1.8.9 renderer/mesher/models ours.
- **P4 (M4+):** LOD/visbuffer/occlusion/far terrain, pregen band, `SharedFrames`/chunk send,
  autotune/dynamic resolution.
- **Always:** two gates per module (parity + budget), Callgrind instruction/alloc counts on hot
  paths, and `.local/` for captures.

## Honest caveats

- Horizon's numbers are 26.3 on its hardware; 1.8.9 is a lighter workload. Claim wins only via
  normalized benchmarks (plan §13).
- Several files are tightly coupled to 26.3 types (`streaming.rs` pulls `horizon-client-render`
  internals); adapt the *pattern* or extract the small device/arena/culling modules, don't
  copy-paste a crate.
- "Max reuse" = reuse the **infrastructure and measurement**, own the **spec** (1.8.9 parity).