# Handoff

Living status for continuing this project in a new session. **Rewritten each handoff**; the
**Session log** at the bottom is append-only. Assume the next session has no memory of any chat:
everything needed to continue lives in files.

## Start every session

Paste this first:

> Read, in order: `handoff.md`, `AGENTS.md`, `plan.md`, `docs/PHILOSOPHY.md`. Then run
> `scripts/verify.ps1` and confirm the baseline is green. Then report: (1) current state, (2) the next
> step from `handoff.md` and its pass condition, (3) your plan. Do not change anything until I say go.

End a session with **"I will continue in a new session"** - that triggers this doc's update and a
commit/push.

---

## Read first (context)

- `plan.md` - the plan (goals, production line, architecture contract, roadmap, parity policy,
  performance targets, philosophy summary).
- `docs/PHILOSOPHY.md` - engineering philosophy (Horizon's, adopted).
- `docs/ARCHITECTURE.md` - crate map, the two gates (parity + performance), rendering + lighting.
- `docs/performance-contract.md` + `docs/performance-baselines.md` - budgets, methodology, baselines.
- `docs/horizon-reuse.md` + `docs/horizon-performance-reuse.md` - Horizon inventory + reuse plan.
- `docs/deviations.md` + `deviations.toml` - intended deviations from vanilla.
- `vendor/tools/NOTES.md` - Windows toolchain + decompile pipeline.

## How to verify

One command (tests + format + all parity harnesses):

```sh
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Or individually:

```sh
cargo test --workspace
cargo fmt --all -- --check
py tools/parity.py            # all parity harnesses
py tools/parity.py random     # one harness by name
```

Expected today: `cargo test` and `fmt` pass (27 unit tests); `py tools/parity.py` reports
`6/6 passed` (`random_parity` 3,848 lines; `noise_parity` 1,047; `noise_batch_parity` 369;
`biome_layer_parity` 4,626; `terrain_parity` 16,191; `math_parity` 492).

`tools/perf_baseline.py` is **manual** (not part of `verify`): it builds release binaries, runs the
Rust/Java chunk benchmarks, and writes `.local/perf/`. `mc-perf` is env-gated: `MC_SPANS=1`,
`MC_ALLOC_COUNT=1`, `MC_WORKERS=N`.

## Environment (mostly local, NOT in git)

- **Inputs:** `vendor/minecraft/1.8.9/` (client/server jars, remapped jars, decompiled
  `vanilla-client`/`vanilla-server`, mappings). Pinned in `research-lock.json`. Gitignored.
- **Rust:** stable `x86_64-pc-windows-gnu`; linker = w64devkit GCC (`.cargo/config.toml`).
  `cargo` lives at `%USERPROFILE%\.cargo\bin` (new shells).
  **Gotcha:** w64devkit lacks `libgcc_eh.a`; we copied Rust's into it (re-copy if re-extracted).
- **Java:** system Java 8 (Temurin) for the 1.8.9 server/Forge; JDK 21 at `vendor/tools/jdk21`
  for Vineflower.
- **Reference:** `vendor/horizon` (commit `2f8e8b7`, LGPL, gitignored) - recreate via
  `docs/horizon-reuse.md`; re-sync before copying code.
- **Windows note:** the console mangles non-ASCII in output; the files themselves are fine.
- **Git:** initialized and pushed. Remote `origin = https://github.com/enthu-ux/rusted_mc`
  (branch `main`, identity `enthu-ux <wassimbariza@gmail.com>`). Gitignored: `vendor/horizon/`,
  `vendor/minecraft/`, `vendor/reference/`, `vendor/tools/jdk21|w64devkit|*.jar|*.zip|*.exe`,
  `target/`, `.local/`, `logs/`. Push a session's work with `git add -A && git commit -m ... && git push`.
- **Verify:** `scripts/verify.ps1` runs tests + format + all parity harnesses in one command.

## Current state

- **Inputs:** done. Official 1.8.9 jars (hash-checked), clean vanilla decompile (0 Forge refs),
  MCP `stable_22`, protocol/block/item data (`vendor/reference/minecraft-data`).
- **Workspace:** 11 crates; `cargo test` + `fmt` green.
- **Performance (P0 substrate done):** `docs/performance-contract.md` + `docs/performance-baselines.md`;
  `mc-perf` (zero-dep: opt-in spans, `Stats`, `work::par_chunks_mut`, `alloc::CountingAllocator`) +
  `tools/perf_baseline.py`; spans on the worldgen hot path. First baseline: Rust base terrain
  **1.35 ms/chunk** vs Java 1.8.9 **3.63 ms/chunk** (~2.7x; caveated - Rust is base-terrain-only vs
  Java `provideChunk`). P0 remainder needs a running binary.
- **Implemented:** `mc-core::random` (exact `java.util.Random`); `mc-core::jmath` (`MathHelper`
  sin/cos table + floor/clamp/denormalize); `mc-worldgen::noise` (`Improved`; `Simplex` incl. 2D
  batch `func_151606_a`; `Octaves`; `Perlin` incl. batch `func_151600_a`/`func_151599_a`);
  `mc-worldgen::layer` (full `GenLayer*` biome stack + metadata); `mc-worldgen::terrain` (full
  overworld base terrain: `setBlocksInChunk` + `func_147423_a` + `replaceBlocksForBiome`, incl.
  Hills/Taiga/Swamp/Savanna.Mutated/Mesa); `mc-perf`.
- **Docs/policy:** `plan.md` (§1-17), `README.md`, `AGENTS.md`, `docs/PHILOSOPHY.md`,
  `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`, `docs/horizon-reuse.md`,
  `docs/horizon-performance-reuse.md`, `docs/performance-contract.md`, `docs/performance-baselines.md`,
  `scripts/verify.ps1`.
- **Repo:** pushed to `github.com/enthu-ux/rusted_mc` (`main`).

## Done so far (parity + perf)

- Harnesses green: `random_parity`, `noise_parity`, `noise_batch_parity`, `biome_layer_parity`,
  `terrain_parity` (full base terrain), `math_parity`. Unit tests: 11 `mc-worldgen`, 6 `mc-core`,
  10 `mc-perf`.
- Notable vanilla quirks encoded: `GenLayerHills.field_151628_d` unseeded top Zooms; Mesa cache
  `Random(0)` + `func_150619_a` orange-loop `+2` step; `ChunkPrimer` x/z transposition in
  `generateBiomeTerrain`.

## Next steps (prioritized, each with a pass condition)

**Recommended next: resume M1 `worldgen_seed`** (caves/ravines). P0's remaining items need a running
binary, so they wait.

1. **`worldgen_seed` — M1 (in progress).** `mc-core::jmath` + `math_parity` done; base terrain done.
   Remaining increments:
   (a) `MapGenCaves` + `MapGenRavine` (needs `mc-core::jmath` sin/cos - ready; `MapGenBase` seeds
   per neighbor chunk; block writes incl. water→lava, sand→sandstone, grass→biome top block);
   (b) structures (mineshaft / village / stronghold / scattered feature / ocean monument);
   (c) decoration (`BiomeDecorator` + the feature generators - biggest);
   (d) heightmap + biome array.
   *Done when:* the full chunk (blocks + metadata + biome + heightmap) matches for the Gate-C seeds
   and chunks.
2. **`light_parity`** - sky/block light arrays after edits match 1.8.9, with the engine **rewritten**
   for performance (plan §7/§13). Starts once `mc-world` chunks exist.
3. **P0 remainder (when a binary exists):** install `mc-perf::alloc::CountingAllocator` (and
   mimalloc) in the game/server binary; adopt `horizon-threads` stats/profiling + `horizon-system`;
   Java client/server FPS/RAM + **startup/world-join** baselines; `.hzperf` capture; CI perf gate.

## Open threads / decisions

- `nextGaussian` needs Java `StrictMath.log` (fdlibm) via `jmath` - same reason Horizon defers it.
  `MathHelper.atan2` (fdlibm tables) is also deferred until entity physics.
- External **sound** assets not downloaded (optional until audio work).
- `vendor/horizon` is gitignored (a nested checkout); recreate via `docs/horizon-reuse.md`.
- Decided but not built: the curated **fixes layer** (registry entries; modelled on Horizon's anticheat).
- **Rendering backend (locked):** **Vulkan via `wgpu`** (Horizon's `gfx` device layer) - **no
  OpenGL/LWJGL**. Internal API swap: pixels are gated by the render harness (same art, no
  glitches), so it is not a Deviation Registry entry (plan.md §13, `docs/ARCHITECTURE.md`).
- **Performance reuse:** `docs/horizon-performance-reuse.md` is the full Horizon performance ledger
  (allocator/startup, measurement, streaming/pregeneration, gfx, world storage, launch/asset prep)
  + phased adoption. `vendor/horizon` is at `2f8e8b7`; **no runtime code is reused yet** (P0 substrate
  is ours). LGPL placement: adapted code goes only in marked `mc-horizon-*` modules.
- **Lighting (decided):** clean rewrite for performance (packed SoA nibbles, incremental queue,
  cached heightmaps, parallel relight); light **values** parity-gated by `light_parity`. Rendered-light
  changes (smoothing/AO/colour) would be presentation deviations, off by default (plan §7/§13).
- **Biome-layer gotcha (verified by reflection):** `GenLayerHills.field_151628_d`'s two top `Zoom`
  nodes are never seeded, so they keep `worldGenSeed = 0`; the `RiverInit`/base chain beneath them is
  shared with the river chain and *is* seeded. `mc-worldgen::layer` pre-seeds the inner chain before
  wrapping it in the two unseeded Zooms.
- Biome/-noise oracles need `net.minecraft.init.Bootstrap.register()` before any `BiomeGenBase`
  static init (touches `Blocks`). Oracle runs drop an empty `logs/` (`gitignore`d).
- **Headless-world oracle:** `TerrainOracle` builds a real `net.minecraft.world.World` via a tiny
  subclass (2 abstract methods) + stub `ISaveHandler`, `WorldProviderSurface`, and
  `new WorldInfo(new WorldSettings(seed, GameType.CREATIVE, false, false, WorldType.DEFAULT)
  .setWorldName(opts), "name")`; `provider.registerWorld(world)` wires the biome manager. Reusable
  for `provideChunk`/tick/protocol oracles.
- Biome terrain constants (min/max height, temp, top/filler id+meta per id incl. mutations) were
  dumped from the live game; the full table is in `mc-worldgen::terrain`.
- **Mesa cache quirk:** `BiomeGenMesa` caches its per-world noise on first use with the *previous*
  `field_150622_aD` (0 on a fresh JVM), so `aE`/`aF` come from `Random(0)`; `TerrainOracle` runs one
  seed per JVM to match. `func_150619_a`'s orange-band loop increments the index by `nextInt(5)+2`.
- **Terrain coverage gap:** `terrain_parity` does not exercise `ice` (frozen-ocean is unreachable in
  the default layer stack) or `red_sandstone` (needs red-sand filler in the default generator; no
  biome uses it).

## Session log (append-only)

- **2026-11-20 - session 1 (build).** Downloaded inputs (jars, mappings, clean decompile, protocol
  data); installed Rust (GNU) + w64devkit linker; scaffolded the 10-crate workspace; implemented
  `JavaRandom` and the four noise generators; **`random_parity` and `noise_parity` green** (the
  noise harness caught a real accumulate-vs-clear bug). Wrote `plan.md` (sections 1-17),
  `docs/PHILOSOPHY.md`, `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`.
- **2026-11-20 - session 1 (handoff + git).** Added the handoff system (`handoff.md`, `AGENTS.md`),
  `scripts/verify.ps1`, `.gitattributes`, `docs/horizon-reuse.md`. Initialized git; remote
  `enthu-ux/rusted_mc`; first push. Fixed a committed 196 MB JDK zip (ignored `*.zip`).
- **2026-11-20 - session 2 (noise batch).** Ported the batch/array noise paths:
  `Simplex.func_151606_a` -> `Simplex::populate_noise_array`; `Perlin.func_151600_a`/`func_151599_a`
  -> `Perlin::generate`/`generate_default` (shared `simplex_dot` helper). Added
  `oracle/java/src/NoiseBatchOracle.java`, `crates/mc-worldgen/examples/noise_batch_probe.rs`,
  `tools/test_noise_batch_parity.py`; 3 golden unit tests. **`noise_batch_parity` green (369 lines);
  `verify.ps1` green: `3/3 passed`.** Existing `noise_parity` (1,047) and `random_parity` (3,848)
  untouched.
- **2026-11-20 - session 2 (biome layers).** Ported the `GenLayer*` biome stack for default
  overworld into `crates/mc-worldgen/src/layer.rs` (+ biome metadata): Island, FuzzyZoom, Zoom,
  AddIsland, RemoveTooMuchOcean, AddSnow, Edge (all 3 modes), AddMushroomIsland, DeepOcean, Biome,
  BiomeEdge, Hills, RareBiome, RiverInit, River, Smooth, Shore, RiverMix, VoronoiZoom. Added
  `oracle/java/src/BiomeLayerOracle.java`, `crates/mc-worldgen/examples/biome_layer_probe.rs`,
  `tools/test_biome_layer_parity.py`, 4 golden unit tests. Found and fixed a subtle vanilla quirk:
  `GenLayerHills.field_151628_d` is unseeded at its two top Zooms but seeded below (shared with the
  river chain) - confirmed via reflection. **`biome_layer_parity` green (4,626 lines);
  `verify.ps1` green: `4/4 passed`.**
- **2026-11-20 - session 3 (terrain shape).** Added a headless-`World` oracle pattern
  (`TerrainOracle`: a tiny `World` subclass + stub `ISaveHandler`, `WorldProviderSurface`,
  `Bootstrap.register()`), and ported the overworld base-terrain *shape*
  (`crates/mc-worldgen/src/terrain.rs`: `setBlocksInChunk` + `func_147423_a` density field, the
  noise stack in vanilla order, `biome_height` table dumped from the game). Added
  `crates/mc-worldgen/examples/terrain_probe.rs`, `tools/test_terrain_parity.py` (seeds
  0/12345/-7 × chunks (0,0),(1,-1); one line per (x,z) column, y=0..255), 1 golden test.
  **`terrain_parity` (shape) green at 1,542 lines; `verify.ps1` green: `5/5 passed`.**
- **2026-11-20 - session 4 (terrain surface).** Completed the base-terrain port in
  `mc-worldgen::terrain`: `replaceBlocksForBiome`, `BiomeGenBase.generateBiomeTerrain`, and the
  Hills / Taiga / Swamp / Savanna.Mutated / Mesa generators (`genTerrainBlocks` + `func_150619_a`
  + `func_180629_a`), with per-id temp + top/filler tables dumped from the game. Oracle switched
  to the real `provideChunk` (caves/ravines/structures off) and one seed per JVM (Mesa caches its
  per-world noise); fixtures expanded to 21 chunks covering mesa/mutations/snow/swamp/taiga/
  savanna/mushroom/jungle/ocean. **`terrain_parity` green at 16,191 lines** (22 block kinds);
  2 golden tests. `verify.ps1` green: `5/5 passed`.
- **2026-11-20 - session 5 (jmath + worldgen_seed start).** Locked the rendering backend
  (Vulkan/wgpu) in the docs. Started `worldgen_seed`: ported `mc-core::jmath`
  (`MathHelper` sin/cos table + floor/clamp/denormalize helpers) with a new `math_parity` harness
  (oracle `MathOracle.java` + probe `math_probe.rs`), 2 golden tests. **`math_parity` green at 492
  lines.** Scoped the rest of M1: caves/ravines, structures, decoration, heightmap/biome array.
  `verify.ps1` green: `6/6 passed`.
- **2026-11-21 - session 5b (Horizon perf audit).** Audited the vendored Horizon tree for
  performance reuse beyond culling: allocator/memory/startup (`horizon-threads`, `horizon-system`),
  measurement (`horizon-perf`/`-format`, `horizon-bench`, Callgrind/alloc tools), chunk
  streaming/pregeneration (`horizon-client-render/streaming.rs`, `docs/pregeneration.md`),
  gfx plumbing, world storage (`horizon-world` region/compression), launch/asset prep
  (`horizon-prepare`, `horizon-assets`). Wrote `docs/horizon-performance-reuse.md` (ledger +
  phased plan) and linked it from `plan.md`, `docs/horizon-reuse.md`, `handoff.md`. Honest finding:
  we reuse **no Horizon runtime code yet**; P0 (contract + measurement + allocator/system) comes
  after M1 worldgen.
- **2026-11-21 - session 6 (P0 start).** Re-synced `vendor/horizon` to `2f8e8b7`. Wrote
  `docs/performance-contract.md` (two-gate budgets, methodology, LGPL placement) and
  `docs/performance-baselines.md`. Added `mc-perf` (dependency-free spans + `Stats`) with tests,
  and `tools/perf_baseline.py` + `terrain_bench.rs` / `TerrainBench.java`. First baseline:
  Rust base terrain **1.35 ms/chunk** vs Java 1.8.9 **3.63 ms/chunk** (~2.7x; Rust is
  base-terrain-only vs Java `provideChunk`, so caveated). Next P0: allocator/system (mimalloc)
  when a binary exists, spans in hot paths, Java client/server + startup/join baselines.
- **2026-11-21 - session 6b (P0 substrate).** Extended `mc-perf`: `alloc::CountingAllocator` +
  `Counters` (per-instance, opt-in) for the zero-allocation budgets, `work::par_chunks_mut` +
  `worker_count` (deterministic independent parts, `MC_WORKERS`), and spans on
  `TerrainGen::{generate_chunk,set_blocks_in_chunk,replace_blocks_for_biome}`. `mc-perf` now has
  10 tests (+ `alloc_probe` example). `verify.ps1` green: `6/6`, 27 tests.
- **2026-11-21 - session 6 (handoff + git).** Rewrote this handoff for the next session; updated
  `README.md` and `plan.md` status. **Next session: resume M1 `worldgen_seed` (caves/ravines).**
  Baseline green: 6/6 harnesses, 27 tests. Committed and pushed.