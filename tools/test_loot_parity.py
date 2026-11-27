#!/usr/bin/env python3
"""loot_parity: does mc-worldgen's loot/enchantment RNG match real 1.8.9?

Runs the game's own code (server remapped jar, JDK 8) - `Enchantment.enchantmentsBookList`,
`EnchantmentHelper.addRandomEnchantment`, the private `StructureMineshaftPieces` weight list +
`Items.enchanted_book.getRandom`, and `WeightedRandomChestContent.generateChestContents` - and
compares it against the Rust probe (`crates/mc-worldgen/examples/loot_probe.rs`).

This is the subsystem `StructureMineshaftPieces.addComponentParts` consumes before writing blocks: a
corridor chest roll draws enchantment RNG (whose map iteration order feeds `WeightedRandom`), so the
*draw sequence* - not just the items - must match.

Three dumps:
  table   - the enchantment registry (id/weight/max level/type), each level's min/max enchantability,
            and every ordered pair's `canApplyTogether`;
  enchant - a random book roll's stored enchantments + the next 32 draws;
  chest   - the mineshaft chest list, every slot write, and the next 32 draws.

Outputs are cached under .local/loot/.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

from _parity_cache import Oracle

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE_SRC = ROOT / "oracle" / "java" / "src" / "LootOracle.java"
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
WORK = ROOT / ".local" / "loot"
ORACLE = Oracle("loot", ORACLE_SRC, "LootOracle", work=WORK, jar=SERVER_JAR)

SEEDS = ["0", "12345", "-7"]
LEVELS = ["0", "5", "15", "30", "50"]
# (count, slots): the corridor range and a small inventory to exercise the slot draw.
CHEST = [("3", "27"), ("6", "27"), ("1", "27"), ("8", "5")]

EXPECTED_ENCHANTS = 25
EXPECTED_EXCL = EXPECTED_ENCHANTS * (EXPECTED_ENCHANTS - 1) // 2


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def one(mode: str, args: list[str]) -> tuple[str, str]:
    """Runs the oracle and the probe for one mode and returns their outputs, comparing them."""
    try:
        vanilla = ORACLE.run([mode] + args)
    except RuntimeError as e:
        raise RuntimeError(f"java oracle failed ({mode}):\n{e}")

    r = run([find_cargo(), "run", "-q", "-p", "mc-worldgen", "--example", "loot_probe", "--", mode] + args)
    if r.returncode != 0:
        raise RuntimeError(f"cargo probe failed ({mode}):\n" + r.stdout + r.stderr)
    rust = r.stdout

    (WORK / f"vanilla-{mode}.txt").write_text(vanilla, encoding="utf-8")
    (WORK / f"rust-{mode}.txt").write_text(rust, encoding="utf-8")

    vl, rl = vanilla.splitlines(), rust.splitlines()
    for i in range(min(len(vl), len(rl))):
        if vl[i] != rl[i]:
            raise AssertionError(
                f"MISMATCH ({mode}), line {i + 1}:\n  vanilla: {vl[i]}\n  rust:    {rl[i]}"
            )
    if len(vl) != len(rl):
        raise AssertionError(f"MISMATCH ({mode}): line count {len(vl)} vs {len(rl)}")
    return vanilla, rust


def main() -> int:
    if not SERVER_JAR.exists():
        print(f"missing {SERVER_JAR}; run the decompile/remap step first")
        return 3

    WORK.mkdir(parents=True, exist_ok=True)

    try:
        table, _ = one("table", [])
        enchant_args = [v for s in SEEDS for level in LEVELS for v in (s, level)]
        enchants, _ = one("enchant", enchant_args)
        chest_args = [v for s in SEEDS for pair in CHEST for v in (s, *pair)]
        chest, _ = one("chest", chest_args)
    except AssertionError as e:
        print(e)
        return 1
    except RuntimeError as e:
        print(e)
        return 2

    # Guards: the fixtures must actually exercise the subsystem.
    n_ench = sum(1 for line in table.splitlines() if line.startswith("ench "))
    n_excl = sum(1 for line in table.splitlines() if line.startswith("excl "))
    if n_ench != EXPECTED_ENCHANTS or n_excl != EXPECTED_EXCL:
        print(f"FIXTURE ERROR: table has {n_ench} enchants / {n_excl} excl (expected "
              f"{EXPECTED_ENCHANTS}/{EXPECTED_EXCL})")
        return 1
    if not any(line.startswith("enchants ") and line != "enchants " for line in enchants.splitlines()):
        print("FIXTURE ERROR: no enchant case produced any enchantment")
        return 1
    if "write " not in chest:
        print("FIXTURE ERROR: no chest case wrote a slot")
        return 1

    total = sum(len(t.splitlines()) for t in (table, enchants, chest))
    cases = 1 + len(SEEDS) * len(LEVELS) + len(SEEDS) * len(CHEST)
    print(f"OK: {total} lines match ({EXPECTED_ENCHANTS} enchantments, {cases} cases)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
