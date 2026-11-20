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
| mc-render | wgpu renderer, meshing, culling | mc-core, mc-world, mc-assets |
| mc-assets | Read assets from own client jar | mc-core |
| mc-client | Prediction, input, camera, integrated server | all of the above |

See plan.md sections 6-7 for the production line and the architecture contract.
