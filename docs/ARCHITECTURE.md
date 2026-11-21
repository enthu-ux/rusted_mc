# Architecture (scaffold)

The tick is deterministic and single-threaded. Parallelism goes to rendering, meshing,
lighting and I/O only. Every module has two gates: **parity** (matches real 1.8.9) and
**performance** (budgets, no "Java in Rust").

| Crate | Role | Depends on |
|---|---|---|
| mc-core | Java-compatible math/RNG/hash + shared types | - |
| mc-nbt | NBT read/write | mc-core |
| mc-proto | Protocol 47 framing/compression/encryption/packets | mc-core, mc-nbt |
| mc-worldgen | Deterministic world generation | mc-core |
| mc-world | Chunk storage, blocks, light, ticks | mc-core, mc-nbt, mc-worldgen |
| mc-entity | Entities, physics, AI, spawning | mc-core, mc-world |
| mc-server | Authoritative tick loop | mc-core, mc-world, mc-entity, mc-proto |
| mc-render | wgpu (Vulkan) renderer, meshing, culling | mc-core, mc-world, mc-assets |
| mc-assets | Read assets from own client jar | mc-core |
| mc-client | Prediction, input, camera, integrated server | all of the above |

## Rendering backend

Rendering targets **Vulkan via `wgpu`** (Horizon's `gfx` device layer) — **not OpenGL/LWJGL**.
The API swap is internal: parity is judged on **pixels** by the render harness, so it is not a
registered deviation. GPU culling / indirect draws / device layer come from `vendor/horizon`.

## Lighting engine

A clean rewrite, **not** a port of 1.8.9's `World`/`Chunk`/`NibbleArray` flood fill. Data-oriented:
packed nibble sky/block-light planes per section, incremental queued propagation, cached
heightmaps, zero per-tick allocation, and parallel relight of independent regions
(propose-parallel / commit-serial). Light **levels** stay bit-identical (`light_parity`); any
rendered-light change (smoothing/AO/colour) is a presentation deviation, off by default.

See plan.md sections 6-7 for the production line and the architecture contract.
