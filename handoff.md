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
- `docs/ARCHITECTURE.md` - crate map and the two gates (parity + performance).
- `docs/deviations.md` + `deviations.toml` - intended deviations from vanilla.
- `docs/horizon-reuse.md` - what we took from Horizon and why (working copy: `vendor/horizon/`).
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

Expected today: `cargo test` and `fmt` pass; `py tools/parity.py` reports `6/6 passed`
(`random_parity` 3,848 lines; `noise_parity` 1,047 lines; `noise_batch_parity` 369 lines;
`biome_layer_parity` 4,626 lines; `terrain_parity` 16,191 lines; `math_parity` 492 lines).

## Environment (mostly local, NOT in git)

- **Inputs:** `vendor/minecraft/1.8.9/` (client/server jars, remapped jars, decompiled
  `vanilla-client`/`vanilla-server`, mappings). Pinned in `research-lock.json`. Gitignored.
- **Rust:** stable `x86_64-pc-windows-gnu`; linker = w64devkit GCC (`.cargo/config.toml`).
  `cargo` lives at `%USERPROFILE%\.cargo\bin` (new shells).
  **Gotcha:** w64devkit lacks `libgcc_eh.a`; we copied Rust's into it (re-copy if re-extracted).
- **Java:** system Java 8 (Temurin) for the 1.8.9 server/Forge; JDK 21 at `vendor/tools/jdk21`
  for Vineflower.
- **Reference:** `vendor/horizon` (commit `0e5055d`, LGPL, gitignored) - recreate via
  `docs/horizon-reuse.md`.
- **Windows note:** the console mangles non-ASCII in output; the files themselves are fine.
- **Git:** initialized and pushed. Remote `origin = https://github.com/enthu-ux/rusted_mc`
  (branch `main`, identity `enthu-ux <wassimbariza@gmail.com>`). Gitignored: `vendor/horizon/`,
  `vendor/minecraft/`, `vendor/reference/`, `vendor/tools/jdk21|w64devkit|*.jar|*.zip|*.exe`,
  `target/`, `.local/`. Push a session's work with `git add -A && git commit -m ... && git push`.
- **Verify:** `scripts/verify.ps1` runs tests + format + all parity harnesses in one command.

## Current state

- **Inputs:** done. Official 1.8.9 jars (hash-checked), clean vanilla decompile (0 Forge refs),
  MCP `stable_22`, protocol/block/item data (`vendor/reference/minecraft-data`).
- **Workspace:** 11 crates scaffolded (incl. `mc-perf`); `cargo test` + `fmt` green.
- **Performance (P0):** `docs/performance-contract.md` (budgets/methodology) + `docs/performance-baselines.md`;
  `mc-perf` (spans/`Stats`, zero-dep) + `tools/perf_baseline.py`; first base-terrain baseline
  (Rust 1.35 ms/chunk vs Java 3.63 ms/chunk, caveated).
- **Harnesses green:** `random_parity` (3,848 lines), `noise_parity` (1,047 lines),
  `noise_batch_parity` (369 lines), `biome_layer_parity` (4,626 lines), `terrain_parity`
  (16,191 lines; full base terrain: shape + surface blocks), `math_parity` (492 lines).
- **Implemented:** `mc-core::random` (exact `java.util.Random`); `mc-core::jmath` (`MathHelper`
  sin/cos table + floor/clamp/denormalize helpers); `mc-worldgen::noise`
  (`Improved`; `Simplex` incl. 2D batch `func_151606_a`; `Octaves`; `Perlin` incl. batch
  `func_151600_a`/`func_151599_a`); `mc-worldgen::layer` (the full `GenLayer*` biome stack +
  biome metadata); `mc-worldgen::terrain` (full overworld base terrain: `setBlocksInChunk` +
  `func_147423_a` + `replaceBlocksForBiome`, incl. the Hills/Taiga/Swamp/Savanna.Mutated/Mesa
  generators). Golden unit tests: 11 in `mc-worldgen`, 6 in `mc-core`.
- **Docs/policy:** `plan.md` (sections 1-17), `README.md`, `AGENTS.md`, `docs/PHILOSOPHY.md`,
  `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`, `docs/horizon-reuse.md`,
  `scripts/verify.ps1`.
- **Repo:** pushed to `github.com/enthu-ux/rusted_mc` (`main`).

## Next steps (prioritized, each with a pass condition)

**P0 — performance foundation (in progress).** Done: `vendor/horizon` re-synced to `2f8e8b7`; the
performance contract (`docs/performance-contract.md`); `mc-perf` + `tools/perf_baseline.py` + first
chunk-gen baseline. Next: adopt `horizon-threads`/`horizon-system` (mimalloc) when the first binary
exists; add spans to hot paths; Java client/server FPS/RAM + startup/join baselines.

0. ~~**`noise_batch_parity`**~~ **DONE (2026-11-20):** ported `Perlin.func_151599_a`/`func_151600_a`
   and `Simplex.func_151606_a`; harness green at 369 lines.
1. ~~**`biome_layer_parity`**~~ **DONE (2026-11-20):** ported the `GenLayer*` stack
   (`mc-worldgen::layer`) + biome metadata; harness green at 4,626 lines (seeds 0/12345/-7, both
   `genBiomes` and the Voronoi `biomeIndexLayer`, positive + negative chunk areas).
2. ~~**`terrain_parity`**~~ **DONE (2026-11-20):** full base terrain ported
   (`mc-worldgen::terrain`): shape + `replaceBlocksForBiome` + the Hills/Taiga/Swamp/
   Savanna.Mutated/Mesa generators, running the real `provideChunk` in the oracle; harness green
   at 16,191 lines (seeds 0/12345/-7 × 21 chunks covering mesa incl. mutations, snow, swamp,
   mega-taiga podzol, savanna/hills mutations, mushroom, jungle, ocean). Covered states: 22 block
   kinds. **Not covered:** `ice` (frozen-ocean is unreachable in the default layer stack) and
   `red_sandstone` (needs red-sand filler in the default generator; no biome uses it).
3. **`worldgen_seed` — M1 (in progress).** `mc-core::jmath` (MathHelper sin/cos table + helpers)
   and `math_parity` are done. Remaining increments:
   (a) `MapGenCaves` + `MapGenRavine`; (b) structures (mineshaft / village / stronghold /
   scattered feature / ocean monument); (c) decoration (`BiomeDecorator` + the feature generators);
   (d) heightmap + biome array. *Done when:* the full chunk (blocks + metadata + biome + heightmap)
   matches for the Gate-C seeds/chunks.
4. **`light_parity`** - sky/block light arrays after edits match 1.8.9, with the engine **rewritten**
   for performance (see plan.md §7/§13). Starts once `mc-world` chunks exist.
5. **Deferred:** performance architecture contract + perf harness (benchmark vs Java 1.8.9 +
   OptiFine, instruction/allocation budgets).

## Open threads / decisions

- `nextGaussian` needs Java `StrictMath.log` (fdlibm) via `jmath` - same reason Horizon defers it.
- External **sound** assets not downloaded (optional until audio work).
- `vendor/horizon` is gitignored (a nested checkout); recreate via `docs/horizon-reuse.md`.
- Decided but not built: the curated **fixes layer** (registry entries; modelled on Horizon's anticheat).
- **Rendering backend (locked):** **Vulkan via `wgpu`** (Horizon's `gfx` device layer) - **no
  OpenGL/LWJGL**. Internal API swap: pixels are gated by the render harness (same art, no
  glitches), so it is not a Deviation Registry entry (plan.md §13, `docs/ARCHITECTURE.md`).
- **Performance reuse:** `docs/horizon-performance-reuse.md` is the full Horizon performance ledger
  (allocator/startup, measurement, streaming/pregeneration, gfx, world storage, launch/asset prep)
  + phased adoption. Today we reuse **no Horizon runtime code**; **P0** = perf architecture contract
  + measurement baseline + `horizon-threads` allocator/system, to land before M3 (does not block M1).
  Re-sync `vendor/horizon` (currently `0e5055d`) before copying any code.
- **Lighting (decided):** clean rewrite for performance (packed SoA nibbles, incremental queue,
  cached heightmaps, parallel relight); light **values** parity-gated by `light_parity`. Rendered-light
  changes (smoothing/AO/colour) would be presentation deviations, off by default (plan §7/§13,
  `docs/ARCHITECTURE.md`).
- **Biome-layer gotcha (verified by reflection):** `GenLayerHills.field_151628_d`'s two top `Zoom`
  nodes are never seeded (`GenLayerHills` doesn't override `initWorldGenSeed`), so they keep
  `worldGenSeed = 0`; the `RiverInit`/base chain beneath them is shared with the river chain and
  *is* seeded. `mc-worldgen::layer` mirrors this by pre-seeding the inner chain before wrapping it
  in the two unseeded Zooms (see the comment in `BiomeLayers::new`).
- Biome/-noise oracles need `net.minecraft.init.Bootstrap.register()` before any `BiomeGenBase`
  static init (touches `Blocks`). Oracle runs drop an empty `logs/` (`gitignore`d).
- **Headless-world oracle:** `TerrainOracle` builds a real `net.minecraft.world.World` via a tiny
  subclass (only 2 abstract methods) + stub `ISaveHandler`, `WorldProviderSurface`, and
  `new WorldInfo(new WorldSettings(seed, GameType.CREATIVE, false, false, WorldType.DEFAULT)
  .setWorldName(opts), "name")`; `provider.registerWorld(world)` wires the biome manager. This is
  the reusable route for `provideChunk`/tick/protocol oracles.
- Biome terrain constants (min/max height, temp, top/filler block id+meta per id incl. mutations)
  were dumped from the live game; the full table is in `mc-worldgen::terrain`.
- **Mesa cache quirk:** `BiomeGenMesa` caches its per-world noise on first use with the *previous*
  `field_150622_aD` (0 on a fresh JVM), so `aE`/`aF` come from `Random(0)`; `TerrainOracle` runs
  one seed per JVM to match. `func_150619_a`'s orange-band loop increments the index by
  `nextInt(5)+2` per step (body `+=` **and** for-loop `++`).

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
