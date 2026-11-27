#!/usr/bin/env python3
"""mineshaft_placement_parity: does mc-worldgen write mineshaft blocks like real 1.8.9?

Runs the game's own `MapGenMineshaft.generateStructure` (server remapped jar, JDK 8) for `populate`'s
mineshaft pass - the four chunks whose `generateStructure` box (offset +8, spanning two chunks per
axis) covers the target chunk - and compares the target chunk's blocks against the Rust probe
(`crates/mc-worldgen/examples/mineshaft_probe.rs`, `TerrainGen::populate_mineshaft_chunk`).

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Scope/guards:
  - **single-start-per-box**: vanilla's `MapGenStructure.structureMap` is a `HashMap`, so when two
    starts meet one box the placement RNG is consumed in a history-dependent order. Fixtures where
    every one of the four boxes meets exactly one start are well defined (checked in the oracle).
  - **order-independent**: the four passes carry corridor state (`spawnerPlaced`, removal), so the
    corpus must give the same chunk when the passes are run in reverse (`MS_REVERSE`).
  - **no `onBlockAdded` effects**: `Chunk.setBlockState` calls `Block.onBlockAdded`, so vanilla
    reshapes rails (`BlockRailBase.updateState`) and drops unsupported torches
    (`BlockTorch.checkForDrop`) as it places them - block-update semantics outside the placement port.
    The corpus excludes fixtures that trigger them.
  - **signal**: at least one structure block (planks/fence/rail/torch/web/spawner) across the corpus.

Outputs are cached under .local/mineshaft/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle, Probe

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "MineshaftPlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "mineshaft"
ORACLE = Oracle("mineshaft", ORACLE_SRC, "MineshaftPlacementOracle", work=WORK, jar=SERVER_JAR)
PROBE = Probe("mineshaft", "mc-worldgen", "mineshaft_probe")

# Single-start-per-box, order-independent fixtures (see the docstring). Mineshaft origins and
# chunks inside a shaft, across three seeds.
CORPUS = {
    "0": [(6, 12), (4, 10), (10, 16), (3, 9), (11, 17), (95, -45), (96, -44)],
    "12345": [(102, -45), (101, -46), (103, -44), (-51, 211), (-52, 210), (503, 510)],
    "-7": [(86, -31)],
}
# `Blocks.planks`, `oak_fence`, `rail`, `torch`, `web`, `mob_spawner`.
STRUCTURE_BLOCKS = {5, 66, 50, 30, 52, 85}


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    e = dict(os.environ)
    e.pop("MS_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    return ORACLE.run(
        [seed] + chunk_args(chunks), env={"MS_REVERSE": "1" if reverse else None}
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

        try:
            rust = PROBE.run([seed] + chunk_args(chunks))
        except RuntimeError as e:
            print(e)
            return 2

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
