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

## At a glance

- **Milestone:** M1 `worldgen_seed` (in progress). Parity-gated so far: base terrain, caves/ravines,
  the canonical chunk (biome array + height map), **all six structure layouts** (mineshaft,
  scattered-feature temple/hut, ocean monument, nether fortress, village, stronghold),
  `java.util.HashMap` order, the loot + enchantment RNG, mineshaft **block placement** (the
  `populate` half; a curated corpus), **all of decoration** (layers 1-4 + the per-biome `decorate`
  overrides), the **swamp hut + desert pyramid + jungle pyramid block placement** (the
  scattered-feature `populate` half), and the **ocean monument block placement** (shell + room graph).
- **Next increment:** the nether fortress's block placement, then the village and stronghold; then the
  full Gate-C chunk.
- **Baseline:** `scripts/verify.ps1` -> **15/15** harnesses, **130** unit tests, fmt clean.

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

Expected today: `cargo test` and `fmt` pass (130 unit tests); `py tools/parity.py` reports
`15/15 passed` (`random_parity` 3,848 lines; `noise_parity` 1,047; `noise_batch_parity` 369;
`biome_layer_parity` 4,626; `terrain_parity` 16,191; `math_parity` 492; `caves_parity` 46,260;
`worldgen_parity` 7,770; `structures_parity` 10,533 lines / 96 starts (mineshaft + scattered feature +
monument + nether fortress + village + stronghold); `jhash_parity` 16 orders / 301 keys;
`loot_parity` 25 enchantments / 28 cases = 695 lines; `mineshaft_placement_parity` 14 chunks = 3,598 lines;
`temple_placement_parity` 4 fixtures = 1,028 lines, 1,562 blocks (swamp hut + desert pyramid + jungle
pyramid); `monument_placement_parity` 3 seeds = 771 lines, 7,518 blocks (ocean monument);
`decoration_parity` 7 modes x 3 seeds = 32,896 lines, all decoration incl. the per-biome overrides).

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
- **Workspace:** 11 crates; `cargo test` + `fmt` green (13/13 harnesses, 110 tests).
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
  `MapGenMineshaft` registration and the whole piece layout + `scattered.rs`:
  `MapGenScatteredFeature` registration and the desert/jungle temple + swamp-hut piece
  constructors + `monument.rs`: `StructureOceanMonument` registration and the `MonumentBuilding`
  start box + `netherbridge.rs`: a standalone `MapGenNetherBridge` (nether) with the full
  `StructureNetherBridgePieces` weighted layout + `village.rs`: `MapGenVillage` with the weighted
  `StructureVillagePieces` road/building tree, plus `StructureStart::sizeable` + `stronghold.rs`:
  `MapGenStronghold` with `findBiomePosition` ring placement and the weighted `StructureStrongholdPieces`
  tree; `mc-worldgen::loot`
  (`WeightedRandom`, `WeightedRandomChestContent.generateChestContents` + the 13-entry mineshaft list +
  `func_177629_a`, the `Enchantment` registry table + `EnchantmentHelper.mapEnchantmentData`/
  `buildEnchantmentList`/`addRandomEnchantment` on the book path); `mc-worldgen::structure::placement`
  (the `StructureComponent` helpers + the four `StructureMineshaftPieces.addComponentParts` bodies +
  `MapGenStructure.generateStructure`/`StructureStart.generateStructure`, and the **swamp hut +
  desert pyramid + jungle pyramid** `addComponentParts` bodies incl. `func_74935_a`'s average-height
  offset, the temple chest loot + `BlockChest.correctFacing`, the jungle `Stones` selector and its
  tripwire/lever/piston/repeater/dispenser states + the generic `getMetadataWithOffset` branches),
  driven by `TerrainGen::populate_mineshaft_chunk` / `populate_scattered_chunk`; `mc-worldgen::structure::monument_placement`
  (the `MonumentBuilding` shell helpers, base pillars, water walls, the room-graph rebuild from
  `(seed, start chunk)`, and the `Entry/Core/Simple/SimpleTop/Double*` and `Wing/Penthouse` room bodies),
  driven by `TerrainGen::populate_monument_chunk`; `mc-worldgen::decorate` (**all decoration layers incl. the per-biome `decorate` overrides**: the
  `DecorateWorld`/`Chunk` block + height-map + column-skylight shim, `ChunkProviderSettings` defaults,
  `WorldGenMinable` ore generation, the surface `WorldGenSand`/`WorldGenClay`/gravel loops, the
  vegetation generators, the tree generators incl. the `genBigTreeChance` hooks and the tree
  `breakBlock` leaf-flagging, the `WorldGenLiquids` springs incl. the one-step liquid tick, and the
  override generators: `WorldGenDoublePlant`, `WorldGenMelon`, `WorldGenVines` (a no-op at the
  jungle override's `y=128`), `WorldGenIceSpike`/`WorldGenIcePath`, `WorldGenBlockBlob`,
  `WorldGenDesertWells`, hills emerald/monster egg; also the plain `BiomeGenMutated` biomes, whose
  `decorate` runs the base biome's decorator), driven by `TerrainGen::decorate_chunk`; `mc-perf`.
- **Docs/policy:** `plan.md` (§1-17), `README.md`, `AGENTS.md`, `docs/PHILOSOPHY.md`,
  `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`, `docs/horizon-reuse.md`,
  `docs/horizon-performance-reuse.md`, `docs/performance-contract.md`, `docs/performance-baselines.md`,
  `scripts/verify.ps1`.
- **Repo:** pushed to `github.com/enthu-ux/rusted_mc` (`main`).

## Done so far (parity + perf)

- Harnesses green: `random_parity`, `noise_parity`, `noise_batch_parity`, `biome_layer_parity`,
  `terrain_parity` (full base terrain), `math_parity`, `caves_parity` (caves + ravines),
  `worldgen_parity` (canonical chunk: blocks + biome array + height map),
  `structures_parity` (all six structure layouts: mineshaft, temple/hut, monument, fortress, village,
  stronghold), `jhash_parity` (`java.util.HashMap` iteration order), `loot_parity` (loot + enchantment
  RNG), `mineshaft_placement_parity` (mineshaft block placement), `temple_placement_parity` (swamp-hut
  + desert-pyramid + jungle-pyramid block placement), `monument_placement_parity` (ocean-monument block
  placement), `decoration_parity` (all decoration:
  ores, surface, vegetation, trees, springs, the full chunk with the per-biome overrides).
  Unit tests: 107 `mc-worldgen`, 13 `mc-core`, 10 `mc-perf`.
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

**Recommended next: the remaining structure block placement** (the nether fortress, then the village
and stronghold). The generic `+8` populate driver and the
`StructureComponent` helpers are shared; the swamp hut + desert/jungle pyramids and the ocean monument
are ported and their harnesses (`temple_placement_parity`/`monument_placement_parity`) are the
templates. Decoration is complete and parity-gated
end-to-end (`decoration_parity`'s `full` mode). With all structure placement done, the full Gate-C
chunk (blocks + metadata + biome + heightmap) is reachable and `worldgen_parity` can take the Gate-C
name. P0's remaining items need a running binary.

1. **`worldgen_seed` — M1 (in progress).** `mc-core::jmath` + `math_parity` done; base terrain done;
   caves + ravines done (`cave.rs` + `caves_parity`); canonical chunk artefacts done (`chunk.rs` +
   `worldgen_parity`); mineshaft **layout** done (`structure/` + `structures_parity`); the **loot +
   enchantment RNG** done (`loot.rs` + `loot_parity`); mineshaft **block placement** done
   (`structure/placement.rs` + `mineshaft_placement_parity`); scattered-feature (temple/hut) **layout**
   done (`structure/scattered.rs` + `structures_parity`); ocean-monument **layout** done
   (`structure/monument.rs` + `structures_parity`); nether-fortress **layout** done
   (`structure/netherbridge.rs` + `structures_parity`); village **layout** done
   (`structure/village.rs` + `structures_parity`); stronghold **layout** done
   (`structure/stronghold.rs` + `structures_parity`); **decoration complete** (`decorate.rs` +
   `decoration_parity`, incl. the per-biome `decorate` overrides in the `full` mode); **swamp hut +
   desert pyramid + jungle pyramid block placement** done (`structure/placement.rs` +
   `temple_placement_parity`); **ocean monument block placement** done
   (`structure/monument_placement.rs` + `monument_placement_parity`). All six structure layouts are
   ported; remaining: **block placement** for
   the nether fortress, village and stronghold. Then the full chunk (blocks + metadata + biome + heightmap) for the Gate-C
   seeds/chunks is reachable and `worldgen_parity` can take the Gate-C name.
   *Done when:* the full chunk (blocks + metadata + biome + heightmap) matches for the Gate-C seeds
   and chunks.

   **Why placement needed the loot RNG (verified in source):** `addComponentParts` rolls the chest
   list with `Items.enchanted_book.getRandom(rand)` (via `func_177629_a`); that runs
   `EnchantmentHelper.buildEnchantmentList` -> `ItemBook.getItemEnchantability() == 1` ->
   `mapEnchantmentData` (a `HashMap<Integer, EnchantmentData>` whose *iteration order* feeds
   `WeightedRandom.getRandomItem`) plus the enchantment registry. A mineshaft makes roughly 5-10 such
   rolls, so this is not a rare branch, and those draws shift the rest of the chunk's block placement.
   **Ported in 1b** - see below.

   **Unblocking sequence (1a/1b done):**
   1a. ✅ `mc-core::jhash` - `java.util.HashMap` iteration order (spread hash, doubling resize,
   `treeifyBin`'s early resize, `Integer`/`Long` hash codes), parity-gated by `jhash_parity`. This is
   the piece the enchantment map needs; `JavaHashMap` also gained order-preserving `retain` for
   `buildEnchantmentList`'s iterator `remove()`.
   1b. ✅ loot + enchantment RNG - `mc-worldgen::loot`: `WeightedRandom.getRandomItem`,
   `WeightedRandomChestContent.generateChestContents` (+ the 13-entry mineshaft weight list and
   `func_177629_a`), the `Enchantment` registry table (id/weight/max level/type + the subclasses'
   enchantability/`canApplyTogether`), `EnchantmentHelper.mapEnchantmentData`/
   `buildEnchantmentList`/`addRandomEnchantment` (book path). Harness `loot_parity`
   (`LootOracle.java` + `examples/loot_probe.rs`): the registry table + exclusivity matrix against the
   live `Enchantment` registry, random book rolls (the stored enchantments pin the *HashMap order*,
   since the weighted pick sums in `values()` order) and the real `generateChestContents` (the list,
   every slot write, and the next 32 draws) - so the **draw sequence** is compared, not just the items.
   Note: the stream alone cannot see map order (only the number of draws); the printed enchantment ids
   are what pin it.
   1c. ✅ mineshaft block placement - `mc-worldgen::structure::placement`: the `StructureComponent`
   helpers (`isLiquidInStructureBoundingBox`, offsets, `fillWithBlocks`/`func_175805_a`/
   `randomlyPlaceBlock`/`randomlyRareFillWithBlocks`), the four `addComponentParts` bodies, and the
   corridor's `generateChestContents` override (rails + the loot draws). `MapGenStructure.generateStructure`'s
   box is offset by 8 (bytecode-verified), so a chunk is written by the four populate passes whose boxes
   cover it; `TerrainGen::populate_mineshaft_chunk` supplies the 3x3 terrain, registers starts, and runs
   the four passes (carrying `spawnerPlaced`/removal state). Harness `mineshaft_placement_parity`
   (`MineshaftPlacementOracle.java` + `examples/mineshaft_probe.rs`) compares the placed chunk's blocks
   on a curated corpus. **Scope:** vanilla's `Chunk.setBlockState` calls `Block.onBlockAdded`, so it
   reshapes rails (`BlockRailBase.updateState`) and drops unsupported torches (`BlockTorch.checkForDrop`)
   as it places them - block-update semantics, not placement; the corpus excludes fixtures that trigger
   them (see "Open threads"), and multi-start/order-dependent boxes are excluded (HashMap order).
   1d. **structure layouts complete:** ✅ **scattered feature**, ✅ **ocean monument**, ✅ **nether
   fortress**, ✅ **village**, ✅ **stronghold** (`MapGenStronghold`: `findBiomePosition` ring placement +
   the weighted `StructureStrongholdPieces` tree + the portal-room re-roll loop). Shared infra from
   these: `terrain::BiomeLookup` (`getBiomeGenerator` / `areBiomesViable` / `findBiomePosition`),
   `structure::component_box` / `StructureStart::{mark_available_height,set_random_height}`. **Next:**
   decoration (`BiomeDecorator` + feature generators - biggest). Then the full chunk
   (blocks + metadata + biome + heightmap) for the Gate-C seeds/chunks is reachable and
   `worldgen_parity` can take the Gate-C name.
   *Done when:* the full chunk (blocks + metadata + biome + heightmap) matches for the Gate-C seeds
   and chunks.
   1e. **decoration (done):** `mc-worldgen::decorate` - the `DecorateWorld` shim (the
   `Chunk` block read/write incl. the replaced block's `breakBlock`, height map, precipitation,
   sections, `World.getTopSolidOrLiquidBlock`/`getHeight`, and a **column skylight** model matching
   `Chunk.generateSkylightMap`/`relightBlock` with `World.checkLight` off, which the oracle does too),
   `ChunkProviderSettings` defaults, `WorldGenMinable` + `BiomeDecorator.generateOres`, the surface
   `WorldGenSand`/`WorldGenClay`/gravel loops, the vegetation generators (big mushrooms, flowers, tall
   grass, dead bush, waterlily, brown/red mushrooms, reeds, pumpkin, cactus) with the per-biome counts +
   flower/grass hooks, the tree generators (`decorate/trees.rs`: `WorldGenTrees`/`BigTree`/`Forest`/
   `Taiga1`/`Taiga2`/`Savanna`/`Canopy`/`Swamp`/`Shrub`/`MegaJungle`/`MegaPine` + the `genBigTreeChance`
   hooks and the stateful `BigTree.heightLimit`), and the springs (`WorldGenLiquids`: the 3-stone/1-air
   pocket + the one-step `BlockDynamicLiquid.updateTick` - spring stays flowing level 0, its open
   neighbour becomes flowing level 1/2, and the flag-3 neighbour notification converts nearby static
   liquids to flowing). `TerrainGen::decorate_chunk` runs the four covering `populate` passes (the `+8`
   window, same driver shape as mineshaft placement). Harness `decoration_parity` (`DecorationOracle.java`
   + `examples/decoration_probe.rs`) with modes
   `ores | surface | plants | trees | springs | ore_surface | full`; the per-layer oracle is a
   **guarded mirror** of `BiomeDecorator.genDecorations` (the feature groups are entangled by
   unconditional RNG draws, so layers are isolated by mode) that reads the real biome counts/generators
   via reflection (and now the *effective* decorator - a plain `BiomeGenMutated` uses its base's).
   **Note:** the tree port also implements the `BlockLeaves.breakBlock`/`BlockLog.breakBlock`
   leaf-`CHECK_DECAY` flagging, which changes leaf metadata when a log replaces a leaf.
1f. **decoration, per-biome `decorate` overrides (done):** the `full` mode calls the **real**
   `BiomeGenBase.decorate` (no mirror) and our whole `decorate`, gating the overrides end-to-end:
   plains/forest/savanna/taiga `WorldGenDoublePlant`, jungle `WorldGenMelon` (+ the `WorldGenVines`
   no-op at the override's `y=128`), `WorldGenIceSpike`/`WorldGenIcePath`, `WorldGenBlockBlob`,
   `WorldGenDesertWells`, and hills emerald/monster egg. The plain `BiomeGenMutated` biomes
   (`decorate` = `baseBiome.theBiomeDecorator.decorate`) are modelled from their base id; the
   roofed-forest mutation (157) forwards to `baseBiome.decorate` and is treated as biome 29.
   **Two latent bugs the `full` mode caught:** `WorldGenCanopyTree`'s second leaf layer is at
   `top_y + 1` (`var15 + var28`), not `top_y` (Canopy was never exercised - biome 29's tree loop is
   `-999`, so the override grid is its only caller); and `WorldGenIceSpike`'s edge `nextFloat()` draw
   is inside the `&&`, so it must be short-circuited (a Rust `let` would draw eagerly). Corpus fixtures
   (per seed) include a jungle (melon), ice spikes (packed ice), mega taiga (boulders), desert well,
   hills (emerald/monster egg) and plains/forest/savanna (double plants) chunk; a guard requires every
   override signature. **Light scope:** the `full` oracle disables `checkLightFor` (in addition to
   `checkLight`), matching our **column-only** skylight model - horizontal flood-fill is
   `light_parity`'s, not decoration's.
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
  layout corpus always has `hps=0`; **all six structure layouts are ported** (mineshaft, scattered
  feature, ocean monument, nether fortress, village, stronghold); a chunk's `structureMap` is
  `HashMap`-ordered in vanilla, so the dump sorts by chunk coordinate (never rely on map order).
- **Scattered features (ported):** `mc-worldgen::structure::scattered`. `MapGenScatteredFeature` is
  grid-placed by `World.setRandomSeed(gx, gz, 14357617)` (a pure function of the world seed), then the
  biome at `chunk*16+8` (the `Voronoi` biome-index layer, via `WorldChunkManager.getBiomeGenerator`)
  decides desert/jungle/swamp; each piece's constructor draws one `nextInt(4)` for
  `EnumFacing.Plane.HORIZONTAL` (`{NORTH, EAST, SOUTH, WEST}`), and the box's X/Z extents swap for an
  east/west orientation. There is no `buildComponent` recursion. `structures_parity` grew swamp-hut
  (seed 0), desert-pyramid (seed -7) and jungle-pyramid (seed 12345) fixtures (the start is reported
  by a *neighbouring* chunk, since `generateStructure`'s box is `chunk*16 + 8`); block placement is
  still unported.
- **Ocean monuments (ported):** `mc-worldgen::structure::monument` (layout) +
  `structure::monument_placement` (blocks). Grid-placed by
  `World.setRandomSeed(gx, gz, 10387313)` (averaging two `nextInt(27)` offsets), requiring a deep-ocean
  `getBiomeGenerator` and `areBiomesViable(.., 29, {ocean, deepOcean, river, frozenOcean, frozenRiver})`.
  `StartMonument.func_175789_b` re-seeds the `MapGenBase` stream with the world seed and adds a single
  `MonumentBuilding` whose box is the fixed 58x23x58 slab `(chunk*16-21, 39, ..)`. Its room graph is
  rebuilt deterministically at placement time from `(seed, start chunk)` (replaying the start's
  `Random`); the shell helpers, base pillars, water walls and every room body are ported
  (`monument_placement_parity`: 3 seeds x 3 chunks = 771 lines, 7,518 structure blocks). The room
  pieces are offset by `(getXWithOffset(9,22), minY, getZWithOffset(9,22))` (easy to miss), and the
  entry room must be marked `chosen` in addition to `is_end` (the fit loop otherwise re-fits it).
  `terrain::BiomeLookup` provides `getBiomeGenerator` (`index_at`) and `areBiomesViable`.
- **Nether fortresses (ported):** `mc-worldgen::structure::netherbridge`. This runs in the nether
  (`ChunkProviderHell`), not the overworld, so it is a **standalone** generator (not a `TerrainGen`
  field); the `structures_probe` drives it separately and the overworld `chunk.structures` never
  contains fortresses. `canSpawnStructureAtCoords` re-seeds the `MapGenBase` stream with
  `(chunk>>4) ^ ((chunk>>4)<<4) ^ worldSeed` (no biome lookup). The layout is the full
  `StructureNetherBridgePieces` weighted graph (primary/secondary `PieceWeight` lists, 5-attempt
  weighted pick, `End` fallback); vanilla quirk replicated: a child outside the 112-block window builds
  an `End` purely for its RNG draw and discards it. The root's pending list is drained in random order,
  then `StructureStart::set_random_height(48, 70)` drops the structure. All 15 piece kinds are
  exercised; block placement is still unported.
- **Villages (ported):** `mc-worldgen::structure::village`. Grid-placed by
  `World.setRandomSeed(gx, gz, 10387312)` + `areBiomesViable(.., 0, {plains, desert, savanna})`. The
  layout is `StructureVillagePieces`: a `Start` (a `Well`) draws the weighted piece list, four roads
  leave the well, roads spawn buildings along their length (and occasionally branch), and each piece
  is gated by `areBiomesViable` around its centre (a rejected piece still consumes its RNG draws).
  Two lists are drained in random order (roads then buildings); `StructureStart::sizeable` mirrors
  `isSizeableStructure` (a start needs > 2 non-road pieces). 12/13 piece kinds are exercised (only
  `Well`/`ViW` is unreachable - villages only create the `Start`, which extends `Well`); block
  placement is still unported.
- **Strongholds (ported):** `mc-worldgen::structure::stronghold`. Placement is a lazily computed
  3-position ring: `Random(worldSeed)` -> `(1.25*spread + nextDouble()) * 32*spread` at 120° steps, each
  refined by `WorldChunkManager.findBiomePosition(.., 112, {biomes with minHeight > 0}, rand)`; the
  chunk spawns iff it is one of those 3 chunks. The layout is `StructureStrongholdPieces`: a `Stairs2`
  root draws a weighted piece (`func_175955_b`, 5 attempts, `Corridor` fallback) and grows via
  `getNextComponent{Normal,X,Z}`; `getStructureStart` re-rolls the whole start until a `PortalRoom`
  exists; `StructureStart::mark_available_height(10)` drops it under sea level. Every piece serializes
  `EntryDoor` (all four door kinds exercised). Vanilla quirk replicated: the `RightTurn` weight
  dispatches through `LeftTurn.func_175867_a`, which builds a **`LeftTurn`** (so `SHRT` is never
  produced). All reachable piece kinds are exercised; block placement is still unported.
  **Caveat:** the ring uses Java `Math.cos`/`Math.sin` (fdlibm); the port uses Rust's `f64::cos`/`sin`,
  which is exact for the corpus (a 1-ULP difference could only flip a `round` at an exact `.5`
  boundary).
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
- **Loot/enchantment RNG (ported):** `mc-worldgen::loot`. `StructureMineshaftPieces.Corridor` rolls a
  chest twice per section (≈5-10 per mineshaft) and the roll consumes enchantment RNG before anything
  else (`Items.enchanted_book.getRandom` -> `buildEnchantmentList` -> `mapEnchantmentData`). Chest
  *contents* also draw from `WeightedRandomChestContent` (array-ordered, portable) + `nextInt(27)` per
  slot. Parity via `loot_parity`. Notable details: the
  `EnchantmentHelper.buildEnchantmentList` map is filled in **id order** but read in **HashMap order**;
  the enchantment table's `canApplyTogether` is **asymmetric** (loot bonus excludes silk touch, but
  silk touch only excludes fortune); `mapEnchantmentData` keeps the *highest* matching level per id
  (`put` overwrites); a stack larger than the item's max stack size fans out into that many single-item
  writes (shifting the draw count). Item ids/max stack sizes are from `Item.registerItems()`. Only the
  **book** path is ported (all enchantment types applicable); non-book applicability needs the item
  registry.
- **Mineshaft placement scope (1c):** the port writes the same blocks in the same order as vanilla's
  four `populate` passes (`mineshaft_placement_parity`, 14 chunks). Two things are deliberately out of
  scope, both documented:
  - **`onBlockAdded`.** `Chunk.setBlockState` ends by calling `Block.onBlockAdded`, so vanilla reshapes
    each placed rail (`BlockRailBase.updateState`, neighbour-dependent shapes 2..9) and drops an
    unsupported torch (`BlockTorch.checkForDrop` -> `setBlockToAir`). Those are block-update semantics
    (the block-update/tick subsystem, cf. `tick_parity`), not placement; the corpus excludes fixtures
    that trigger them. Porting them (or the general block-update system) is the next step toward
    comparing the final chunk instead of a curated corpus.
  - **Multi-start / order-dependent boxes.** `MapGenStructure.structureMap` is a `HashMap`, so when
    several starts meet one box the placement RNG is consumed in a history-dependent order; and the
    four covering passes carry corridor state (`spawnerPlaced`, piece removal), so vanilla's result
    depends on chunk-load order. The port uses a deterministic pass order and `structureMap` order, and
    the corpus keeps only single-start-per-box, order-independent fixtures (checked by the oracle's
    `MS_REVERSE`).
- **Populate driver (1c):** `TerrainGen::populate_mineshaft_chunk(cx, cz)` generates the surrounding
  3x3 (whose generation also registers every start the passes can meet), then runs the four covering
  passes `MapGenStructure.generateStructure` would (`(cx-1,cz-1)`, `(cx-1,cz)`, `(cx,cz-1)`, `(cx,cz)`).
  The oracle's headless world needs a stub `IChunkProvider` that stores chunks produced by a
  `ChunkProviderGenerate` (only caves/ravines/mineshafts enabled, `mapFeaturesEnabled = true`), because
  `world.setBlockState` -> `getChunkFromBlockCoords` -> `provider.provideChunk`; the 3x3 is
  pre-provided so a write never generates a chunk mid-`generateStructure` (which would mutate
  `structureMap`).
- **Temple placement (1g, swamp hut + desert + jungle pyramids done):** `TerrainGen::populate_scattered_chunk(cx, cz)`
  reuses the same `+8` driver shape, walking the registered scattered starts whose box meets each pass
  box (`ScatteredFeature::for_each_start_meeting_mut`). The swamp hut, desert pyramid and jungle
  pyramid are ported. Key details:
  `ComponentScatteredFeaturePieces.Feature.func_74935_a` averages
  `max(getTopSolidOrLiquidBlock, WorldProvider.getAverageGroundLevel())` over the piece/pass-box
  intersection and offsets the piece (`getAverageGroundLevel` is `seaLevel + 1` = **64**, not 63); the
  jungle/swamp pieces call it on the first covering pass, so state carries across passes (the corpus
  checks forward vs reverse with `TEMPLE_REVERSE`). The desert pyramid has **no** height adjustment; it
  uses `replaceAirAndLiquidDownwards` for its foundation and places four chests (one per horizontal
  facing, in `Plane.HORIZONTAL` order) whose `BlockChest.correctFacing` metadata depends on the solid
  neighbours; its loot list is `loot::chest::TEMPLE_CHEST_CONTENT` (+ a random book) and the inventory
  draws are `TileEntityChest`'s 27 slots. Harness `temple_placement_parity`
  (`TemplePlacementOracle.java` + `examples/temple_probe.rs`): swamp huts at seed 0 (3,47) / seed 12345
  (-26,49), the desert pyramid at seed -7 (4,2) and the jungle pyramid at seed 12345 (72,-155) -
  **1,028 lines / 1,562 structure blocks** match. The jungle pyramid needed the generic
  `getMetadataWithOffset` branches (`tripwire_hook`/`repeater` horizontal-index rotation and
  `piston`/`lever`/`dispenser` full-index rotation) and `fillWithRandomizedBlocks`'s cobble/mossy
  `Stones` selector (one `nextFloat()` per position). **Order caveat:** the jungle's main-chest
  `generateChestContents` runs mid-body, so its book draw shifts the following stone-selector draws;
  the chunk is genuinely order-dependent in vanilla (chunk-load order). The port commits to a fixed pass
  order, so the harness compares it against the oracle run in that same order
  (`CORPUS`'s `order_independent=False` entry; the swamp/desert fixtures still require
  forward==reverse). onBlockAdded stayed a no-op for this fixture's dispensers/tripwire/redstone.
- **Decoration (1e/1f):** the per-layer oracle is a **guarded mirror** of
  `BiomeDecorator.genDecorations` (the feature groups are entangled: `generateOres` always runs,
  `treesPerChunk`/plant counts gate loops but the two mushroom `if`s, the unconditional 10-reeds loop
  and the tree `nextInt(10)` always draw), so layers are isolated by mode
  (`ores | surface | plants | trees | springs | ore_surface`) and the mirror reads the real biome
  counts/generator instances by reflection - using the **effective** decorator, i.e. a plain
  `BiomeGenMutated` runs its *base* biome's counts (the port mirrors that in `decorate_plants`/
  `decorate_trees`). The `full` mode instead calls the **real** `BiomeGenBase.decorate` (no mirror) and
  our whole `decorate`, so the per-biome overrides are compared against unmirrored vanilla. **Skylight:**
  `BlockMushroom.canBlockStay` reads `World.getLight`, so the port models `Chunk`'s column skylight
  (`generateSkylightMap`/`relightBlock`); the oracle disables `World.checkLight` **and**
  `World.checkLightFor` (the latter is the horizontal BFS `Chunk.relightBlock` triggers via
  `updateSkylightNeighborHeight`), so the comparison is column-only - horizontal flood-fill belongs to
  `light_parity`. **`breakBlock`:** `DecorateWorld::set` mirrors `Chunk.setBlockState`'s
  `breakBlock` for leaves/logs (`BlockLeaves`/`BlockLog` flag nearby leaves' `CHECK_DECAY`, which is
  visible in the leaf metadata when a tree log replaces a leaf); other `breakBlock` bodies are no-ops
  for decoration. **Springs:** `WorldGenLiquids` is ported (`decoration_parity` mode `springs`); on a
  plain `World` the immediate `BlockLiquid.updateTick` is a single level-0 step, and the flag-3
  neighbour notification converts nearby static liquids to flowing (ported). **Coverage gap:** a lava
  spring adjacent to water runs `BlockLiquid.checkForMixing` (obsidian/cobblestone) whose flag-3 set
  cascades into neighbouring static lava - a block-update cascade (cf. mineshaft `onBlockAdded`) - so
  both the `springs` and `full` corpora keep that fixture (seed -7 chunk (-48,-48)) out until the
  block-update subsystem (`tick_parity`) exists.

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
- **2026-11-22 - session 11 (handoff + git).** Rewrote the at-a-glance/status sections, refreshed
  `README.md` (it still listed six harnesses) and confirmed the baseline: **10/10** harnesses, 60
  tests, fmt clean. **Next session: `1b` loot + enchantment RNG, then `1c` mineshaft placement +
  the `populate` oracle.** Committed and pushed.
- **2026-11-22 - session 12 (1b: loot + enchantment RNG).** Ported the subsystem the mineshaft blocks
  were blocked on into `crates/mc-worldgen/src/loot.rs`: `WeightedRandom` (`weighted_index`/
  `weighted_index_from`), `WeightedRandomChestContent.generateChestContents` + `func_177629_a`
  (`with_extra`) + the 13-entry `MINESHAFT_CHEST_CONTENT` list, the `Enchantment` registry table
  (`ENCHANTMENTS`, 25 entries: id/weight/max level/type/subclass) with the subclasses'
  `min`/`max_enchantability` and `can_apply_together`, and the book path of
  `EnchantmentHelper.mapEnchantmentData`/`buildEnchantmentList`/`addRandomEnchantment` (using
  `mc_core::jhash::JavaHashMap`). Added order-preserving `JavaHashMap::retain` (+ a `jhash` unit test)
  for `buildEnchantmentList`'s iterator `remove()`. New harness `loot_parity`
  (`oracle/java/src/LootOracle.java` + `crates/mc-worldgen/examples/loot_probe.rs` +
  `tools/test_loot_parity.py`): 3 modes (`table` = registry + per-level enchantability + the ordered
  `canApplyTogether` matrix; `enchant` = random book rolls; `chest` = the real `generateChestContents`),
  each printing the next 32 draws, **695 lines match** with fixture guards (25 enchantments / 300
  exclusivity pairs; at least one non-empty enchant list and one chest write). 12 new unit tests + 1 in
  `mc-core` (**73 total**). A negative self-test (weight 10 -> 9) failed as expected. The harness caught
  a wrong unit-test assertion (silk touch excludes fortune *both* ways; only silk<->looting is
  asymmetric) - the port itself was already right. Whole-suite verify: **11/11** harnesses. Next
  session: `1c` mineshaft placement + the `populate` oracle, then `1d` the other structures.
- **2026-11-22 - session 13 (1c: mineshaft block placement).** Added
  `crates/mc-worldgen/src/structure/placement.rs`: the `StructureComponent` helpers
  (`isLiquidInStructureBoundingBox`, the offset accessors, `fillWithBlocks`, `func_175805_a`,
  `randomlyPlaceBlock`, `randomlyRareFillWithBlocks`), the four `StructureMineshaftPieces.addComponentParts`
  bodies, and the corridor's `generateChestContents` override (rails + the `loot.rs` chest draws);
  plus `MapGenStructure.generateStructure`/`StructureStart.generateStructure` (with vanilla's
  piece-removal and carried `spawnerPlaced`). Key finding (bytecode-verified): the `generateStructure`
  box is `(chunk << 4) + 8` (a `+8` offset spanning two chunks per axis), so a chunk is the union of the
  four covering populate passes; `TerrainGen::populate_mineshaft_chunk` generates the 3x3 (registering
  starts) and runs them in a fixed order. Added `chunk::{material_is_liquid, is_full_block}` (dumped
  from `BlockTableOracle`), `Mineshaft::{starts_meeting, for_each_start_meeting_mut}`, and the oracle
  `MineshaftPlacementOracle.java` + `examples/mineshaft_probe.rs` + `tools/test_mineshaft_placement_parity.py`
  (**3,598 lines match**, 3 seeds x 14 chunks, 426 structure blocks, with order-independence and
  single-start guards). 7 new unit tests (**80 total**). Diagnosis worth recording: the residual
  differences on some fixtures are **not** placement bugs - `Chunk.setBlockState` calls
  `Block.onBlockAdded`, so vanilla reshapes rails (`BlockRailBase.updateState`) and drops unsupported
  torches (`BlockTorch.checkForDrop`) while placing; those are block-update semantics, so the corpus
  excludes fixtures that trigger them (see "Open threads"). `verify.ps1`: **12/12** harnesses. Next
  session: `1d` the other structures' layouts, then decoration.
- **2026-11-22 - session 14 (1d: scattered feature).** Ported `MapGenScatteredFeature` (desert/jungle
  temple + swamp hut) as the first of `1d`'s "other structures". New
  `crates/mc-worldgen/src/structure/scattered.rs`: the grid placement
  (`World.setRandomSeed(gx, gz, 14357617)` + two `nextInt(24)` offsets), the biome test
  (`WorldChunkManager.getBiomeGenerator` at `chunk*16 + 8`, i.e. the `Voronoi` biome-index layer -
  added `terrain::biome_index_at`), and the single-piece `Start` (`ComponentScatteredFeaturePieces.
  {DesertPyramid,JunglePyramid,SwampHut}`). `structure/mod.rs` gained the three `Temple*` `PieceKind`s
  (size + `HPos` + the subclass booleans); `placement.rs`/`mineshaft.rs` got no-op arms (temple block
  placement is still unported). Wired through `GenOptions.scattered_features` (on in `vanilla()`),
  `TerrainGen::{generate_chunk}` and `GeneratedChunk.structures` (now `[mineshafts..., temples...]`,
  each group sorted); `structures_probe` prints the temple NBT (`Width`/`Height`/`Depth`/`HPos` +
  booleans). Generalised `StructuresOracle` + `structures_probe` to dump mineshafts *and* temples per
  chunk, and grew `tools/test_structures_parity.py` with temple fixtures (swamp hut: seeds 0/12345;
  desert pyramid: seed -7; jungle pyramid: seed 12345) plus a per-seed "at least one start of each
  kind" guard: **3,360 lines match** (3 seeds x 22 chunks, 33 starts; all three piece kinds). 5 new
  golden unit tests (**85 total**). A throwaway `TempleScan.java` (in gitignored `.local/`) found the
  fixtures; vanilla's `Vec`/box layout is unchanged elsewhere (registration writes no blocks), so every
  other harness is untouched. `verify.ps1`: **12/12**. Next session: `1d` continues (village /
  stronghold / ocean monument / nether bridge), then decoration.
- **2026-11-22 - session 15 (1d: ocean monument).** Second of `1d`'s "other structures":
  `StructureOceanMonument`. New `crates/mc-worldgen/src/structure/monument.rs` - the grid placement
  (`World.setRandomSeed(gx, gz, 10387313)` + averaged `nextInt(27)` offsets), the biome gate
  (deep ocean via `getBiomeGenerator`, plus `areBiomesViable(.., 29, ...)`), and `StartMonument`
  (re-seed with the world seed, `EnumFacing.Plane.HORIZONTAL` orientation, one `MonumentBuilding`).
  Key scope finding: `MonumentBuilding` keeps its room graph in a **private** list walked by
  `addComponentParts`, so the *serialized* layout is a single fixed 58x23x58 box - the room graph is
  placement-only and deferred. Added `terrain::BiomeLookup` (`index_at` = `getBiomeGenerator`,
  `are_viable` = `areBiomesViable`) shared by the remaining structures; `structure/mod.rs` gained the
  `MonumentBuilding` piece kind; `GenOptions.monuments` + `worldgen.monuments` span + `chunk.structures`
  union. Generalised `StructuresOracle` + probe to a third generator and grew `structures_parity` with
  monument fixtures (seed 0: (77,-50),(86,-12); seed 12345: (-695,458); seed -7: (-40,-23),(-28,21))
  and a per-seed "each of the three kinds" guard: **3,815 lines match** (3 seeds x 27 chunks, 42
  starts). 3 new golden unit tests (**88 total**). A throwaway `MonumentScan.java` found the fixtures.
  `verify.ps1`: **12/12**. Next session: `1d` continues (village / stronghold / nether bridge), then
  decoration.
- **2026-11-22 - session 16 (1d: nether fortress).** Third of `1d`'s "other structures":
  `MapGenNetherBridge`. New `crates/mc-worldgen/src/structure/netherbridge.rs` - a **standalone**
  generator (the nether runs it via `ChunkProviderHell`, not the overworld `TerrainGen`; the probe
  drives it separately). `canSpawnStructureAtCoords` re-seeds the `MapGenBase` stream with
  `(chunk>>4) ^ ((chunk>>4)<<4) ^ worldSeed` and rolls `nextInt()` / `nextInt(3)` / two conditional
  `nextInt(8)`. The layout is the full `StructureNetherBridgePieces` weighted graph: primary/secondary
  `PieceWeight` lists (`structure/mod.rs` gained `NetherKind` + a `PieceKind::Nether` carrying the
  serialized `Chest`/`Seed`/`Mob`), a 5-attempt weighted pick (`func_175871_a`) falling back to `End`,
  `getNextComponent{Normal,X,Z}` offsets, and the root's pending list drained in random order. Shared
  helpers added: `structure::component_box` (`getComponentToAddBoundingBox`) and
  `StructureStart::set_random_height(48,70)`. Vanilla quirk replicated: a child outside the 112-block
  window builds an `End` purely for its RNG draw and **discards** it. Generalised `StructuresOracle` +
  probe to a fourth generator and grew `structures_parity` with fortress fixtures + a four-kind
  per-seed guard: **6,589 lines match** (3 seeds x 33 chunks, 64 starts; all 15 nether piece kinds
  exercised). 4 new unit tests (**92 total**). A throwaway `NetherScan.java` found the origins.
  `verify.ps1`: **12/12**. Next session: `1d` continues (village / stronghold), then decoration.
- **2026-11-22 - session 17 (1d: village).** Fourth of `1d`'s "other structures": `MapGenVillage`.
  New `crates/mc-worldgen/src/structure/village.rs` - grid placement
  (`World.setRandomSeed(gx, gz, 10387312)` + two `nextInt(24)` offsets) + `areBiomesViable(.., 0,
  {plains, desert, savanna})`; the weighted `StructureVillagePieces` layout (`getStructureVillageWeightedPieceList`,
  `func_176067_c` weighted selection, `Well`/`Path` `buildComponent`, `func_176066_d`/`func_176069_e`
  with the per-piece 112-block window + `areBiomesViable` gate, roads-then-buildings drain loop).
  `structure/mod.rs` gained `VillageKind` + a `PieceKind::Village(VillagePiece)` (the `Village` base's
  `HPos`/`VCount`/`Desert` + `CA..CD` crops / `Chest` / `Terrace` / `T`+`C` / `Length`) and
  `StructureStart::sizeable` (villages need > 2 non-road pieces). Wired through `GenOptions.villages` +
  `worldgen.villages` span + the `chunk.structures` union (mineshaft, scattered, monument, village).
  Generalised `StructuresOracle` + probe to a fifth generator and grew `structures_parity` with village
  fixtures + a five-kind per-seed guard: **7,543 lines match** (3 seeds x 39 chunks, 76 starts; 12/13
  village piece kinds - only `ViW` is unreachable - and a desert village exercises `Desert=1`). 4 new
  unit tests (**96 total**). A throwaway
  `VillageScan.java` found the origins; one fixture needed a neighbour chunk (the well's own chunk can
  miss the box). `verify.ps1`: **12/12**. Next session: `1d` finishes with the stronghold, then
  decoration.
- **2026-11-22 - session 18 (1d: stronghold - structure layouts complete).** The last of `1d`'s
  structures: `MapGenStronghold`. New `crates/mc-worldgen/src/structure/stronghold.rs` - the lazily
  computed 3-position ring (`Random(worldSeed)` -> `(1.25*spread + nextDouble())*32*spread` at 120°,
  refined by `findBiomePosition(.., 112, strongholdBiomes, rand)`); added
  `BiomeLookup::find_biome_position` + `terrain::STRONGHOLD_BIOMES` (registered biomes with
  `minHeight > 0`). The layout is the weighted `StructureStrongholdPieces` (`PIECE_WEIGHTS` with the
  Library/PortalRoom depth gates, `func_175955_b` 5-attempt selection with the one-shot
  `strongComponentType` and a `Corridor` fallback), plus `getStructureStart`'s re-roll-until-PortalRoom
  loop and `mark_available_height(10)`. `structure/mod.rs` gained `StrongholdKind`, `Door` (the
  `EntryDoor` NBT string) and a `PieceKind::Stronghold(StrongholdPiece)` carrying `Steps`/`Chest`/
  `leftLow..`/`Tall`/`Mob`/`Type`/`Source`/`Left`+`Right`. **Found a vanilla quirk:** the `RightTurn`
  weight dispatches through `LeftTurn.func_175867_a`, which builds a **`LeftTurn`** - so `SHRT` is
  never produced (replicated). Generalised `StructuresOracle` + probe to a sixth generator and grew
  `structures_parity` with the 9 ring chunks + a six-kind per-seed guard: **10,533 lines match**
  (3 seeds x 48 chunks, 96 starts; all 4 door kinds, 11 stronghold starts, all reachable piece kinds -
  `SHRT` is the only exception). 4 new unit tests (**100 total**). A throwaway `StrongholdScan.java`
  found the ring coords. `verify.ps1`: **12/12**. **`1d` structure layouts are complete**; next
  session: decoration (`BiomeDecorator` + feature generators).
- **2026-11-22 - session 18 (handoff + git).** Rewrote the at-a-glance/status for the completed `1d`
  structure layouts, refreshed `README.md`/`plan.md`/`docs/performance-baselines.md`, and confirmed the
  baseline: **12/12** harnesses, **100** tests, fmt clean, `structures_parity` **10,533 lines / 96
  starts**. Committed and pushed (`8b2d704`). **Next session: decoration (`BiomeDecorator` + feature
  generators), then structure block placement for the non-mineshaft structures** (both are needed
  before `worldgen_parity` can take the Gate-C name).
- **2026-11-22 - session 19 (decoration, layer 1).** Started M1's decoration increment. New
  `crates/mc-worldgen/src/decorate.rs` + `decorate/blocks.rs`: the `DecorateWorld` shim (per-chunk
  blocks + `Chunk`'s height map/precipitation/section/`getTopFilledSegment` state, maintained exactly
  as `Chunk.setBlockState` does - `generateSkylightMap` on a new section above the surface, else
  `relightBlock`; `World.getTopSolidOrLiquidBlock`/`getHeight`), the `ChunkProviderSettings` ore
  defaults, `WorldGenMinable` + `BiomeDecorator.generateOres` (`genStandardOre1/2`, all 11 ore kinds),
  and the surface `WorldGenSand`/`WorldGenClay`/gravel-as-sand loops. Wired via
  `TerrainGen::decorate_chunk` (the four covering `populate` passes, `+8` window). Material flags came
  from a new `MaterialTableOracle.java` (dumped, not guessed). New harness `decoration_parity`
  (`DecorationOracle.java` + `examples/decoration_probe.rs` + `tools/test_decoration_parity.py`),
  modes `ores | surface | ore_surface`: **13,878 lines match** (3 modes x 3 seeds x 6 chunks; an
  ore-presence guard). 3 new unit tests (**103 total**). **Design note:** decoration's feature groups
  are entangled by unconditional RNG draws (the two mushroom `if`s, the 10-reeds loop, the tree
  `nextInt(10)`), and the per-biome `decorate` overrides add to `genDecorations`, so the oracle is a
  *guarded mirror* of `BiomeDecorator.genDecorations` and layers are validated by mode. `verify.ps1`:
  **13/13**. **Next session: decoration layers 2+ (trees, vegetation, springs, biome overrides).**
- **2026-11-22 - session 20 (decoration, layer 2: vegetation).** Ported the `BiomeDecorator`
  vegetation section into `decorate.rs`: big mushrooms (`WorldGenBigMushroom`, incl. the shared
  once-per-biome `mushroomType` colour state), flowers (`WorldGenFlowers` + the biome
  `pickRandomFlower` overrides, incl. the plains/forest `GRASS_COLOR_NOISE` hooks), tall grass
  (`WorldGenTallGrass` + the taiga/jungle `getRandomWorldGenForGrass` hooks), dead bush, waterlily,
  brown/red mushrooms (`GeneratorBushFeature`), reeds, pumpkin (with the `Plane.HORIZONTAL` facing
  draw) and cactus. Added a **column skylight** model to `DecorateWorld` because
  `BlockMushroom.canBlockStay` reads `World.getLight` (`Chunk.generateSkylightMap`/`relightBlock`,
  matching the oracle, which disables `World.checkLight`). The per-biome counts were **dumped** from
  the game (`BiomeDecorTableOracle.java`) rather than transcribed; the oracle's plants mirror reads
  the real biome counts and generator instances by reflection, and now treats the pre-provided chunks
  as loaded so `World.getHeight` works. `decoration_parity` gained a `plants` mode plus a
  signature-coverage guard (tall grass / flower / small mushroom / cactus / dead bush): **18,504 lines
  match** (4 modes x 3 seeds x 6 chunks). 4 new unit tests (**106 total**). `verify.ps1`: **13/13**.
  Remaining decoration: trees, springs (fluid `updateTick`), per-biome `decorate` overrides.- **2026-11-22 - session 21 (decoration, layer 3: trees).** Ported the tree loop into
  `crates/mc-worldgen/src/decorate/trees.rs`: `WorldGenTrees` (incl. jungle vines + cocoa),
  `WorldGenForest` (birch), `WorldGenBigTree` (incl. its stateful shared `heightLimit` and its internal
  `Random(nextLong())`), `WorldGenTaiga1`/`Taiga2`, `WorldGenSavannaTree`, `WorldGenCanopyTree`,
  `WorldGenSwamp`, `WorldGenShrub`, and `WorldGenHugeTrees` (+ `WorldGenMegaJungle`/`WorldGenMegaPineTree`
  incl. the podzol post). Added the per-biome `genBigTreeChance` hooks (forest type / taiga type /
  jungle edge / hills / savanna / mesa / snow) and the `treesPerChunk` table (dumped from the game).
  Two vanilla side effects the harness forced: (1) **`breakBlock`** - when a tree log replaces a leaf,
  `Chunk.setBlockState` calls `BlockLeaves.breakBlock`, which flags nearby leaves `CHECK_DECAY`
  (metadata bit 8); `DecorateWorld::set` now mirrors that for leaves/logs. (2) the mega-pine podzol
  **5x5 blob** (`func_175933_b`), not just the single-column scan. `decoration_parity` gained a `trees`
  mode + tree-presence guard: **23,130 lines match** (5 modes x 3 seeds x 6 chunks). 3 new unit tests
  (**109 total**). `verify.ps1`: **13/13**. Remaining decoration: springs (fluid `updateTick`) and the
  per-biome `decorate` overrides (double plants, melons, vines, ice, boulders, emerald/monster-egg,
  desert wells).
- **2026-11-22 - session 22 (decoration, layer 4: springs).** Ported `WorldGenLiquids` into
  `decorate.rs` (`springs` in `BiomeDecorator.genDecorations`: 50 water + 20 lava attempts with the
  exact draw order). The oracle runs a plain `World` (not `WorldServer`), so `scheduleUpdate` is a
  no-op and `BlockDynamicLiquid.updateTick` on the placed level-0 liquid is a **single step**: the
  static-liquid write is immediately reverted to flowing level 0 by the flag-3 neighbour
  notification (`BlockStaticLiquid.updateLiquid`), the one open neighbour becomes flowing at level 1
  (water) / 2 (overworld lava), and nearby static water/lava become flowing. Implemented all three.
  `decoration_parity` gained a `springs` mode (water + lava flow-block guard): **26,985 lines match**
  (6 modes x 3 seeds; springs uses 5 chunks). 1 new unit test (**110 total**). **Coverage gap:** a
  lava spring next to water triggers `checkForMixing` (obsidian/cobblestone) whose flag-3 set cascades
  into neighbouring static lava - a block-update cascade - so that fixture (seed -7 chunk (-48,-48))
  is excluded from the springs corpus until `tick_parity`. `verify.ps1`: **13/13**. Remaining
  decoration: the per-biome `decorate` overrides.
- **2026-11-23 - session 23 (decoration, per-biome `decorate` overrides - decoration complete).**
  Ported the overrides into `decorate.rs` (`biome_override_pre`/`biome_override_post` + the generators):
  plains/forest/savanna/taiga `WorldGenDoublePlant`, jungle `WorldGenMelon` + the `WorldGenVines` no-op
  (the override starts it at `y=128`), `WorldGenIceSpike`/`WorldGenIcePath`, `WorldGenBlockBlob`,
  `WorldGenDesertWells`, hills emerald/monster egg. Added `DecorateOptions.overrides`; `decorate()`
  now mirrors the `BiomeGenBase.decorate` dispatch (pre-extras -> genDecorations -> post-extras) and
  models the plain `BiomeGenMutated` biomes (base decorator counts + `BiomeGenBase` default hooks;
  the roofed-forest mutation 157 forwards to `baseBiome.decorate` and is treated as biome 29).
  Extended `DecorationOracle` with a **`full`** mode that calls the **real** `BiomeGenBase.decorate`
  (no mirror) - the strongest oracle - and the `trees`/`plants` mirrors now read the *effective*
  decorator (base for mutated). `decoration_parity` `full` corpus is per-seed (jungle -> melon,
  ice spikes -> packed ice, mega taiga -> boulders, a desert well at seed 0 (-207,-109), hills ->
  emerald/monster egg, plains/forest/savanna -> double plants) with an every-override-signature guard:
  **32,896 lines match** (7 modes x 3 seeds). **Two latent bugs the `full` mode caught:** (1)
  `WorldGenCanopyTree`'s second leaf layer is at `top_y + 1` (`var15 + var28`), not `top_y` - Canopy
  was never exercised (biome 29's tree loop is `-999`, so the roofed-forest override grid is its only
  caller); (2) `WorldGenIceSpike`'s edge `nextFloat()` draw must short-circuit inside the `&&`
  (a Rust `let` drew eagerly, shifting the whole chunk). The `full` oracle also disables
  `World.checkLightFor` (the horizontal BFS `relightBlock` triggers) to match the port's column-only
  skylight model - `light_parity` owns flood-fill. 5 new unit tests (**92 `mc-worldgen`**, 115 total).
  Fixture-hunting throwaways (`BiomeScan`, desert-well scan) live in gitignored `.local/`.
  `verify.ps1`: **13/13**. **Decoration is complete; next: structure block placement for the five
  non-mineshaft structures, then the Gate-C chunk.**
- **2026-11-23 - session 24 (structure block placement: swamp hut).** Started the non-mineshaft
  placement with the smallest scattered feature. Added the shared helpers to
  `structure/placement.rs`: `ChunkWorld::get_top_solid_or_liquid` (`World.getTopSolidOrLiquidBlock`,
  liquids do not block movement), `replaceAirAndLiquidDownwards`, the stair
  `getMetadataWithOffset`, and `temple_adjust_height`
  (`ComponentScatteredFeaturePieces.Feature.func_74935_a`). Key vanilla detail: the average uses
  `WorldProvider.getAverageGroundLevel()` = `seaLevel + 1` = **64**, not 63 - the first harness run
  caught the off-by-one (the hut sat one block low). Ported
  `ComponentScatteredFeaturePieces.SwampHut.addComponentParts` (planks/log/fence/flower pot/crafting
  table/cauldron + spruce-stair roof + downward support logs; the witch entity is flag-only).
  Added `ScatteredFeature::for_each_start_meeting_mut` and `TerrainGen::populate_scattered_chunk`
  (the same `+8` four-pass driver as mineshafts; `func_74935_a` state carries across passes). New
  harness `temple_placement_parity` (`TemplePlacementOracle.java` + `examples/temple_probe.rs` +
  `tools/test_temple_placement_parity.py`): swamp huts at seed 0 (3,47) and seed 12345 (-26,49),
  **514 lines / 282 structure blocks** match, with an order-independence guard (`TEMPLE_REVERSE`).
  4 new unit tests (**96 `mc-worldgen`**, 119 total). `verify.ps1`: **14/14**. **Remaining structure
  block placement:** desert/jungle temple bodies (chest lists + `BlockChest.correctFacing`, jungle
  dispensers/tripwires), then ocean monument, nether fortress, village, stronghold.
- **2026-11-23 - session 25 (structure block placement: desert pyramid).** Ported
  `ComponentScatteredFeaturePieces.DesertPyramid.addComponentParts` into `structure/placement.rs`:
  the sandstone stepped shell + foundation (`replaceAirAndLiquidDownwards`), the three entrances and
  their `getMetadataWithOffset` stairs, the facade strips, the treasure chamber (TNT, pressure plate,
  chiseled/smooth sandstone) and the four chests. Added `loot::chest::TEMPLE_CHEST_CONTENT` (the
  10-entry temple list, total weight 71) and the chest helpers: `chest_facing_meta`
  (`BlockChest.correctFacing`, the facing-index meta) and `temple_chest`
  (`generateChestContents` -> 27 slots; the loot draws are consumed only when the chest is placed,
  while the book + `2 + nextInt(5)` draws happen unconditionally). Added `Facing::{index, opposite,
  rotate_y, offset}` to the structure model. Key spec details: `BlockSandStone.EnumType`
  DEFAULT/CHISELED/SMOOTH = 0/1/2; the stair default state is meta **3** (`5 - NORTH.index()`);
  stained clay `~ORANGE.dyeDamage & 15` = orange meta 1 and `~BLUE.dyeDamage & 15` = blue meta 11.
  `temple_placement_parity` grew the desert-pyramid fixture (seed -7 (4,2)): **771 lines / 340
  structure blocks** across 3 seeds, order-independent. 2 new unit tests (**98 `mc-worldgen`**, 121
  total). `verify.ps1`: **14/14**. **Next:** the jungle pyramid (randomized cobble/mossy blocks,
  dispensers, tripwire hooks/levers/pistons/repeaters -> the generic `getMetadataWithOffset` branches),
  then ocean monument, nether fortress, village, stronghold.
- **2026-11-27 - session 26 (structure block placement: jungle pyramid).** Ported
  `ComponentScatteredFeaturePieces.JunglePyramid.addComponentParts` into `structure/placement.rs`:
  the base slab/stepped shell, the room core and underground stairwell, both tripwire traps (hooks +
  tripwire + redstone wire + mossy cobble), the treasure chamber (chiseled stonebrick, levers, sticky
  pistons, repeater, vines), the main/hidden chests and the two dispensers. Added
  `fill_with_randomized_blocks` (the `Stones` selector: one `nextFloat() < 0.4` per position ->
  cobblestone/mossy) and the generic `getMetadataWithOffset` branches:
  `directional_meta_with_offset` (tripwire hook / `BlockDirectional` repeater, horizontal indices),
  `directional_index_with_offset` (piston/lever/dispenser, full facing indices) and
  `lever_meta_for_index`; plus `temple_dispenser` (9-slot `TileEntityDispenser`, arrows) and
  `loot::chest::JUNGLE_DISPENSER_CONTENT`. **Bug caught by the harness:** `Blocks.stone_stairs` is
  id **67**, not 109 (which is stone_brick_stairs) - first run showed 20 mismatched stair blocks.
  The jungle list is byte-identical to `TEMPLE_CHEST_CONTENT`, so it is reused. **Order caveat:** the
  mid-body main-chest book draw shifts the following stone-selector draws, so the chunk is genuinely
  order-dependent in vanilla; `temple_placement_parity`'s `CORPUS` gained an ordered
  `(seed, chunks, order_independent)` shape and compares the jungle against the oracle run in the
  port's committed fixed order (the swamp/desert fixtures still require forward==reverse).
  **Harness:** `temple_placement_parity` now **1,028 lines / 1,562 structure blocks** (4 fixtures x 4
  chunks). 5 new unit tests (**103 `mc-worldgen`**, 126 total). `verify.ps1`: **14/14**.
  **Next:** the ocean monument, nether fortress, village, stronghold, then the Gate-C chunk.
- **2026-11-27 - session 27 (structure block placement: ocean monument).** Added
  `structure/monument_placement.rs`: `MonumentBuilding`'s shell helpers (`func_175840_a`/`_b`/`_c`/`_d`/
  `_e`/`_f`/`_g`), the 7x7 base pillars, the five water-wall rings, and the `Piece` helpers
  (`func_181655_a` air/water fill, `func_175821_a` room floor, `func_175819_a` water replace,
  `func_175818_a` intersection). Ported the room graph (`func_175836_a` grid + 6-way connections, the
  three end rooms, the core selection, `Collections.shuffle`, the edge-removal connectivity loop) and
  the seven `MonumentRoomFitHelper`s + all room bodies (`EntryRoom`, `MonumentCoreRoom`, `SimpleRoom`
  x3 variants, `SimpleTopRoom`, `DoubleX/XY/Y/YZ/ZRoom`, `WingRoom`, `Penthouse`). Added
  `TerrainGen::populate_monument_chunk` and `MonumentPlacementOracle.java` + `monument_probe.rs` +
  `tools/test_monument_placement_parity.py`. **Two bugs the harness/room dump caught:** (1) vanilla
  offsets every fitted room piece by `(getXWithOffset(9,22), minY, getZWithOffset(9,22))` before adding
  the wings - without it the rooms sat one chunk off (240/257 columns differed); (2) the entry room
  must be marked `chosen` as well as `is_end` (`field_175845_o.field_175963_d = true`), otherwise the
  fit loop re-fits it as an extra `DoubleZ`. Final: **771 lines / 7,518 structure blocks** (3 seeds x 3
  chunks, order-independent). 4 new unit tests (**107 `mc-worldgen`**, 130 total). `verify.ps1`:
  **15/15**. **Next:** the nether fortress, village, stronghold, then the Gate-C chunk.
