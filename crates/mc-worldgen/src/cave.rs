//! Port of the 1.8.9 cave and ravine carving passes (`MapGenBase`, `MapGenCaves`,
//! `MapGenRavine`).
//!
//! `provideChunk` order (see `ChunkProviderGenerate.provideChunk`): shape -> surface replacement
//! -> caves -> ravines. Both passes walk a 17x17 neighbourhood of *neighbour* chunks (`range = 8`)
//! and carve only within the chunk being generated; each neighbour gets its own RNG stream seeded
//! from `worldGenSeed`, and each tunnel gets a local `Random(nextLong)`.
//!
//! Only the target chunk's biome ids are consulted (`World.getBiomeGenForCoords` is always called
//! with a target-chunk coordinate). Vanilla reaches them through the biome cache
//! (`BiomeCache.Block.getBiomeGenAt`), which indexes the Voronoi array as `x + z * 16` - *not*
//! transposed, unlike `ChunkProviderGenerate.replaceBlocksForBiome` (which reads `z + x * 16`).
//! The two conventions coexist in vanilla; this module follows the cache's.
//!
//! `ChunkPrimer` states are `id << 4 | meta`, indexed `x << 12 | z << 8 | y`.

use mc_core::jmath;
use mc_core::random::JavaRandom;
use std::f32::consts::{FRAC_PI_2, PI};

use crate::mapgen::{self, RANGE};
use crate::terrain::biome_top_filler;

// Block ids (states are `id << 4 | meta`).
const ID_STONE: u16 = 1;
const ID_GRASS: u16 = 2;
const ID_DIRT: u16 = 3;
const ID_FLOWING_WATER: u16 = 8;
const ID_WATER: u16 = 9;
const ID_FLOWING_LAVA: u16 = 10;
const ID_LAVA: u16 = 11;
const ID_SAND: u16 = 12;
const ID_GRAVEL: u16 = 13;
const ID_SANDSTONE: u16 = 24;
const ID_SNOW_LAYER: u16 = 78;
const ID_MYCELIUM: u16 = 110;
const ID_STAINED_CLAY: u16 = 159;
const ID_HARDENED_CLAY: u16 = 172;
const ID_RED_SANDSTONE: u16 = 179;

const AIR: u16 = 0;
const SANDSTONE: u16 = ID_SANDSTONE << 4;
const RED_SANDSTONE: u16 = ID_RED_SANDSTONE << 4;
const LAVA: u16 = ID_LAVA << 4; // Blocks.lava (caves)
const FLOWING_LAVA: u16 = ID_FLOWING_LAVA << 4; // Blocks.flowing_lava (ravines)

#[inline]
fn block_id(v: u16) -> u16 {
    v >> 4
}

/// `Blocks.flowing_water` / `Blocks.water` (`Material.water`).
#[inline]
fn is_water(v: u16) -> bool {
    let id = block_id(v);
    id == ID_FLOWING_WATER || id == ID_WATER
}

#[inline]
fn get(primer: &[u16], x: i32, y: i32, z: i32) -> u16 {
    primer[((x << 12) | (z << 8) | y) as usize]
}

#[inline]
fn set(primer: &mut [u16], x: i32, y: i32, z: i32, v: u16) {
    primer[((x << 12) | (z << 8) | y) as usize] = v;
}

/// The biome id the game reports for a target-chunk-local column, via the biome cache layout.
#[inline]
fn biome_at(biomes: &[i32], x: i32, z: i32) -> i32 {
    biomes[(x + z * 16) as usize]
}

/// `BiomeGenBase.topBlock.getBlock().getDefaultState()` - the top *block* with its default
/// metadata. For mesa biomes `topBlock` is red sand (`sand` + variant 1), so this differs from
/// `BiomeGenBase.topBlock` itself; caves use this form and ravines do not.
#[inline]
fn biome_top_block_default(biome: i32) -> u16 {
    biome_top_filler(biome).0 & !15
}

/// `MapGenCaves`. (`MapGenBase` state is only the RNG, reseeded per call.)
pub struct Caves {
    rand: JavaRandom,
}

impl Default for Caves {
    fn default() -> Self {
        Self::new()
    }
}

impl Caves {
    pub fn new() -> Caves {
        Caves {
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenCaves.generate` - carves `primer` in place for the target chunk.
    pub fn generate(
        &mut self,
        world_seed: i64,
        target_x: i32,
        target_z: i32,
        primer: &mut [u16],
        biomes: &[i32],
    ) {
        let rand = &mut self.rand;
        mapgen::generate(world_seed, rand, target_x, target_z, |rand, nx, nz| {
            caves_recursive_generate(rand, nx, nz, target_x, target_z, primer, biomes);
        });
    }
}

/// `MapGenCaves.recursiveGenerate`.
fn caves_recursive_generate(
    rand: &mut JavaRandom,
    nx: i32,
    nz: i32,
    target_x: i32,
    target_z: i32,
    primer: &mut [u16],
    biomes: &[i32],
) {
    let inner = rand.next_int_bound(15) + 1;
    let mid = rand.next_int_bound(inner) + 1;
    let mut count = rand.next_int_bound(mid);
    if rand.next_int_bound(7) != 0 {
        count = 0;
    }

    for _ in 0..count {
        let x = nx as f64 * 16.0 + rand.next_int_bound(16) as f64;
        let depth = rand.next_int_bound(120) + 8;
        let y = rand.next_int_bound(depth) as f64;
        let z = nz as f64 * 16.0 + rand.next_int_bound(16) as f64;
        let mut tunnels = 1;
        if rand.next_int_bound(4) == 0 {
            // `func_180703_a(nextLong, ...)` -> `func_180702_a(..., 1.0F + rand.nextFloat() * 6.0F,
            // 0, 0, -1, -1, 0.5)`: the caller's stream supplies the seed, then the width.
            cave_tunnel(
                rand.next_long(),
                target_x,
                target_z,
                primer,
                biomes,
                x,
                y,
                z,
                1.0 + rand.next_float() * 6.0,
                0.0,
                0.0,
                -1,
                -1,
                0.5,
            );
            tunnels += rand.next_int_bound(4);
        }

        for _ in 0..tunnels {
            let yaw = rand.next_float() * PI * 2.0;
            let pitch = (rand.next_float() - 0.5) * 2.0 / 8.0;
            let mut horiz = rand.next_float() * 2.0 + rand.next_float();
            if rand.next_int_bound(10) == 0 {
                horiz *= rand.next_float() * rand.next_float() * 3.0 + 1.0;
            }

            cave_tunnel(
                rand.next_long(),
                target_x,
                target_z,
                primer,
                biomes,
                x,
                y,
                z,
                horiz,
                yaw,
                pitch,
                0,
                0,
                1.0,
            );
        }
    }
}

/// `MapGenCaves.func_180702_a` - walk and carve one tunnel.
///
/// The `!(x < a)` / `!(x > b)` guard is vanilla's own negation (it is not the same as `x >= a`
/// when a value is NaN), so it is kept verbatim.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[allow(clippy::too_many_arguments)]
fn cave_tunnel(
    seed: i64,
    target_x: i32,
    target_z: i32,
    primer: &mut [u16],
    biomes: &[i32],
    mut x: f64,
    mut y: f64,
    mut z: f64,
    horiz: f32,
    mut yaw: f32,
    mut pitch: f32,
    mut step: i32,
    mut max_step: i32,
    thick: f64,
) {
    let ox = target_x as f64 * 16.0 + 8.0;
    let oz = target_z as f64 * 16.0 + 8.0;
    let mut yaw_delta = 0.0f32;
    let mut pitch_delta = 0.0f32;
    let mut rand = JavaRandom::new(seed);
    if max_step <= 0 {
        let span = RANGE * 16 - 16;
        max_step = span - rand.next_int_bound(span / 4);
    }

    let mut single = false;
    if step == -1 {
        step = max_step / 2;
        single = true;
    }

    let turn_step = rand.next_int_bound(max_step / 2) + max_step / 4;
    let odd = rand.next_int_bound(6) == 0;

    while step < max_step {
        let radius = 1.5 + (jmath::sin(step as f32 * PI / max_step as f32) * horiz * 1.0) as f64;
        let yscale = radius * thick;
        let c = jmath::cos(pitch);
        let vy = jmath::sin(pitch);
        x += (jmath::cos(yaw) * c) as f64;
        y += vy as f64;
        z += (jmath::sin(yaw) * c) as f64;
        if odd {
            pitch *= 0.92;
        } else {
            pitch *= 0.7;
        }

        pitch += pitch_delta * 0.1;
        yaw += yaw_delta * 0.1;
        pitch_delta *= 0.9;
        yaw_delta *= 0.75;
        pitch_delta += (rand.next_float() - rand.next_float()) * rand.next_float() * 2.0;
        yaw_delta += (rand.next_float() - rand.next_float()) * rand.next_float() * 4.0;
        if !single && step == turn_step && horiz > 1.0 && max_step > 0 {
            cave_tunnel(
                rand.next_long(),
                target_x,
                target_z,
                primer,
                biomes,
                x,
                y,
                z,
                rand.next_float() * 0.5 + 0.5,
                yaw - FRAC_PI_2,
                pitch / 3.0,
                step,
                max_step,
                1.0,
            );
            cave_tunnel(
                rand.next_long(),
                target_x,
                target_z,
                primer,
                biomes,
                x,
                y,
                z,
                rand.next_float() * 0.5 + 0.5,
                yaw + FRAC_PI_2,
                pitch / 3.0,
                step,
                max_step,
                1.0,
            );
            return;
        }

        if single || rand.next_int_bound(4) != 0 {
            let dx = x - ox;
            let dz = z - oz;
            let dl = (max_step - step) as f64;
            let reach = (horiz + 2.0 + 16.0) as f64;
            if dx * dx + dz * dz - dl * dl > reach * reach {
                return;
            }

            if !(x < ox - 16.0 - radius * 2.0)
                && !(z < oz - 16.0 - radius * 2.0)
                && !(x > ox + 16.0 + radius * 2.0)
                && !(z > oz + 16.0 + radius * 2.0)
            {
                let mut x0 = jmath::floor_double(x - radius) - target_x * 16 - 1;
                let mut x1 = jmath::floor_double(x + radius) - target_x * 16 + 1;
                let mut y0 = jmath::floor_double(y - yscale) - 1;
                let mut y1 = jmath::floor_double(y + yscale) + 1;
                let mut z0 = jmath::floor_double(z - radius) - target_z * 16 - 1;
                let mut z1 = jmath::floor_double(z + radius) - target_z * 16 + 1;
                if x0 < 0 {
                    x0 = 0;
                }
                if x1 > 16 {
                    x1 = 16;
                }
                if y0 < 1 {
                    y0 = 1;
                }
                if y1 > 248 {
                    y1 = 248;
                }
                if z0 < 0 {
                    z0 = 0;
                }
                if z1 > 16 {
                    z1 = 16;
                }

                // Bail out if the box already touches water.
                let mut water = false;
                let mut a = x0;
                while !water && a < x1 {
                    let mut b = z0;
                    while !water && b < z1 {
                        let mut cy = y1 + 1;
                        while !water && cy >= y0 - 1 {
                            if (0..256).contains(&cy) {
                                if is_water(get(primer, a, cy, b)) {
                                    water = true;
                                }
                                if cy != y0 - 1 && a != x0 && a != x1 - 1 && b != z0 && b != z1 - 1
                                {
                                    cy = y0;
                                }
                            }
                            cy -= 1;
                        }
                        b += 1;
                    }
                    a += 1;
                }

                if !water {
                    let mut a = x0;
                    while a < x1 {
                        let fx = (a as f64 + target_x as f64 * 16.0 + 0.5 - x) / radius;
                        let mut b = z0;
                        while b < z1 {
                            let fz = (b as f64 + target_z as f64 * 16.0 + 0.5 - z) / radius;
                            let mut grass = false;
                            if fx * fx + fz * fz < 1.0 {
                                let mut cy = y1;
                                while cy > y0 {
                                    let fy = (cy as f64 - 1.0 + 0.5 - y) / yscale;
                                    if fy > -0.7 && fx * fx + fy * fy + fz * fz < 1.0 {
                                        let st = get(primer, a, cy, b);
                                        let above = get(primer, a, cy + 1, b);
                                        if block_id(st) == ID_GRASS || block_id(st) == ID_MYCELIUM {
                                            grass = true;
                                        }

                                        if carveable(st, above) {
                                            if cy - 1 < 10 {
                                                set(primer, a, cy, b, LAVA);
                                            } else {
                                                set(primer, a, cy, b, AIR);
                                                if block_id(above) == ID_SAND {
                                                    set(
                                                        primer,
                                                        a,
                                                        cy + 1,
                                                        b,
                                                        if (above & 15) == 1 {
                                                            RED_SANDSTONE
                                                        } else {
                                                            SANDSTONE
                                                        },
                                                    );
                                                }

                                                if grass
                                                    && block_id(get(primer, a, cy - 1, b))
                                                        == ID_DIRT
                                                {
                                                    set(
                                                        primer,
                                                        a,
                                                        cy - 1,
                                                        b,
                                                        biome_top_block_default(biome_at(
                                                            biomes, a, b,
                                                        )),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    cy -= 1;
                                }
                            }
                            b += 1;
                        }
                        a += 1;
                    }

                    if single {
                        break;
                    }
                }
            }
        }

        step += 1;
    }
}

/// `MapGenCaves.func_175793_a`.
fn carveable(st: u16, above: u16) -> bool {
    match block_id(st) {
        ID_STONE | ID_DIRT | ID_GRASS | ID_HARDENED_CLAY | ID_STAINED_CLAY | ID_SANDSTONE
        | ID_RED_SANDSTONE | ID_MYCELIUM | ID_SNOW_LAYER => true,
        ID_SAND | ID_GRAVEL => !is_water(above),
        _ => false,
    }
}

/// `MapGenRavine`.
pub struct Ravines {
    rand: JavaRandom,
}

impl Default for Ravines {
    fn default() -> Self {
        Self::new()
    }
}

impl Ravines {
    pub fn new() -> Ravines {
        Ravines {
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenRavine.generate`.
    pub fn generate(
        &mut self,
        world_seed: i64,
        target_x: i32,
        target_z: i32,
        primer: &mut [u16],
        biomes: &[i32],
    ) {
        let rand = &mut self.rand;
        mapgen::generate(world_seed, rand, target_x, target_z, |rand, nx, nz| {
            ravines_recursive_generate(rand, nx, nz, target_x, target_z, primer, biomes);
        });
    }
}

/// `MapGenRavine.recursiveGenerate`.
fn ravines_recursive_generate(
    rand: &mut JavaRandom,
    nx: i32,
    nz: i32,
    target_x: i32,
    target_z: i32,
    primer: &mut [u16],
    biomes: &[i32],
) {
    if rand.next_int_bound(50) == 0 {
        let x = nx as f64 * 16.0 + rand.next_int_bound(16) as f64;
        let depth = rand.next_int_bound(40) + 8;
        let y = rand.next_int_bound(depth) as f64 + 20.0;
        let z = nz as f64 * 16.0 + rand.next_int_bound(16) as f64;

        for _ in 0..1 {
            let yaw = rand.next_float() * PI * 2.0;
            let pitch = (rand.next_float() - 0.5) * 2.0 / 8.0;
            let horiz = (rand.next_float() * 2.0 + rand.next_float()) * 2.0;
            ravine_tunnel(
                rand.next_long(),
                target_x,
                target_z,
                primer,
                biomes,
                x,
                y,
                z,
                horiz,
                yaw,
                pitch,
                0,
                0,
                3.0,
            );
        }
    }
}

/// `MapGenRavine.func_180707_a` - walk and carve one ravine.
///
/// As in caves, the boundary guard is vanilla's `!(...)` form and is kept verbatim.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[allow(clippy::too_many_arguments)]
fn ravine_tunnel(
    seed: i64,
    target_x: i32,
    target_z: i32,
    primer: &mut [u16],
    biomes: &[i32],
    mut x: f64,
    mut y: f64,
    mut z: f64,
    horiz: f32,
    mut yaw: f32,
    mut pitch: f32,
    mut step: i32,
    mut max_step: i32,
    thick: f64,
) {
    // `field_75046_d`: a per-instance scratch array, fully rewritten below for indices 0..255.
    let mut field = [0.0f32; 256];
    let mut rand = JavaRandom::new(seed);
    let ox = target_x as f64 * 16.0 + 8.0;
    let oz = target_z as f64 * 16.0 + 8.0;
    let mut yaw_delta = 0.0f32;
    let mut pitch_delta = 0.0f32;
    if max_step <= 0 {
        let span = RANGE * 16 - 16;
        max_step = span - rand.next_int_bound(span / 4);
    }

    let mut single = false;
    if step == -1 {
        step = max_step / 2;
        single = true;
    }

    let mut width = 1.0f32;
    for (i, slot) in field.iter_mut().enumerate() {
        if i == 0 || rand.next_int_bound(3) == 0 {
            width = 1.0 + rand.next_float() * rand.next_float() * 1.0;
        }
        *slot = width * width;
    }

    while step < max_step {
        let mut radius =
            1.5 + (jmath::sin(step as f32 * PI / max_step as f32) * horiz * 1.0) as f64;
        let mut yscale = radius * thick;
        radius *= rand.next_float() as f64 * 0.25 + 0.75;
        yscale *= rand.next_float() as f64 * 0.25 + 0.75;
        let c = jmath::cos(pitch);
        let vy = jmath::sin(pitch);
        x += (jmath::cos(yaw) * c) as f64;
        y += vy as f64;
        z += (jmath::sin(yaw) * c) as f64;
        pitch *= 0.7;
        pitch += pitch_delta * 0.05;
        yaw += yaw_delta * 0.05;
        pitch_delta *= 0.8;
        yaw_delta *= 0.5;
        pitch_delta += (rand.next_float() - rand.next_float()) * rand.next_float() * 2.0;
        yaw_delta += (rand.next_float() - rand.next_float()) * rand.next_float() * 4.0;

        if single || rand.next_int_bound(4) != 0 {
            let dx = x - ox;
            let dz = z - oz;
            let dl = (max_step - step) as f64;
            let reach = (horiz + 2.0 + 16.0) as f64;
            if dx * dx + dz * dz - dl * dl > reach * reach {
                return;
            }

            if !(x < ox - 16.0 - radius * 2.0)
                && !(z < oz - 16.0 - radius * 2.0)
                && !(x > ox + 16.0 + radius * 2.0)
                && !(z > oz + 16.0 + radius * 2.0)
            {
                let mut x0 = jmath::floor_double(x - radius) - target_x * 16 - 1;
                let mut x1 = jmath::floor_double(x + radius) - target_x * 16 + 1;
                let mut y0 = jmath::floor_double(y - yscale) - 1;
                let mut y1 = jmath::floor_double(y + yscale) + 1;
                let mut z0 = jmath::floor_double(z - radius) - target_z * 16 - 1;
                let mut z1 = jmath::floor_double(z + radius) - target_z * 16 + 1;
                if x0 < 0 {
                    x0 = 0;
                }
                if x1 > 16 {
                    x1 = 16;
                }
                if y0 < 1 {
                    y0 = 1;
                }
                if y1 > 248 {
                    y1 = 248;
                }
                if z0 < 0 {
                    z0 = 0;
                }
                if z1 > 16 {
                    z1 = 16;
                }

                let mut water = false;
                let mut a = x0;
                while !water && a < x1 {
                    let mut b = z0;
                    while !water && b < z1 {
                        let mut cy = y1 + 1;
                        while !water && cy >= y0 - 1 {
                            if (0..256).contains(&cy) {
                                if is_water(get(primer, a, cy, b)) {
                                    water = true;
                                }
                                if cy != y0 - 1 && a != x0 && a != x1 - 1 && b != z0 && b != z1 - 1
                                {
                                    cy = y0;
                                }
                            }
                            cy -= 1;
                        }
                        b += 1;
                    }
                    a += 1;
                }

                if !water {
                    let mut a = x0;
                    while a < x1 {
                        let fx = (a as f64 + target_x as f64 * 16.0 + 0.5 - x) / radius;
                        let mut b = z0;
                        while b < z1 {
                            let fz = (b as f64 + target_z as f64 * 16.0 + 0.5 - z) / radius;
                            let mut grass = false;
                            if fx * fx + fz * fz < 1.0 {
                                let mut cy = y1;
                                while cy > y0 {
                                    let fy = (cy as f64 - 1.0 + 0.5 - y) / yscale;
                                    if (fx * fx + fz * fz) * field[(cy - 1) as usize] as f64
                                        + fy * fy / 6.0
                                        < 1.0
                                    {
                                        let st = get(primer, a, cy, b);
                                        if block_id(st) == ID_GRASS {
                                            grass = true;
                                        }

                                        let id = block_id(st);
                                        if id == ID_STONE || id == ID_DIRT || id == ID_GRASS {
                                            if cy - 1 < 10 {
                                                set(primer, a, cy, b, FLOWING_LAVA);
                                            } else {
                                                set(primer, a, cy, b, AIR);
                                                if grass
                                                    && block_id(get(primer, a, cy - 1, b))
                                                        == ID_DIRT
                                                {
                                                    set(
                                                        primer,
                                                        a,
                                                        cy - 1,
                                                        b,
                                                        biome_top_filler(biome_at(biomes, a, b)).0,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    cy -= 1;
                                }
                            }
                            b += 1;
                        }
                        a += 1;
                    }

                    if single {
                        break;
                    }
                }
            }
        }

        step += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    /// Vanilla's chunk-primer index.
    fn idx(x: usize, z: usize, y: usize) -> usize {
        (x << 12) | (z << 8) | y
    }

    /// Caves carve stone to air (oracle: seed 0, chunk (-169, 30), x=15 z=15 y=34).
    #[test]
    fn caves_carve_stone_to_air() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-169, 30, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(15, 15, 34)], AIR);
    }

    /// Caves carve grass to air (oracle: seed 0, chunk (-3, 0), x=0 z=14 y=67).
    #[test]
    fn caves_carve_grass_to_air() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-3, 0, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(0, 14, 67)], AIR);
    }

    /// Bedrock survives carving (oracle: seed 0, chunk (-169, 30), x=0 z=0 y=0).
    #[test]
    fn caves_leave_bedrock() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-169, 30, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(0, 0, 0)], 7 << 4);
    }

    /// Caves fill below y=11 with lava, not air (oracle: seed 0, chunk (-124, -128),
    /// x=4 z=8 y=10: stone -> `Blocks.lava`).
    #[test]
    fn caves_flood_below_y11_with_lava() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-124, -128, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(4, 8, 10)], 11 << 4);
    }

    /// Caves replace sand under a carved floor with sandstone (oracle: seed -7, chunk (-1, 0),
    /// x=12 z=13 y=64: sand -> sandstone).
    #[test]
    fn caves_replace_sand_with_sandstone() {
        let mut gen = TerrainGen::new(-7);
        let primer = gen.generate_chunk(-1, 0, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(12, 13, 64)], 24 << 4);
    }

    /// Caves rewrite floor dirt to the biome top block (oracle: seed 0, chunk (-3, 0),
    /// x=14 z=0 y=63: dirt -> grass).
    #[test]
    fn caves_rewrite_floor_to_biome_top_block() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-3, 0, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(14, 0, 63)], 2 << 4);
    }

    /// Ravines carve stone to air (oracle: seed 0, chunk (-5, 1), x=0 z=11 y=22).
    #[test]
    fn ravines_carve_stone_to_air() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-5, 1, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(0, 11, 22)], AIR);
    }

    /// A ravine runs after caves (oracle: the same cell is carved with ravines only).
    #[test]
    fn ravine_carves_the_same_cell_alone() {
        let mut gen = TerrainGen::new(0);
        let ravines = GenOptions {
            caves: false,
            ravines: true,
            ..GenOptions::none()
        };
        let primer = gen.generate_chunk(-5, 1, ravines).blocks;
        assert_eq!(primer[idx(0, 11, 22)], AIR);
    }

    /// Ravines flood below y=11 with `Blocks.flowing_lava` (id 10), *not* the `Blocks.lava` (id
    /// 11) caves use (oracle: seed 0, chunk (-7, 7), x=4 z=3 y=10: stone -> flowing lava).
    #[test]
    fn ravines_flood_below_y11_with_flowing_lava() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_chunk(-7, 7, GenOptions::vanilla()).blocks;
        assert_eq!(primer[idx(4, 3, 10)], 10 << 4);
    }

    /// With both passes off, `generate_chunk` must equal the base terrain.
    #[test]
    fn carving_off_matches_base_terrain() {
        let mut gen = TerrainGen::new(0);
        let full = gen.generate_chunk(7, -5, GenOptions::none()).blocks;
        let base = gen.generate_base_chunk(7, -5);
        assert_eq!(full, base);
    }

    /// `BiomeGenMesa.topBlock` is red sand (`sand` variant 1). Caves write
    /// `topBlock.getBlock().getDefaultState()` (plain sand, `MapGenCaves:318-322`) while ravines
    /// write `topBlock` itself (`MapGenRavine:247`), so the two passes disagree on mesa floors.
    /// No corpus chunk carves a mesa floor, so the transcription difference is pinned here.
    #[test]
    fn mesa_floor_top_block_differs_between_caves_and_ravines() {
        for mesa in [37, 38, 39, 165, 166, 167] {
            assert_eq!(
                biome_top_filler(mesa).0,
                (12 << 4) | 1,
                "mesa {mesa} top block"
            );
            assert_eq!(
                biome_top_block_default(mesa),
                12 << 4,
                "mesa {mesa} cave floor"
            );
        }
    }
}
