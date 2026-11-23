# Plan — Minecraft 1.8.9, Rewritten in Rust (1:1 Parity + Horizon-grade Performance)

## 1. Goal

Rewrite **Minecraft: Java Edition 1.8.9** from scratch in Rust — a **superior 1.8.9**: clean, maintainable, stable, fast internals, and **without the implementation defects** — while staying **vanilla-faithful and interoperable by default**.

Scope: full **client and server**, vanilla **protocol 47** and **Anvil world** compatibility, all features (singleplayer, multiplayer, worldgen, redstone, AI, rendering).

**Stance:** not a strict bug-for-bug clone and not a different game. Vanilla is the default everywhere; a **curated, documented, tested set of fixes** applies where our engine is authoritative. See **§16 Parity Policy & Deviation Registry**.

## 2. Priorities (in order)

1. **Fidelity** — vanilla behavior and "same art, no glitches" visuals by default: same worldgen for a seed, same redstone/physics quirks, same protocol.
2. **Performance** — beat Java 1.8.9; architecture in the class of Horizon (see §9).
3. **Clean, stable internals** — no "Java in Rust"; fix implementation defects.

> Every decision test: *"Does this keep vanilla behavior while making it cleaner/faster?"* Observable changes are **only** allowed through the **Deviation Registry** (§16) — documented, scoped, and tested. Unregistered differences are failures.

## 3. Non-goals

- **Not** a Horizon fork (Horizon targets 26.3, not 1.8.9).
- **No** Mojang code or assets shipped.
- **No** modern features in the default path (HDR, LOD, shaders, dynamic lights are opt-in only).

## 4. Strategic decisions

- **Do not fork Horizon.** Its *code* encodes 26.3 semantics; its *methods and infrastructure* are what we want.
- **Borrow Horizon's production line** (differential harnesses, port ledger, agent loop) and its **version-agnostic optimized modules** under LGPL-3.0.
- **Behavior comes from the decompiled 1.8.9 source.** Architecture comes from a Horizon-derived design we specify. The AI fills the intersection.
- **Two gates for every module:** correctness (parity) **and** performance (budgets). Parity alone cannot tell "ported" from "transliterated."

## 5. Legal & assets

- Use **your own copy** of the 1.8.9 **client + server jars** (and assets). Jars: official version manifest (`launchermeta.mojang.com/.../version_manifest.json` → `1.8.9`) or your own launcher install.
- **Keep decompiled source outside the Rust repo.** Pin it by hash in `research-lock.json`. Never commit Mojang code or assets.
- **Licensing:** reusing Horizon code (LGPL-3.0) makes the derived parts LGPL-3.0; publish modifications to those parts and keep them isolated (`vendor/`). Confirm license status before relying on it (their docs are inconsistent: MIT vs LGPL-3.0).
- The **oracle** always runs the real jars, never the decompiled source.

## 6. The Production Line (how work actually gets done)

```
decompiled 1.8.9  →  AI agents port  →  differential harness vs real game  →  repeat
                                   ↘  port ledger (coverage)  ↘  perf harness (architecture)
```

### 6.1 Spec source

- **Primary:** clean vanilla decompile. Decompile your own jar with **Vineflower** + **MCP 1.8.9 `stable_22`** (MCP-only, no Forge) → `src/minecraft/`. `stable_22` is what most 1.8.9 mods use, so names line up with community references.
- **Secondary:** the existing tree at
  `C:\Users\wassi\Desktop\Minecraft Rust Edition\minecraft java edition 1.8.9 source code`
  — usable for reading, but it is **Forge-patched** (978 / 1,641 files reference `net.minecraftforge`), **partially mapped** (376 files use `func_`, 292 use `field_`), and **not buildable** (no Forge package, no build files, no assets).
- Rule: **port the vanilla branch**; ignore `net.minecraftforge.*` hooks. Bytecode (`javap`) is the truth when a name is ambiguous.
- Which jar for which oracle: **server jar** → worldgen, ticks, redstone, physics, AI, protocol; **client jar** → meshing, models, lighting, rendering.

### 6.2 Oracle

A **dev-only driver mod** (Forge 1.8.9) that *only calls* the real game code — never patches it — to generate chunks / simulate ticks / capture packets and dump canonical results. This is Horizon's "dev-only driver mod" pattern. **Chosen route: driver mod** (one setup, reused for every harness).

### 6.3 Harness families

| Harness | Compares | Oracle |
|---|---|---|
| `random_parity` | `java.util.Random` sequences | server jar |
| `noise_parity` | Perlin/Simplex/Octaves samples | server jar |
| `worldgen_seed` | chunk blocks + metadata + biome + heightmap | server jar |
| `light_parity` | sky/block light arrays after edits | server jar |
| `physics_parity` | entity pos/velocity per tick | server jar |
| `tick_parity` | block-update trace per tick | server jar |
| `redstone_parity` | circuit state per tick | server jar |
| `meshing_parity` | per-vertex quads for fixed scenes | client jar |
| `protocol_parity` | packet bytes (capture + replay) | real session |
| `ai_parity` | mob decision/trajectory | server jar |
| `render_parity` | fixed-scene frames (tolerant pixel diff) | client jar |

Determinism: fixed seeds, fixed time/weather, fixed camera, fixed input; every result records jar hash + fixture hash.

### 6.4 Port ledger

Machine-readable, both directions:

```
vanilla method  ↔  Rust fn  ↔  harness
```

Every behavior-touching Rust function points to a ledger entry; every unported vanilla method shows as a gap. Query: `port where ChunkProviderGenerate.func_xxx`.

### 6.5 Agent loop

- Issue = `"port Class.method to crates/...; must pass harness Y and perf budget Z"` — small, verifiable.
- Agent writes code → runs harness locally → iterates until green → updates ledger.
- **PR gates:** `verify` (fmt, `clippy -D warnings`, tests, no-Mojang-files) + `parity` (affected harnesses) + **`perf` (budgets)**.
- Humans/architect review hot paths and anything a test can't judge.
- Mark agent-authored work (Horizon convention), and rewrite weak parts.

### 6.6 Performance harness

- Benchmark vs real 1.8.9 (vanilla, and vanilla + OptiFine for a fair ceiling), same machine/world/path/settings; repeats + confidence intervals.
- Frame-time distribution, **1% / 0.1% lows**, memory, startup.
- **Instruction + allocation counts** (Callgrind) on tick / meshing / protocol.
- AB-test individual renderer switches to separate real gains from noise.
- Default build = parity; speed features that change pixels are opt-in.

## 7. Architecture Contract (prevents "Java in Rust")

Two specs per module:

| Spec | Source | AI may not invent |
|---|---|---|
| **Behavior** — what it does | decompiled 1.8.9 | must match (harness) |
| **Architecture** — how it is built | Horizon-derived design | data layout, threading, allocation policy |

Rules every ported module must satisfy:

- **Port semantics, not structure.** Don't port `ChunkProviderGenerate`'s class shape or `ArrayList`s — port its exact draw/block-write sequence into a function over an SoA chunk buffer.
- **No inheritance-tree ports.** `Block`/`Item`/`Entity` hierarchies become **data tables + behavior modules** (cf. Horizon `BlockFacts`), behavior preserved.
- **Data-oriented:** SoA for hot arrays, arenas/pools, indices instead of pointers/`Rc<RefCell<>>`.
- **Allocation policy:** **zero allocations in hot paths per tick** (tick, meshing inner loop, packet codec inner loop).
- **Integer parity:** `i32` matches Java `int`; use **wrapping** arithmetic where Java overflows (Rust debug panics where Java wraps).
- **Exact Java semantics where observable:** `java.util.Random` LCG + rejection sampling, `MathHelper` sin/cos table, `Math.atan2`/`sqrt`, `ChunkPos` hashing, and Java `HashMap` iteration order (Horizon had to reimplement it — expect the same).
- **Lighting engine (clean rewrite; values parity-gated):** 1.8.9's lighting (`World`/`Chunk`/
  `NibbleArray` flood fill, whole-chunk `generateSkylightMap`, heightmap invalidation) is a known
  hot spot. Rewrite it data-oriented: packed nibble sky/block-light planes per section, incremental
  queued propagation, cached heightmaps, **no per-tick allocation**, and parallel relight of
  independent regions (propose-parallel / commit-serial). **Light levels and update results stay
  bit-identical** (`light_parity`); any *rendered* change (smoothing, AO, coloured light) is a
  presentation deviation, off by default.
- **Threading model:** the **tick is deterministic and single-threaded**; parallelism goes to rendering, meshing, lighting, I/O. Any parallel phase must be provably order-independent (propose-parallel / commit-serial + digest).
- **Perf budgets per module** (examples): ≤ X µs per entity tick, ≤ N allocations per tick, mesh a section in < X ms, tick under 50 ms at N players.

## 8. Repo Layout

```
crates/    mc-core mc-perf mc-nbt mc-proto mc-worldgen mc-world mc-entity
           mc-server mc-client mc-render mc-assets
vendor/    horizon/ = shallow sparse clone of reusable paths (LGPL-3.0) + REUSE-NOTES.md
oracle/    Java 1.8.9 driver mod + harnesses (never shipped)
tools/     parity.py  port_ledger.py  test_<name>_parity.py
           perf.py  check_public_tree.py
fixtures/  seeds, scenes, packet captures (no Mojang assets)
research-lock.json   # jar + decompiled-source hashes
```

## 9. Horizon Components to Reuse / Adapt (LGPL-3.0)

Version-agnostic and directly address "don't reinvent the wheel":

- **Culling:** `frustum.rs`, `occlusion.rs`, `cull.rs`, `gpu_culling.rs` (+ WGSL), GPU compute + indirect draws, cave culling.
- **Data:** section arenas, packed light/heightmaps, `SharedFrames` (compress once for all players).
- **Threading:** worker pools, `io_pool` (epoll/kqueue via mio), parallel tick (propose-parallel / commit-serial), off-tick saving.
- **Rendering:** wgpu device layer, translucent batching, LOD.
- **Measurement:** `perf_bench`, `hot_paths` (Callgrind instructions/allocations), pacing bench.
- **Tooling:** `parity.py` / `port_ledger.py` skeletons, `check_public_tree.py`.

**Reuse depth: medium.** Take the plumbing and performance infrastructure above; write all gameplay ourselves.

**Rebuild against 1.8.9 (do not copy):** worldgen math, redstone/update order, combat, physics, old lighting model, block models/renderer, protocol packet set.

## 10. Roadmap (risk-first; each milestone shippable)

- **M0 — Production line.** Clean vanilla decompile, jars pinned, oracle driver mod, `parity.py` + `port_ledger.py` + `check_public_tree`. Prove the loop on `random_parity`.
- **M1 — Worldgen parity.** Same seed → identical chunks (blocks+metadata+biome+heightmap). No renderer needed. *(Highest technical risk, first real proof.)*
- **M2 — Protocol 47.** Bot joins a real 1.8.9 server and receives the world; then a vanilla client joins your server (flat/void first).
- **M3 — Physics + first frames.** Movement/collision parity; render flat-world blocks and walk.
- **M4 — Real world.** Lighting, worldgen integrated, chunk meshing + textures, culling pipeline.
- **M5 — Entities & AI.** Mobs, spawning, pathfinding (propose-parallel where order-independent).
- **M6 — Interaction & redstone.** Blocks, inventory, crafting, then redstone tick parity (the final boss).
- **M7 — UI.** GUI, inventory, chat, menus.
- **M8 — Parity + perf pass.** Singleplayer + multiplayer; performance budgets and tuning.

## 11. First Two Weeks

1. **Day 1–2:** Get own 1.8.9 client+server jars. Clean vanilla decompile (MCP 1.8.9). Pin hashes in `research-lock.json`. Stand up `check_public_tree`.
2. **Day 2–4:** `parity.py` skeleton, import NBT/region plumbing, oracle scaffold. **Gate A green: `random_parity`.**
3. **Day 4–7:** Port noise. **Gate B green: `noise_parity` + `noise_batch_parity`** (proves the loop end-to-end).
4. **Week 2:** Worldgen skeleton (RNG → noise → biomes → base terrain); wire chunk oracle; iterate **Gate C**.

### Gate C — `worldgen_seed` (first meaningful milestone)

- Seeds `{12345, 0, -7}` × chunks `(0,0)`, `(100,-50)`, `(-37,210)`, `(512,512)`, overworld.
- Dump blocks + metadata + biome + heightmap canonically; hash.
- **Pass:** `python tools/parity.py worldgen_seed` exits 0, **0 mismatched chunks**; on failure, names the **first differing block coordinate**.

**Staged.** All six structure layouts are ported and **decoration is complete** (`decoration_parity`
gates ores, surface, vegetation, trees, springs and the per-biome `decorate` overrides via its `full`
mode); it changes blocks, so the Gate-C name stays reserved for the final chunk. The pieces that exist
are pinned separately:
`worldgen_parity`
(`tools/test_worldgen_parity.py`) covers blocks + metadata + biome + heightmap for the
**pre-decoration** `provideChunk` output on these seeds/chunks (plus cave/ravine/mesa/ocean
fixtures), and `structures_parity` covers the structure *layout* registered during `provideChunk`
(starts, pieces, bounding boxes) for mineshafts, scattered features (temples/huts), ocean monuments,
nether fortresses, villages and strongholds. Structure *blocks* are written in `populate`: the
mineshaft half is
now ported (`mineshaft_placement_parity`) together with the RNG its chests consume (`loot_parity`:
`WeightedRandomChestContent` + `EnchantmentHelper.buildEnchantmentList`); all six structure layouts
are now ported, so the remaining work for Gate C is **structure block placement** for the
desert/jungle pyramids, ocean monument, nether fortress, village and stronghold (the swamp hut and
mineshaft are done; `temple_placement_parity` gates the hut). (Vanilla's `onBlockAdded`
block-update consequences - rail reshaping, torch support - are outside the placement port; see
`handoff.md`.)

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| AI transliterates Java | Architecture-first spec + perf budgets + hot-path review |
| AI writes clean-but-wrong code | Parity harness is mandatory; never merge without it |
| "1:1" drifts silently | Deterministic harnesses on every behavior change; ledger gaps visible |
| Perf regresses invisibly | Callgrind instruction/alloc counts + budgets in CI |
| Decompiler/bytecode ambiguity | Vineflower vs CFR cross-check; `javap` as truth |
| Forge patches mislead ports | Clean vanilla decompile primary; ignore `net.minecraftforge.*` |
| LGPL contamination | Isolate `vendor/`, publish modifications, keep own crates separate |
| Assets/legal | Own copy; build assets at setup; never commit Mojang files |
| Motivation (solo, long) | Ship demos early: M1 and M2 need no renderer |

## 13. Locked Decisions & Performance Targets

### Locked decisions

- **Mappings:** MCP **`stable_22`** — matches the 1.8.9 modding ecosystem.
- **Oracle route:** **driver mod** — a dev-only Forge 1.8.9 mod that only calls the real game code (the Horizon pattern).
- **`vendor/` reuse depth:** **medium** — reuse plumbing + performance infrastructure; write gameplay ourselves.
  - **Take:** NBT, region files, varint/framing/encryption, wgpu device layer, thread pools / io pool, arenas, culling (frustum / occlusion / cave / GPU), `SharedFrames`, benchmark + `parity.py` / `port_ledger.py` scaffolding, `check_public_tree`.
  - **Write ourselves:** worldgen, redstone / update order, combat, physics, old lighting, block models / renderer, protocol packet set.
- **License:** reused parts are LGPL-3.0; keep them isolated in `vendor/` and publish modifications.
- **Rendering backend:** **Vulkan via wgpu** (Horizon's `gfx` device layer) — **no OpenGL/LWJGL**.
  This is an internal API swap, not a player-visible change: the **render harness still gates the
  pixels** (same art, no glitches), so it needs no Deviation Registry entry (cf. §17 "performance
  internals may differ").
- **Lighting:** **rewrite** the engine, don't port 1.8.9's (packed nibble SoA, incremental queue,
  cached heightmaps, parallel relight). Light **values** are parity-gated by `light_parity`; only
  rendered-light changes would be presentation deviations (off by default). See §7.

### Performance targets

"Way more performant than Horizon" is the **ambition**, but it is not directly testable (Horizon runs 26.3 — different workload, different hardware). We track three tiers; only the first two are release gates.

1. **Absolute floors (hard gates):**
   - 20 TPS (≤50 ms/tick) at 50 players on the reference server.
   - **Zero allocations per tick** in hot paths; zero per-section allocations in the meshing inner loop.
   - No frame over 33 ms on the standard benchmark path (settled world).
2. **Relative to Java 1.8.9 (the meaningful comparison):**
   - Client: ≥ 3× vanilla 1.8.9 FPS and ≥ 2× vanilla + OptiFine, same scene/settings/hardware.
   - ≥ 5× fewer long stutters; memory ≤ 50% of vanilla's process RSS.
   - Server: hold 20 TPS where vanilla 1.8.9 drops below it; lower p99 tick time.
3. **Relative to Horizon (aspiration, not a gate):**
   - Horizon-class architecture; aim to **meet or beat Horizon's published numbers on equivalent hardware/scene**.
   - Honest caveat: 1.8.9 is a **lighter game** than 26.3, so part of any raw win reflects the smaller workload, not superior engineering. Claim wins only via **normalized benchmarks** (same scene, same hardware, documented).

**Budgets are set empirically:** M0/M1 establish Java 1.8.9 baselines (vanilla + OptiFine); each module gets concrete numbers derived from those before its port starts.

## 14. Harness & Tooling Inventory (what to take from Horizon)

Horizon's **tooling is reusable; its fixtures/oracles are 26.3-specific.** Take the machine, rewrite the tests. All reused files are Horizon's original work (LGPL-3.0); keep notices in `vendor/`.

**Vendored copy:** a shallow sparse clone of these paths is at **`vendor/horizon`** (commit `0e5055d`, gitignored). Exactly what was taken/excluded, why, the license boundary, and the command to recreate it are recorded in **`docs/horizon-reuse.md`** (and mirrored in the working copy's `REUSE-NOTES.md`).

**Performance reuse:** the performance inventory (allocator/startup, measurement, streaming/pregeneration, gfx, world storage, launch/asset prep) and the phased adoption plan are in **`docs/horizon-performance-reuse.md`**. Horizon contributes infrastructure + measurement; parity-critical 1.8.9 math stays ours. Today we reuse no Horizon runtime code yet.

### 14.1 Tooling — take / adapt

| Horizon file | Purpose | Action |
|---|---|---|
| `tools/parity.py` | Runs `test_*_parity.py` in parallel; `--changed/--failed/--repeat/--status/--fresh` | Adapt |
| `tools/port_ledger.py` | `build/issue/where/class/what/check/gaps` — vanilla ↔ Rust map | Adapt to 1.8.9 |
| `tools/parity_shim.py` | Intercepts `java`/`javac`, **caches + replays oracle runs** | Take |
| `tools/gate.py` | Branch gate: public-tree, fmt, clippy, tests, harnesses | Take |
| `tools/check_public_tree.py` | Blocks Mojang files/decompiled code | Take as-is |
| `tools/pr_ready.py` | Selects harnesses a change affects | Take |
| `tools/lab.py` | `prepare/decompile/status` for jars | Adapt (stable_22, Vineflower) |
| `tools/rea_vanilla.py` | Bytecode queries: callers, overloads, 2nd decompiler view | Take |
| `tools/class_index.py`, `tools/anvil_dump.py` | Class index; region dump | Adapt |
| `tools/bench.py`, `build_times.py`, `hot_paths.py`, `perf_bench.py`, `server_bench.py`, `pacing_bench.py`, `bench_stats.py` | Benchmarks incl. Callgrind instruction/alloc counts | Adapt |
| `tools/parity_fuzz.py`, `parity_fuzz_scenarios.py` | Generated scenarios, shrunk repros | Take concept |
| `tools/state_digest_*`, `seed_agent.py`, `SeedAgent.java` | Per-tick state hash; deterministic RNG sync | Take concept (1.8.9 RNG differs) |
| `tools/check_layers.py`, `check_imports.py`, `tools/layering/` | **Enforce architecture / dependency rules** | Take |
| `.claude/skills/vanilla-parity/SKILL.md` | Agent workflow (ledger-first) | Retarget |
| `AGENTS.md`, `docs/parity-workflow.md`, `docs/port-ledger.md`, `docs/architecture.md` | Process docs | Adapt |
| `.github/workflows/`, `tools/ci/`, `.githooks` | CI (verify / parity / release) | Adapt |

### 14.2 Crates — take (medium depth)

| Crate | Why |
|---|---|
| `horizon-core` | **NBT, exact `java.util.Random`, `jmath` (Java `Math` semantics), Java-style `HashMap`** — most valuable for 1:1 |
| `horizon-world` | Anvil `region.rs`, chunk-storage primitives (not worldgen) |
| `horizon-protocol` | varint framing, AES/CFB8, compression (packet set → rewrite for 47) |
| `horizon-threads` | Thread pools / worker model |
| `horizon-system` | System info, allocator probing |
| `horizon-client-render` (`gfx/`) | `cull.rs`, `occlusion.rs`, `gpu_culling.rs`, `frustum.rs`, `arena.rs`, device layer |
| `horizon-bench`, `horizon-perf`, `horizon-perf-format`, `horizon-hot-paths-game` | Measurement + report format |
| `horizon-assets`, `horizon-prepare` | Asset extraction from own jar (adapt to 1.8.9) |
| `horizon-java-properties` | `server.properties` |
| `horizon-bots` | Bot architecture (rewrite protocol 47) |
| `horizon-mcp` | Optional agent control surface |

**Skip:** `horizon-game`, `horizon-gameplay`, `horizon-client-gameplay`, `horizon-items`, `horizon-command`, all mod-system crates, cosmetics, friends/relay, Discord, chat-filter, anticheat, narrator, telemetry, server supervisor, port-pack, theme, release-sign.

### 14.3 Build ourselves (1.8.9-specific)

- Java **oracle driver mod** (Forge 1.8.9).
- Every `test_<area>_parity.py` + fixture: worldgen, noise, random, light, physics, tick, redstone, meshing, protocol, AI, combat, render.
- Canonical serializers: chunk (blocks+metadata+biome+heightmap), entity state, packet capture, per-tick digest (1.8.9 layouts).
- `port_map/*.tsv` for 1.8.9 classes.
- 1.8.9 asset-preparation pipeline.
- Protocol 47 packet set, worldgen, redstone, physics, combat, AI, lighting, block models/renderer.

### 14.4 The five files that matter most

1. `horizon_core::jmath` — Java `Math`/float semantics (fdlibm).
2. `horizon_core::random` — exact `java.util.Random` (LCG + `nextInt(bound)`).
3. `tools/parity_shim.py` — oracle result caching (keeps the agent loop fast).
4. `tools/port_ledger.py` — coverage: ported / missing / wired wrong.
5. `.claude/skills/vanilla-parity/SKILL.md` — teaches agents the workflow so they don't guess.

## 15. Local Inputs (downloaded 2026-11-20)

All under `vendor/minecraft/1.8.9/` unless noted. Pinned in `vendor/minecraft/1.8.9/research-lock.json` (jar SHA-1/SHA-256, mapping versions, derived artifacts).

| Input | Path | Status |
|---|---|---|
| Client jar (official) | `client.jar` | ✅ SHA-1 matches Mojang manifest |
| Server jar (official) | `server.jar` | ✅ SHA-1 matches Mojang manifest |
| Version metadata | `1.8.9.json` | ✅ |
| Asset index (1.8) | `1.8-assets-index.json` | ✅ (external sound objects not fetched yet) |
| MCP names (`stable_22`) | `mcp_stable-22.zip` → `mappings/mcp_stable-22/` | ✅ methods/fields/params CSVs |
| SRG mappings | `mcp-1.8.9-srg.zip` → `mcp-srg/joined.srg` | ✅ includes `joined.exc`, Forge patches |
| Merged obf→MCP mapping | `mappings/joined-mcp.srg` | ✅ generated (6,585 fields + 13,693 methods renamed) |
| **Clean vanilla decompile — client** | `decompiled/vanilla-client/` | ✅ 1,612 `.java`, **0 Forge refs** |
| **Clean vanilla decompile — server** | `decompiled/vanilla-server/` | ✅ 2,941 `.java`, **0 Forge refs** |
| Forge 1.8.9 MDK | `forge-1.8.9-11.15.1.2318-mdk.zip` → `forge-mdk/` | ✅ for the oracle driver mod |
| Protocol/block/item data | `../reference/minecraft-data/data/pc/1.8/` | ✅ `protocol.json`, `blocks.json`, `items.json`, `biomes.json`, `entities.json`, recipes, etc. |
| Toolchain | `vendor/tools/`, system | ✅ Rust 1.100.0 (GNU) + w64devkit GCC 16.2, Vineflower 1.12.0, JDK 21.0.13.1+2, SpecialSource 1.11.4-shaded |
| Java 8 | system | ✅ Temurin 1.8.0_492 (for 1.8.9 server/Forge) |
| Workspace scaffold | `Cargo.toml`, `crates/`, `.cargo/config.toml`, `rust-toolchain.toml` | ✅ 11 crates; `cargo test --workspace` + `cargo fmt --check` pass |
| First harness | `tools/test_random_parity.py` (+ `oracle/java/src/RandomOracle.java`, `crates/mc-core/examples/random_probe.rs`) | ✅ `random_parity` green: 3,848 lines match `java.util.Random` |
| Noise | `crates/mc-worldgen/src/noise.rs` + `tools/test_noise_parity.py` (+ `NoiseOracle.java`) | ✅ `noise_parity` green: 1,047 lines match (Improved, Simplex, Perlin, Octaves) |
| Noise (batch) | `crates/mc-worldgen/src/noise.rs` + `tools/test_noise_batch_parity.py` (+ `NoiseBatchOracle.java`) | ✅ `noise_batch_parity` green: 369 lines match (`Perlin.func_151599_a`/`func_151600_a`, `Simplex.func_151606_a`) |
| Biome layers | `crates/mc-worldgen/src/layer.rs` + `tools/test_biome_layer_parity.py` (+ `BiomeLayerOracle.java`) | ✅ `biome_layer_parity` green: 4,626 lines match (full `GenLayer*` stack; default overworld) |
| Terrain | `crates/mc-worldgen/src/terrain.rs` + `tools/test_terrain_parity.py` (+ `TerrainOracle.java`) | ✅ `terrain_parity` green: 16,191 lines match (full base terrain: shape + surface, incl. Mesa) |
| Caves/ravines | `crates/mc-worldgen/src/cave.rs` + `tools/test_caves_parity.py` (+ `CaveOracle.java`) | ✅ `caves_parity` green: 46,260 lines match (`MapGenBase` + `MapGenCaves` + `MapGenRavine`, caves/ravines/both/base) |
| Chunk artefacts | `crates/mc-worldgen/src/chunk.rs` + `tools/test_worldgen_parity.py` (+ `WorldgenOracle.java`) | ✅ `worldgen_parity` green: 7,770 lines match (blocks + biome array + height map; pre-decoration chunk) |
| Mineshafts + scattered features + monuments + nether fortresses + villages + strongholds (layout) | `crates/mc-worldgen/src/structure/` (mod + `mineshaft.rs` + `scattered.rs` + `monument.rs` + `netherbridge.rs` + `village.rs` + `stronghold.rs`) + `tools/test_structures_parity.py` (+ `StructuresOracle.java`) | ✅ `structures_parity` green: 10,533 lines match (all six generators' starts, pieces, boxes; block placement pending) |
| Loot + enchantment RNG | `crates/mc-worldgen/src/loot.rs` + `tools/test_loot_parity.py` (+ `LootOracle.java`) | ✅ `loot_parity` green: 695 lines match (registry + `buildEnchantmentList` + `generateChestContents`; unblocks chest placement) |
| Mineshaft block placement | `crates/mc-worldgen/src/structure/placement.rs` + `tools/test_mineshaft_placement_parity.py` (+ `MineshaftPlacementOracle.java`) | ✅ `mineshaft_placement_parity` green: 3,598 lines match (3 seeds x 14 chunks, 426 structure blocks; curated corpus - see `handoff.md`) |
| Temple block placement (swamp hut) | `crates/mc-worldgen/src/structure/placement.rs` + `tools/test_temple_placement_parity.py` (+ `TemplePlacementOracle.java`) | ✅ `temple_placement_parity` green: 514 lines match (2 seeds x 2 chunks, 282 structure blocks; desert/jungle pyramids still layout-only) |
| Decoration (all layers + per-biome overrides) | `crates/mc-worldgen/src/decorate.rs` + `decorate/blocks.rs` + `decorate/trees.rs` + `tools/test_decoration_parity.py` (+ `DecorationOracle.java`) | ✅ `decoration_parity` green: 32,896 lines match (7 modes x 3 seeds: ores, surface, plants, trees, springs, ore_surface, `full` = real `BiomeGenBase.decorate` incl. double plants/melon/vines/ice/boulders/emerald/monster-egg/desert wells) |
| Java HashMap order | `crates/mc-core/src/jhash.rs` + `tools/test_jhash_parity.py` (+ `HashOrderOracle.java`) | ✅ `jhash_parity` green: 16 insertion orders / 301 keys match `keySet()` order (unblocks chest-enchantment RNG) |
| Block table | `oracle/java/src/BlockTableOracle.java` → `chunk::light_opacity` | ✅ dumped from the live game (opacity per block id + material-air set) |
| Math | `crates/mc-core/src/jmath.rs` + `tools/test_math_parity.py` (+ `MathOracle.java`) | ✅ `math_parity` green: 492 lines match (`MathHelper` sin/cos table + floor/clamp helpers) |
| Perf substrate | `crates/mc-perf` + `docs/performance-contract.md` + `tools/perf_baseline.py` | ✅ P0: contract + spans/`Stats`/alloc/`work`; baseline recorded |
| Handoff / repo | `handoff.md`, `AGENTS.md`, `scripts/verify.ps1`, `.gitattributes`, git remote | ✅ pushed to `enthu-ux/rusted_mc` (`main`); `scripts/verify.ps1` runs tests + fmt + parity |

**Notes**
- The clean vanilla trees are the **primary spec**; the Forge-patched tree at `C:\Users\wassi\Desktop\Minecraft Rust Edition\...` remains a secondary cross-check.
- Decompiles are readable but partially mapped (`func_`/`field_` remain where MCP has no name); **bytecode is the truth** — use `javap`/`rea_vanilla`.
- **Never commit** jars, zips, mappings, decompiled sources, or reference data (see `.gitignore`).
- **Still optional/pending:** external Minecraft **sound** assets (from the 1.8 asset-index objects) — only needed when audio work starts.
- Conversion pipeline used: `joined.srg` + `stable_22` CSVs → `joined-mcp.srg` → SpecialSource remap → Vineflower decompile.

## 16. Parity Policy & Deviation Registry

**Stance:** a *superior* 1.8.9 — vanilla-faithful by default, with a **curated, documented, tested** set of fixes. Not a strict bug-for-bug clone; not a different game.

### 16.1 The three layers

| Layer | Examples | Policy |
|---|---|---|
| **Implementation defects** | crashes, leaks, GC pauses, races, overflow | **Always fix** (invisible) |
| **Presentation (client-side)** | model seams/gaps, z-fighting, texture/mipmap bleeding | **Fix** — same art, no glitches (recorded divergence) |
| **Authoritative gameplay** | dupes, worldgen, redstone, physics, protocol | **Vanilla by default**; fix only if registered |

### 16.2 Where fixes apply

- **Our client on a vanilla server** → behaves vanilla (we don't control the server).
- **Our server / singleplayer** → our engine is authoritative, so registered fixes apply.
- **Vanilla client on our server** → connects fine and sees our server's rules (server-side mechanics cause no protocol desync).

Interoperability is preserved: we speak **protocol 47** and read/write **vanilla Anvil** worlds.

### 16.3 Hard constraints (never violate)

- **Worldgen stays bug-for-bug** — same seed → same terrain (fixing a worldgen bug breaks seeds/saves/servers).
- **Redstone / physics quirks stay** — contraptions and feel depend on them.
- **No protocol divergence** from vanilla peers.

### 16.4 Deviation Registry

Every intentional change from vanilla is a first-class entry:

```
id, title, category (presentation|gameplay|stability|security), scope (client|server|both),
default (on|off), description, rationale, risk, parity-harness test, status
```

- Lives at `docs/deviations.md` (human) + `deviations.toml` (machine).
- The differential harness treats registered deviations as **expected**; **any unregistered difference is a failure**.

### 16.5 Seed candidates to classify

- **Presentation:** model seams/gaps, z-fighting, texture/mipmap bleeding, lighting seams / light-update artifacts.
- **Stability:** crashes, leaks, chunk-corruption paths.
- **Security:** packet / auth / chat exploits.
- **Gameplay (decide per-dup):** TNT / carpet / rail duping — enumerate each and choose; default **keep** unless registered.

### 16.6 Decision rule for any candidate

*"Would a player notice it, and would someone depend on it?"*
- No / no → **fix** (implementation defect).
- Presentation artifact → **fix** (record it).
- Gameplay → **vanilla** unless registered; **worldgen / redstone / physics → always vanilla**.

## 17. Engineering Philosophy (adopted from Horizon)

We adopt Horizon's philosophy wholesale (vendored at `vendor/horizon/`). The full statement is in
**`docs/PHILOSOPHY.md`**. The rules that shape every decision:

- **Vanilla first, then better** — 1.8.9 behavior first, improvements second.
- **Vanilla is the spec** — down to the order of random calls; every port of vanilla behavior ships with
  a harness; **never delete a parity harness** (it is the spec).
- **When it doesn't have to be 1:1** — performance internals may differ, but must **interoperate**
  (vanilla clients/servers, worlds open both ways), **play the same**, differ **only where no player can
  rely on it**, **gate player-noticeable changes behind a setting**, and **never copy vanilla's slow paths**.
- **Layers** — engine / platform / app; lower layers never depend on higher ones; extension points name no
  type from above; **portable crates** hold vanilla rules/data with no window/GPU/sound/network.
- **Repo hygiene** — never commit Mojang files or artifacts (`vendor/minecraft/`, `.local/`).
- **Everything players run is Rust**; Python/Java only for tools and oracles.
- **Tests with signal** — observable behavior, independent oracle, one per changed behavior.
- **Report validated scope, never claim 100%.**
- **Our one extension** — the Deviation Registry (§16) for curated fixes where our engine is authoritative.