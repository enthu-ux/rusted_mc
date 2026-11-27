#!/usr/bin/env python3
"""Shared result cache for the parity harnesses.

Both halves of a harness are deterministic and expensive, so each caches its stdout:
- `Oracle`: a Java oracle is a pure function of `(oracle source, server jar, argv, env)`.
- `Probe`: a Rust example is a pure function of `(example binary, argv, env)`. The example is
  rebuilt before hashing (unless `PARITY_PREBUILT=1`, set by `parity.py` after its one build),
  so a Rust edit changes the binary identity instead of returning a stale chunk.

Cache layout: `.local/<harness>/oracle-cache|probe-cache/<sha256>.txt`; oracle class files live
in `.local/<harness>/oracle-classes/` and are recompiled only when the source or jar changes.

`PARITY_FRESH=1` (set by `scripts/verify.ps1 -Fresh`) bypasses the caches. Keys cover only the
inputs, never the comparison, so a hit can never hide a changed fixture.
"""

from __future__ import annotations

import hashlib
import os
import pathlib
import subprocess
import threading

ROOT = pathlib.Path(__file__).resolve().parent.parent

_TRUE = {"1", "true", "yes", "on"}


def fresh() -> bool:
    """Whether the caller forced a cold run (`PARITY_FRESH=1`)."""
    return os.environ.get("PARITY_FRESH", "").strip().lower() in _TRUE


def _identity(path: pathlib.Path | None) -> str:
    """A cheap file identity: size + mtime is enough to notice a rebuild/replace."""
    if path is None:
        return "-"
    try:
        st = path.stat()
    except OSError:
        return "missing"
    return f"{st.st_size}:{st.st_mtime_ns}"


def find_cargo() -> str:
    from shutil import which

    c = which("cargo")
    if c:
        return c
    return str(pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe")


class Oracle:
    """A Java oracle whose stdout is cached by source/jar/argv/env."""

    def __init__(
        self,
        name: str,
        source: pathlib.Path,
        main_class: str,
        *,
        work: pathlib.Path,
        jar: pathlib.Path | None = None,
    ) -> None:
        self.name = name
        self.source = source
        self.main_class = main_class
        self.work = work
        self.jar = jar
        self.classes = work / "oracle-classes"
        self.cache = work / "oracle-cache"
        # Hash the source once; harnesses call `run()` many times.
        self._source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
        self._jar_id = _identity(jar)
        self._compile_lock = threading.Lock()

    # -- compilation (only reached on a cache miss) -------------------------------------
    def compile(self) -> None:
        with self._compile_lock:
            self._compile_locked()

    def _compile_locked(self) -> None:
        self.classes.mkdir(parents=True, exist_ok=True)
        stamp = self.classes / ".stamp"
        class_file = self.classes / f"{self.main_class}.class"
        identity = f"{self._source_hash}|{self._jar_id}"
        if stamp.exists() and class_file.exists():
            try:
                if stamp.read_text(encoding="utf-8") == identity:
                    return
            except OSError:
                pass
        cmd = ["javac"]
        if self.jar is not None:
            cmd += ["-cp", str(self.jar)]
        cmd += ["-d", str(self.classes), str(self.source)]
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError("javac failed:\n" + r.stdout + r.stderr)
        stamp.write_text(identity, encoding="utf-8")

    # -- run + cache --------------------------------------------------------------------
    def run(self, argv: list[str], *, env: dict[str, str | None] | None = None) -> str:
        argv = [str(a) for a in argv]
        env = {str(k): (None if v is None else str(v)) for k, v in (env or {}).items()}
        key = self._key(argv, env)
        cached = self.cache / f"{key}.txt"
        if cached.exists() and not fresh():
            return cached.read_text(encoding="utf-8")

        self.compile()
        cmd = ["java", "-cp", self._classpath(), self.main_class, *argv]
        run_env = dict(os.environ)
        for k, v in env.items():
            if v is None:
                run_env.pop(k, None)
            else:
                run_env[k] = v
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=run_env)
        if r.returncode != 0:
            raise RuntimeError("java oracle failed:\n" + r.stdout + r.stderr)
        self.cache.mkdir(parents=True, exist_ok=True)
        cached.write_text(r.stdout, encoding="utf-8")
        return r.stdout

    def _key(self, argv: list[str], env: dict[str, str | None]) -> str:
        h = hashlib.sha256()
        h.update(b"oracle-v1\0")
        h.update(self.main_class.encode())
        h.update(b"\0source\0")
        h.update(self._source_hash.encode())
        h.update(b"\0jar\0")
        h.update(self._jar_id.encode())
        h.update(b"\0argv\0")
        h.update("\0".join(argv).encode())
        for k in sorted(env):
            h.update(b"\0env\0")
            h.update(k.encode())
            h.update(b"=")
            h.update((env[k] if env[k] is not None else "<unset>").encode())
        return h.hexdigest()

    def _classpath(self) -> str:
        parts = [str(self.classes)]
        if self.jar is not None:
            parts.append(str(self.jar))
        return os.pathsep.join(parts)


class Probe:
    """A Rust example binary whose stdout is cached by binary identity + argv/env.

    Before hashing, the example is rebuilt (unless `PARITY_PREBUILT=1`, which `parity.py`
    sets after its one workspace-wide build), so a Rust edit changes the binary identity and
    invalidates the cache instead of returning a stale chunk.
    """

    def __init__(self, name: str, crate: str, example: str) -> None:
        self.name = name
        self.crate = crate
        self.example = example
        suffix = ".exe" if os.name == "nt" else ""
        self.exe = ROOT / "target" / "debug" / "examples" / f"{example}{suffix}"
        self.cache = ROOT / ".local" / name / "probe-cache"
        self._lock = threading.Lock()
        self._built = False

    def _ensure_built(self) -> None:
        with self._lock:
            if self._built:
                return
            if not os.environ.get("PARITY_PREBUILT"):
                r = subprocess.run(
                    [find_cargo(), "build", "-q", "-p", self.crate, "--example", self.example],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                )
                if r.returncode != 0:
                    raise RuntimeError("cargo build failed:\n" + r.stdout + r.stderr)
            self._built = True

    def run(self, argv: list[str], *, env: dict[str, str | None] | None = None) -> str:
        argv = [str(a) for a in argv]
        env = {str(k): (None if v is None else str(v)) for k, v in (env or {}).items()}
        self._ensure_built()

        has_exe = self.exe.exists()
        cached: pathlib.Path | None = None
        if has_exe:
            cached = self.cache / f"{self._key(argv, env)}.txt"
            if cached.exists() and not fresh():
                return cached.read_text(encoding="utf-8")

        run_env = dict(os.environ)
        for k, v in env.items():
            if v is None:
                run_env.pop(k, None)
            else:
                run_env[k] = v
        if has_exe:
            cmd = [str(self.exe), *argv]
        else:
            cmd = [
                find_cargo(), "run", "-q", "-p", self.crate, "--example", self.example, "--", *argv
            ]
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=run_env)
        if r.returncode != 0:
            raise RuntimeError("cargo probe failed:\n" + r.stdout + r.stderr)
        if has_exe and cached is not None:
            self.cache.mkdir(parents=True, exist_ok=True)
            cached.write_text(r.stdout, encoding="utf-8")
        return r.stdout

    def _key(self, argv: list[str], env: dict[str, str | None]) -> str:
        h = hashlib.sha256()
        h.update(b"probe-v1\0")
        h.update(self.example.encode())
        h.update(b"\0exe\0")
        h.update(_identity(self.exe).encode())
        h.update(b"\0argv\0")
        h.update("\0".join(argv).encode())
        for k in sorted(env):
            h.update(b"\0env\0")
            h.update(k.encode())
            h.update(b"=")
            h.update((env[k] if env[k] is not None else "<unset>").encode())
        return h.hexdigest()