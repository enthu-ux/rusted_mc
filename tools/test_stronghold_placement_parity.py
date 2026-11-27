#!/usr/bin/env python3
"""stronghold_placement_parity: does mc-worldgen write stronghold blocks like real 1.8.9?

Runs the game's own `MapGenStronghold.generateStructure` (server remapped jar, JDK 8) for the four
chunks whose `+8` `generateStructure` box covers the target chunk, and compares the target chunk's
blocks against the Rust probe (`crates/mc-worldgen/examples/stronghold_probe.rs`,
`TerrainGen::populate_stronghold_chunk`).

Covers all reachable piece kinds (`RightTurn`/`SHRT` is never produced by the layout; `Stairs2`
reuses `Stairs`) plus the `Stones` selector, the four `EntryDoor` kinds, the end-portal frames and
the stronghold chests. Every body except `PortalRoom` starts with `isLiquidInStructureBoundingBox`
(a `true` drops the piece) - real vanilla behavior for an underground structure.

A piece can be dropped by the first pass that meets it (the liquid box is clipped to the pass box),
so placement can be order-dependent. Our port commits to a fixed pass order, so the harness compares
against the oracle run in that same order; order-independent fixtures also require
forward==reverse (`STRONGHOLD_REVERSE`).

Scope: the oracle's `TestWorld` is a *remote* world, so `Chunk.setBlockState` skips `onBlockAdded`
(torch support, door neighbours) - block-update semantics are outside the placement port.

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Outputs are cached under .local/stronghold/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle, Probe

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "StrongholdPlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "stronghold"
ORACLE = Oracle("stronghold", ORACLE_SRC, "StrongholdPlacementOracle", work=WORK, jar=SERVER_JAR)
PROBE = Probe("stronghold", "mc-worldgen", "stronghold_probe")

# The 3 ring positions per seed (the origin chunk reports its own stronghold).
CORPUS = [
    ("0", [(-37, 39), (-9, -41), (67, 32)], True),
    ("12345", [(-44, 59), (-16, -38), (61, 18)], True),
    ("-7", [(-44, -34), (-7, 61), (61, -19)], True),
]

# Stronghold-only blocks: stonebrick (+mossy/cracked), monster egg, iron bars, stone slab,
# double stone slab, bookshelf, torch, planks, cobblestone, oak fence, ladder, doors, stone button,
# web, chest, mob spawner, end portal frame, stone stairs, stone brick stairs.
STRUCTURE_BLOCKS = {
    98, 97, 101, 44, 43, 47, 50, 5, 4, 85, 65, 64, 71, 77, 30, 54, 52, 120, 67, 109,
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
    e.pop("STRONGHOLD_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    return ORACLE.run(
        [seed] + chunk_args(chunks), env={"STRONGHOLD_REVERSE": "1" if reverse else None}
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
            print(f"note: seed {seed} chunks {chunks}: order-dependent (port uses its fixed pass order)")

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