#!/usr/bin/env python3
"""perf_baseline.py: record performance baselines for the performance contract.

Currently benchmarks base-terrain chunk generation, in-process, on both sides:

  - Rust: `crates/mc-worldgen/examples/terrain_bench.rs` (release)
  - Java: `oracle/java/src/TerrainBench.java` against the real 1.8.9 server code

Writes `.local/perf/<kind>-<utc>.json` and prints a table. The point is a reproducible reference,
not a gate yet (the M1 `worldgen_seed` gate is separate).
"""

from __future__ import annotations

import datetime as _dt
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER_JAR = ROOT / "vendor" / "minecraft" / "1.8.9" / "server-mcp.jar"
BENCH_SRC = ROOT / "oracle" / "java" / "src" / "TerrainBench.java"
WORK = ROOT / ".local" / "perf"
ORACLE_CLASSES = WORK / "oracle-classes"
REPEAT = 3


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    return str(pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe")


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def json_line(text: str) -> dict:
    for line in reversed(text.splitlines()):
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)
    raise ValueError("no JSON line in output:\n" + text)


def bench_rust() -> dict:
    r = run(
        [
            find_cargo(),
            "run",
            "--release",
            "-q",
            "-p",
            "mc-worldgen",
            "--example",
            "terrain_bench",
        ]
    )
    if r.returncode != 0:
        raise RuntimeError("cargo bench failed:\n" + r.stdout + r.stderr)
    return json_line(r.stdout)


def bench_java() -> dict:
    if not SERVER_JAR.exists():
        raise RuntimeError(f"missing {SERVER_JAR}")
    ORACLE_CLASSES.mkdir(parents=True, exist_ok=True)
    r = run(["javac", "-cp", str(SERVER_JAR), "-d", str(ORACLE_CLASSES), str(BENCH_SRC)])
    if r.returncode != 0:
        raise RuntimeError("javac failed:\n" + r.stdout + r.stderr)
    cp = os.pathsep.join([str(ORACLE_CLASSES), str(SERVER_JAR)])
    r = run(["java", "-cp", cp, "TerrainBench"])
    if r.returncode != 0:
        raise RuntimeError("java bench failed:\n" + r.stdout + r.stderr)
    return json_line(r.stdout)


def best(runs: list[dict]) -> dict:
    # Keep the run with the lowest median; ranges are recorded too.
    runs = sorted(runs, key=lambda d: d["per_chunk_ms"]["median"])
    out = dict(runs[0])
    out["runs"] = [
        {"median": d["per_chunk_ms"]["median"], "p95": d["per_chunk_ms"]["p95"]} for d in runs
    ]
    return out


def main() -> int:
    WORK.mkdir(parents=True, exist_ok=True)
    result = {"utc": _dt.datetime.now(_dt.timezone.utc).isoformat(timespec="seconds")}

    for name, fn in (("rust", bench_rust), ("java", bench_java)):
        runs = [fn() for _ in range(REPEAT)]
        result[name] = best(runs)

    stamp = _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = WORK / f"terrain-{stamp}.json"
    out.write_text(json.dumps(result, indent=2), encoding="utf-8")

    print(f"{'side':<6} {'samples':>7} {'min':>9} {'median':>9} {'p95':>9} {'max':>9}  (ms/chunk)")
    for side in ("rust", "java"):
        m = result[side]["per_chunk_ms"]
        print(
            f"{side:<6} {result[side]['samples']:>7} {m['min']:>9.4f} {m['median']:>9.4f} "
            f"{m['p95']:>9.4f} {m['max']:>9.4f}"
        )
    r, j = result["rust"]["per_chunk_ms"]["median"], result["java"]["per_chunk_ms"]["median"]
    if r > 0:
        print(f"\nRust/Java median ratio: {r / j:.3f}x")
    print(f"wrote {out.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 - CLI
        print(f"error: {e}")
        sys.exit(1)