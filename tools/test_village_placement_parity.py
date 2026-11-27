#!/usr/bin/env python3
"""village_placement_parity: does mc-worldgen write village blocks like real 1.8.9?

Runs the game's own `MapGenVillage.generateStructure` (server remapped jar, JDK 8) for the four
chunks whose `+8` `generateStructure` box covers the target chunk, and compares the target chunk's
blocks against the Rust probe (`crates/mc-worldgen/examples/village_probe.rs`,
`TerrainGen::populate_village_chunk`).

Covers every village piece kind plus the desert material substitution (`func_175847_a`). A piece
caches its `HPos` (average ground level) on the first pass that meets it, so placement can be
order-dependent in vanilla (the real order is chunk-load order). Our port commits to a fixed pass
order, so the harness compares against the oracle run in that same order; order-independent fixtures
also require forward==reverse (`VILLAGE_REVERSE`).

Scope/guards:
  - **no block-update effects**: the oracle's `TestWorld` is a *remote* world, so
    `Chunk.setBlockState` skips `onBlockAdded` (torch support, furnace facing, chest facing) and
    `notifyBlockOfStateChange` is a no-op. Block-update semantics are outside the placement port.
  - **signal**: at least one village block across the corpus.

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Outputs are cached under .local/village/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "VillagePlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "village"
ORACLE = Oracle("village", ORACLE_SRC, "VillagePlacementOracle", work=WORK, jar=SERVER_JAR)

# Village origins (their own chunk reports the start). Each entry: (seed, chunks, order_independent).
CORPUS = [
    # This village's pieces span several pass boxes and cache different `HPos` values depending on
    # which pass meets them first; the port uses its fixed order (the oracle runs it the same way).
    ("0", [(-53, -16)], False),
    # desert village (exercises `func_175847_a`; origin (-210,-151))
    ("0", [(-211, -151)], True),
    ("12345", [(-47, -26), (-43, 5)], True),
    ("-7", [(34, 1), (37, -12)], True),
]

# Village blocks that do not occur in un-decorated base terrain: cobblestone, planks, log, wool,
# double_stone_slab, stone_slab, bookshelf, torch, oak_stairs, chest, crafting_table, wheat, farmland,
# furnace, oak_door, ladder, stone_stairs, wooden_pressure_plate, oak_fence, iron_bars, glass_pane,
# sandstone_stairs, carrots, potatoes.
STRUCTURE_BLOCKS = {
    4, 5, 17, 35, 43, 44, 47, 50, 53, 54, 58, 59, 60, 61, 64, 65, 67, 72, 85, 101, 102, 128, 141, 142,
}


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    e = dict(os.environ)
    e.pop("VILLAGE_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    return ORACLE.run(
        [seed] + chunk_args(chunks), env={"VILLAGE_REVERSE": "1" if reverse else None}
    )


def first_mismatch(vanilla: str, rust: str) -> str | None:
    vl, rl = vanilla.splitlines(), rust.splitlines()
    header = "?"
    for i in range(min(len(vl), len(rl))):
        if vl[i].startswith("seed "):
            header = vl[i]
        if vl[i] != rl[i]:
            if vl[i].startswith("seed ") or rl[i].startswith("seed "):
                return f"line {i + 1}: chunk list differs\n  vanilla: {vl[i]}\n  rust:    {rl[i]}"
            p1, p2 = vl[i].split(), rl[i].split()
            for y, (a, b) in enumerate(zip(p1[2:], p2[2:])):
                if a != b:
                    return (
                        f"{header}\n  block x={p1[0]} z={p1[1]} y={y}: "
                        f"vanilla {a} (id {int(a) >> 4} meta {int(a) & 15}) "
                        f"vs rust {b} (id {int(b) >> 4} meta {int(b) & 15})"
                    )
            return f"{header}\n  column {p1[0]} {p1[1]} differs"
    if len(vl) != len(rl):
        return f"line count differs: vanilla {len(vl)} vs rust {len(rl)}"
    return None


def structure_block_count(text: str) -> int:
    total = 0
    for line in text.splitlines():
        if line.startswith("seed "):
            continue
        for v in line.split()[2:]:
            if (int(v) >> 4) in STRUCTURE_BLOCKS:
                total += 1
    return total


def main() -> int:
    if not SERVER_JAR.exists():
        print(f"missing {SERVER_JAR}; run the decompile/remap step first")
        return 3

    WORK.mkdir(parents=True, exist_ok=True)

    total_lines = 0
    total_chunks = 0
    blocks = 0
    for seed, chunks, order_independent in CORPUS:
        try:
            vanilla = oracle(seed, chunks, reverse=False)
            reversed_out = oracle(seed, chunks, reverse=True)
        except RuntimeError as e:
            print(e)
            return 2

        independent = vanilla == reversed_out
        if order_independent and not independent:
            print(f"FIXTURE ERROR: seed {seed}: the corpus is order-dependent (forward vs reverse)")
            return 1
        if not independent:
            print(f"note: seed {seed} chunk {chunks}: order-dependent (port uses its fixed pass order)")

        r = run(
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "village_probe", "--",
             seed] + chunk_args(chunks)
        )
        if r.returncode != 0:
            print("cargo probe failed:\n" + r.stdout + r.stderr)
            return 2
        rust = r.stdout

        (WORK / f"vanilla-{seed}-{chunks[0][0]}_{chunks[0][1]}.txt").write_text(
            vanilla, encoding="utf-8"
        )
        (WORK / f"rust-{seed}-{chunks[0][0]}_{chunks[0][1]}.txt").write_text(
            rust, encoding="utf-8"
        )

        problem = first_mismatch(vanilla, rust)
        if problem is not None:
            print(f"MISMATCH (seed {seed}):")
            print("  " + problem)
            return 1

        total_lines += len(vanilla.splitlines())
        total_chunks += len(chunks)
        blocks += structure_block_count(rust)

    if blocks == 0:
        print("FIXTURE ERROR: no structure blocks placed; the corpus has no signal")
        return 1

    print(
        f"OK: {total_lines} lines match ({len(CORPUS)} fixtures x {total_chunks} chunks, "
        f"{blocks} structure blocks)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())