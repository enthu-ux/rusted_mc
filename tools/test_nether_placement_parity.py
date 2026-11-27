#!/usr/bin/env python3
"""nether_placement_parity: does mc-worldgen write nether-fortress blocks like real 1.8.9?

Runs the game's own `MapGenNetherBridge.generateStructure` (server remapped jar, JDK 8) for the four
chunks whose `+8` `generateStructure` box covers the target chunk, and compares the target chunk's
blocks against the Rust probe (`crates/mc-worldgen/examples/nether_probe.rs`,
`NetherBridge::populate_chunk`).

The nether terrain is not ported, so both sides start from the same **synthetic** base chunk
(bedrock at y=0, netherrack y=1..=39, air above). Fortress placement only reads the base through
`replaceAirAndLiquidDownwards`; the nether bodies never call `isLiquidInStructureBoundingBox`, so no
piece is dropped.

RNG: `ChunkProviderHell.populate` uses the shared `hellRNG`, which `provideChunk` seeds with
`cx * 341873128712 + cz * 132897987541` and `func_180516_b` advances by a fixed pattern. The oracle
and the port replay that pattern (`hell_populate_rand`). The harness also compares the first 8 draws
after the replay for a fixed chunk list, because the block dump cannot see this stream (only the
corridor chest *contents* draw from it). Vanilla's real state also depends on chunk-load history (an
already-loaded chunk is not re-seeded); the corpus uses the canonical generate-then-populate sequence.

Scope/guards:
  - **order-independence**: the nether pieces never remove themselves, so the corpus must give the
    same chunk with the passes reversed (`NETHER_REVERSE`).
  - **no block-update effects**: `Entrance` forces a flowing-lava tick; the oracle neutralises
    `forceBlockUpdateTick` (block-update semantics are outside the placement port).
  - **signal**: at least one nether structure block across the corpus.

Format per chunk: `seed <seed> chunk <cx> <cz>`, then 256 lines `x z v0 ... v255`
(`vi = blockId * 16 + metadata`).

Outputs are cached under .local/nether/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "NetherPlacementOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "nether"
ORACLE_CLASSES = WORK / "oracle-classes"

# Fortress origins (their own chunk reports the whole start box). Each entry: (seed, chunks).
CORPUS = [
    ("0", [(-38, -39), (-24, 24)]),
    ("12345", [(-25, -41), (-7, -10)]),
    ("-7", [(-12, -39), (4, -21)]),
]

# nether_brick, nether_brick_fence, nether_brick_stairs, soul_sand, nether_wart, mob_spawner, chest,
# flowing_lava. The synthetic base is bedrock (7) + netherrack (87), which is excluded.
STRUCTURE_BLOCKS = {112, 113, 114, 88, 115, 52, 54, 10}

# The block dump cannot see the `hellRNG` stream (only chest contents consume it), so the oracle's
# `rng` mode and `nether_rng_probe` pin the first 8 draws after the replay for these chunks too.
RNG_CHUNKS = [(0, 0), (-1, -1), (-38, -39), (-24, 24), (100, -50), (512, 512)]


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    e = dict(os.environ)
    e.pop("NETHER_REVERSE", None)
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=e)


def chunk_args(chunks: list[tuple[int, int]]) -> list[str]:
    return [str(v) for c in chunks for v in c]


def oracle(seed: str, chunks: list[tuple[int, int]], reverse: bool) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    env = {"NETHER_REVERSE": "1"} if reverse else None
    r = run(["java", "-cp", cp, "NetherPlacementOracle", seed] + chunk_args(chunks), env)
    if r.returncode != 0:
        raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


def oracle_rng(chunks: list[tuple[int, int]]) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "NetherPlacementOracle", "rng"] + chunk_args(chunks))
    if r.returncode != 0:
        raise RuntimeError("java rng oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


def probe_rng(chunks: list[tuple[int, int]]) -> str:
    r = run(
        [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "nether_rng_probe", "--"]
        + chunk_args(chunks)
    )
    if r.returncode != 0:
        raise RuntimeError("cargo rng probe failed:\n" + r.stdout + r.stderr)
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

    # Pin the placement RNG stream itself (the block dump only sees deterministic writes).
    try:
        vanilla_rng = oracle_rng(RNG_CHUNKS)
        rust_rng = probe_rng(RNG_CHUNKS)
    except RuntimeError as e:
        print(e)
        return 2
    if vanilla_rng != rust_rng:
        print("RNG MISMATCH (hell_populate_rand):")
        for a, b in zip(vanilla_rng.splitlines(), rust_rng.splitlines()):
            if a != b:
                print(f"  vanilla: {a}\n  rust:    {b}")
                break
        return 1
    rng_draws = 8 * len(RNG_CHUNKS)

    total_lines = 0
    total_chunks = 0
    blocks = 0
    for seed, chunks in CORPUS:
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
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "nether_probe", "--",
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
        f"OK: {total_lines} lines match ({len(CORPUS)} fixtures x {total_chunks} chunks, "
        f"{blocks} structure blocks; hellRNG {rng_draws} draws)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())