#!/usr/bin/env python3
"""caves_parity: do mc-worldgen's cave and ravine passes match real 1.8.9?

Runs the game's own `ChunkProviderGenerate.provideChunk` (server remapped jar, JDK 8) for fixed
seeds/chunks with caves and/or ravines toggled (structures off) and compares the full block column
against the Rust probe (`crates/mc-worldgen/examples/cave_probe.rs`).

Four modes are compared:
    0 = caves only   1 = ravines only   2 = both   3 = neither (base terrain)

Mode 3 is redundant with `terrain_parity`, but it pins that the "carving off" path of the new
`generate_chunk` still produces the base terrain.

Signal guard: for every seed the vanilla caves-only and ravines-only output must actually differ
from the vanilla base terrain, otherwise the fixture proves nothing.

One fresh JVM per (seed, mode) - `BiomeGenMesa` caches its per-world noise on first use.
Outputs cached under .local/caves/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "CaveOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "caves"
ORACLE_CLASSES = WORK / "oracle-classes"

SEEDS = ["0", "12345", "-7"]
MODES = [0, 1, 2, 3]
CHUNKS = [
    (-3, 0), (-1, 0), (-2, -1), (7, -5), (-5, 1), (6, 5), (-124, -128),
    (36, -163), (100, -52), (-3, 3), (2, 1), (-6, 47), (-169, 30), (-47, 47),
    (-7, 7),
]


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def chunk_args() -> list[str]:
    return [str(v) for c in CHUNKS for v in c]


def oracle(seed: str, mode: int) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "CaveOracle", seed, str(mode)] + chunk_args())
    if r.returncode != 0:
        raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


def groups(text: str) -> list[tuple[str, list[str]]]:
    """Split oracle/probe output into (header, column-lines) per chunk."""
    out: list[tuple[str, list[str]]] = []
    cur: tuple[str, list[str]] | None = None
    for line in text.splitlines():
        if line.startswith("seed "):
            cur = (line, [])
            out.append(cur)
        elif cur is not None:
            cur[1].append(line)
    return out


def carved_cells(with_pass: str, base: str) -> int:
    """Number of blocks that differ between a carving pass and the base terrain."""
    diff = 0
    for (_, a), (_, b) in zip(groups(with_pass), groups(base)):
        for la, lb in zip(a, b):
            if la == lb:
                continue
            pa, pb = la.split(), lb.split()
            diff += sum(1 for va, vb in zip(pa[2:], pb[2:]) if va != vb)
    return diff


def first_mismatch(vanilla: str, rust: str) -> str | None:
    vg, rg = groups(vanilla), groups(rust)
    if len(vg) != len(rg):
        return f"chunk-count mismatch: vanilla {len(vg)} vs rust {len(rg)}"
    for (hv, lv), (hr, lr) in zip(vg, rg):
        if hv != hr:
            return f"header mismatch:\n  vanilla: {hv}\n  rust:    {hr}"
        if len(lv) != len(lr):
            return f"{hv}: column-count mismatch {len(lv)} vs {len(lr)}"
        for i, (l1, l2) in enumerate(zip(lv, lr)):
            if l1 == l2:
                continue
            p1, p2 = l1.split(), l2.split()
            x, z = p1[0], p1[1]
            for y, (a, b) in enumerate(zip(p1[2:], p2[2:])):
                if a != b:
                    return (
                        f"{hv}\n  column x={x} z={z} y={y}: "
                        f"vanilla {a} (id {int(a) >> 4} meta {int(a) & 15}) "
                        f"vs rust {b} (id {int(b) >> 4} meta {int(b) & 15})"
                    )
            return f"{hv}\n  line {i + 1} differs:\n  vanilla: {l1}\n  rust:    {l2}"
    return None


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

    modes = ",".join(str(m) for m in MODES)
    total_lines = 0
    for seed in SEEDS:
        vanilla_by_mode = {m: oracle(seed, m) for m in MODES}

        for m in (0, 1):
            n = carved_cells(vanilla_by_mode[m], vanilla_by_mode[3])
            if n == 0:
                print(f"FIXTURE ERROR: seed {seed} mode {m} carves nothing; pick new chunks")
                return 1

        vanilla = "".join(vanilla_by_mode[m] for m in MODES)

        r = run(
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "cave_probe", "--",
             seed, modes] + chunk_args()
        )
        if r.returncode != 0:
            print("cargo probe failed:\n" + r.stdout + r.stderr)
            return 2
        rust = r.stdout

        (WORK / f"vanilla-{seed}.txt").write_text(vanilla, encoding="utf-8")
        (WORK / f"rust-{seed}.txt").write_text(rust, encoding="utf-8")

        problem = first_mismatch(vanilla, rust)
        if problem is not None:
            print(f"MISMATCH (seed {seed}, modes {modes}):")
            print("  " + problem)
            return 1
        total_lines += len(vanilla.splitlines())

    print(f"OK: {total_lines} lines match ({len(SEEDS)} seeds x {len(MODES)} modes x {len(CHUNKS)} chunks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
