#!/usr/bin/env python3
"""Minimal parity runner (adapted from Horizon's tools/parity.py).

Discovers tools/test_*_parity.py and runs them, optionally filtered by name.

    py tools/parity.py                # every harness
    py tools/parity.py random         # only harnesses whose name contains "random"
    py tools/parity.py --list         # list without running
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOLS = ROOT / "tools"


def discover() -> list[pathlib.Path]:
    return sorted(p for p in TOOLS.glob("test_*_parity.py"))


def main(argv: list[str]) -> int:
    args = argv[1:]
    if "--list" in args:
        for p in discover():
            print(p.name)
        return 0

    tests = discover()
    if args:
        tests = [p for p in tests if any(a in p.name for a in args)]
    if not tests:
        print("no harnesses matched")
        return 0

    failed = 0
    for t in tests:
        print(f"===== {t.name} =====", flush=True)
        r = subprocess.run([sys.executable, str(t)], cwd=ROOT)
        if r.returncode != 0:
            failed += 1

    print(f"----- {len(tests) - failed}/{len(tests)} passed -----")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
