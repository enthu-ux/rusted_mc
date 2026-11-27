#!/usr/bin/env python3
"""noise_batch_parity: do mc-worldgen's *batch* noise paths match real 1.8.9?

Covers the array paths: `Perlin.func_151599_a`/`func_151600_a` and
`Simplex.func_151606_a`. The Java oracle runs the game's own
`net.minecraft.world.gen` noise classes from the remapped server jar (JDK 8);
the Rust probe prints the same canonical samples. Compared line-by-line.
Outputs are cached under .local/noise_batch/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "NoiseBatchOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "noise_batch"
ORACLE = Oracle("noise_batch", ORACLE_SRC, "NoiseBatchOracle", work=WORK, jar=SERVER_JAR)


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

    try:
        vanilla = ORACLE.run([])
    except RuntimeError as e:
        print(e)
        return 2

    r = run([find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "noise_batch_probe"])
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
