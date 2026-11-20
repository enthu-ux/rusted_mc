$ErrorActionPreference = 'Stop'
$root = (Get-Location).Path

$crateDocs = [ordered]@{
  'mc-core'     = 'Shared primitives: Java-compatible math, java.util.Random, Java HashMap iteration order, common types.'
  'mc-nbt'      = 'NBT read/write for 1.8.9 tag formats.'
  'mc-proto'    = 'Protocol 47: varint framing, compression, encryption, packet codecs.'
  'mc-worldgen' = 'Deterministic 1.8.9 world generation: RNG -> noise -> biome layers -> terrain -> structures -> decoration.'
  'mc-world'    = 'Chunk storage, block states, light engine, tick scheduling.'
  'mc-entity'   = 'Entities, collision/physics, mob AI, spawning.'
  'mc-server'   = 'Authoritative deterministic single-threaded tick loop; server-side gameplay.'
  'mc-client'   = 'Client prediction, input, camera, and the integrated singleplayer server.'
  'mc-render'   = 'wgpu renderer, chunk meshing, entity/block/GUI rendering, culling.'
  'mc-assets'   = 'Reads textures/models/lang from the user-owned 1.8.9 client jar.'
}

$deps = [ordered]@{
  'mc-core'     = @()
  'mc-nbt'      = @('mc-core')
  'mc-proto'    = @('mc-core','mc-nbt')
  'mc-worldgen' = @('mc-core')
  'mc-world'    = @('mc-core','mc-nbt','mc-worldgen')
  'mc-entity'   = @('mc-core','mc-world')
  'mc-server'   = @('mc-core','mc-world','mc-entity','mc-proto')
  'mc-client'   = @('mc-core','mc-world','mc-entity','mc-render','mc-assets','mc-proto')
  'mc-render'   = @('mc-core','mc-world','mc-assets')
  'mc-assets'   = @('mc-core')
}

foreach ($c in $crateDocs.Keys) {
  $dir = Join-Path $root ("crates/" + $c)
  New-Item -ItemType Directory -Force -Path (Join-Path $dir 'src') | Out-Null
  $depLines = ($deps[$c] | ForEach-Object { "$_ = { workspace = true }" }) -join "`r`n"
  $toml = "[package]`r`nname = ""$c""`r`nversion.workspace = true`r`nedition.workspace = true`r`npublish = false`r`n"
  if ($depLines) { $toml = $toml + "`r`n[dependencies]`r`n" + $depLines + "`r`n" }
  [System.IO.File]::WriteAllText((Join-Path $dir 'Cargo.toml'), $toml)
  $lib = "//! " + $c + " - " + $crateDocs[$c] + "`r`n`r`n//! See docs/ARCHITECTURE.md and plan.md for the two gates (parity + performance).`r`n"
  [System.IO.File]::WriteAllText((Join-Path $dir 'src/lib.rs'), $lib)
}

$ws = @(
  '[workspace]',
  'resolver = "2"',
  'members = ["crates/*"]',
  '',
  '[workspace.package]',
  'version = "0.0.0"',
  'edition = "2021"',
  '',
  '[workspace.dependencies]',
  'mc-core     = { path = "crates/mc-core" }',
  'mc-nbt      = { path = "crates/mc-nbt" }',
  'mc-proto    = { path = "crates/mc-proto" }',
  'mc-worldgen = { path = "crates/mc-worldgen" }',
  'mc-world    = { path = "crates/mc-world" }',
  'mc-entity   = { path = "crates/mc-entity" }',
  'mc-server   = { path = "crates/mc-server" }',
  'mc-client   = { path = "crates/mc-client" }',
  'mc-render   = { path = "crates/mc-render" }',
  'mc-assets   = { path = "crates/mc-assets" }',
  '',
  '[profile.dev]',
  'debug = 1',
  '',
  '[profile.release]',
  'lto = "thin"'
) -join "`r`n"
[System.IO.File]::WriteAllText((Join-Path $root 'Cargo.toml'), $ws + "`r`n")

$toolchain = @(
  '[toolchain]',
  'channel = "stable"',
  'components = ["rustfmt", "clippy"]'
) -join "`r`n"
[System.IO.File]::WriteAllText((Join-Path $root 'rust-toolchain.toml'), $toolchain + "`r`n")

$readme = @(
  '# Minecraft 1.8.9 - Rust Reimplementation',
  '',
  'A from-scratch Rust rewrite of Minecraft: Java Edition 1.8.9 with **1:1 gameplay and',
  'visuals**, clean internals, and Horizon-class performance.',
  '',
  '## Status',
  '',
  '- Inputs downloaded and verified: 1.8.9 jars, clean vanilla decompile, MCP stable_22, protocol data.',
  '- Workspace scaffolded (10 crates). Next: M0 oracle driver mod + first harness (random_parity).',
  '',
  '## Layout',
  '',
  '```',
  'crates/   mc-core mc-nbt mc-proto mc-worldgen mc-world mc-entity',
  '          mc-server mc-client mc-render mc-assets',
  'oracle/   Java 1.8.9 driver mod + harnesses (never shipped)',
  'tools/    parity / port ledger / bench tooling',
  'fixtures/ seeds, scenes, packet captures (no Mojang assets)',
  'vendor/   horizon/ (LGPL reference) + minecraft/1.8.9/ (local inputs, gitignored)',
  'docs/     architecture and process',
  '```',
  '',
  '## Build',
  '',
  '```sh',
  'cargo check',
  '```',
  '',
  '## Legal',
  '',
  'No Mojang code or assets are committed. Downloaded jars, mappings, decompiled sources and',
  'reference data are local and gitignored. Reused Horizon code is LGPL-3.0 - see',
  'vendor/horizon/REUSE-NOTES.md.'
) -join "`r`n"
[System.IO.File]::WriteAllText((Join-Path $root 'README.md'), $readme + "`r`n")

$arch = @(
  '# Architecture (scaffold)',
  '',
  'The tick is deterministic and single-threaded. Parallelism goes to rendering, meshing,',
  'lighting and I/O only. Every module has two gates: **parity** (matches real 1.8.9) and',
  '**performance** (budgets, no "Java in Rust").',
  '',
  '| Crate | Role | Depends on |',
  '|---|---|---|',
  '| mc-core | Java-compatible math/RNG/hash + shared types | - |',
  '| mc-nbt | NBT read/write | mc-core |',
  '| mc-proto | Protocol 47 framing/compression/encryption/packets | mc-core, mc-nbt |',
  '| mc-worldgen | Deterministic world generation | mc-core |',
  '| mc-world | Chunk storage, blocks, light, ticks | mc-core, mc-nbt, mc-worldgen |',
  '| mc-entity | Entities, physics, AI, spawning | mc-core, mc-world |',
  '| mc-server | Authoritative tick loop | mc-core, mc-world, mc-entity, mc-proto |',
  '| mc-render | wgpu renderer, meshing, culling | mc-core, mc-world, mc-assets |',
  '| mc-assets | Read assets from own client jar | mc-core |',
  '| mc-client | Prediction, input, camera, integrated server | all of the above |',
  '',
  'See plan.md sections 6-7 for the production line and the architecture contract.'
) -join "`r`n"
New-Item -ItemType Directory -Force -Path (Join-Path $root 'docs') | Out-Null
[System.IO.File]::WriteAllText((Join-Path $root 'docs/ARCHITECTURE.md'), $arch + "`r`n")

$oracle = @(
  '# Oracle (Java 1.8.9)',
  '',
  'Dev-only Forge 1.8.9 driver mod that **only calls** the real game code to produce',
  'canonical outputs (chunks, ticks, packets, frames) for the Rust harnesses. It is never',
  'shipped and never patches worldgen/gameplay.',
  '',
  'Planned harnesses live in tools/test_<name>_parity.py; the first is random_parity,',
  'then noise_parity, then worldgen_seed.',
  '',
  'Uses: vendor/minecraft/1.8.9/forge-mdk/ and the user-owned jars.'
) -join "`r`n"
New-Item -ItemType Directory -Force -Path (Join-Path $root 'oracle') | Out-Null
[System.IO.File]::WriteAllText((Join-Path $root 'oracle/README.md'), $oracle + "`r`n")

New-Item -ItemType Directory -Force -Path (Join-Path $root 'tools') | Out-Null
$tools = @(
  '# Tools',
  '',
  'Adapted from Horizon (vendor/horizon/tools) for 1.8.9:',
  '',
  '- parity.py - runs test_*_parity.py harnesses',
  '- port_ledger.py - vanilla method <-> Rust fn coverage',
  '- parity_shim.py - caches/replays Java oracle runs',
  '- gate.py, check_public_tree.py - merge gate',
  '- benchmark/profiling scripts',
  '',
  'Not copied yet; M0 adapts them to our paths and jar layout.'
) -join "`r`n"
[System.IO.File]::WriteAllText((Join-Path $root 'tools/README.md'), $tools + "`r`n")

$fixtures = @(
  '# Fixtures',
  '',
  'Deterministic inputs for harnesses: seeds, chunk coordinates, redstone scenes, packet',
  'captures. **No Mojang assets or decompiled code here.** Large/generated data stays local',
  'and is gitignored.'
) -join "`r`n"
New-Item -ItemType Directory -Force -Path (Join-Path $root 'fixtures') | Out-Null
[System.IO.File]::WriteAllText((Join-Path $root 'fixtures/README.md'), $fixtures + "`r`n")

Write-Output "scaffold written"
Get-ChildItem -LiteralPath (Join-Path $root 'crates') -Directory | Select-Object -ExpandProperty Name