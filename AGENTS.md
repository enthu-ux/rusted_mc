# Project rules (agents)

A Rust rewrite of Minecraft 1.8.9: a **superior 1.8.9** - vanilla-faithful by default, with curated
fixes, clean internals and Horizon-class performance.

**Start here:** read `handoff.md`, then `plan.md`, then `docs/PHILOSOPHY.md`.

## Rules that bite

- **Vanilla 1.8.9 is the specification**, down to the order of random calls. Port from the decompiled
  source (`vendor/minecraft/1.8.9/decompiled/vanilla-server|client`), recording which classes/methods
  you followed.
- **Every port of vanilla behavior ships with a harness** (`tools/test_*_parity.py` + a Java oracle in
  `oracle/java/` + a Rust probe). **Never delete a parity harness** - it is the spec.
- **Two gates** for a module: parity (matches vanilla) and performance (budgets; no "Java in Rust").
- **Speed from the mechanism, never from observable behavior.** Anything a player could notice is
  gated behind a setting or registered as a deviation (`docs/deviations.md`, `deviations.toml`).
  Worldgen / redstone / physics / protocol never deviate.
- **Never commit Mojang files** (jars, assets, decompiled source) or artifacts: they live in
  `vendor/minecraft/` and `.local/`, both gitignored.
- **Math/RNG mirror Java**: use `mc-core` (`random`, `jmath`) so draws happen in vanilla's order.
- **No hardcoded user-facing strings**: keep text translation-ready.
- **Tests with signal**: observable behavior, independent expected values or a vanilla oracle, one
  test per changed behavior; never derive the answer from the code under test.
- **Everything players run is Rust**; Python/Java only for dev tools and oracles.

## Workflow

- **Find code with the map**, not by reading whole files.
- **Compile cheaply** (`cargo check -p <crate>`); run `fmt`, `clippy`, tests once, at the end.
- **Don't wait in loops**; background long commands.
- **One change per task**, with a clear pass condition.
- **Update `handoff.md`** (and the status docs) at the end of a session.

## Verify

One command (tests + format + all parity harnesses):

```sh
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Or individually:

```sh
cargo test --workspace
cargo fmt --all -- --check
py tools/parity.py
```

See `vendor/tools/NOTES.md` for the Windows toolchain (GNU + w64devkit) and `handoff.md` for
currently green harnesses and next steps.
