#!/usr/bin/env python3
"""structures_parity: does mc-worldgen register the same structures as real 1.8.9?

Runs the game's own `MapGenMineshaft` registration (server remapped jar, JDK 8) - the half that
`ChunkProviderGenerate.provideChunk` performs - and compares it against the Rust probe
(`crates/mc-worldgen/examples/structures_probe.rs`). The dump is vanilla's own structure NBT
(`StructureStart.writeStructureComponentsToNBT`): each start's bounding box and each piece's type,
box, `componentType` (`GD`), orientation (`O`) and kind-specific fields.

Blocks are written later, in `populate` (`MapGenStructure.generateStructure`), which is not ported
yet - so this harness pins the layout half only; `plan.md` Gate C waits for both.

Fixtures: the Gate-C chunks plus chunks that are mineshaft origins (or fall inside one) in at least
one seed, so every seed has signal. Guard: every seed must produce at least one start.

Outputs are cached under .local/structures/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "StructuresOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "structures"
ORACLE_CLASSES = WORK / "oracle-classes"

SEEDS = ["0", "12345", "-7"]
CHUNKS = [
    # Gate C
    (0, 0), (100, -50), (-37, 210), (512, 512),
    # Mineshaft origins (or chunks a mineshaft reaches into), per seed:
    # seed 0: (6,12), (95,-45), (-49,223), (516,506)
    (6, 12), (95, -45), (-49, 223), (516, 506),
    # seed 12345: (102,-45), (-51,211), (504,511)
    (102, -45), (-51, 211), (504, 511),
    # seed -7: (86,-31), (-33,198), (500,513)
    (86, -31), (-33, 198), (500, 513),
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


def oracle(seed: str) -> str:
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "StructuresOracle", seed] + chunk_args())
    if r.returncode != 0:
        raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
    return r.stdout


def guard(text: str, seed: str) -> str | None:
    if "\nstart " not in "\n" + text:
        return f"seed {seed}: no structure starts registered at all; fixture has no signal"
    return None


def first_mismatch(vanilla: str, rust: str) -> str | None:
    vl, rl = vanilla.splitlines(), rust.splitlines()
    chunk = "?"
    for i in range(min(len(vl), len(rl))):
        if vl[i].startswith("chunk "):
            chunk = vl[i]
        if vl[i] != rl[i]:
            if vl[i].startswith("chunk ") or rl[i].startswith("chunk "):
                return f"line {i + 1}: chunk list differs\n  vanilla: {vl[i]}\n  rust:    {rl[i]}"
            return (
                f"{chunk}, line {i + 1}:\n  vanilla: {vl[i]}\n  rust:    {rl[i]}"
            )
    if len(vl) != len(rl):
        return f"line count differs: vanilla {len(vl)} vs rust {len(rl)} ({chunk})"
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

    total_lines = 0
    starts_seen = 0
    for seed in SEEDS:
        vanilla = oracle(seed)

        problem = guard(vanilla, seed)
        if problem is not None:
            print("FIXTURE ERROR: " + problem)
            return 1
        starts_seen += vanilla.count("\nstart ") + vanilla.startswith("start ")

        r = run(
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "structures_probe", "--",
             seed] + chunk_args()
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

    print(
        f"OK: {total_lines} lines match ({len(SEEDS)} seeds x {len(CHUNKS)} chunks, {starts_seen} starts)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
