#!/usr/bin/env python3
"""terrain_parity: does mc-worldgen's base terrain match real 1.8.9?

Runs the game's own `ChunkProviderGenerate.provideChunk` (server remapped jar, JDK 8; caves,
ravines and structures disabled) for fixed seeds/chunks and compares the full block column
against the Rust probe (`crates/mc-worldgen/examples/terrain_probe.rs`). One fresh JVM per seed
(Mesa caches per-world noise on first use). Outputs cached under .local/terrain/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "TerrainOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "terrain"
ORACLE = Oracle("terrain", ORACLE_SRC, "TerrainOracle", work=WORK, jar=SERVER_JAR)
SEEDS = ["0", "12345", "-7"]


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def main() -> int:
    if not SERVER_JAR.exists():
        print(f"missing {SERVER_JAR}; run the decompile/remap step first")
        return 3

    WORK.mkdir(parents=True, exist_ok=True)

    vanilla_parts = []
    for seed in SEEDS:
        try:
            vanilla_parts.append(ORACLE.run([seed]))
        except RuntimeError as e:
            print(e)
            return 2
    vanilla = "".join(vanilla_parts)

    r = run([find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "terrain_probe"])
    if r.returncode != 0:
        print("cargo probe failed:\n" + r.stdout + r.stderr)
        return 2
    rust = r.stdout

    (WORK / "vanilla.txt").write_text(vanilla, encoding="utf-8")
    (WORK / "rust.txt").write_text(rust, encoding="utf-8")

    vl = vanilla.splitlines()
    rl = rust.splitlines()
    if vl == rl:
        print(f"OK: {len(vl)} lines match")
        return 0

    for i in range(min(len(vl), len(rl))):
        if vl[i] != rl[i]:
            print(f"MISMATCH at line {i + 1}:")
            print(f"  vanilla: {vl[i]}")
            print(f"  rust:    {rl[i]}")
            return 1
    print(f"LENGTH mismatch: vanilla {len(vl)} vs rust {len(rl)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
