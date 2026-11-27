//! `ChunkProviderGenerate.populate`'s non-decoration stages: `WorldGenLakes` (water + lava),
//! `WorldGenDungeons`, and the freeze/snow pass (`World.canBlockFreezeWater` / `canSnowAt`).
//!
//! These run on the shared [`DecorateWorld`] (through the pass's single `Random`, after the
//! structure generators and before/after `BiomeGenBase.decorate`) - see
//! [`crate::terrain::TerrainGen::populate_chunk`]. Ported from the decompiled vanilla
//! `net.minecraft.world.gen.feature.{WorldGenLakes,WorldGenDungeons}` and
//! `net.minecraft.world.World.{canBlockFreeze,canSnowAt}`. The dungeon chest is a 27-slot
//! `TileEntityChest` filled with 8 stacks; its contents are drawn only for the RNG sequence (the
//! chunk artefact is blocks + metadata).

use mc_core::random::JavaRandom;

use crate::decorate::blocks::{
    material_is_liquid, material_is_solid, COBBLESTONE, DIRT, GRASS, ICE, MOSSY_COBBLESTONE,
    MYCELIUM, SNOW_LAYER, STONE,
};
pub use crate::decorate::blocks::{LAVA, WATER};
use crate::decorate::DecorateWorld;
use crate::loot::chest::{
    enchanted_book_content, generate_chest_contents, with_extra, DUNGEON_CHEST_CONTENT,
};
use crate::structure::placement::chest_facing_meta_with;
use crate::structure::Facing;

/// `Blocks.chest`.
const CHEST: i32 = 54;
/// `Blocks.mob_spawner`.
const MOB_SPAWNER: i32 = 52;
/// `WorldGenDungeons.SPAWNERTYPES.length`.
const SPAWNER_TYPES: i32 = 4;

/// `WorldGenLakes.generate`: carve a small ellipsoid lake of `block` (water/lava) around a position.
///
/// `(px, py, pz)` is the `populate` position (`chunk*16 + 8..23` on X/Z, a random y). The generator
/// first walks down to the local surface, then fills a union of 4..7 ellipsoids in a 16x8x16 box,
/// validating the shell, replacing air above y=4 and `block` below, then grass/mycelium, a lava
/// stone shell and (for water) the ice pass.
pub fn generate_lake(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    block: i32,
    px: i32,
    py: i32,
    pz: i32,
) -> bool {
    let ox = px - 8;
    let mut oy = py;
    let oz = pz - 8;

    // `while (pos.y > 5 && world.isAirBlock(pos)) pos = pos.down();`
    while oy > 5 && world.is_air(ox, oy, oz) {
        oy -= 1;
    }
    if oy <= 4 {
        return false;
    }
    oy -= 4;

    let mut mask = [false; 2048];
    let count = rand.next_int_bound(4) + 4;
    for _ in 0..count {
        // Draw order: x radius, y radius, z radius, x centre, y centre, z centre.
        let dx_r = rand.next_double() * 6.0 + 3.0;
        let dy_r = rand.next_double() * 4.0 + 2.0;
        let dz_r = rand.next_double() * 6.0 + 3.0;
        let cx = rand.next_double() * (16.0 - dx_r - 2.0) + 1.0 + dx_r / 2.0;
        let cy = rand.next_double() * (8.0 - dy_r - 4.0) + 2.0 + dy_r / 2.0;
        let cz = rand.next_double() * (16.0 - dz_r - 2.0) + 1.0 + dz_r / 2.0;

        for ix in 1..15i32 {
            for iz in 1..15i32 {
                for iy in 1..7i32 {
                    let nx = (ix as f64 - cx) / (dx_r / 2.0);
                    let ny = (iy as f64 - cy) / (dy_r / 2.0);
                    let nz = (iz as f64 - cz) / (dz_r / 2.0);
                    if nx * nx + ny * ny + nz * nz < 1.0 {
                        mask[((ix * 16 + iz) * 8 + iy) as usize] = true;
                    }
                }
            }
        }
    }

    // Shell validation: no liquid in the upper half, no unsupported (non-solid, non-lake) block below.
    for ix in 0..16i32 {
        for iz in 0..16i32 {
            for iy in 0..8i32 {
                let idx = ((ix * 16 + iz) * 8 + iy) as usize;
                let edge = !mask[idx]
                    && ((ix < 15 && mask[(((ix + 1) * 16 + iz) * 8 + iy) as usize])
                        || (ix > 0 && mask[(((ix - 1) * 16 + iz) * 8 + iy) as usize])
                        || (iz < 15 && mask[((ix * 16 + iz + 1) * 8 + iy) as usize])
                        || (iz > 0 && mask[((ix * 16 + (iz - 1)) * 8 + iy) as usize])
                        || (iy < 7 && mask[((ix * 16 + iz) * 8 + iy + 1) as usize])
                        || (iy > 0 && mask[((ix * 16 + iz) * 8 + (iy - 1)) as usize]));
                if edge {
                    let id = (world.get(ox + ix, oy + iy, oz + iz) >> 4) as i32;
                    if iy >= 4 && material_is_liquid(id) {
                        return false;
                    }
                    if iy < 4 && !material_is_solid(id) && id != block {
                        return false;
                    }
                }
            }
        }
    }

    // Fill: air above y=4, the lake block below.
    for ix in 0..16i32 {
        for iz in 0..16i32 {
            for iy in 0..8i32 {
                if mask[((ix * 16 + iz) * 8 + iy) as usize] {
                    let v = if iy >= 4 { 0 } else { (block << 4) as u16 };
                    world.set(ox + ix, oy + iy, oz + iz, v);
                }
            }
        }
    }

    // Grass/mycelium under the lake edge (dirt below a marked cell with sky light).
    for ix in 0..16i32 {
        for iz in 0..16i32 {
            for iy in 4..8i32 {
                if mask[((ix * 16 + iz) * 8 + iy) as usize] {
                    let bx = ox + ix;
                    let by = oy + iy - 1;
                    let bz = oz + iz;
                    if (world.get(bx, by, bz) >> 4) as i32 == DIRT
                        && world.get_light(ox + ix, oy + iy, oz + iz) > 0
                    {
                        let top = (crate::terrain::biome_top_filler(world.biome_at(bx, bz)).0 >> 4)
                            as i32;
                        let v = if top == MYCELIUM { MYCELIUM } else { GRASS };
                        world.set(bx, by, bz, (v << 4) as u16);
                    }
                }
            }
        }
    }

    // Lava lakes get a stone shell around the marked cells.
    if block == LAVA {
        for ix in 0..16i32 {
            for iz in 0..16i32 {
                for iy in 0..8i32 {
                    let idx = ((ix * 16 + iz) * 8 + iy) as usize;
                    let edge = !mask[idx]
                        && ((ix < 15 && mask[(((ix + 1) * 16 + iz) * 8 + iy) as usize])
                            || (ix > 0 && mask[(((ix - 1) * 16 + iz) * 8 + iy) as usize])
                            || (iz < 15 && mask[((ix * 16 + iz + 1) * 8 + iy) as usize])
                            || (iz > 0 && mask[((ix * 16 + (iz - 1)) * 8 + iy) as usize])
                            || (iy < 7 && mask[((ix * 16 + iz) * 8 + iy + 1) as usize])
                            || (iy > 0 && mask[((ix * 16 + iz) * 8 + (iy - 1)) as usize]));
                    // Java short-circuits: `nextInt(2)` is drawn only when `iy >= 4`.
                    let place = edge
                        && (iy < 4 || rand.next_int_bound(2) != 0)
                        && material_is_solid((world.get(ox + ix, oy + iy, oz + iz) >> 4) as i32);
                    if place {
                        world.set(ox + ix, oy + iy, oz + iz, (STONE << 4) as u16);
                    }
                }
            }
        }
    }

    // Water lakes freeze their surface layer.
    if block == WATER {
        for ix in 0..16i32 {
            for iz in 0..16i32 {
                if world.can_block_freeze_water(ox + ix, oy + 4, oz + iz) {
                    world.set(ox + ix, oy + 4, oz + iz, (ICE << 4) as u16);
                }
            }
        }
    }

    true
}

/// `WorldGenDungeons.generate`: carve a cobblestone room, place up to two loot chests and a mob
/// spawner at `(cx, cy, cz)`. The chest contents + spawner name are drawn even though the artefact
/// only records blocks.
pub fn generate_dungeon(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    cx: i32,
    cy: i32,
    cz: i32,
) -> bool {
    let rx = rand.next_int_bound(2) + 2;
    let x_min = -rx - 1;
    let x_max = rx + 1;
    let zr = rand.next_int_bound(2) + 2;
    let z_min = -zr - 1;
    let z_max = zr + 1;

    // Count the open boundary cells (1..=5 required).
    let mut openings = 0;
    for ix in x_min..=x_max {
        for iy in -1..=4 {
            for iz in z_min..=z_max {
                let id = (world.get(cx + ix, cy + iy, cz + iz) >> 4) as i32;
                let solid = material_is_solid(id);
                if iy == -1 && !solid {
                    return false;
                }
                if iy == 4 && !solid {
                    return false;
                }
                if (ix == x_min || ix == x_max || iz == z_min || iz == z_max)
                    && iy == 0
                    && world.is_air(cx + ix, cy, cz + iz)
                    && world.is_air(cx + ix, cy + 1, cz + iz)
                {
                    openings += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&openings) {
        return false;
    }

    for ix in x_min..=x_max {
        for iy in (-1..=3).rev() {
            for iz in z_min..=z_max {
                let px = cx + ix;
                let py = cy + iy;
                let pz = cz + iz;
                let interior =
                    ix != x_min && iy != -1 && iz != z_min && ix != x_max && iy != 4 && iz != z_max;
                if interior {
                    if (world.get(px, py, pz) >> 4) as i32 != CHEST {
                        world.set(px, py, pz, 0);
                    }
                } else if py >= 0 && !material_is_solid((world.get(px, py - 1, pz) >> 4) as i32) {
                    world.set(px, py, pz, 0);
                } else if material_is_solid((world.get(px, py, pz) >> 4) as i32)
                    && (world.get(px, py, pz) >> 4) as i32 != CHEST
                {
                    if iy == -1 && rand.next_int_bound(4) != 0 {
                        world.set(px, py, pz, (MOSSY_COBBLESTONE << 4) as u16);
                    } else {
                        world.set(px, py, pz, (COBBLESTONE << 4) as u16);
                    }
                }
            }
        }
    }

    for _ in 0..2 {
        for _ in 0..3 {
            let px = cx + rand.next_int_bound(rx * 2 + 1) - rx;
            let pz = cz + rand.next_int_bound(zr * 2 + 1) - zr;
            if world.is_air(px, cy, pz) {
                let mut solids = 0;
                for f in [Facing::North, Facing::East, Facing::South, Facing::West] {
                    let (dx, dz) = f.offset();
                    if material_is_solid((world.get(px + dx, cy, pz + dz) >> 4) as i32) {
                        solids += 1;
                    }
                }
                if solids == 1 {
                    let meta = chest_facing_meta_with(|a, b, c| world.get(a, b, c), px, cy, pz);
                    world.set(px, cy, pz, ((CHEST as u16) << 4) | meta);
                    let book = enchanted_book_content(rand);
                    let list = with_extra(&DUNGEON_CHEST_CONTENT, book);
                    generate_chest_contents(rand, &list, 27, 8, |_, _, _| {});
                    break;
                }
            }
        }
    }

    world.set(cx, cy, cz, (MOB_SPAWNER << 4) as u16);
    let _ = rand.next_int_bound(SPAWNER_TYPES);
    true
}

/// The tail of `ChunkProviderGenerate.populate`: for every column of the `+8`-offset 16x16 window,
/// freeze a still water surface to ice and lay a snow layer where eligible. No RNG.
pub fn freeze_and_snow(world: &mut DecorateWorld, chunk_x: i32, chunk_z: i32) {
    let bx = (chunk_x << 4) + 8;
    let bz = (chunk_z << 4) + 8;
    for x in 0..16 {
        for z in 0..16 {
            let px = bx + x;
            let pz = bz + z;
            let top = world.precipitation_height(px, pz);
            let below = top - 1;
            if world.can_block_freeze_water(px, below, pz) {
                world.set(px, below, pz, (ICE << 4) as u16);
            }
            if world.can_snow_at(px, top, pz) {
                world.set(px, top, pz, (SNOW_LAYER << 4) as u16);
            }
        }
    }
}
