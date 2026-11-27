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
one seed, chunks that report a scattered feature (temple/hut) start, chunks that report an ocean
monument start, chunks that report a nether fortress start, and village origins. Guards: every seed
must produce at least one start *of each kind* (mineshaft, temple, monument, fortress, village).

Outputs are cached under .local/structures/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle, Probe

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "StructuresOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "structures"
ORACLE = Oracle("structures", ORACLE_SRC, "StructuresOracle", work=WORK, jar=SERVER_JAR)
PROBE = Probe("structures", "mc-worldgen", "structures_probe")

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
    # Scattered features (temples). A start at chunk (sx, sz) is reported by the chunks whose
    # box-matching area touches it, so these include a neighbour of the origin chunk:
    # seed 0: swamp hut at (3,47)  -> reported by (2,46), (3,46)
    (2, 46), (3, 46),
    # seed 12345: swamp hut at (-26,49) -> reported by (-27,48), (-26,48)
    (-27, 48), (-26, 48),
    # seed -7: desert pyramid at (4,2) -> reported by (3,1), (4,1)
    (3, 1), (4, 1),
    # seed 12345: jungle pyramid at (72,-155) -> reported by (71,-156), (71,-155)
    (71, -156), (71, -155),
    # Ocean monuments (the origin chunk reports its own building box):
    # seed 0: monument at (77,-50) and (86,-12)
    (77, -50), (86, -12),
    # seed 12345: monument at (-695,458)
    (-695, 458),
    # seed -7: monuments at (-40,-23) and (-28,21)
    (-40, -23), (-28, 21),
    # Nether fortresses (a standalone nether generator; the origin chunk reports its own box):
    # seed 0: (-38,-39), (-24,24)
    (-38, -39), (-24, 24),
    # seed 12345: (-25,-41), (-7,-10)
    (-25, -41), (-7, -10),
    # seed -7: (-12,-39), (4,-21)
    (-12, -39), (4, -21),
    # Villages (the origin chunk reports its own well):
    # seed 0: (-53,-16)
    (-53, -16),
    # seed 12345: (-47,-26), (-43,5)
    (-47, -26), (-43, 5),
    # seed -7: (34,1) reports the village at (35,1); (37,-12) reports its own
    (34, 1), (37, -12),
    # desert village (exercises the `inDesert` / `Desert=1` branch): seed 0, origin (-210,-151)
    (-211, -151),
    # Strongholds (3 ring positions per seed; the origin chunk reports its own):
    # seed 0: (-37,39), (-9,-41), (67,32)
    (-37, 39), (-9, -41), (67, 32),
    # seed 12345: (-44,59), (-16,-38), (61,18)
    (-44, 59), (-16, -38), (61, 18),
    # seed -7: (-44,-34), (-7,61), (61,-19)
    (-44, -34), (-7, 61), (61, -19),
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
    return ORACLE.run([seed] + chunk_args())


def guard(text: str, seed: str) -> str | None:
    body = "\n" + text
    if "\nstart " not in body:
        return f"seed {seed}: no structure starts registered at all; fixture has no signal"
    if "\nstart Mineshaft " not in body:
        return f"seed {seed}: no mineshaft start; mineshaft fixture has no signal"
    if "\nstart Temple " not in body:
        return f"seed {seed}: no scattered-feature start; temple fixture has no signal"
    if "\nstart Monument " not in body:
        return f"seed {seed}: no ocean-monument start; monument fixture has no signal"
    if "\nstart Fortress " not in body:
        return f"seed {seed}: no nether-fortress start; fortress fixture has no signal"
    if "\nstart Village " not in body:
        return f"seed {seed}: no village start; village fixture has no signal"
    if "\nstart Stronghold " not in body:
        return f"seed {seed}: no stronghold start; stronghold fixture has no signal"
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

    total_lines = 0
    starts_seen = 0
    for seed in SEEDS:
        vanilla = oracle(seed)

        problem = guard(vanilla, seed)
        if problem is not None:
            print("FIXTURE ERROR: " + problem)
            return 1
        starts_seen += vanilla.count("\nstart ") + vanilla.startswith("start ")

        try:
            rust = PROBE.run([seed] + chunk_args())
        except RuntimeError as e:
            print(e)
            return 2

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
