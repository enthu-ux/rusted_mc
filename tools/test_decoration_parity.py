#!/usr/bin/env python3
"""decoration_parity: does mc-worldgen's decorated chunk match real 1.8.9?

Runs the game's own `ChunkProviderGenerate` for terrain, then a *guarded* mirror of
`BiomeDecorator.genDecorations` for the layers named by the mode
(`oracle/java/src/DecorationOracle.java`), and compares the target chunk's blocks against the Rust
probe (`crates/mc-worldgen/examples/decoration_probe.rs`).

Decoration is `populate`'s `+8`-offset 16x16 window, so the target chunk is the union of the four
covering passes; both sides run the same fixed pass order (vanilla's is chunk-load order).

Modes: ores | surface | ore_surface.

Signal guard: the corpus must actually contain a decorated ore block, else the fixture proves nothing.
Outputs cached under .local/decoration/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "DecorationOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "decoration"
ORACLE_CLASSES = WORK / "oracle-classes"

SEEDS = ["0", "12345", "-7"]
CHUNKS = [(0, 0), (100, -50), (-37, 210), (-3, 0), (-48, -48), (6, 5)]
MODES = ["ores", "surface", "plants", "trees", "ore_surface"]
ORE_IDS = {14, 15, 16, 21, 56, 73}
PLANT_IDS = {31, 32, 37, 38, 39, 40, 81, 83, 86, 99, 100, 111}
TREE_IDS = {17, 18, 161, 162, 106}
# Each vegetation generator the corpus must actually exercise (else the fixture proves nothing).
PLANT_SIGNATURE = {
    "tall grass": {31},
    "flower": {37, 38},
    "small mushroom (skylight)": {39, 40},
    "cactus": {81},
    "dead bush": {32},
}


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


def oracle(seed: str, mode: str) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "DecorationOracle", seed, mode] + chunk_args())
    if r.returncode != 0:
        raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


def rust(seed: str, mode: str) -> str:
    r = run(
        [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "decoration_probe", "--",
         seed, mode] + chunk_args()
    )
    if r.returncode != 0:
        raise RuntimeError("cargo probe failed:\n" + r.stdout + r.stderr)
    return r.stdout


def first_mismatch(vanilla: str, rust_out: str) -> str | None:
    v = vanilla.splitlines()
    r = rust_out.splitlines()
    if len(v) != len(r):
        return f"line-count mismatch: vanilla {len(v)} vs rust {len(r)}"
    header = None
    for l1, l2 in zip(v, r):
        if l1.startswith("seed "):
            header = l1
            if l1 != l2:
                return f"header mismatch:\n  vanilla: {l1}\n  rust:    {l2}"
            continue
        if l1 == l2:
            continue
        p1, p2 = l1.split(), l2.split()
        for y, (a, b) in enumerate(zip(p1[2:], p2[2:])):
            if a != b:
                return (
                    f"{header}\n  block x={p1[0]} z={p1[1]} y={y}: "
                    f"vanilla {a} (id {int(a) >> 4} meta {int(a) & 15}) "
                    f"vs rust {b} (id {int(b) >> 4} meta {int(b) & 15})"
                )
        return f"{header}\n  column {p1[0]} {p1[1]} differs"
    return None


def ore_count(text: str) -> int:
    return block_count(text, ORE_IDS)


def plant_count(text: str) -> int:
    return block_count(text, PLANT_IDS)


def block_count(text: str, ids: set[int]) -> int:
    return len(_block_ids(text) & ids)


def _block_ids(text: str) -> set[int]:
    seen: set[int] = set()
    for line in text.splitlines():
        if line.startswith("seed ") or len(line.split()) < 3:
            continue
        for v in line.split()[2:]:
            seen.add(int(v) >> 4)
    return seen


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
    plant_seen: set[int] = set()
    for mode in MODES:
        for seed in SEEDS:
            vanilla = oracle(seed, mode)
            rust_out = rust(seed, mode)

            if "plant" in mode:
                plant_seen |= _block_ids(vanilla)

            if "ore" in mode and ore_count(vanilla) == 0:
                print(f"FIXTURE ERROR: mode {mode} seed {seed}: no ore blocks placed")
                return 1
            if "plant" in mode and plant_count(vanilla) == 0:
                print(f"FIXTURE ERROR: mode {mode} seed {seed}: no plant blocks placed")
                return 1
            if "tree" in mode and block_count(vanilla, TREE_IDS) == 0:
                print(f"FIXTURE ERROR: mode {mode} seed {seed}: no tree blocks placed")
                return 1

            (WORK / f"vanilla-{mode}-{seed}.txt").write_text(vanilla, encoding="utf-8")
            (WORK / f"rust-{mode}-{seed}.txt").write_text(rust_out, encoding="utf-8")

            problem = first_mismatch(vanilla, rust_out)
            if problem is not None:
                print(f"MISMATCH (mode {mode}, seed {seed}):")
                print("  " + problem)
                return 1
            total_lines += len(vanilla.splitlines())

    for name, ids in PLANT_SIGNATURE.items():
        if not (plant_seen & ids):
            print(f"FIXTURE ERROR: the corpus never places a {name}; add a chunk that exercises it")
            return 1

    print(f"OK: {total_lines} lines match ({len(MODES)} modes x {len(SEEDS)} seeds x {len(CHUNKS)} chunks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
