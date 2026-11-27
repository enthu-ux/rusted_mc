#!/usr/bin/env python3
"""Parity runner (adapted from Horizon's tools/parity.py).

Discovers tools/test_*_parity.py and runs them, optionally filtered by name. Harnesses are
independent (each owns its `.local/<name>/` work dir), so they run in a bounded thread pool.

    py tools/parity.py                # every harness, up to 4 workers
    py tools/parity.py random         # only harnesses whose name contains "random"
    py tools/parity.py --jobs 1       # serial
    py tools/parity.py --fresh        # ignore the oracle caches (PARITY_FRESH=1)
    py tools/parity.py --list         # list without running

Oracle output is cached per harness (`tools/_parity_cache.py`), so a rerun after a Rust-only
change is fast; `--fresh` forces the Java oracles to run again. Before dispatch, all examples
are built once so the probes' `cargo run` calls do not contend on the build lock.
"""

from __future__ import annotations

import concurrent.futures
import os
import pathlib
import subprocess
import sys
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOLS = ROOT / "tools"


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    guess = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    return str(guess)


def discover() -> list[pathlib.Path]:
    return sorted(p for p in TOOLS.glob("test_*_parity.py"))


def build_examples() -> bool:
    r = subprocess.run(
        [find_cargo(), "build", "--workspace", "--examples", "-q"], cwd=ROOT
    )
    return r.returncode == 0


def run_one(test: pathlib.Path) -> tuple[pathlib.Path, int, str, float]:
    started = time.perf_counter()
    r = subprocess.run([sys.executable, str(test)], cwd=ROOT, capture_output=True, text=True)
    elapsed = time.perf_counter() - started
    return test, r.returncode, r.stdout + r.stderr, elapsed


def parse_args(args: list[str]) -> tuple[int, list[str], bool] | None:
    jobs = 0
    fresh = False
    names: list[str] = []
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("--jobs", "-j"):
            i += 1
            if i >= len(args):
                print(f"{a} needs a value")
                return None
            jobs = int(args[i])
        elif a.startswith("--jobs="):
            jobs = int(a.split("=", 1)[1])
        elif a == "--serial":
            jobs = 1
        elif a == "--fresh":
            fresh = True
        elif a.startswith("-"):
            print(f"unknown option: {a}")
            return None
        else:
            names.append(a)
        i += 1
    return jobs, names, fresh


def run_batch(tests: list[pathlib.Path], jobs: int) -> dict[str, tuple[int, str, float]]:
    """Run a batch, printing each harness's output; returns name -> (code, output, seconds)."""
    results: dict[str, tuple[int, str, float]] = {}
    if jobs == 1:
        for t in tests:
            print(f"===== {t.name} =====", flush=True)
            _, code, out, elapsed = run_one(t)
            sys.stdout.write(out)
            sys.stdout.flush()
            results[t.name] = (code, out, elapsed)
            status = "ok" if code == 0 else f"FAILED (exit {code})"
            print(f"----- {t.name} {status} ({elapsed:.1f}s) -----", flush=True)
    else:
        progress_lock = threading.Lock()

        def done(fut: concurrent.futures.Future) -> None:
            test, code, out, elapsed = fut.result()
            results[test.name] = (code, out, elapsed)
            with progress_lock:
                status = "OK" if code == 0 else f"FAILED (exit {code})"
                print(f"  {test.name:<40} {status:<18} {elapsed:6.1f}s", flush=True)

        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
            futures = [pool.submit(run_one, t) for t in tests]
            for fut in concurrent.futures.as_completed(futures):
                done(fut)

        for t in tests:
            code, out, elapsed = results[t.name]
            print(f"===== {t.name} =====", flush=True)
            sys.stdout.write(out)
            sys.stdout.flush()
    return results


def main(argv: list[str]) -> int:
    args = argv[1:]
    if "--list" in args:
        for p in discover():
            print(p.name)
        return 0

    parsed = parse_args(args)
    if parsed is None:
        return 2
    jobs, names, fresh = parsed

    if fresh:
        os.environ["PARITY_FRESH"] = "1"

    tests = discover()
    if names:
        tests = [p for p in tests if any(n in p.name for n in names)]
    if not tests:
        print("no harnesses matched")
        return 0

    if jobs <= 0:
        jobs = min(4, os.cpu_count() or 1)
    jobs = max(1, min(jobs, len(tests)))

    print(f"== building examples once ==", flush=True)
    if build_examples():
        os.environ["PARITY_PREBUILT"] = "1"
    else:
        print("warning: example prebuild failed; harnesses will build on demand", flush=True)

    print(f"== running {len(tests)} harness(es), {jobs} worker(s) ==", flush=True)
    started = time.perf_counter()
    results: dict[str, tuple[int, str, float]] = {}

    if jobs > 1 and len(tests) > 1:
        heavy = [t for t in tests if "decoration" in t.name]
        rest = [t for t in tests if "decoration" not in t.name]
        if heavy and rest:
            # Decoration has 21 independent (seed, mode) pairs; run it alone so its inner pool
            # gets the whole machine instead of oversubscribing the top-level workers.
            os.environ["PARITY_INNER_JOBS"] = str(min(4, os.cpu_count() or 1))
            results.update(run_batch(heavy, 1))
            os.environ["PARITY_INNER_JOBS"] = "1"
            results.update(run_batch(rest, jobs))
        else:
            os.environ["PARITY_INNER_JOBS"] = "1"
            results.update(run_batch(tests, jobs))
    else:
        os.environ["PARITY_INNER_JOBS"] = (
            str(min(4, os.cpu_count() or 1)) if len(tests) == 1 else "1"
        )
        results.update(run_batch(tests, jobs))

    failed = sum(1 for code, _, _ in results.values() if code != 0)
    total = time.perf_counter() - started
    print(f"----- {len(tests) - failed}/{len(tests)} passed in {total:.1f}s -----", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))