# Horizon reuse notes (tracked record)

This is the **tracked** record of what we take from Horizon and why. The working copy lives at
`vendor/horizon/` (gitignored; a shallow sparse clone). If it is missing, recreate it:

```sh
git clone --depth 1 --filter=blob:none --sparse https://github.com/horizon-rust/horizon.git vendor/horizon
git -C vendor/horizon sparse-checkout set \
  .claude/skills/vanilla-parity tools docs \
  crates/horizon-core crates/horizon-world crates/horizon-protocol crates/horizon-threads \
  crates/horizon-system crates/horizon-platform crates/horizon-addresses \
  crates/horizon-client-render crates/horizon-bench crates/horizon-perf crates/horizon-perf-format \
  crates/horizon-hot-paths-game crates/horizon-assets crates/horizon-prepare \
  crates/horizon-java-properties crates/horizon-bots crates/horizon-mcp
```

- **Upstream:** https://github.com/horizon-rust/horizon
- **Commit taken:** `0e5055d` (2026-11-20)
- **License:** **LGPL-3.0** (see the working copy's `LICENSE` and `THIRD_PARTY_NOTICES.md`). Reused
  code stays LGPL-3.0: keep it isolated, retain notices, publish modifications to those parts.

## What was taken

### Tooling - `tools/`

Runner / coverage / gate: `parity.py`, `port_ledger.py`, `parity_shim.py`, `gate.py`, `pr_ready.py`,
`check_public_tree.py`, `check_changed.py`, `check_imports.py`, `check_layers.py`, `parity_reach.py`,
`parity_main.py`, `supervise.py`, `class_index.py`.

Vanilla reading / oracle setup: `lab.py`, `rea_vanilla.py`, `anvil_dump.py`, `seed_agent.py`,
`SeedAgent.java`, `JavaCharacterTables.java`.

Benchmarks / profiling: `bench.py`, `build_times.py`, `hot_paths.py`, `perf_bench.py`,
`server_bench.py`, `pacing_bench.py`, `bench_stats.py`, `entity_occlusion_bench.py`.

Fuzzing / determinism: `parity_fuzz.py`, `parity_fuzz_scenarios.py`, `state_digest_scene.py`,
`state_digest_players.py`, `state_digest_compare.py`.

Supporting dirs: `port_map/`, `layering/`, `ci/`, `fixtures/`, `hotspots/`, `benchmark-deps/`,
`command_exec_groups/`, `sync/`, `fn_replace/`.

### Docs - `docs/` (whole folder)

Architecture, parity workflow, port ledger, world generation, section-meshing parity, redstone diff,
world compare, benchmarking, hot-path trends, profiling, memory, graphics.

### Agent workflow

`.claude/skills/vanilla-parity/SKILL.md` - "ledger-first" parity workflow for agents.

### Crates (17)

`horizon-core`, `horizon-world`, `horizon-protocol`, `horizon-threads`, `horizon-system`,
`horizon-platform`, `horizon-addresses`, `horizon-client-render`, `horizon-bench`, `horizon-perf`,
`horizon-perf-format`, `horizon-hot-paths-game`, `horizon-assets`, `horizon-prepare`,
`horizon-java-properties`, `horizon-bots`, `horizon-mcp`.

### Root files

`Cargo.toml`, `Cargo.lock`, `AGENTS.md`, `CLAUDE.md`, `research-lock.json`, `README.md`, `LICENSE`,
`THIRD_PARTY_NOTICES.md`, `clippy.toml`.

## The most important pieces for 1:1 parity

| Piece | Why |
|---|---|
| `horizon-core` -> `jmath` | Java `Math` / float semantics (fdlibm) |
| `horizon-core` -> `random` | exact `java.util.Random` (LCG + `nextInt(bound)`) |
| `horizon-core` -> `nbt` | NBT read/write |
| `horizon-core` -> Java `HashMap` | Java iteration order matters for behavior |
| `tools/parity_shim.py` | caches/replays Java oracle runs - keeps the agent loop fast |
| `tools/port_ledger.py` | ported / missing / mis-wired coverage |
| `tools/gate.py` | merge gate (public-tree, fmt, clippy, tests, harnesses) |
| `.claude/skills/vanilla-parity/SKILL.md` | teaches agents the workflow |
| `horizon-client-render/src/gfx/` | culling, occlusion, GPU culling, frustum, arenas |
| `horizon-threads` | thread pools / worker model |

## Deliberately excluded

All 26.3 gameplay and non-reusable parts: `horizon-game`, `horizon-gameplay`,
`horizon-client-gameplay`, `horizon-client`, `horizon-client-lib`, `horizon-client-core`,
`horizon-client-input`, `horizon-items`, `horizon-command`, every `horizon-mod-*` crate,
`cargo-horizon-mod`, `horizon-internals`, `horizon-engine-slots`, `horizon-guest-stack`,
`horizon-hooks`, `horizon-cosmetics`, `horizon-friends`, `horizon-relay`, `horizon-discord`,
`horizon-chat-filter`, `horizon-anticheat`, `horizon-narrator`, `horizon-telemetry*`,
`horizon-server*`, `horizon-official*`, `horizon-port-pack*`, `horizon-shader*`, `horizon-shaders`,
`horizon-hdr`, `horizon-polytone`, `horizon-theme`, `horizon-optifine`, `horizon-game-discovery`,
`horizon-heartbeat`, `horizon-release-sign`.

## How the tree is used

- **Reference + source to adapt**, not a buildable project (the workspace is incomplete on purpose).
- Copied code **stays LGPL-3.0**: keep it isolated, retain notices, publish modifications.
- **Never commit Mojang code or assets.** This tree contains no Minecraft files.

## Updating

```sh
git -C vendor/horizon fetch --depth 1 origin main
git -C vendor/horizon reset --hard FETCH_HEAD
```
