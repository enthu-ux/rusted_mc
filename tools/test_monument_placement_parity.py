#!/usr/bin/env python3
"""monument_placement_parity: does mc-worldgen write ocean-monument blocks like real 1.8.9?

Runs the game's own `StructureOceanMonument.generateStructure` (server remapped jar, JDK 8) for
`populate`'s monument pass - the four chunks whose `generateStructure` box (offset +8, spanning two
chunks per axis) covers the target chunk - and compares the target chunk's blocks against the Rust
probe (`crates/mc-worldgen/examples/monument_probe.rs`, `TerrainGen::populate_monument_chunk`).

`MonumentBuilding` rebuilds its room graph deterministically from `(seed, start chunk)`; the room
parts use the per-pass `Random` (`SimpleRoom`/`SimpleTopRoom`), so a fixture must give the same chunk
with the passes reversed (`MONUMENT_REVERSE`).

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Outputs are cached under .local/monument/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "MonumentPlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "monument"
ORACLE_CLASSES = WORK / "oracle-classes"

# Monument start chunks (the building spans ~4x4 chunks around each).
CORPUS = {
    "0": [(77, -50)],
    "12345": [(-695, 458)],
    "-7": [(-28, 21)],
}
# Monument signatures: prismarine (168: rough/bricks/dark), sea lantern (169), gold block (41),
# sponge (19), water (9).
STRUCTURE_BLOCKS = {168, 169, 41, 19}


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    e = dict(os.environ)
    e.pop("MONUMENT_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    env = {"MONUMENT_REVERSE": "1"} if reverse else None
    r = run(["java", "-cp", cp, "MonumentPlacementOracle", seed] + chunk_args(chunks), env)
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
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "monument_probe", "--",
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