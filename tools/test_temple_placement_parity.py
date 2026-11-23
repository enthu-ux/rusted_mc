#!/usr/bin/env python3
"""temple_placement_parity: does mc-worldgen write temple blocks like real 1.8.9?

Runs the game's own `MapGenScatteredFeature.generateStructure` (server remapped jar, JDK 8) for
`populate`'s scattered-feature pass - the four chunks whose `generateStructure` box (offset +8,
spanning two chunks per axis) covers the target chunk - and compares the target chunk's blocks
against the Rust probe (`crates/mc-worldgen/examples/temple_probe.rs`,
`TerrainGen::populate_scattered_chunk`).

Currently only the **swamp hut** has block placement; the desert and jungle pyramids are layout-only,
so the corpus is limited to swamp-hut chunks (the harness grows as those are ported).

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Scope/guards:
  - **order-independent**: a temple's `func_74935_a` offsets its box on the first placement pass
    (state carries across passes), so the corpus must give the same chunk with the passes reversed
    (`TEMPLE_REVERSE`).
  - **no `onBlockAdded` effects**: `Chunk.setBlockState` calls `Block.onBlockAdded`; block-update
    side effects are outside the placement port, so the corpus keeps fixtures that do not trigger
    them.
  - **signal**: at least one structure block across the corpus.

Outputs are cached under .local/temple/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "TemplePlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "temple"
ORACLE_CLASSES = WORK / "oracle-classes"

# Swamp-hut origins (their own chunk): seed 0 (3,47), seed 12345 (-26,49); desert pyramid: seed -7 (4,2).
CORPUS = {
    "0": [(3, 47)],
    "12345": [(-26, 49)],
    "-7": [(4, 2)],
}
# Swamp-hut blocks (`planks`, `log`, `oak_fence`, `flower_pot`, `crafting_table`, `cauldron`,
# `spruce_stairs`) + desert-pyramid signatures (`sandstone_stairs`, stained hardened clay,
# `stone_slab`, `stone_pressure_plate`, `tnt`, `chest`).
STRUCTURE_BLOCKS = {5, 17, 85, 140, 58, 118, 134, 128, 159, 44, 70, 46, 54}


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    e = dict(os.environ)
    e.pop("TEMPLE_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    env = {"TEMPLE_REVERSE": "1"} if reverse else None
    r = run(["java", "-cp", cp, "TemplePlacementOracle", seed] + chunk_args(chunks), env)
    if r.returncode != 0:
        raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


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
    ORACLE_CLASSES.mkdir(parents=True, exist_ok=True)

    r = run(["javac", "-cp", str(SERVER_JAR), "-d", str(ORACLE_CLASSES), str(ORACLE_SRC)])
    if r.returncode != 0:
        print("javac failed:\n" + r.stdout + r.stderr)
        return 2

    total_lines = 0
    total_chunks = 0
    blocks = 0
    for seed, chunks in CORPUS.items():
        try:
            vanilla = oracle(seed, chunks, reverse=False)
            reversed_out = oracle(seed, chunks, reverse=True)
        except RuntimeError as e:
            print(e)
            return 2

        if vanilla != reversed_out:
            print(f"FIXTURE ERROR: seed {seed}: the corpus is order-dependent (forward vs reverse)")
            return 1

        r = run(
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "temple_probe", "--",
             seed] + chunk_args(chunks)
        )
        if r.returncode != 0:
            print("cargo probe failed:\n" + r.stdout + r.stderr)
            return 2
        rust = r.stdout

        (WORK / f"vanilla-{seed}.txt").write_text(vanilla, encoding="utf-8")
        (WORK / f"rust-{seed}.txt").write_text(rust, encoding="utf-8")

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
        f"OK: {total_lines} lines match ({len(CORPUS)} seeds x {total_chunks} chunks, "
        f"{blocks} structure blocks)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())