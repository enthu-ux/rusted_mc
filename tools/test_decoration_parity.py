#!/usr/bin/env python3
"""decoration_parity: does mc-worldgen's decorated chunk match real 1.8.9?

Runs the game's own `ChunkProviderGenerate` for terrain, then a *guarded* mirror of
`BiomeDecorator.genDecorations` for the layers named by the mode
(`oracle/java/src/DecorationOracle.java`), and compares the target chunk's blocks against the Rust
probe (`crates/mc-worldgen/examples/decoration_probe.rs`).

Decoration is `populate`'s `+8`-offset 16x16 window, so the target chunk is the union of the four
covering passes; both sides run the same fixed pass order (vanilla's is chunk-load order).

Modes: ores | surface | plants | trees | springs | ore_surface | full.

`full` runs the real `BiomeGenBase.decorate` on the game side (genDecorations + the per-biome
`decorate` overrides) and the whole `decorate` on ours, so it gates the override layer end-to-end.
Its corpus includes a jungle (melon), ice-spikes (packed ice), mega taiga (boulders), desert well,
hills (emerald/monster egg) and plains/forest/savanna (double plants) chunk per seed; a guard
requires every override signature to appear. The fixture `seed -7 chunk (-48,-48)` (a lava spring
next to water) stays out of `full` too: its `checkForMixing` cascade is a block-update effect (see
`handoff.md`), not decoration.

Signal guard: the corpus must actually contain a decorated ore block, else the fixture proves nothing.
Outputs cached under .local/decoration/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

from _parity_cache import Oracle, Probe

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "DecorationOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "decoration"
ORACLE = Oracle("decoration", ORACLE_SRC, "DecorationOracle", work=WORK, jar=SERVER_JAR)
PROBE = Probe("decoration", "mc-worldgen", "decoration_probe")

SEEDS = ["0", "12345", "-7"]
CHUNKS = [(0, 0), (100, -50), (-37, 210), (-3, 0), (-48, -48), (6, 5)]
# `springs`: (-48, -48) at seed -7 places a lava spring next to water, whose `checkForMixing`
# obsidian/cobblestone conversion uses a flag-3 set that cascades into the neighbouring static
# lava (a block-update cascade, cf. mineshaft `onBlockAdded`). Spring *placement* is unaffected,
# so that fixture is kept out of this mode's corpus until the block-update subsystem is ported.
SPRING_CHUNKS = [(0, 0), (100, -50), (-37, 210), (-3, 0), (6, 5)]
MODES = ["ores", "surface", "plants", "trees", "springs", "ore_surface", "full"]
# Per-seed corpus for the `full` mode: the base chunks plus overrides fixtures (jungle -> melon,
# ice spikes -> packed ice, mega taiga -> boulders, desert well, hills -> emerald/monster egg,
# plains/forest/savanna -> double plants). `-48,-48` is kept except at seed -7 (lava-spring cascade).
FULL_CHUNKS = {
    "0": [(0, 0), (100, -50), (-37, 210), (-3, 0), (-48, -48), (6, 5),
          (-500, -260), (-207, -109)],
    "12345": [(0, 0), (100, -50), (-37, 210), (-3, 0), (-48, -48), (6, 5)],
    "-7": [(0, 0), (100, -50), (-37, 210), (-3, 0), (6, 5),
           (-124, -127), (-118, -127), (-487, -112), (-500, -296)],
}
ORE_IDS = {14, 15, 16, 21, 56, 73}
PLANT_IDS = {31, 32, 37, 38, 39, 40, 81, 83, 86, 99, 100, 111}
TREE_IDS = {17, 18, 161, 162, 106}
# Each vegetation generator the corpus must actually exercise (else the fixture proves nothing).
PLANT_SIGNATURE = {
    "tall grass": {31},
    "flower": {37, 38},
    "small mushroom (skylight)": {39, 40},
    "cactus": {81},
    "dead bush": {32},
}
# Each per-biome `decorate` override the `full` corpus must actually exercise.
OVERRIDE_SIGNATURE = {
    "double plant": {175},
    "melon (jungle)": {103},
    "packed ice (ice spikes)": {174},
    "mossy cobblestone (taiga boulders)": {48},
    "stone slab (desert well)": {44},
    "emerald ore (hills)": {129},
    "monster egg (hills)": {97},
}


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def inner_jobs() -> int:
    """How many (seed, mode) pairs to run at once; `parity.py` sets this to avoid oversubscription."""
    try:
        return max(1, int(os.environ.get("PARITY_INNER_JOBS", "1")))
    except ValueError:
        return 1


def oracle(seed: str, mode: str, chunks: list[tuple[int, int]]) -> str:
    return ORACLE.run([seed, mode] + chunk_args(chunks))


def rust(seed: str, mode: str, chunks: list[tuple[int, int]]) -> str:
    return PROBE.run([seed, mode] + chunk_args(chunks))


def first_mismatch(vanilla: str, rust_out: str) -> str | None:
    v = vanilla.splitlines()
    r = rust_out.splitlines()
    if len(v) != len(r):
        return f"line-count mismatch: vanilla {len(v)} vs rust {len(r)}"
    header = None
    for l1, l2 in zip(v, r):
        if l1.startswith("seed "):
            header = l1
            if l1 != l2:
                return f"header mismatch:\n  vanilla: {l1}\n  rust:    {l2}"
            continue
        if l1 == l2:
            continue
        p1, p2 = l1.split(), l2.split()
        for y, (a, b) in enumerate(zip(p1[2:], p2[2:])):
            if a != b:
                return (
                    f"{header}\n  block x={p1[0]} z={p1[1]} y={y}: "
                    f"vanilla {a} (id {int(a) >> 4} meta {int(a) & 15}) "
                    f"vs rust {b} (id {int(b) >> 4} meta {int(b) & 15})"
                )
        return f"{header}\n  column {p1[0]} {p1[1]} differs"
    return None


def ore_count(text: str) -> int:
    return block_count(text, ORE_IDS)


def plant_count(text: str) -> int:
    return block_count(text, PLANT_IDS)


def block_count(text: str, ids: set[int]) -> int:
    return len(_block_ids(text) & ids)


def _block_ids(text: str) -> set[int]:
    seen: set[int] = set()
    for line in text.splitlines():
        if line.startswith("seed ") or len(line.split()) < 3:
            continue
        for v in line.split()[2:]:
            seen.add(int(v) >> 4)
    return seen


def spring_count(text: str) -> int:
    """The spring flow blocks: flowing water level 1 (id 8) or overworld lava level 2 (id 10)."""
    n = 0
    for line in text.splitlines():
        if line.startswith("seed ") or len(line.split()) < 3:
            continue
        for v in line.split()[2:]:
            i = int(v)
            if i == (8 << 4) | 1 or i == (10 << 4) | 2:
                n += 1
    return n


def main() -> int:
    if not SERVER_JAR.exists():
        print(f"missing {SERVER_JAR}; run the decompile/remap step first")
        return 3

    WORK.mkdir(parents=True, exist_ok=True)

    pairs: list[tuple[str, str, list[tuple[int, int]]]] = []
    for mode in MODES:
        for seed in SEEDS:
            if mode == "springs":
                chunks = SPRING_CHUNKS
            elif mode == "full":
                chunks = FULL_CHUNKS[seed]
            else:
                chunks = CHUNKS
            pairs.append((mode, seed, chunks))

    def compute(pair: tuple[str, str, list[tuple[int, int]]]) -> tuple[str, str]:
        mode, seed, chunks = pair
        return oracle(seed, mode, chunks), rust(seed, mode, chunks)

    jobs = inner_jobs()
    if jobs > 1:
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            outputs = list(pool.map(compute, pairs))
    else:
        outputs = [compute(pair) for pair in pairs]

    total_lines = 0
    plant_seen: set[int] = set()
    override_seen: set[int] = set()
    for (mode, seed, _chunks), (vanilla, rust_out) in zip(pairs, outputs):
        if "plant" in mode:
            plant_seen |= _block_ids(vanilla)
        if mode == "full":
            override_seen |= _block_ids(vanilla)

        if "ore" in mode and ore_count(vanilla) == 0:
            print(f"FIXTURE ERROR: mode {mode} seed {seed}: no ore blocks placed")
            return 1
        if "plant" in mode and plant_count(vanilla) == 0:
            print(f"FIXTURE ERROR: mode {mode} seed {seed}: no plant blocks placed")
            return 1
        if "tree" in mode and block_count(vanilla, TREE_IDS) == 0:
            print(f"FIXTURE ERROR: mode {mode} seed {seed}: no tree blocks placed")
            return 1
        if "spring" in mode and spring_count(vanilla) == 0:
            print(f"FIXTURE ERROR: mode {mode} seed {seed}: no spring blocks placed")
            return 1

        (WORK / f"vanilla-{mode}-{seed}.txt").write_text(vanilla, encoding="utf-8")
        (WORK / f"rust-{mode}-{seed}.txt").write_text(rust_out, encoding="utf-8")

        problem = first_mismatch(vanilla, rust_out)
        if problem is not None:
            print(f"MISMATCH (mode {mode}, seed {seed}):")
            print("  " + problem)
            return 1
        total_lines += len(vanilla.splitlines())

    for name, ids in PLANT_SIGNATURE.items():
        if not (plant_seen & ids):
            print(f"FIXTURE ERROR: the corpus never places a {name}; add a chunk that exercises it")
            return 1

    for name, ids in OVERRIDE_SIGNATURE.items():
        if not (override_seen & ids):
            print(f"FIXTURE ERROR: the full corpus never places a {name}; add a chunk that exercises it")
            return 1

    print(f"OK: {total_lines} lines match ({len(MODES)} modes x {len(SEEDS)} seeds; "
          f"{len(CHUNKS)} chunks, springs {len(SPRING_CHUNKS)}, full per-seed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
