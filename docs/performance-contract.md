# Performance architecture contract

Status: **P0 in progress.** This is the concrete form of `plan.md` §7/§13 (the "architecture
contract" that was deferred until M0/M1). It exists so performance work is designed and measured
from day one, not retrofitted.

Vanilla 1.8.9 remains the behavioral spec. Performance may differ **inside** (data layout,
threading, allocation) as long as observable behavior is unchanged and the module passes both
gates: **parity** + **budget**.

## 1. Measurement principles

- **Normalized only.** Same machine, world, path, settings, render distance, and time of day; Java
  1.8.9 vanilla **and** vanilla + OptiFine are the baselines. 1.8.9 is a lighter game than 26.3, so
  Horizon's raw numbers are not our targets.
- **Distributions, not averages.** Report p50 / p95 / p99 and max; 1% / 0.1% lows for frames.
- **In-process where it matters.** Process wall time includes startup/JIT; use in-process spans for
  tick, chunk generation, meshing, lighting, and protocol.
- **Counts beat vibes.** Instruction and allocation counts (Callgrind / alloc counters) on hot
  paths, per `plan.md` §6.6.
- **Quiet machine, repeats, medians.** A run under load is discarded, not averaged.
- **Every number is reproducible**: the harness records commit, OS, CPU, and fixture hashes.

## 2. Instrumentation

- `mc-perf` (ours, dependency-free): spans, wall-clock `Stopwatch`, and `Stats`
  (min/median/p95/max). Spans are **zero-cost when disabled** (`MC_SPANS`).
- Adopt Horizon's `horizon-perf` / `horizon-perf-format` (`.hzperf` captures) and
  `horizon-bench` once a runtime exists (P0 later slice; see
  `docs/horizon-performance-reuse.md`).
- `tools/perf_baseline.py` records baselines under `.local/perf/` (gitignored).

## 3. Budgets

Two tiers (from `plan.md` §13):

- **Absolute floors (hard gates):** 20 TPS (≤ 50 ms/tick) at 50 players on the reference server;
  zero allocations per tick in hot paths; zero per-section allocations in the meshing inner loop;
  no frame over 33 ms on the standard benchmark path (settled world).
- **Relative to Java 1.8.9:** client ≥ 3× vanilla FPS and ≥ 2× vanilla+OptiFine; ≥ 5× fewer long
  stutters; memory ≤ 50% of vanilla's RSS; server holds 20 TPS where vanilla drops below it.

Per-module budgets (set from measured baselines; `TBD` = filled when the harness can measure it):

| Area | Metric | Budget |
|---|---|---|
| Server tick | ms/tick @ 50 players | ≤ 50 (hard); p99 recorded |
| Worldgen | ms/chunk (base terrain, no caves) | baseline recorded (§6); target set from it |
| Worldgen | allocations/chunk (hot path) | 0 after warm scratch |
| Meshing | ms/section | TBD (M3) |
| Meshing | allocations/section (inner loop) | 0 |
| Lighting | ms/relight (edited section) | no whole-chunk relight; TBD |
| Lighting | allocations/relight | 0 |
| Protocol | allocations/packet encode | 0 in inner loop |
| Startup | cold start to first frame | TBD (M3); beat vanilla |
| World join | load/generate to playable | TBD (M3); bounded in-flight band |
| Memory | RSS vs vanilla | ≤ 50% |

## 4. Rules for hot paths

- **SoA / flat arrays**, arenas and indices instead of `Rc<RefCell<_>>`/pointer trees.
- **No per-tick / per-section allocation**; scratch buffers reused (`mc-perf` helps find churn).
- **Parallelism only where order-independent** (rendering, meshing, lighting, I/O), with
  propose-parallel / commit-serial and a digest in debug.
- **The tick is deterministic and single-threaded.**
- **Never** trade observable behavior for speed; anything a player can notice is gated or a
  registered deviation (`plan.md` §16).

## 5. LGPL placement (Horizon reuse)

Copied/adapted Horizon code is LGPL-3.0 and lives **only** in clearly-marked `mc-horizon-*`
modules (or `vendor/`), with notices; our `mc-core`/`mc-worldgen`/etc. stay ours. **Parity-critical
math never comes from Horizon** (26.3 semantics). P0 copies no code yet; this is the layout we will
use when a module is adapted.

## 6. Recorded baselines

`tools/perf_baseline.py` (Rust `terrain_bench` vs Java `TerrainBench`, same seeds/chunks, release
build, JIT warm-up, in-process timings). First run: see `docs/performance-baselines.md`.

Caveat: at this stage the two sides are the *base terrain* only (no caves/structures/decoration),
so this is a starting reference, not the M1 gate.