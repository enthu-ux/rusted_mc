#!/usr/bin/env python3
"""Shared oracle-output cache for the parity harnesses.

A parity oracle is a pure function of `(oracle source, server jar, argv, env)`, so its
stdout can be reused until one of those inputs changes. That is the expensive half of a
harness (JVM startup + worldgen); the Rust probe still runs every time.

Cache layout: `.local/<harness>/oracle-cache/<sha256>.txt`.
The class files are shared per harness in `.local/<harness>/oracle-classes/` and are only
recompiled when the oracle source or jar changes (a stamp file pins the identity).

`PARITY_FRESH=1` (set by `scripts/verify.ps1 -Fresh`) bypasses the cache. Caching is keyed
on the *inputs*, never on the comparison, so a hit can never hide a changed fixture.
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