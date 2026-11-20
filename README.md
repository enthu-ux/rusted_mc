# Minecraft 1.8.9 - Rust Reimplementation

A from-scratch Rust rewrite of Minecraft: Java Edition 1.8.9 — a **superior 1.8.9**:
vanilla-faithful by default, with curated fixes, clean internals, and Horizon-class
performance. See `plan.md` §16 for the parity policy and Deviation Registry, and
`docs/PHILOSOPHY.md` for the engineering philosophy (Horizon's, adopted).

## Status

- Inputs downloaded and verified: 1.8.9 jars, clean vanilla decompile, MCP stable_22, protocol data.
- Workspace scaffolded (10 crates); `cargo test` and `cargo fmt --check` are green.
- Parity harnesses green: `random_parity` (3,848 lines), `noise_parity` (1,047 lines),
  `noise_batch_parity` (369 lines), `biome_layer_parity` (4,626 lines) and `terrain_parity`
  (16,191 lines; full overworld base terrain).
- Next: the worldgen ladder — `worldgen_seed` (milestone M1: caves/ravines, structures, decoration,
  heightmap).

## Layout

```
crates/   mc-core mc-nbt mc-proto mc-worldgen mc-world mc-entity
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
