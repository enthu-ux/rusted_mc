#!/usr/bin/env python3
"""temple_placement_parity: does mc-worldgen write temple blocks like real 1.8.9?

Runs the game's own `MapGenScatteredFeature.generateStructure` (server remapped jar, JDK 8) for
`populate`'s scattered-feature pass - the four chunks whose `generateStructure` box (offset +8,
spanning two chunks per axis) covers the target chunk - and compares the target chunk's blocks
against the Rust probe (`crates/mc-worldgen/examples/temple_probe.rs`,
`TerrainGen::populate_scattered_chunk`).

Covers the swamp hut, the desert pyramid and the jungle pyramid. The desert/jungle pyramids differ:
the desert's chest rolls happen after all its block writes, so its chunk is order-independent; the
jungle pyramid's main-chest `generateChestContents` runs *before* later writes, so its book draw
shifts the following stone-selector draws - the chunk is genuinely order-dependent in vanilla (the
real order is chunk-load order). Our port commits to a fixed pass order, so the jungle is compared
against the oracle run in that same order; only the order-independent fixtures also require
forward==reverse.

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Scope/guards:
  - **order-independence**: for the order-independent fixtures a temple's `func_74935_a` offsets its
    box on the first placement pass (state carries across passes), so the corpus must give the same
    chunk with the passes reversed (`TEMPLE_REVERSE`). The jungle fixture is exempt (see above).
  - **no `onBlockAdded` effects**: `Chunk.setBlockState` calls `Block.onBlockAdded`; block-update
    side effects are outside the placement port, so the corpus keeps fixtures that do not trigger
    them (the jungle's dispensers/tripwire/redstone happen to be no-ops here).
  - **signal**: at least one structure block across the corpus.

Outputs are cached under .local/temple/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle, Probe

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "TemplePlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "temple"
ORACLE = Oracle("temple", ORACLE_SRC, "TemplePlacementOracle", work=WORK, jar=SERVER_JAR)
PROBE = Probe("temple", "mc-worldgen", "temple_probe")

# Temple origins (their own chunk): swamp hut seed 0 (3,47), seed 12345 (-26,49); desert pyramid
# seed -7 (4,2); jungle pyramid seed 12345 (72,-155). Each entry: (seed, chunks, order_independent).
CORPUS = [
    ("0", [(3, 47)], True),
    ("12345", [(-26, 49)], True),
    ("-7", [(4, 2)], True),
    ("12345", [(72, -155)], False),
]
# Swamp-hut blocks (`planks`, `log`, `oak_fence`, `flower_pot`, `crafting_table`, `cauldron`,
# `spruce_stairs`), desert-pyramid signatures (`sandstone_stairs`, stained hardened clay,
# `stone_slab`, `stone_pressure_plate`, `tnt`, `chest`), and the jungle-pyramid set (`cobblestone`,
# `mossy_cobblestone`, `stone_stairs`, `tripwire_hook`, `tripwire`, `redstone_wire`, `vine`,
# `stonebrick`, `lever`, `sticky_piston`, `unpowered_repeater`, `dispenser`).
STRUCTURE_BLOCKS = {
    5, 17, 85, 140, 58, 118, 134, 128, 159, 44, 70, 46, 54,
    4, 48, 67, 131, 132, 55, 106, 98, 69, 29, 93, 23,
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
    e.pop("TEMPLE_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    return ORACLE.run(
        [seed] + chunk_args(chunks), env={"TEMPLE_REVERSE": "1" if reverse else None}
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
            if order_independent:
                reversed_out = oracle(seed, chunks, reverse=True)
        except RuntimeError as e:
            print(e)
            return 2

        if order_independent and vanilla != reversed_out:
            print(f"FIXTURE ERROR: seed {seed}: the corpus is order-dependent (forward vs reverse)")
            return 1

        try:
            rust = PROBE.run([seed] + chunk_args(chunks))
        except RuntimeError as e:
            print(e)
            return 2

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