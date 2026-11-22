//! Decoration (the `BiomeDecorator` half of `populate`).
//!
//! This is being ported in layers; each layer is parity-gated by `decoration_parity`
//! (`tools/test_decoration_parity.py`) with a mode. Implemented so far:
//!
//! * `ores` — `BiomeDecorator.generateOres` (`WorldGenMinable` over the ore table).
//! * `surface` — the `WorldGenSand`/`WorldGenClay`/gravel-as-sand loops.
//!
//! Everything else in `BiomeDecorator.genDecorations` (trees, flowers, grass, dead bush, waterlily,
//! mushrooms, reeds, pumpkin, cactus, springs) and the per-biome `decorate` overrides are pending.
//!
//! Ported from the decompiled vanilla `net.minecraft.world.biome.BiomeDecorator`,
//! `net.minecraft.world.gen.feature.{WorldGenMinable,WorldGenSand,WorldGenClay}` and
//! `net.minecraft.world.chunk.Chunk`/`net.minecraft.world.World` (the block read/write and height-map
//! semantics the generators depend on).

use std::collections::BTreeMap;

use mc_core::jmath::{cos, floor_double, sin};
use mc_core::random::JavaRandom;

pub mod blocks;

use blocks::{
    material_blocks_movement, material_is_leaves, material_is_water, CLAY, DIAMOND_ORE, DIRT,
    GRASS, GRAVEL, SAND, STONE,
};

/// `ChunkProviderSettings.Factory` defaults (`func_177863_a`), limited to the fields decoration uses.
#[derive(Clone, Copy, Debug)]
pub struct DecorateSettings {
    pub sea_level: i32,
    pub dirt_size: i32,
    pub dirt_count: i32,
    pub dirt_min_height: i32,
    pub dirt_max_height: i32,
    pub gravel_size: i32,
    pub gravel_count: i32,
    pub gravel_min_height: i32,
    pub gravel_max_height: i32,
    pub granite_size: i32,
    pub granite_count: i32,
    pub granite_min_height: i32,
    pub granite_max_height: i32,
    pub diorite_size: i32,
    pub diorite_count: i32,
    pub diorite_min_height: i32,
    pub diorite_max_height: i32,
    pub andesite_size: i32,
    pub andesite_count: i32,
    pub andesite_min_height: i32,
    pub andesite_max_height: i32,
    pub coal_size: i32,
    pub coal_count: i32,
    pub coal_min_height: i32,
    pub coal_max_height: i32,
    pub iron_size: i32,
    pub iron_count: i32,
    pub iron_min_height: i32,
    pub iron_max_height: i32,
    pub gold_size: i32,
    pub gold_count: i32,
    pub gold_min_height: i32,
    pub gold_max_height: i32,
    pub redstone_size: i32,
    pub redstone_count: i32,
    pub redstone_min_height: i32,
    pub redstone_max_height: i32,
    pub diamond_size: i32,
    pub diamond_count: i32,
    pub diamond_min_height: i32,
    pub diamond_max_height: i32,
    pub lapis_size: i32,
    pub lapis_count: i32,
    pub lapis_center_height: i32,
    pub lapis_spread: i32,
}

impl Default for DecorateSettings {
    fn default() -> DecorateSettings {
        DecorateSettings {
            sea_level: 63,
            dirt_size: 33,
            dirt_count: 10,
            dirt_min_height: 0,
            dirt_max_height: 256,
            gravel_size: 33,
            gravel_count: 8,
            gravel_min_height: 0,
            gravel_max_height: 256,
            granite_size: 33,
            granite_count: 10,
            granite_min_height: 0,
            granite_max_height: 80,
            diorite_size: 33,
            diorite_count: 10,
            diorite_min_height: 0,
            diorite_max_height: 80,
            andesite_size: 33,
            andesite_count: 10,
            andesite_min_height: 0,
            andesite_max_height: 80,
            coal_size: 17,
            coal_count: 20,
            coal_min_height: 0,
            coal_max_height: 128,
            iron_size: 9,
            iron_count: 20,
            iron_min_height: 0,
            iron_max_height: 64,
            gold_size: 9,
            gold_count: 2,
            gold_min_height: 0,
            gold_max_height: 32,
            redstone_size: 8,
            redstone_count: 8,
            redstone_min_height: 0,
            redstone_max_height: 16,
            diamond_size: 8,
            diamond_count: 1,
            diamond_min_height: 0,
            diamond_max_height: 16,
            lapis_size: 7,
            lapis_count: 1,
            lapis_center_height: 16,
            lapis_spread: 16,
        }
    }
}

/// Which layers of `BiomeDecorator.genDecorations` run. Vanilla always runs all of them; the
/// development layers each run a subset so partial ports can be parity-gated (cf. `caves_parity`'s
/// modes). `full` is the shipped behaviour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecorateOptions {
    pub ores: bool,
    pub surface: bool,
}

impl DecorateOptions {
    pub fn none() -> DecorateOptions {
        DecorateOptions {
            ores: false,
            surface: false,
        }
    }

    pub fn vanilla() -> DecorateOptions {
        DecorateOptions {
            ores: true,
            surface: true,
        }
    }
}

/// The `World`/`Chunk` view the feature generators write through. Blocks are `id << 4 | meta`,
/// indexed `x << 12 | z << 8 | y`. Each chunk mirrors `Chunk`'s state: the height map, the
/// precipitation-height cache, which 16-block sections are allocated, and `getTopFilledSegment`.
pub struct DecorateWorld {
    chunks: BTreeMap<(i32, i32), ChunkState>,
    pub sea_level: i32,
}

struct ChunkState {
    blocks: Vec<u16>,
    /// `Chunk.heightMap`, index `z << 4 | x` (one past the highest light-blocking block).
    height: Vec<i32>,
    /// `Chunk.precipitationHeightMap`, index `x + z * 16`; `-999` = stale.
    precip: Vec<i32>,
    /// Allocated `ExtendedBlockStorage` sections (bit `s` = section `s` exists).
    sections: u16,
    /// `Chunk.getTopFilledSegment()` (the highest allocated section's base y).
    top_filled: i32,
}

#[inline]
fn chunk_index(x: i32, y: i32, z: i32) -> usize {
    (((x & 15) as usize) << 12) | (((z & 15) as usize) << 8) | y as usize
}

#[inline]
fn col_index(x: i32, z: i32) -> usize {
    (((z & 15) << 4) | (x & 15)) as usize
}

impl DecorateWorld {
    pub fn new(sea_level: i32) -> DecorateWorld {
        DecorateWorld {
            chunks: BTreeMap::new(),
            sea_level,
        }
    }

    /// Supplies a chunk's generated terrain (`provideChunk`), initialising `Chunk`-equivalent state.
    pub fn insert_chunk(&mut self, chunk_x: i32, chunk_z: i32, blocks: Vec<u16>) {
        let mut sections = 0u16;
        let mut top_filled = 0i32;
        for section in 0..16usize {
            let base = section * 16;
            let has = (base..base + 16).any(|y| {
                (0..16).any(|x| (0..16).any(|z| !matches!(blocks[chunk_index(x, y as i32, z)], 0)))
            });
            if has {
                sections |= 1 << section;
                top_filled = base as i32;
            }
        }
        let mut state = ChunkState {
            blocks,
            height: vec![0; 256],
            precip: vec![-999; 256],
            sections,
            top_filled,
        };
        state.recompute_height();
        self.chunks.insert((chunk_x, chunk_z), state);
    }

    pub fn has_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunks.contains_key(&(chunk_x, chunk_z))
    }

    /// The chunk's block array (`id << 4 | meta`).
    pub fn chunk_blocks(&self, chunk_x: i32, chunk_z: i32) -> &[u16] {
        &self.chunks[&(chunk_x, chunk_z)].blocks
    }

    /// `World.getBlockState` at absolute coordinates (air outside the build height or a missing
    /// chunk; the driver supplies every reachable chunk).
    pub fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if !(0..256).contains(&y) {
            return 0;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.blocks[chunk_index(x, y, z)],
            None => 0,
        }
    }

    #[inline]
    fn get_id(&self, x: i32, y: i32, z: i32) -> i32 {
        (self.get(x, y, z) >> 4) as i32
    }

    /// `World.isAirBlock`.
    #[inline]
    pub fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_id(x, y, z) == 0
    }

    /// `World.setBlockState` -> `Chunk.setBlockState`'s block write and height-map maintenance.
    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u16) {
        if !(0..256).contains(&y) {
            return;
        }
        let Some(chunk) = self.chunks.get_mut(&(x >> 4, z >> 4)) else {
            return;
        };
        let idx = chunk_index(x, y, z);
        let old = chunk.blocks[idx];
        if old == value {
            return;
        }
        let col = col_index(x, z);

        // `if (var4 >= this.precipitationHeightMap[var6] - 1) this.precipitationHeightMap[var6] = -999;`
        if y >= chunk.precip[col] - 1 {
            chunk.precip[col] = -999;
        }

        let height = chunk.height[col];
        let section = (y >> 4) as usize;
        let mut created_section = false;
        if chunk.sections & (1 << section) == 0 {
            if value >> 4 == 0 {
                return;
            }
            chunk.sections |= 1 << section;
            chunk.top_filled = chunk.top_filled.max((section as i32) << 4);
            created_section = true;
        }

        chunk.blocks[idx] = value;

        let new_opacity = blocks::light_opacity((value >> 4) as i32);
        let old_opacity = blocks::light_opacity((old >> 4) as i32);
        if created_section && y >= height {
            chunk.recompute_height();
        } else {
            if new_opacity > 0 {
                if y >= height {
                    chunk.relight(x & 15, y + 1, z & 15);
                }
            } else if y == height - 1 {
                chunk.relight(x & 15, y, z & 15);
            }
            let _ = old_opacity;
        }
    }

    pub fn set_to_air(&mut self, x: i32, y: i32, z: i32) {
        self.set(x, y, z, 0);
    }

    /// `World.getHeight`: the chunk's height value (`z << 4 | x`), or 0 when the chunk is absent.
    pub fn height(&self, x: i32, z: i32) -> i32 {
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.height[col_index(x, z)],
            None => 0,
        }
    }

    /// `Chunk.getTopFilledSegment`.
    pub fn top_filled_segment(&self, x: i32, z: i32) -> i32 {
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.top_filled,
            None => 0,
        }
    }

    /// `World.getTopSolidOrLiquidBlock`: the y of the first position (scanning down) whose block
    /// below has a movement-blocking, non-leaf material.
    pub fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
        let mut y = self.top_filled_segment(x, z) + 16;
        while y >= 0 {
            let by = y - 1;
            let id = self.get_id(x, by, z);
            if material_blocks_movement(id) && !material_is_leaves(id) {
                break;
            }
            y = by;
        }
        y
    }

    /// `Chunk.getPrecipitationHeight` (the result is recomputed; the cache is pure memoisation).
    pub fn precipitation_height(&self, x: i32, z: i32) -> i32 {
        let mut y = self.top_filled_segment(x, z) + 15;
        let mut result = -1;
        while y > 0 && result == -1 {
            let id = self.get_id(x, y, z);
            if material_blocks_movement(id) || blocks::material_is_liquid(id) {
                result = y + 1;
            } else {
                y -= 1;
            }
        }
        result
    }
}

impl ChunkState {
    /// `Chunk.generateSkylightMap`'s height column scan (light values ignored).
    fn recompute_height(&mut self) {
        let top = self.top_filled;
        for x in 0..16i32 {
            for z in 0..16i32 {
                self.precip[col_index(x, z)] = -999;
                let mut h = 0;
                for y in (1..=top + 16).rev() {
                    if blocks::light_opacity((self.blocks[chunk_index(x, y - 1, z)] >> 4) as i32)
                        != 0
                    {
                        h = y;
                        break;
                    }
                }
                self.height[col_index(x, z)] = h;
            }
        }
    }

    /// `Chunk.relightBlock` (light writes ignored; only the height map matters here).
    fn relight(&mut self, x: i32, y: i32, z: i32) {
        let col = col_index(x, z);
        let old = self.height[col];
        let mut v = old.max(y);
        while v > 0
            && blocks::light_opacity((self.blocks[chunk_index(x, v - 1, z)] >> 4) as i32) == 0
        {
            v -= 1;
        }
        if v != old {
            self.height[col] = v;
        }
    }
}

/// `BiomeDecorator.decorate` + `genDecorations`, restricted to the implemented layers.
///
/// `biome_x`/`biome_z` are the chunk origin (`ChunkProviderGenerate.populate`'s `new BlockPos(x*16,
/// 0, z*16)`), so features are placed in the `+8`-offset 16x16 window (spanning into the neighbour
/// chunks), exactly as vanilla does.
pub fn decorate(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_x: i32,
    biome_z: i32,
    settings: &DecorateSettings,
    opts: DecorateOptions,
) {
    if opts.ores {
        generate_ores(world, rand, biome_x, biome_z, settings);
    }
    if opts.surface {
        surface(world, rand, biome_x, biome_z);
    }
}

/// `BiomeDecorator.generateOres`.
fn generate_ores(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    s: &DecorateSettings,
) {
    let stone_variant = |meta: u16| ((STONE as u16) << 4) | meta;
    let dirt = (DIRT as u16) << 4;
    let gravel = (GRAVEL as u16) << 4;

    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        dirt,
        s.dirt_size,
        s.dirt_count,
        s.dirt_min_height,
        s.dirt_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        gravel,
        s.gravel_size,
        s.gravel_count,
        s.gravel_min_height,
        s.gravel_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        stone_variant(3),
        s.diorite_size,
        s.diorite_count,
        s.diorite_min_height,
        s.diorite_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        stone_variant(1),
        s.granite_size,
        s.granite_count,
        s.granite_min_height,
        s.granite_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        stone_variant(5),
        s.andesite_size,
        s.andesite_count,
        s.andesite_min_height,
        s.andesite_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        16 << 4,
        s.coal_size,
        s.coal_count,
        s.coal_min_height,
        s.coal_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        15 << 4,
        s.iron_size,
        s.iron_count,
        s.iron_min_height,
        s.iron_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        14 << 4,
        s.gold_size,
        s.gold_count,
        s.gold_min_height,
        s.gold_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        73 << 4,
        s.redstone_size,
        s.redstone_count,
        s.redstone_min_height,
        s.redstone_max_height,
    );
    gen_standard_ore1(
        world,
        rand,
        bx,
        bz,
        (DIAMOND_ORE as u16) << 4,
        s.diamond_size,
        s.diamond_count,
        s.diamond_min_height,
        s.diamond_max_height,
    );

    gen_standard_ore2(
        world,
        rand,
        bx,
        bz,
        21 << 4,
        s.lapis_size,
        s.lapis_count,
        s.lapis_center_height,
        s.lapis_spread,
    );
}

/// `BiomeDecorator.genStandardOre1`.
#[allow(clippy::too_many_arguments)]
fn gen_standard_ore1(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    ore: u16,
    size: i32,
    count: i32,
    min_h: i32,
    max_h: i32,
) {
    let (min_h, max_h) = if max_h < min_h {
        (max_h, min_h)
    } else if max_h == min_h {
        if min_h < 255 {
            (min_h, max_h + 1)
        } else {
            (min_h - 1, max_h)
        }
    } else {
        (min_h, max_h)
    };

    for _ in 0..count {
        let x = bx + rand.next_int_bound(16);
        let y = rand.next_int_bound(max_h - min_h) + min_h;
        let z = bz + rand.next_int_bound(16);
        gen_minable(world, rand, x, y, z, ore, size);
    }
}

/// `BiomeDecorator.genStandardOre2`.
#[allow(clippy::too_many_arguments)]
fn gen_standard_ore2(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    ore: u16,
    size: i32,
    count: i32,
    center: i32,
    spread: i32,
) {
    for _ in 0..count {
        let x = bx + rand.next_int_bound(16);
        let y = rand.next_int_bound(spread) + rand.next_int_bound(spread) + center - spread;
        let z = bz + rand.next_int_bound(16);
        gen_minable(world, rand, x, y, z, ore, size);
    }
}

/// `WorldGenMinable.generate` (predicate `Blocks.stone`).
fn gen_minable(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    ore: u16,
    size: i32,
) {
    let f = rand.next_float() * std::f32::consts::PI;
    let d1 = x as f64 + 8.0 + (sin(f) * size as f32 / 8.0) as f64;
    let d2 = x as f64 + 8.0 - (sin(f) * size as f32 / 8.0) as f64;
    let d3 = z as f64 + 8.0 + (cos(f) * size as f32 / 8.0) as f64;
    let d4 = z as f64 + 8.0 - (cos(f) * size as f32 / 8.0) as f64;
    let d5 = y as f64 + (rand.next_int_bound(3) - 2) as f64;
    let d6 = y as f64 + (rand.next_int_bound(3) - 2) as f64;

    for i in 0..size {
        let t = i as f32 / size as f32;
        let x0 = d1 + (d2 - d1) * t as f64;
        let y0 = d5 + (d6 - d5) * t as f64;
        let z0 = d3 + (d4 - d3) * t as f64;
        let scale = rand.next_double() * size as f64 / 16.0;
        let r1 = ((sin(std::f32::consts::PI * t) + 1.0) as f64) * scale + 1.0;
        let r2 = ((sin(std::f32::consts::PI * t) + 1.0) as f64) * scale + 1.0;
        let x1 = floor_double(x0 - r1 / 2.0);
        let y1 = floor_double(y0 - r2 / 2.0);
        let z1 = floor_double(z0 - r1 / 2.0);
        let x2 = floor_double(x0 + r1 / 2.0);
        let y2 = floor_double(y0 + r2 / 2.0);
        let z2 = floor_double(z0 + r1 / 2.0);

        for xx in x1..=x2 {
            let dx = (xx as f64 + 0.5 - x0) / (r1 / 2.0);
            if dx * dx < 1.0 {
                for yy in y1..=y2 {
                    let dy = (yy as f64 + 0.5 - y0) / (r2 / 2.0);
                    if dx * dx + dy * dy < 1.0 {
                        for zz in z1..=z2 {
                            let dz = (zz as f64 + 0.5 - z0) / (r1 / 2.0);
                            if dx * dx + dy * dy + dz * dz < 1.0
                                && world.get_id(xx, yy, zz) == STONE
                            {
                                world.set(xx, yy, zz, ore);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `BiomeDecorator.genDecorations`' sand / clay / gravel-as-sand loops.
fn surface(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    for _ in 0..3 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.top_solid_or_liquid(bx + x, bz + z);
        gen_sand(world, rand, bx + x, y, bz + z, (SAND as u16) << 4, 7);
    }

    for _ in 0..1 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.top_solid_or_liquid(bx + x, bz + z);
        gen_clay(world, rand, bx + x, y, bz + z, 4);
    }

    for _ in 0..1 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.top_solid_or_liquid(bx + x, bz + z);
        gen_sand(world, rand, bx + x, y, bz + z, (GRAVEL as u16) << 4, 6);
    }
}

/// `WorldGenSand.generate` (replaces dirt/grass with the block).
fn gen_sand(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    block: u16,
    radius: i32,
) {
    if !material_is_water((world.get(x, y, z) >> 4) as i32) {
        return;
    }

    let r = rand.next_int_bound(radius - 2) + 2;
    for xx in x - r..=x + r {
        for zz in z - r..=z + r {
            let dx = xx - x;
            let dz = zz - z;
            if dx * dx + dz * dz <= r * r {
                for yy in y - 2..=y + 2 {
                    let id = world.get_id(xx, yy, zz);
                    if id == DIRT || id == GRASS {
                        world.set(xx, yy, zz, block);
                    }
                }
            }
        }
    }
}

/// `WorldGenClay.generate`.
fn gen_clay(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    number_of_blocks: i32,
) {
    if !material_is_water((world.get(x, y, z) >> 4) as i32) {
        return;
    }

    let r = rand.next_int_bound(number_of_blocks - 2) + 2;
    for xx in x - r..=x + r {
        for zz in z - r..=z + r {
            let dx = xx - x;
            let dz = zz - z;
            if dx * dx + dz * dz <= r * r {
                for yy in y - 1..=y + 1 {
                    let id = world.get_id(xx, yy, zz);
                    if id == DIRT || id == CLAY {
                        world.set(xx, yy, zz, (CLAY as u16) << 4);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone_world(sea_level: i32) -> DecorateWorld {
        let mut world = DecorateWorld::new(sea_level);
        let mut blocks = vec![0u16; 65536];
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..64 {
                    blocks[chunk_index(x, y, z)] = (blocks::STONE as u16) << 4;
                }
            }
        }
        world.insert_chunk(0, 0, blocks);
        world
    }

    /// `WorldGenMinable` replaces stone with the ore (and nothing else) within its ellipsoid.
    #[test]
    fn minable_replaces_only_stone() {
        let mut world = stone_world(63);
        // A pocket of air around the target so the "only stone" rule is exercised.
        world.set(8, 20, 8, 0);
        let mut rand = JavaRandom::new(1);
        gen_minable(
            &mut world,
            &mut rand,
            8,
            20,
            8,
            (blocks::COAL_ORE as u16) << 4,
            17,
        );

        let mut coal = 0;
        for y in 0..64 {
            for x in 0..16 {
                for z in 0..16 {
                    if world.get(x, y, z) >> 4 == blocks::COAL_ORE as u16 {
                        coal += 1;
                    }
                }
            }
        }
        assert!(
            coal > 0,
            "expected the ore vein to place at least one block"
        );
        assert_eq!(world.get(8, 20, 8), 0, "air must not become ore");
    }

    /// The world shim maintains `Chunk`'s height map: `top_solid_or_liquid` returns one past the
    /// topmost movement-blocking non-leaf block.
    #[test]
    fn top_solid_tracks_the_surface() {
        let world = stone_world(63);
        assert_eq!(world.top_filled_segment(0, 0), 48); // top section of the 0..64 stone
        assert_eq!(world.top_solid_or_liquid(3, 7), 64); // stone top (y=63) + 1
    }

    /// A plant (a pass-material block) does not raise the height map's `top_solid_or_liquid`, but an
    /// opaque block placed above the surface does.
    #[test]
    fn height_map_ignores_pass_blocks() {
        let mut world = stone_world(63);
        world.set(3, 64, 7, (blocks::TALLGRASS as u16) << 4); // pass material
        assert_eq!(world.top_solid_or_liquid(3, 7), 64);
        world.set(3, 70, 7, (blocks::DIRT as u16) << 4); // opaque
        assert_eq!(world.top_solid_or_liquid(3, 7), 71);
    }
}
