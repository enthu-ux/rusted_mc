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

Expected today: `cargo test` and `fmt` pass (60 unit tests); `py tools/parity.py` reports
`10/10 passed` (`random_parity` 3,848 lines; `noise_parity` 1,047; `noise_batch_parity` 369;
`biome_layer_parity` 4,626; `terrain_parity` 16,191; `math_parity` 492; `caves_parity` 46,260;
`worldgen_parity` 7,770; `structures_parity` 2,702; `jhash_parity` 16 orders / 301 keys).

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
  sin/cos table + floor/clamp/denormalize); `mc-core::jhash` (`java.util.HashMap` iteration order:
  spread hash, doubling resize, `treeifyBin` early resize, `Integer`/`Long` hash codes);
  `mc-worldgen::noise` (`Improved`; `Simplex` incl. 2D
  batch `func_151606_a`; `Octaves`; `Perlin` incl. batch `func_151600_a`/`func_151599_a`);
  `mc-worldgen::layer` (full `GenLayer*` biome stack + metadata); `mc-worldgen::terrain` (full
  overworld base terrain: `setBlocksInChunk` + `func_147423_a` + `replaceBlocksForBiome`, incl.
  Hills/Taiga/Swamp/Savanna.Mutated/Mesa); `mc-worldgen::cave` (`MapGenBase` + `MapGenCaves` +
  `MapGenRavine`, wired into `TerrainGen::generate_chunk(cx, cz, GenOptions)`);
  `mc-worldgen::chunk` (canonical chunk artefacts: biome array + height map, `light_opacity`
  table); `mc-worldgen::mapgen` (`MapGenBase` driver, shared by the carve passes and structures);
  `mc-worldgen::structure` (`StructureStart`/`Piece`/`BoundingBox` model + `mineshaft.rs`:
  `MapGenMineshaft` registration and the whole piece layout); `mc-perf`.
- **Docs/policy:** `plan.md` (§1-17), `README.md`, `AGENTS.md`, `docs/PHILOSOPHY.md`,
  `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`, `docs/horizon-reuse.md`,
  `docs/horizon-performance-reuse.md`, `docs/performance-contract.md`, `docs/performance-baselines.md`,
  `scripts/verify.ps1`.
- **Repo:** pushed to `github.com/enthu-ux/rusted_mc` (`main`).

## Done so far (parity + perf)

- Harnesses green: `random_parity`, `noise_parity`, `noise_batch_parity`, `biome_layer_parity`,
  `terrain_parity` (full base terrain), `math_parity`, `caves_parity` (caves + ravines),
  `worldgen_parity` (canonical chunk: blocks + biome array + height map),
  `structures_parity` (mineshaft layout), `jhash_parity` (`java.util.HashMap` iteration order).
  Unit tests: 38 `mc-worldgen`, 12 `mc-core`, 10 `mc-perf`.
- Notable vanilla quirks encoded: `GenLayerHills.field_151628_d` unseeded top Zooms; Mesa cache
  `Random(0)` + `func_150619_a` orange-loop `+2` step; `ChunkPrimer` x/z transposition in
  `generateBiomeTerrain`; caves and ravines reach biomes through the **biome cache** (index
  `x + z*16`) while the surface pass reads the same array transposed (`z + x*16`); caves fill below
  y=11 with `Blocks.lava` (id 11) but ravines with `Blocks.flowing_lava` (id 10), and caves strip
  the meta off the replaced floor (`topBlock.getBlock().getDefaultState()`) where ravines keep it;
  the chunk **biome array** is a *linear* copy of that same (non-transposed) array, and
  `Chunk.generateSkylightMap` (not the primer constructor) builds the **height map** by scanning for
  the highest block with `getLightOpacity() != 0` (so water/ice count, lava and snow layers do not).

## Next steps (prioritized, each with a pass condition)

**Recommended next: the loot/enchantment RNG subsystem, then structure *placement*; or structure
layouts (b2) in the meantime.** P0's remaining items need a running binary.

1. **`worldgen_seed` — M1 (in progress).** `mc-core::jmath` + `math_parity` done; base terrain done;
   caves + ravines done (`cave.rs` + `caves_parity`); canonical chunk artefacts done (`chunk.rs` +
   `worldgen_parity`); mineshaft **layout** done (`structure/` + `structures_parity`). Remaining:
   (b1) mineshaft **block placement** — **blocked**, see below; (b2) the other structures' layouts
   (village / stronghold / scattered feature / ocean monument / nether bridge) - the harness already
   generalises (dump vanilla's structure NBT); (c) decoration (`BiomeDecorator` + feature generators -
   biggest); (d) structure/decoration blocks, once the placement infrastructure exists. Then the full
   chunk (blocks + metadata + biome + heightmap) for the Gate-C seeds/chunks is reachable and
   `worldgen_parity` can take the Gate-C name.
   *Done when:* the full chunk (blocks + metadata + biome + heightmap) matches for the Gate-C seeds
   and chunks.

   **Why placement is blocked (verified in source):** `addComponentParts` cannot be ported without the
   **loot RNG**. Each corridor rolls `rand.nextInt(100) == 0` twice per section; on a hit it calls
   `Items.enchanted_book.getRandom(rand)` as an argument, i.e. `EnchantmentHelper.buildEnchantmentList`
   -> `ItemBook.getItemEnchantability() == 1` -> `mapEnchantmentData` (a `HashMap<Integer,
   EnchantmentData>` whose *iteration order* feeds `WeightedRandom.getRandomItem`) plus the
   enchantment registry. A mineshaft makes roughly 5-10 such rolls, so this is not a rare branch, and
   those draws shift the rest of the chunk's block placement.

   **Unblocking sequence (in progress):**
   1a. ✅ `mc-core::jhash` - `java.util.HashMap` iteration order (spread hash, doubling resize,
   `treeifyBin`'s early resize, `Integer`/`Long` hash codes), parity-gated by `jhash_parity`. This is
   the piece the enchantment map needs.
   1b. **next:** loot + enchantment RNG - `WeightedRandom.getRandomItem`, `WeightedRandomChestContent`
   (+ the 13-entry mineshaft weight list and `func_177629_a`), `Enchantment` table (id/weight/
   min-max level/applicability - dump it from the game like `BlockTableOracle`), then
   `EnchantmentHelper.buildEnchantmentList` + `mapEnchantmentData`. Harness: an oracle that runs the
   real `generateChestContents` over fixed seeds and prints the *random stream afterwards*
   (e.g. the next 32 `nextInt` values), so the draw sequence is compared, not the items.
   1c. mineshaft placement (`StructureComponent` helpers + the four `addComponentParts` bodies),
   the `populate` oracle (design below) and the block harness.
   1d. the other structures' layouts (village / stronghold / scattered / monument / nether bridge);
   then decoration (`BiomeDecorator` + feature generators - biggest). Then the full chunk (blocks +
   metadata + biome + heightmap) for the Gate-C seeds/chunks is reachable and `worldgen_parity` can
   take the Gate-C name.
   *Done when:* the full chunk (blocks + metadata + biome + heightmap) matches for the Gate-C seeds
   and chunks. Porting it means porting enchantments
   (weights/levels per item) **and** Java `HashMap` iteration order for `Integer` keys - i.e. the
   `plan.md` §7 "expect to reimplement Java HashMap order" item. Sequenced properly: (1) loot tables +
   `WeightedRandomChestContent` (pure arrays, order-independent), (2) enchantment registry + a
   `java-hashmap` iteration-order helper, (3) mineshaft placement, (4) the `populate` oracle (design
   below) and the block harness, (5) decoration. Doing (3) before (1)+(2) would produce chunks that
   match only where no chest rolled - which is almost none of them.
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
- **Cave/ravine oracle note:** the oracle would NPE in `World.getBiomeGenForCoords` (no chunk
  provider), so `CaveOracle`'s `TestWorld` overrides `isChunkLoaded(..) -> false`. That is the real
  game's state while generating a chunk: `ChunkProviderServer.loadChunk` registers the chunk in
  `id2ChunkMap` only *after* `generateChunk`, so `chunkExists` is false and the biome-provider
  (cache) path is taken. Verified in `ChunkProviderServer`/`World`.
- **Cave/ravine coverage gap:** `caves_parity` exercises every carve branch except two rare ones -
  ravines writing a top block and caves stripping the meta off a **mesa** floor (red sand ->
  plain sand). Both are transcribed and pinned by a spec unit test
  (`mesa_floor_top_block_differs_between_caves_and_ravines`), but no corpus chunk reaches them.
  Mesa chunks with a cave-carved grass floor would close the gap.
- **Height map / biome array notes:** `chunk::light_opacity` and the material-air set are dumped from
  the live game by `BlockTableOracle` (re-run it if the block set changes). The height map is built by
  `Chunk.generateSkylightMap`, i.e. it is `1 +` the highest block with `getLightOpacity() != 0`
  (water/ice are opaque-ish, lava/snow layers are not); `heightMapMinimum` and the skylight/block-light
  arrays are **not** ported yet (they belong to `light_parity`). `worldgen_parity` therefore compares
  the **pre-decoration** `provideChunk` chunk; Gate C (`worldgen_seed`) waits for structures +
  decoration, which change blocks.
- **Structure phases (important):** `provideChunk` only *registers* structures
  (`MapGenBase.generate` -> `MapGenStructure.recursiveGenerate` -> `canSpawnStructureAtCoords` ->
  `getStructureStart`); **no blocks are written**. Blocks come from `populate` ->
  `MapGenStructure.generateStructure` -> `StructureStart.generateStructure` ->
  `StructureComponent.addComponentParts`, which writes through `world.setBlockState`. Populate uses a
  *different* RNG stream (`rand.setSeed(worldSeed)`, then two `nextLong()/2*2+1` draws, then
  `x*a + z*b ^ seed`), so layout and placement are independently testable.
- **Populate oracle (needed for b1/decoration, design):** `world.setBlockState` needs a world that can
  hold chunks, so the headless world needs a stub `IChunkProvider` that stores chunks generated by a
  `ChunkProviderGenerate` (and `populate` must be driven explicitly, as `ChunkProviderServer` does).
  `Chunk.setBlockState` also touches the height map and skylight/relight queues, so the final artifact
  will include those (another reason to pair it with `light_parity`).
- **Mineshaft layout quirks (found by the harness):** `EnumFacing.getHorizontalIndex()` is
  *not* declaration order (`SOUTH 0, WEST 1, NORTH 2, EAST 3`) - it is written to NBT as `O`/`D`;
  `Cross`'s constructor never sets `coordBaseMode`, so its `O` is always -1 while its own `D` holds
  the direction; `func_175890_b` forwards its *own* `var1` (the root Room) down the recursion, so the
  `+/-80` window test always measures from the root; `Cross`/`Corridor` bounding-box helpers draw RNG
  (`nextInt(4)` / `nextInt(3)+2`+loop) *before* the intersection test.
- **Structure coverage gaps:** `spawnerPlaced` (`hps`) is only ever set during block placement, so the
  layout corpus always has `hps=0`; villages/strongholds/scattered features/monuments and the nether
  bridge are unported; a chunk's `structureMap` is `HashMap`-ordered in vanilla, so the dump sorts by
  chunk coordinate (never rely on map order).
- **Java `HashMap` iteration order is a live parity dependency:** `mc-core::jhash` models it
  (verified: 16 insertion orders / 301 keys, including the `treeifyBin` early resize at the 9th
  colliding key). Two consumers:
  - `EnchantmentHelper.mapEnchantmentData` (Integer ids 0..35, cap 64, no bin ever reaches 8) - fully
    modelled, and this is the one that decides chest-book draws;
  - `MapGenStructure.structureMap` (Long chunk keys) - **only single-start chunks are well defined.**
    With two starts meeting a chunk, vanilla's iteration order depends on the map's whole history
    (how many chunks the world ever generated, hence capacity/treeification), so it is *not* a
    function of (seed, chunk): two worlds with the same seed but different exploration orders can
    place those chunks differently. Consequence: structure placement corpora use single-start
    chunks; our port keeps a deterministic (chunk-coordinate) order and does **not** try to
    replicate the history-dependent case. Actual treeification (8+ entries in a bin at capacity
    >= 64) is deliberately not modelled (`jhash.rs` documents it).
- **Loot/enchantment dependency for placement:** `StructureMineshaftPieces.Corridor` rolls a chest
  twice per section (≈5-10 per mineshaft) and the roll consumes enchantment RNG before anything else
  (`Items.enchanted_book.getRandom` -> `buildEnchantmentList` -> `mapEnchantmentData`). Chest *contents*
  also draw from `WeightedRandomChestContent` (array-ordered, portable) + `nextInt(27)` per slot.
  Plan the subsystem before any `addComponentParts` port.
- **Populate oracle design (ready to build):** the headless world needs a stub `IChunkProvider` that
  stores chunks produced by a `ChunkProviderGenerate` (with `mapFeaturesEnabled = true` and only the
  wanted feature enabled), because `world.setBlockState` -> `getChunkFromBlockCoords` ->
  `provider.provideChunk`. Drive `ChunkProviderGenerate.populate(provider, x, z)` explicitly (as
  `ChunkProviderServer` does), then read the chunk back. `isLiquidInStructureBoundingBox` and the
  piece writers only read/write inside the populating chunk, so the result is order-independent.

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
- **2026-11-21/22 - session 7 (caves + ravines).** Ported `MapGenBase` + `MapGenCaves` +
  `MapGenRavine` to `crates/mc-worldgen/src/cave.rs` and wired them into
  `TerrainGen::generate_chunk(cx, cz, GenOptions)` (`generate_chunk` renamed to
  `generate_base_chunk` for the carving-off path; `terrain_probe`/`terrain_bench` follow, so
  `terrain_parity` and the recorded baseline are unchanged). New harness `caves_parity`
  (`CaveOracle.java` + `cave_probe.rs` + `tools/test_caves_parity.py`) over 3 seeds x 14->15 chunks x
  modes {caves, ravines, both, base} = **46,260 lines match**, with a per-seed signal guard
  (vanilla caves/ravines must differ from vanilla base terrain) and first-differing-block-coordinate
  reporting. Found and encoded: caves/ravines read biomes via the biome **cache** (`x + z*16`) while
  the surface pass reads the same array transposed; caves use `Blocks.lava` (11) below y=11 and
  ravines `Blocks.flowing_lava` (10); caves strip meta on the replaced floor, ravines keep it;
  `MapGenBase`'s `int * long` seed mix needs wrapping arithmetic. 11 new golden unit tests (22
  `mc-worldgen`, **38 total**) + 1 spec test for the mesa meta difference. `verify.ps1`: **7/7**.
  Perf: spans `worldgen.caves`/`worldgen.ravines` + committed `carve_bench` example; carve passes
  cost **+0.29 ms/chunk** over base (median 1.31 -> 1.60), recorded in
  `docs/performance-baselines.md` and the contract's budget table.
  Coverage gaps recorded: ravine top-block writes and mesa cave floors are transcribed but not in the
  corpus. (A throwaway fixture-hunting script was over-provisioned and OOM-killed; replaced with a
  streaming version - nothing else was affected.)
- **2026-11-22 - session 8 (canonical chunk: biome array + height map).** Added
  `crates/mc-worldgen/src/chunk.rs`: `GeneratedChunk { blocks, biomes, height_map }`, returned by
  `TerrainGen::generate_chunk` (so `generate_base_chunk` stays blocks-only for `terrain_parity` and
  the base-terrain baseline). Ported the tail of `provideChunk`: the biome array is a *linear* copy of
  the generation biome ids (`(byte) biomesForGeneration[i].biomeID`, index `x + z*16` - which also
  pins the layout the cave biome lookup relies on), and the height map is `Chunk.generateSkylightMap`'s
  scan for the highest `getLightOpacity() != 0` block. Opacity/material tables were dumped from the
  live game with a new `BlockTableOracle` (the block registry is *defaulted to air*, so unregistered
  ids must be skipped - a first dump silently listed 198-255 as air). New harness `worldgen_parity`
  (`WorldgenOracle.java` + `worldgen_probe.rs` + `tools/test_worldgen_parity.py`): Gate-C seeds/chunks
  + cave/ravine/mesa/ocean fixtures, blocks + biome + heightmap = **7,770 lines match**, with a
  per-seed vacuity guard (heights must have relief with min >= 1; biomes must not be constant) and
  per-artefact first-coordinate reporting (verified with a negative self-test). 7 new golden unit
  tests (**45 total**). `verify.ps1`: **8/8**. `worldgen_parity` covers the *pre-decoration* chunk;
  Gate C's `worldgen_seed` is reserved until structures + decoration land (noted in `plan.md`).
  Re-measured the perf table: carving +0.37 ms/chunk, artefacts +0.06 ms (`worldgen.artifacts`).
- **2026-11-22 - session 9 (mineshaft registration + layout).** Split `MapGenBase` into
  `mc-worldgen::mapgen` (shared driver; `cave.rs` now uses it) and added `mc-worldgen::structure`
  (`StructureStart`/`Piece`/`PieceKind`/`BoundingBox` + `mineshaft.rs`): `MapGenStructure`
  registration (neighbourhood walk, discard `nextInt()`, `canSpawnStructureAtCoords`), the root Room
  and the whole `StructureMineshaftPieces` layout (`createPiece` dispatch, Corridor/Cross/Room/Stairs
  bounding boxes and `buildComponent` recursion), plus `markAvailableHeight`. Wire-up:
  `GenOptions.mineshafts`, `GeneratedChunk.structures` (the starts meeting the chunk, sorted), and a
  `worldgen.mineshafts` span. New harness `structures_parity` (`StructuresOracle.java` +
  `structures_probe.rs` + `tools/test_structures_parity.py`) dumps vanilla's *own* structure NBT
  (`writeStructureComponentsToNBT`) and compares: **2,702 lines match** (3 seeds x 14 chunks, 21
  starts, up to 188 pieces each), with a per-seed "at least one start" guard. The harness caught three
  real transcription bugs on the way (`getHorizontalIndex` order, NBT key sort order, `Cross`'s null
  `coordBaseMode`). 9 new unit tests (**54 total**). Oracle notes: the headless world needs
  `mapStorage = new MapStorage(handler)` because `World` leaves it null (only `WorldServer` builds
  it); structure blocks are *not* written here - that is `populate`, next. `verify.ps1`: **9/9**
  (`structures_parity` auto-discovered by `parity.py`). Perf: re-ran `carve_bench` (now with a
  `+mineshafts` row); the dev box was loaded, so only the spans were recorded
  (`worldgen.mineshafts` 0.05-0.08 ms/chunk) and the median table was left as-is, per the contract's
  "discard loaded runs" rule.
- **2026-11-22 - session 10 (b1 scoped, blocked; no code).** Started mineshaft block placement. Read
  `StructureComponent`'s placement helpers (`isLiquidInStructureBoundingBox`, offsets,
  `fillWithBlocks`/`fillWithRandomizedBlocks`/`func_175805_a`/`randomlyPlaceBlock`) and
  `StructureMineshaftPieces.addComponentParts` for all four pieces. Finding: corridor chests roll
  twice per section (≈5-10 per mineshaft) and each roll consumes **enchantment** RNG
  (`Items.enchanted_book.getRandom` -> `EnchantmentHelper.buildEnchantmentList` ->
  `ItemBook.getItemEnchantability() == 1` -> `mapEnchantmentData`, a `HashMap` whose iteration order
  feeds `WeightedRandom`). So placement cannot be finished without the loot/enchantment subsystem and
  a Java `HashMap` order model; a placement port without them would match chunks only where no chest
  rolled. Also confirmed the *other* HashMap-order dependency: two starts meeting one chunk share the
  `populate` rand in `structureMap.values()` order, so placement corpora must use single-start chunks.
  Recorded the sequencing and the ready-to-build populate-oracle design in "Next steps"/"Open
  threads". Tree left clean; nothing committed beyond session 9.
- **2026-11-22 - session 11 (unblocking: Java HashMap order).** Chose the parity-first path. Added
  `mc-core::jhash` (`JavaHashMap`: OpenJDK 8 spread hash, 16/0.75 doubling resize with lo/hi order
  preserved, `treeifyBin`'s early resize, `Integer`/`Long` hash codes) with 6 unit tests, plus
  `HashOrderOracle.java` + `jhash_probe.rs` + `tools/test_jhash_parity.py`: **16 insertion orders /
  301 keys match** the real `HashMap` (`keySet()` order). The harness immediately caught a real
  semantic miss: a run of colliding keys resizes earlier than `size > threshold`, because `putVal`
  calls `treeifyBin` first and with a table of 16/32 that *resizes* instead of treeifying (the 9th
  colliding entry triggers it). Also established (and recorded above) that multi-start chunk
  placement is history-dependent in vanilla, so only the enchantment map needs full order parity.
  `verify.ps1`: **10/10**, 60 tests. Next session: 1b (loot + enchantment RNG), then 1c (placement +
  populate oracle).