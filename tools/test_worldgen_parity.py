#!/usr/bin/env python3
"""worldgen_parity: does mc-worldgen's canonical chunk match real 1.8.9?

Runs the game's own `ChunkProviderGenerate.provideChunk` (server remapped jar, JDK 8) with the
`ChunkProviderSettings.Factory` defaults (caves + ravines on, structures off) and compares the whole
chunk against the Rust probe (`crates/mc-worldgen/examples/worldgen_probe.rs`):

  - 256 block columns (blockId * 16 + metadata), the Gate-C artefact
  - `Chunk.getBiomeArray()`  (index `x + z * 16`)
  - `Chunk.getHeightValue(x, z)`  (index `z * 16 + x`)

Scope note: this is the **pre-decoration** chunk. Gate C's `worldgen_seed` also needs structures and
decoration (both unported), which change blocks, so this harness pins the parts that exist and will
be extended into Gate C. It covers the Gate-C seeds/chunks plus cave/ravine/mesa/ocean chunks.

Signal guard per seed: heights must have >1 distinct value with a minimum of at least 1 (bedrock is
opaque), and the biome array must have >1 distinct id - otherwise the fixture proves nothing.

One fresh JVM per seed (BiomeGenMesa caches its per-world noise on first use).
Outputs cached under .local/worldgen/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "WorldgenOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "worldgen"
ORACLE = Oracle("worldgen", ORACLE_SRC, "WorldgenOracle", work=WORK, jar=SERVER_JAR)

SEEDS = ["0", "12345", "-7"]
# Gate-C chunks first, then fixtures with cave/ravine/mesa/ocean signal.
CHUNKS = [
    (0, 0), (100, -50), (-37, 210), (512, 512),
    (-3, 0), (-5, 1), (-48, -48), (-124, -128), (36, -163), (6, 5),
]
COLUMNS = 256


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


class ChunkOut:
    """One chunk's printed artefact."""

    def __init__(self) -> None:
        self.header = ""
        self.columns: list[str] = []
        self.biome: str = ""
        self.height: str = ""


def parse(text: str) -> list[ChunkOut]:
    out: list[ChunkOut] = []
    cur: ChunkOut | None = None
    for line in text.splitlines():
        if line.startswith("seed "):
            cur = ChunkOut()
            cur.header = line
            out.append(cur)
        elif cur is not None:
            if line.startswith("biome "):
                cur.biome = line
            elif line.startswith("height "):
                cur.height = line
            else:
                cur.columns.append(line)
    return out


def column_coord(line: str) -> tuple[str, str]:
    p = line.split()
    return p[0], p[1]


def vector_mismatch(kind: str, a: str, b: str) -> str | None:
    """Compare a 'biome ...' / 'height ...' line; report the first differing column."""
    pa, pb = a.split(), b.split()
    for i, (va, vb) in enumerate(zip(pa[1:], pb[1:])):
        if va != vb:
            return f"  {kind} differs at column x={i & 15} z={i >> 4}: vanilla {va} vs rust {vb}"
    return None


def first_mismatch(vanilla: str, rust: str) -> str | None:
    vg, rg = parse(vanilla), parse(rust)
    if len(vg) != len(rg):
        return f"chunk-count mismatch: vanilla {len(vg)} vs rust {len(rg)}"
    for a, b in zip(vg, rg):
        if a.header != b.header:
            return f"header mismatch:\n  vanilla: {a.header}\n  rust:    {b.header}"
        if len(a.columns) != len(b.columns):
            return f"{a.header}: column-count mismatch {len(a.columns)} vs {len(b.columns)}"
        for l1, l2 in zip(a.columns, b.columns):
            if l1 == l2:
                continue
            x, z = column_coord(l1)
            p1, p2 = l1.split(), l2.split()
            for y, (v1, v2) in enumerate(zip(p1[2:], p2[2:])):
                if v1 != v2:
                    return (
                        f"{a.header}\n  block x={x} z={z} y={y}: "
                        f"vanilla {v1} (id {int(v1) >> 4} meta {int(v1) & 15}) "
                        f"vs rust {v2} (id {int(v2) >> 4} meta {int(v2) & 15})"
                    )
            return f"{a.header}\n  column {x} {z} differs"
        for kind, la, lb in (("biome", a.biome, b.biome), ("height", a.height, b.height)):
            if la != lb:
                detail = vector_mismatch(kind, la, lb)
                return f"{a.header}\n{detail}"
    return None


def guard(chunks: list[ChunkOut], seed: str) -> str | None:
    heights = [int(v) for c in chunks for v in c.height.split()[1:]]
    biomes = [int(v) for c in chunks for v in c.biome.split()[1:]]
    if min(heights) < 1:
        return f"seed {seed}: a height is < 1 ({min(heights)}); bedrock should be opaque"
    if len(set(heights)) < 2:
        return f"seed {seed}: heights are constant ({heights[0]}); fixture has no relief"
    if len(set(biomes)) < 2:
        return f"seed {seed}: biome array is constant ({biomes[0]}); fixture has no biome signal"
    return None


def main() -> int:
    if not SERVER_JAR.exists():
        print(f"missing {SERVER_JAR}; run the decompile/remap step first")
        return 3

    WORK.mkdir(parents=True, exist_ok=True)

    total_lines = 0
    for seed in SEEDS:
        vanilla = oracle(seed)

        problem = guard(parse(vanilla), seed)
        if problem is not None:
            print("FIXTURE ERROR: " + problem)
            return 1

        r = run(
            [find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "worldgen_probe", "--",
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

    print(f"OK: {total_lines} lines match ({len(SEEDS)} seeds x {len(CHUNKS)} chunks, blocks + biome + heightmap)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
