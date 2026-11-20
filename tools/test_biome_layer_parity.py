#!/usr/bin/env python3
"""biome_layer_parity: do mc-worldgen's biome layers match real 1.8.9?

Runs the game's own `net.minecraft.world.gen.layer` stack (server remapped jar, JDK 8)
for fixed seeds/regions and compares the emitted biome ids against the Rust probe
(`crates/mc-worldgen/examples/biome_layer_probe.rs`). Outputs cached under
.local/biome_layer/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "BiomeLayerOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "biome_layer"
ORACLE_CLASSES = WORK / "oracle-classes"


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
    ORACLE_CLASSES.mkdir(parents=True, exist_ok=True)

    r = run(["javac", "-cp", str(SERVER_JAR), "-d", str(ORACLE_CLASSES), str(ORACLE_SRC)])
    if r.returncode != 0:
        print("javac failed:\n" + r.stdout + r.stderr)
        return 2

    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "BiomeLayerOracle"])
    if r.returncode != 0:
        print("java oracle failed:\n" + r.stdout + r.stderr)
        return 2
    vanilla = r.stdout

    r = run([find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "biome_layer_probe"])
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
