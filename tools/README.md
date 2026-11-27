# Tools

Adapted from Horizon (vendor/horizon/tools) for 1.8.9:

- parity.py - runs test_*_parity.py harnesses (bounded parallel pool)
- _parity_cache.py - caches/replays Java oracle runs (keyed by source/jar/argv/env)
- port_ledger.py - vanilla method <-> Rust fn coverage
- gate.py, check_public_tree.py - merge gate
- benchmark/profiling scripts

Not copied yet; M0 adapts them to our paths and jar layout.
