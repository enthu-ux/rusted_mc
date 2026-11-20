# Engineering Philosophy (adopted from Horizon)

This project is a **superior Minecraft 1.8.9**. We follow Horizon's philosophy: vanilla 1.8.9 is the
specification; we make it faster, cleaner and more stable, and we diverge only through the
**Deviation Registry** (`plan.md` section 16). Sources are vendored at `vendor/horizon/`.

---

## 1. Vanilla first, then better

Two stages: (1) play **exactly** like vanilla 1.8.9; (2) improve where players gain (speed, memory,
stability, no defects). Never skip stage 1.

## 2. Vanilla is the spec

- Wherever vanilla has a feature, vanilla is the specification, **down to the order of random calls**.
- **Parity** = our engine gives the same observable results as the real 1.8.9 client/server for the
  same inputs, checked by a **harness** (`tools/test_*_parity.py`: a Java oracle against the game, a
  Rust probe against us, compared).
- **Read the decompiled vanilla code before writing Rust**, and record which classes/methods you followed.
- **Every port of vanilla behavior ships with a harness** (new or extended).
- **Never delete a parity harness** that pins vanilla behavior: it is the spec.

## 3. When it doesn't have to be 1:1

Parity is about what players see, not about copying vanilla's internals. Performance work may differ,
but it must:

- **Interoperate** - vanilla 1.8.9 clients play on our server; we join vanilla servers; worlds/saves
  open in both.
- **Play the same** - farms, redstone, mobs, drops behave as players know.
- **Only differ where no player can rely on it** (e.g. which of two mobs ticks first, or exactly which
  random number a mob gets).
- **Gate anything a player could notice behind a setting.**
- **Not copy vanilla's slow paths** - "copying a performance problem isn't parity."
- **Say what changes** - name the behavior no longer reproduced exactly, why it's safe, and the evidence.

## 4. Deviations (our one extension)

Horizon puts beyond-vanilla features **off by default**. We additionally allow a **curated set of
fixes** applied where our engine is authoritative (our server / singleplayer).

- Every deviation is a **Deviation Registry** entry (`plan.md` section 16, `docs/deviations.md`,
  `deviations.toml`): category, scope, default, rationale, risk, test.
- Categories: **implementation defects** (fix freely, not listed), **presentation** (recorded),
  **gameplay** (registered; never worldgen/redstone/physics/protocol).
- Model server-side fixes on **Horizon's anticheat**: configurable, with a vanilla mode
  (`enabled = false` -> vanilla behavior).

## 5. Layers and the dependency rule

- **Engine** - vanilla 1.8.9 rules and data (worldgen, blocks, redstone, mobs, protocol, rendering,
  saves). Never depends on anything above.
- **Platform** - makes the engine easier to change, port and run (mod/UI/HDR/controller hooks,
  dedicated-server extras like anticheat).
- **App / Extras** - our own features and product. Uses the platform only, never the engine.
- **Rule:** lower layers never depend on higher ones. When lower code needs something above, it
  exposes an **extension point** (trait, callback list, event or registry) that names no type from above.
- **Portable crates** (`mc-core`, `mc-nbt`, `mc-world`, `mc-worldgen`, `mc-proto`, `mc-assets`) hold
  vanilla rules and data with no window, GPU, sound or network client.
- **With nothing registered/hooked, the engine does no work** (an empty slice where it would dispatch).

## 6. Where a new feature goes

1. Vanilla has it -> **engine**.
2. It is about our product (name, official servers, addresses) -> **app**.
3. Others build on it (mods, ports, other clients) -> **platform**.
4. Everything else -> **app**, or a mod a player adds.

## 7. Repo hygiene

- **Never commit Mojang files**: game jars, assets, decompiled source, prepared resources. They live
  under `vendor/minecraft/` and `.local/`, both gitignored. A gate (`check_public_tree` equivalent)
  rejects them.
- Oracle output, corpora and reports stay under `.local/` (gitignored); they never enter the repo.

## 8. Everything players run is Rust

Python and Java are only for dev tools and harness drivers (the oracle). Player-facing code is Rust.

## 9. Tests with signal

- Test **observable behavior**, not words in docs, comments or source.
- Use **independent expected values or a vanilla oracle**; never derive the answer from the code under test.
- **One test per changed behavior**; none for a pure refactor or move.
- Make slow tests faster without removing cases or weakening coverage.
- **Never delete** parity harnesses/tests that pin vanilla behavior.

## 10. Report validated scope, never claim 100%

No finite corpus proves every vanilla behavior. Report **implemented/validated scope** with counts and
hashes (`vendor/horizon/docs/rust-parity.md` is the model). Do not label a partial port as 100% parity.

## 11. Implementation conventions

- Rust APIs return **typed errors** instead of Java exceptions.
- Sizes and indices use `usize`; raw decoded values use `u32` bit patterns.
- **Math and RNG mirror Java exactly**: use `mc-core` (`random`, `jmath`) so draws happen in vanilla's
  order. (Horizon defers Gaussian sampling for exactly this reason: Java `Math.log` differs from the
  platform logarithm.)
- Keep user-facing text **translation-ready** (keys/strings, never hardcoded English).
- Data-oriented hot paths; no per-tick allocation (`plan.md` section 7).

## 12. Agent workflow (we run AI agents)

- **Check nobody else is already on it** before starting.
- **Find code with the ledger/map**, not by reading whole files.
- **Compile cheaply** (`cargo check -p <crate>`); run `fmt`, `clippy`, tests once, at the end.
- **Don't wait in loops**; background long commands.
- **Mark agent-authored work.**
- **One test per changed behavior.**

## 13. Sources (vendored)

- Horizon `CONTRIBUTING.md` - "Vanilla first, then better", "When it doesn't have to be 1:1", PR kinds.
- Horizon `AGENTS.md` - rules that bite, tests with signal.
- Horizon `docs/layers.md` - engine / platform / Horizon, and where a new feature goes.
- Horizon `docs/rust-parity.md` - report validated scope; reproduction conventions.
- Horizon `docs/design-system.md` - game screens and HUD are vanilla's (parity is the point).
