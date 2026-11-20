# Local toolchain notes (Windows)

## Rust

- Toolchain: **stable-x86_64-pc-windows-gnu** (rustup), pinned by `rust-toolchain.toml`.
- Install dir: `%USERPROFILE%\.cargo\bin` (added to user PATH by rustup; restart shells).
- Linker: **w64devkit 2.11.0** at `vendor/tools/w64devkit/w64devkit/bin/gcc.exe`, wired in
  `.cargo/config.toml` for the `x86_64-pc-windows-gnu` target.
- Rust 1.100.0, GCC 16.2.0 (x86_64-w64-mingw32).

### Workaround: `libgcc_eh.a`

w64devkit ships `libgcc.a` but not `libgcc_eh.a`, which Rust's GNU target asks for.
Copied from rust-mingw:

```
%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-gnu\
  lib\rustlib\x86_64-pc-windows-gnu\lib\self-contained\libgcc_eh.a
```

into:

```
vendor/tools/w64devkit/w64devkit/lib/gcc/x86_64-w64-mingw32/16.2.0/libgcc_eh.a
vendor/tools/w64devkit/w64devkit/lib/libgcc_eh.a
```

If w64devkit is re-extracted or updated, re-copy this file.

## Java

- **Java 8** (system, Temurin 1.8.0_492): runs the 1.8.9 server and Forge 1.8.9.
- **JDK 21** (`vendor/tools/jdk21`): runs Vineflower.

## Decompile / remap

- Vineflower 1.12.0: `vendor/tools/vineflower-1.12.0.jar`
- SpecialSource 1.11.4-shaded: `vendor/tools/SpecialSource-1.11.4-shaded.jar`
- Pipeline: `joined.srg` + `stable_22` CSVs -> `mappings/joined-mcp.srg` -> SpecialSource -> Vineflower.
- Output: `vendor/minecraft/1.8.9/decompiled/vanilla-{client,server}`.

## Verify

```sh
cargo test --workspace --no-run
cargo test --workspace
cargo fmt --all -- --check
```