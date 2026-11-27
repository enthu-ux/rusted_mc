#!/usr/bin/env python3
"""jhash_parity: does mc-core's JavaHashMap reproduce `java.util.HashMap` iteration order?

Runs the real `HashMap` (system JDK) over fixed insertion sequences and compares `keySet()` order
against `crates/mc-core/examples/jhash_probe.rs`.

Why it matters: vanilla worldgen reads `MapGenStructure.structureMap` (Long chunk keys) and
`EnchantmentHelper.mapEnchantmentData` (Integer enchantment ids) in HashMap order, and that order
decides which structure draws first from the shared `populate` random and which enchantment a chest
book rolls - both observable in the generated blocks.

Covers: the first resize (12 -> 32), several resizes (0..64), bucket collisions (multiples of 16),
negative keys, and Long chunk keys (`chunkXZ2Int`).
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "HashOrderOracle.java"
WORK = ROOT / ".local" / "jhash"
ORACLE = Oracle("jhash", ORACLE_SRC, "HashOrderOracle", work=WORK)


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def chunk_key(x: int, z: int) -> int:
    """`ChunkCoordIntPair.chunkXZ2Int`: `(long)x & 0xFFFFFFFF | ((long)z & 0xFFFFFFFF) << 32`."""
    value = (x & 0xFFFFFFFF) | ((z & 0xFFFFFFFF) << 32)
    return value - (1 << 64) if value >= 1 << 63 else value


def cases() -> list[list[str]]:
    out = []
    for n in (1, 5, 12, 13, 24, 25, 64):
        out.append(["int"] + [str(i) for i in range(n)])

    # Insertion orders that collide in a bucket (hash & 15 equal), reversed and shuffled.
    out.append(["int"] + [str(16 * i) for i in range(20)])
    out.append(["int"] + [str(16 * i) for i in reversed(range(20))])
    out.append(["int"] + [str(v) for v in (17, 1, 33, 49, 2, 18, 34, 50, 3, 19, 35, 51)])

    # Negative keys and Integer.MIN/MAX.
    out.append(["int"] + ["-1", "-16", "-17", "-2147483648", "2147483647", "0", "1"])
    out.append(["int"] + [str(-i) for i in range(1, 26)])

    # Enchantment-id-like set: 0..N in enrolment order, and a sparse subset.
    out.append(["int"] + [str(i) for i in range(36)])
    out.append(["int"] + [str(i) for i in (0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 32, 34, 35)])

    # Long chunk keys around the origin and far out. (Big windows of chunk keys collide heavily and
    # make Java *treeify* a bin at capacity >= 64; that iteration order comes from the tree and is
    # not modelled - see `jhash.rs`. Vanilla's own multi-start placement is history-dependent for the
    # same reason, so nothing observable relies on it.)
    coords = [(0, 0), (1, 0), (0, 1), (-1, -1), (8, 8), (-37, 210), (512, 512), (100, -50)]
    out.append(["long"] + [str(chunk_key(x, z)) for x, z in coords])
    out.append(
        ["long"]
        + [str(v) for v in (
            0, 1, 4294967297, 8589934594, 4294967296, -1, -4294967296, 2, 8589934592,
            12884901889, 12884901888, 30064771072,
        )]
    )
    return out


def main() -> int:
    WORK.mkdir(parents=True, exist_ok=True)

    checked = 0
    keys = 0
    for args in cases():
        try:
            vanilla = ORACLE.run(args).strip()
        except RuntimeError as e:
            print(e)
            return 2

        r = run([find_cargo(), "run", "-q", "-p", "mc-core", "--example", "jhash_probe", "--"] + args)
        if r.returncode != 0:
            print("cargo probe failed:\n" + r.stdout + r.stderr)
            return 2
        rust = r.stdout.strip()

        if not vanilla and len(args) > 1:
            print(f"FIXTURE ERROR: empty oracle output for {args[:3]}...")
            return 1
        if vanilla != rust:
            print(f"MISMATCH ({args[0]} keys, {len(args) - 1} inserted):")
            print(f"  vanilla: {vanilla}")
            print(f"  rust:    {rust}")
            return 1

        checked += 1
        keys += len(args) - 1

    print(f"OK: {checked} insertions orders match across {keys} keys")
    return 0


if __name__ == "__main__":
    sys.exit(main())