#!/usr/bin/env python3
"""random_parity: does mc-core's JavaRandom match java.util.Random exactly?

Runs the Java oracle (JDK 8) and the Rust probe on the same canonical program and
compares the output line-by-line. Oracle results are cached under .local/random/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "RandomOracle.java"
WORK = ROOT / ".local" / "random"
ORACLE = Oracle("random", ORACLE_SRC, "RandomOracle", work=WORK)


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
    WORK.mkdir(parents=True, exist_ok=True)

    try:
        vanilla = ORACLE.run([])
    except RuntimeError as e:
        print(e)
        return 2

    r = run([find_cargo(), "run", "-q", "-p", "mc-core", "--example", "random_probe"])
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
