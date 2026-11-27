# Minecraft 1.8.9 - Rust Reimplementation

A from-scratch Rust rewrite of Minecraft: Java Edition 1.8.9 — a **superior 1.8.9**:
vanilla-faithful by default, with curated fixes, clean internals, and Horizon-class
performance. See `plan.md` §16 for the parity policy and Deviation Registry, and
`docs/PHILOSOPHY.md` for the engineering philosophy (Horizon's, adopted).

## Status

- Inputs downloaded and verified: 1.8.9 jars, clean vanilla decompile, MCP stable_22, protocol data.
- Workspace scaffolded (11 crates); `cargo test` and `cargo fmt --check` are green (135 tests).
- Parity harnesses green (18): `random_parity`, `noise_parity`, `noise_batch_parity`,
  `biome_layer_parity`, `terrain_parity` (full overworld base terrain), `math_parity`,
  `caves_parity` (caves + ravines), `worldgen_parity` (canonical chunk: blocks + biome + height map),
  `structures_parity` (all six structure layouts: mineshaft, temple/hut, monument, fortress, village,
  stronghold), `jhash_parity` (`java.util.HashMap` iteration order), `loot_parity` (loot + enchantment
  RNG), `mineshaft_placement_parity` (mineshaft block placement), `temple_placement_parity`
  (swamp-hut + desert-pyramid + jungle-pyramid block placement), `monument_placement_parity`
  (ocean-monument block placement), `decoration_parity` (all decoration
  layers incl. the per-biome `decorate` overrides, via the `full` mode), `nether_placement_parity`
  (nether-fortress block placement, on a synthetic base - nether terrain is unported),
  `village_placement_parity` (all village pieces incl. the desert material substitution),
  `stronghold_placement_parity` (all reachable pieces + the liquid-drop path).
- Performance (P0 substrate): `mc-perf` + `docs/performance-contract.md` / `docs/performance-baselines.md`
  (base terrain Rust 1.35 ms/chunk vs Java 1.8.9 3.63 ms/chunk, caveated; per-stage spans recorded).
- Next: the `populate` pieces the full Gate-C chunk still needs (water/lava lakes, dungeons, the
  freeze/snow pass), then the full Gate-C chunk. See `handoff.md` for the exact sequencing and the
  known scope limits (vanilla's block-update/fluid-mixing cascades are outside the
  placement/decoration port).

## Layout

```
crates/   mc-core mc-perf mc-nbt mc-proto mc-worldgen mc-world mc-entity
          mc-server mc-client mc-render mc-assets
oracle/   Java 1.8.9 driver mod + harnesses (never shipped)
tools/    parity / port ledger / bench tooling
fixtures/ seeds, scenes, packet captures (no Mojang assets)
vendor/   horizon/ (LGPL reference) + minecraft/1.8.9/ (local inputs, gitignored)
docs/     architecture and process
```

## Build

Requires the project toolchain (see `vendor/tools/NOTES.md`): Rust stable **GNU** target with
w64devkit as the linker, configured by `rust-toolchain.toml` + `.cargo/config.toml`.

```sh
cargo test --workspace
cargo fmt --all -- --check
```

## Legal

No Mojang code or assets are committed. Downloaded jars, mappings, decompiled sources and
reference data are local and gitignored. Reused Horizon code is LGPL-3.0 - see
vendor/horizon/REUSE-NOTES.md.
