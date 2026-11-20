# Handoff

Living status for continuing this project in a new session. **Rewritten each handoff**; the
**Session log** at the bottom is append-only. Assume the next session has no memory of any chat:
everything needed to continue lives in files.

---

## Read first (context)

- `plan.md` - the plan (goals, production line, architecture contract, roadmap, parity policy,
  performance targets, philosophy summary).
- `docs/PHILOSOPHY.md` - engineering philosophy (Horizon's, adopted).
- `docs/ARCHITECTURE.md` - crate map and the two gates (parity + performance).
- `docs/deviations.md` + `deviations.toml` - intended deviations from vanilla.
- `docs/horizon-reuse.md` - what we took from Horizon and why (working copy: `vendor/horizon/`, gitignored).
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

Expected today: `cargo test` and `fmt` pass; `py tools/parity.py` reports `2/2 passed`.

## Environment (mostly local, NOT in git)

- **Inputs:** `vendor/minecraft/1.8.9/` (client/server jars, remapped jars, decompiled
  `vanilla-client`/`vanilla-server`, mappings). Pinned in `research-lock.json`. Gitignored.
- **Rust:** stable `x86_64-pc-windows-gnu`; linker = w64devkit GCC (`.cargo/config.toml`).
  Grader `cargo` lives at `%USERPROFILE%\.cargo\bin` (new shells).
  **Gotcha:** w64devkit lacks `libgcc_eh.a`; we copied Rust's into it (re-copy if re-extracted).
- **Java:** system Java 8 (Temurin) for the 1.8.9 server/Forge; JDK 21 at `vendor/tools/jdk21`
  for Vineflower.
- **Reference:** `vendor/horizon` (commit `0e5055d`, LGPL) - tools, docs, selected crates.
- **Windows note:** the console mangles non-ASCII in output; the files themselves are fine.
- **Git:** initialized and pushed. Remote `origin = https://github.com/enthu-ux/rusted_mc` (branch
  `main`, identity `enthu-ux <wassimbariza@gmail.com>`). `vendor/horizon/`, `vendor/minecraft/`,
  `vendor/reference/`, `vendor/tools/jdk21|w64devkit|*.jar|*.zip|*.exe`, `target/`, `.local/` are gitignored.
  Push a session's work with `git add -A && git commit -m ... && git push`.
- **Verify:** `scripts/verify.ps1` runs tests + format + all parity harnesses in one command.

## Current state

- **Inputs:** done. Official 1.8.9 jars (hash-checked), clean vanilla decompile (0 Forge refs),
  MCP `stable_22`, protocol/block/item data (`vendor/reference/minecraft-data`).
- **Workspace:** 10 crates scaffolded; `cargo test` + `fmt` green.
- **Harnesses green:** `random_parity` (3,848 lines), `noise_parity` (1,047 lines).
- **Implemented:** `mc-core::random` (exact `java.util.Random`); `mc-worldgen::noise`
  (`Improved`, `Simplex`, `Octaves`, `Perlin`).
- **Policy/docs:** `plan.md` (sections 1-17), `README.md`, `docs/PHILOSOPHY.md`,
  `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`.

## Next steps (prioritized, each with a pass condition)

1. **`noise_batch_parity`** - port `Perlin.func_151599_a`/`func_151600_a` and
   `Simplex.func_151606_a`; add a Java oracle case + Rust probe case.
   *Done when:* the harness is green (line count recorded).
2. **`biome_layer_parity`** - port the `GenLayer*` stack; compare biome ids for a chunk area.
3. **`terrain_parity`** - `ChunkProviderGenerate` base terrain (blocks before decoration).
4. **`worldgen_seed`** - full chunk parity (blocks + metadata + biome + heightmap) = **milestone M1**.
5. **Deferred:** performance architecture contract + perf harness (benchmark vs Java 1.8.9 +
   OptiFine, instruction/allocation budgets).

## Open threads / decisions

- `nextGaussian` needs Java `StrictMath.log` (fdlibm) via `jmath` - same reason Horizon defers it.
- External **sound** assets not downloaded (optional until audio work).
- `vendor/horizon` is a nested git repo - decide submodule vs plain vendored copy if git is initialized.
- Decided but not built: the curated **fixes layer** (registry entries; modelled on Horizon's anticheat).

## Session log (append-only)

- **2026-11-20 - session 1.** Downloaded inputs (jars, mappings, clean decompile, protocol data);
  installed Rust (GNU) + w64devkit linker; scaffolded the 10-crate workspace; implemented
  `JavaRandom` and the four noise generators; **`random_parity` and `noise_parity` green** (the
  noise harness caught a real accumulate-vs-clear bug). Wrote `plan.md` (§1-17),
  `docs/PHILOSOPHY.md`, `docs/ARCHITECTURE.md`, `docs/deviations.md`, `deviations.toml`, this file.
  Added `scripts/verify.ps1`, `.gitattributes`, `docs/horizon-reuse.md`; initialized git (first commit).
