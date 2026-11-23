# Oracle (Java 1.8.9)

Dev-only Forge 1.8.9 driver mod that **only calls** the real game code to produce
canonical outputs (chunks, ticks, packets, frames) for the Rust harnesses. It is never
shipped and never patches worldgen/gameplay.

Planned harnesses live in tools/test_<name>_parity.py; so far: random_parity,
noise_parity, noise_batch_parity, biome_layer_parity, terrain_parity, caves_parity,
worldgen_parity, structures_parity, jhash_parity, loot_parity, mineshaft_placement_parity,
decoration_parity, math_parity.

Uses: vendor/minecraft/1.8.9/forge-mdk/ and the user-owned jars.
