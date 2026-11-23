//! Decoration (the `BiomeDecorator` half of `populate`).
//!
//! This is ported in layers; each layer is parity-gated by `decoration_parity`
//! (`tools/test_decoration_parity.py`) with a mode. Implemented:
//!
//! * `ores` — `BiomeDecorator.generateOres` (`WorldGenMinable` over the ore table).
//! * `surface` — the `WorldGenSand`/`WorldGenClay`/gravel-as-sand loops.
//! * `trees` — the `genBigTreeChance` tree loop (`decorate/trees.rs`).
//! * `plants` — the vegetation section (big mushrooms, flowers, grass, dead bush, waterlily,
//!   mushrooms, reeds, pumpkin, cactus).
//! * `springs` — `WorldGenLiquids` + its one-step liquid tick.
//! * `full` — the real `BiomeGenBase.decorate`, i.e. the layers above plus the per-biome `decorate`
//!   overrides: plains/forest/savanna/taiga double plants, jungle melon (+ the no-op vines), ice
//!   spikes/paths, taiga boulders, hills emerald/monster egg, desert wells. Also fixes the plain
//!   `BiomeGenMutated` biomes, whose `decorate` runs the base biome's decorator.
//!
//! Ported from the decompiled vanilla `net.minecraft.world.biome.*` (`BiomeDecorator` + the
//! `BiomeGen*` overrides), `net.minecraft.world.gen.feature.*` and `net.minecraft.world.chunk.Chunk`
//! /`net.minecraft.world.World` (the block read/write and height-map semantics the generators use).

use std::collections::BTreeMap;

use mc_core::jmath::{abs_int, ceiling_float_int, clamp_double, cos, floor_double, sin};
use mc_core::random::JavaRandom;

pub mod blocks;
pub mod trees;

use crate::noise::Perlin;
use blocks::{
    material_blocks_movement, material_is_leaves, material_is_replaceable, material_is_water, CLAY,
    DIAMOND_ORE, DIRT, GRASS, GRAVEL, SAND, STONE,
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
    pub trees: bool,
    pub plants: bool,
    pub springs: bool,
    /// The per-biome `decorate` overrides (double plants, melons, vines, ice, boulders, emerald,
    /// monster egg, desert wells). These wrap `genDecorations` (before/after), so they only run with
    /// the full layer set; the `full` harness mode enables every flag.
    pub overrides: bool,
}

impl DecorateOptions {
    pub fn none() -> DecorateOptions {
        DecorateOptions {
            ores: false,
            surface: false,
            trees: false,
            plants: false,
            springs: false,
            overrides: false,
        }
    }

    pub fn vanilla() -> DecorateOptions {
        DecorateOptions {
            ores: true,
            surface: true,
            trees: true,
            plants: true,
            springs: true,
            overrides: true,
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
    /// `ExtendedBlockStorage`'s sky-light values, indexed `x << 12 | z << 8 | y` (0 above the top).
    sky: Vec<u8>,
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
            sky: vec![0; 65536],
            precip: vec![-999; 256],
            sections,
            top_filled,
        };
        state.recompute_lighting();
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

    /// `World.getLight` == `Chunk.getLightSubtracted(pos, 0)` == the sky light (block light is 0
    /// during worldgen). A null section reads as 15 above the height map, else 0.
    pub fn get_light(&self, x: i32, y: i32, z: i32) -> i32 {
        if y < 0 {
            return 0;
        }
        let y = if y >= 256 { 255 } else { y };
        let Some(c) = self.chunks.get(&(x >> 4, z >> 4)) else {
            return 0;
        };
        let section = (y >> 4) as usize;
        if c.sections & (1 << section) == 0 {
            return if y >= c.height[col_index(x, z)] {
                15
            } else {
                0
            };
        }
        c.sky[chunk_index(x, y, z)] as i32
    }

    /// `World.setBlockState` -> `Chunk.setBlockState`'s block write, `breakBlock` side effect and
    /// height-map/light maintenance.
    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u16) {
        if !(0..256).contains(&y) {
            return;
        }
        let key = (x >> 4, z >> 4);
        let old = {
            let Some(chunk) = self.chunks.get(&key) else {
                return;
            };
            let old = chunk.blocks[chunk_index(x, y, z)];
            if old == value {
                return;
            }
            old
        };
        // `Chunk.setBlockState` calls the replaced block's `breakBlock` when the block id changes.
        if (old >> 4) as i32 != (value >> 4) as i32 {
            self.break_block((old >> 4) as i32, x, y, z);
        }
        let chunk = self.chunks.get_mut(&key).expect("chunk present");
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
            chunk.recompute_lighting();
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

    /// `BlockLeaves.breakBlock` (radius 1) / `BlockLog.breakBlock` (radius 4): flag nearby leaves
    /// whose `CHECK_DECAY` is off (metadata bit 8).
    fn break_block(&mut self, old_id: i32, x: i32, y: i32, z: i32) {
        let r = if old_id == blocks::LEAVES || old_id == blocks::LEAVES2 {
            1
        } else if old_id == blocks::LOG || old_id == blocks::LOG2 {
            4
        } else {
            return;
        };
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    let (px, py, pz) = (x + dx, y + dy, z + dz);
                    let id = self.get_id(px, py, pz);
                    if id == blocks::LEAVES || id == blocks::LEAVES2 {
                        let v = self.get(px, py, pz);
                        if v & 8 == 0 {
                            self.set(px, py, pz, v | 8);
                        }
                    }
                }
            }
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
    /// `Chunk.generateSkylightMap`: the height map and the column sky-light values (horizontal
    /// propagation is `World.checkLight`, which the decoration oracle disables).
    fn recompute_lighting(&mut self) {
        let top = self.top_filled;
        for x in 0..16i32 {
            for z in 0..16i32 {
                let col = col_index(x, z);
                self.precip[col] = -999;
                let mut h = 0;
                for y in (1..=top + 16).rev() {
                    if blocks::light_opacity((self.blocks[chunk_index(x, y - 1, z)] >> 4) as i32)
                        != 0
                    {
                        h = y;
                        break;
                    }
                }
                self.height[col] = h;

                let mut light: i32 = 15;
                let mut y = top + 15;
                loop {
                    let mut op =
                        blocks::light_opacity((self.blocks[chunk_index(x, y, z)] >> 4) as i32);
                    if op == 0 && light != 15 {
                        op = 1;
                    }
                    light -= op;
                    if light > 0 {
                        self.sky[chunk_index(x, y, z)] = light as u8;
                    }
                    y -= 1;
                    if y <= 0 || light <= 0 {
                        break;
                    }
                }
            }
        }
    }

    /// `Chunk.relightBlock`: recompute one column's height map and sky-light values.
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
            if v < old {
                for y2 in v..old {
                    self.sky[chunk_index(x, y2, z)] = 15;
                }
            } else {
                for y2 in old..v {
                    self.sky[chunk_index(x, y2, z)] = 0;
                }
            }
            let mut light = 15i32;
            while v > 0 && light > 0 {
                v -= 1;
                let mut op = blocks::light_opacity((self.blocks[chunk_index(x, v, z)] >> 4) as i32);
                if op == 0 {
                    op = 1;
                }
                light -= op;
                if light < 0 {
                    light = 0;
                }
                self.sky[chunk_index(x, v, z)] = light as u8;
            }
        }
    }
}

/// `BiomeDecorator.decorate` (the full `BiomeGenBase.decorate` dispatch) — the base
/// `genDecorations` layers plus the per-biome `decorate` overrides that wrap them.
///
/// `biome_x`/`biome_z` are the chunk origin (`ChunkProviderGenerate.populate`'s `new BlockPos(x*16,
/// 0, z*16)`), so features are placed in the `+8`-offset 16x16 window (spanning into the neighbour
/// chunks), exactly as vanilla does. `biome_id` is the generation biome at that window's centre
/// (`ChunkProviderGenerate.populate`'s `getBiomeGenForCoords(field.add(16, 0, 16))`), which selects
/// the plant counts, the flower/grass hooks and the override. `grass_noise` is
/// `BiomeGenBase.GRASS_COLOR_NOISE` (`Perlin(Random(2345), 1)`), used by the plains/forest flower
/// hooks and the plains override. `big_mushroom` carries the shared `WorldGenBigMushroom`'s
/// once-per-biome colour (`mushroomType`), keyed by biome id.
#[allow(clippy::too_many_arguments)]
pub fn decorate(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_x: i32,
    biome_z: i32,
    biome_id: i32,
    settings: &DecorateSettings,
    opts: DecorateOptions,
    grass_noise: &mut Perlin,
    big_mushroom: &mut BTreeMap<i32, bool>,
    big_tree_height: &mut BTreeMap<i32, i32>,
) {
    // `BiomeGenForest.createMutatedBiome` builds a `BiomeGenMutated` whose `decorate` forwards to
    // `baseBiome.decorate`; for the roofed-forest mutation (29 + 128) that makes it decoration-
    // identical to its base, so resolve it through the base id (counts, hooks and override).
    let biome_id = if biome_id == 157 { 29 } else { biome_id };

    let mut plant_counts = decorate_plants(biome_id);
    let trees = decorate_trees(biome_id);

    if opts.overrides {
        biome_override_pre(
            world,
            rand,
            biome_id,
            biome_x,
            biome_z,
            &mut plant_counts,
            grass_noise,
            big_tree_height,
        );
    }
    if opts.ores {
        generate_ores(world, rand, biome_x, biome_z, settings);
    }
    if opts.surface {
        surface(world, rand, biome_x, biome_z);
    }
    if opts.trees {
        trees_loop(
            world,
            rand,
            biome_x,
            biome_z,
            biome_id,
            trees,
            big_tree_height,
        );
    }
    if opts.plants {
        plants(
            world,
            rand,
            biome_x,
            biome_z,
            biome_id,
            plant_counts,
            grass_noise,
            big_mushroom,
        );
    }
    if opts.springs {
        springs(world, rand, biome_x, biome_z);
    }
    if opts.overrides {
        biome_override_post(world, rand, biome_id, biome_x, biome_z);
    }
}

/// The decorator counts the biome's `decorate` actually uses: a plain `BiomeGenMutated` runs the
/// *base* biome's `theBiomeDecorator` counts while its `pickRandomFlower`/`getRandomWorldGenForGrass`
/// stay at the `BiomeGenBase` default (they are not overridden by `BiomeGenMutated`).
fn decorate_plants(biome_id: i32) -> BiomePlants {
    match mutated_base(biome_id) {
        Some(base) => {
            let mut cfg = biome_plants(base);
            cfg.flower_kind = FlowerKind::Default;
            cfg.grass_kind = GrassKind::Default;
            cfg
        }
        None => biome_plants(biome_id),
    }
}

/// `treesPerChunk` from the decorator the biome's `decorate` uses.
fn decorate_trees(biome_id: i32) -> i32 {
    match mutated_base(biome_id) {
        Some(base) => biome_trees(base),
        None => biome_trees(biome_id),
    }
}

/// The `BiomeGenMutated` biomes whose `decorate` runs `baseBiome.theBiomeDecorator.decorate` (their
/// own decorator is never read): the mutated id -> base id. `157` is excluded here because it
/// forwards to `baseBiome.decorate` and is mapped to `29` at the top of `decorate`.
fn mutated_base(biome_id: i32) -> Option<i32> {
    match biome_id {
        130 => Some(2),
        133 => Some(5),
        134 => Some(6),
        149 => Some(21),
        151 => Some(23),
        155 => Some(27),
        156 => Some(28),
        158 => Some(30),
        _ => None,
    }
}

/// The `treesPerChunk` count per biome (dumped from the game; the mutation ids carry their own
/// decorator counts).
pub fn biome_trees(biome_id: i32) -> i32 {
    match biome_id {
        1 | 2 | 16 | 17 | 25 | 26 | 29 | 37 | 39 | 129 | 165 | 167 => -999,
        14 | 15 => -100,
        4 | 18 | 27 | 28 => 10,
        5 | 19 | 30 | 31 | 32 | 33 | 160 | 161 => 10,
        6 => 2,
        20 | 34 => 3,
        21 | 22 => 50,
        23 => 2,
        35 | 36 => 1,
        38 | 166 => 5,
        132 => 6,
        163 | 164 => 2,
        _ => 0,
    }
}

/// `BiomeDecorator.genDecorations`' tree loop (`WorldGenBigTree`'s shared `heightLimit` is per biome).
fn trees_loop(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    biome_id: i32,
    mut count: i32,
    big_tree_height: &mut BTreeMap<i32, i32>,
) {
    if rand.next_int_bound(10) == 0 {
        count += 1;
    }
    for _ in 0..count {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let kind = trees::big_tree_chance(biome_id, rand);
        let px = bx + x;
        let pz = bz + z;
        let py = world.height(px, pz);
        let height_limit = big_tree_height.entry(biome_id).or_insert(0);
        if trees::generate(world, rand, px, py, pz, kind, height_limit) {
            trees::post_generate(world, rand, px, py, pz, kind);
        }
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

// ---- vegetation (big mushrooms, flowers, grass, dead bush, waterlily, mushrooms, reeds, pumpkin, cactus) ----

/// The biome hook `BiomeGenBase.pickRandomFlower` (only the overrides differ from the default).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowerKind {
    Default,
    Plains,
    ForestType1,
    Swamp,
}

/// The biome hook `BiomeGenBase.getRandomWorldGenForGrass`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrassKind {
    Default,
    Taiga,
    Jungle,
}

/// One biome's `BiomeDecorator` plant counts + hooks (the constructor-time values; the per-biome
/// `decorate` overrides that mutate them belong to the `full` layer).
#[derive(Clone, Copy)]
pub struct BiomePlants {
    pub big_mushrooms: i32,
    pub flowers: i32,
    pub grass: i32,
    pub dead_bush: i32,
    pub mushrooms: i32,
    pub reeds: i32,
    pub cacti: i32,
    pub waterlily: i32,
    pub flower_kind: FlowerKind,
    pub grass_kind: GrassKind,
}

/// `BiomeDecorator`'s plant counts per biome id, dumped from the live game
/// (`oracle/java/src/BiomeDecorTableOracle.java`); the hooks are keyed by the biome's class (the
/// mutated biomes are `BiomeGenMutated` and use the base hooks).
pub fn biome_plants(biome_id: i32) -> BiomePlants {
    // (big_mushrooms, flowers, grass, dead_bush, mushrooms, reeds, cacti, waterlily).
    let (big, fl, gr, db, mu, re, ca, wl): (i32, i32, i32, i32, i32, i32, i32, i32) = match biome_id
    {
        1 | 129 => (0, 4, 10, 0, 0, 0, 0, 0), // plains / sunflower plains
        2 | 17 => (0, 2, 1, 2, 0, 50, 10, 0), // desert / desert hills
        4 | 18 | 27 | 28 | 29 => (0, 2, 2, 0, 0, 0, 0, 0), // forests
        5 | 19 | 30 | 31 => (0, 2, 1, 0, 1, 0, 0, 0), // taiga family
        6 => (0, 1, 5, 1, 8, 10, 0, 4),       // swamp
        14 | 15 => (1, -100, -100, 0, 1, 0, 0, 0), // mushroom island
        21..=23 => (0, 4, 25, 0, 0, 0, 0, 0), // jungle
        32 | 33 | 160 | 161 => (0, 2, 7, 1, 3, 0, 0, 0), // mega taiga
        35 | 36 => (0, 4, 20, 0, 0, 0, 0, 0), // savanna
        37 | 38 | 39 | 165 | 166 | 167 => (0, 0, 1, 20, 0, 3, 5, 0), // mesa
        132 => (0, 100, 1, 0, 0, 0, 0, 0),    // flower forest
        163 | 164 => (0, 2, 5, 0, 0, 0, 0, 0), // savanna M
        _ => (0, 2, 1, 0, 0, 0, 0, 0),        // everything else
    };
    let flower_kind = match biome_id {
        1 | 129 => FlowerKind::Plains,
        132 => FlowerKind::ForestType1,
        6 => FlowerKind::Swamp,
        _ => FlowerKind::Default,
    };
    let grass_kind = match biome_id {
        5 | 19 | 30 | 31 | 32 | 33 | 160 | 161 => GrassKind::Taiga,
        21..=23 => GrassKind::Jungle,
        _ => GrassKind::Default,
    };
    BiomePlants {
        big_mushrooms: big,
        flowers: fl,
        grass: gr,
        dead_bush: db,
        mushrooms: mu,
        reeds: re,
        cacti: ca,
        waterlily: wl,
        flower_kind,
        grass_kind,
    }
}

/// `BlockFlower.EnumFlowerType`, in declaration order (the array `pickRandomFlower` indexes).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flower {
    Dandelion,
    Poppy,
    BlueOrchid,
    Allium,
    Houstonia,
    RedTulip,
    OrangeTulip,
    WhiteTulip,
    PinkTulip,
    OxeyeDaisy,
}

const FLOWER_TYPES: [Flower; 10] = [
    Flower::Dandelion,
    Flower::Poppy,
    Flower::BlueOrchid,
    Flower::Allium,
    Flower::Houstonia,
    Flower::RedTulip,
    Flower::OrangeTulip,
    Flower::WhiteTulip,
    Flower::PinkTulip,
    Flower::OxeyeDaisy,
];

impl Flower {
    fn meta(self) -> u16 {
        match self {
            Flower::Dandelion | Flower::Poppy => 0,
            Flower::BlueOrchid => 1,
            Flower::Allium => 2,
            Flower::Houstonia => 3,
            Flower::RedTulip => 4,
            Flower::OrangeTulip => 5,
            Flower::WhiteTulip => 6,
            Flower::PinkTulip => 7,
            Flower::OxeyeDaisy => 8,
        }
    }

    /// The `BlockFlower` state (`yellow_flower` for the dandelion, else `red_flower`).
    fn state(self) -> u16 {
        if self == Flower::Dandelion {
            (blocks::YELLOW_FLOWER as u16) << 4
        } else {
            ((blocks::RED_FLOWER as u16) << 4) | self.meta()
        }
    }
}

/// `BiomeGenBase.pickRandomFlower` (dispatching on the biome's override).
fn pick_random_flower(
    kind: FlowerKind,
    rand: &mut JavaRandom,
    grass_noise: &mut Perlin,
    x: i32,
    z: i32,
) -> Flower {
    match kind {
        FlowerKind::Default => {
            if rand.next_int_bound(3) > 0 {
                Flower::Dandelion
            } else {
                Flower::Poppy
            }
        }
        FlowerKind::Swamp => Flower::BlueOrchid,
        FlowerKind::Plains => {
            let v = grass_noise.sample_2d(x as f64 / 200.0, z as f64 / 200.0);
            if v < -0.8 {
                match rand.next_int_bound(4) {
                    0 => Flower::OrangeTulip,
                    1 => Flower::RedTulip,
                    2 => Flower::PinkTulip,
                    _ => Flower::WhiteTulip,
                }
            } else if rand.next_int_bound(3) > 0 {
                match rand.next_int_bound(3) {
                    0 => Flower::Poppy,
                    1 => Flower::Houstonia,
                    _ => Flower::OxeyeDaisy,
                }
            } else {
                Flower::Dandelion
            }
        }
        FlowerKind::ForestType1 => {
            let v = clamp_double(
                (1.0 + grass_noise.sample_2d(x as f64 / 48.0, z as f64 / 48.0)) / 2.0,
                0.0,
                0.9999,
            );
            let t = FLOWER_TYPES[(v * 10.0) as usize];
            if t == Flower::BlueOrchid {
                Flower::Poppy
            } else {
                t
            }
        }
    }
}

/// `BiomeGenBase.getRandomWorldGenForGrass` -> the `BlockTallGrass` metadata (GRASS = 1, FERN = 2).
fn grass_meta(kind: GrassKind, rand: &mut JavaRandom) -> u16 {
    match kind {
        GrassKind::Default => 1,
        GrassKind::Taiga => {
            if rand.next_int_bound(5) > 0 {
                2
            } else {
                1
            }
        }
        GrassKind::Jungle => {
            if rand.next_int_bound(4) == 0 {
                2
            } else {
                1
            }
        }
    }
}

/// `BlockBush.canPlaceBlockOn` (grass / dirt / farmland).
#[inline]
fn bush_soil(below_id: i32) -> bool {
    below_id == blocks::GRASS || below_id == blocks::DIRT || below_id == blocks::FARMLAND
}

/// `WorldGenFlowers.generate`.
fn gen_flowers(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    ox: i32,
    oy: i32,
    oz: i32,
    state: u16,
) {
    for _ in 0..64 {
        let x = ox + rand.next_int_bound(8) - rand.next_int_bound(8);
        let y = oy + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(x, y, z) && bush_soil(world.get_id(x, y - 1, z)) {
            world.set(x, y, z, state);
        }
    }
}

/// `WorldGenTallGrass.generate`.
fn gen_tall_grass(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    meta: u16,
) {
    let mut y = y;
    while (world.is_air(x, y, z) || blocks::material_is_leaves(world.get_id(x, y, z))) && y > 0 {
        y -= 1;
    }

    for _ in 0..128 {
        let px = x + rand.next_int_bound(8) - rand.next_int_bound(8);
        let py = y + rand.next_int_bound(4) - rand.next_int_bound(4);
        let pz = z + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(px, py, pz) && bush_soil(world.get_id(px, py - 1, pz)) {
            world.set(px, py, pz, ((blocks::TALLGRASS as u16) << 4) | meta);
        }
    }
}

/// `WorldGenDeadBush.generate`.
fn gen_dead_bush(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) {
    let mut y = y;
    while (world.is_air(x, y, z) || blocks::material_is_leaves(world.get_id(x, y, z))) && y > 0 {
        y -= 1;
    }

    for _ in 0..4 {
        let px = x + rand.next_int_bound(8) - rand.next_int_bound(8);
        let py = y + rand.next_int_bound(4) - rand.next_int_bound(4);
        let pz = z + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(px, py, pz) {
            let below = world.get_id(px, py - 1, pz);
            if below == blocks::SAND
                || below == blocks::HARDENED_CLAY
                || below == blocks::STAINED_HARDENED_CLAY
                || below == blocks::DIRT
            {
                world.set(px, py, pz, (blocks::DEADBUSH as u16) << 4);
            }
        }
    }
}

/// `BlockReed.canPlaceBlockAt` (grass/dirt/sand or reed below, and a water neighbour beside it).
fn reed_can_stay(world: &DecorateWorld, x: i32, y: i32, z: i32) -> bool {
    let below = world.get_id(x, y - 1, z);
    if below == blocks::REEDS {
        return true;
    }
    if below != blocks::GRASS && below != blocks::DIRT && below != blocks::SAND {
        return false;
    }
    let by = y - 1;
    material_is_water(world.get_id(x - 1, by, z))
        || material_is_water(world.get_id(x + 1, by, z))
        || material_is_water(world.get_id(x, by, z - 1))
        || material_is_water(world.get_id(x, by, z + 1))
}

/// `WorldGenReed.generate`.
fn gen_reed(world: &mut DecorateWorld, rand: &mut JavaRandom, ox: i32, oy: i32, oz: i32) {
    for _ in 0..20 {
        let x = ox + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(4) - rand.next_int_bound(4);
        if world.is_air(x, oy, z) {
            let by = oy - 1;
            let water = material_is_water(world.get_id(x - 1, by, z))
                || material_is_water(world.get_id(x + 1, by, z))
                || material_is_water(world.get_id(x, by, z - 1))
                || material_is_water(world.get_id(x, by, z + 1));
            if water {
                let inner = rand.next_int_bound(3) + 1;
                let height = 2 + rand.next_int_bound(inner);
                for i in 0..height {
                    if reed_can_stay(world, x, oy, z) {
                        world.set(x, oy + i, z, (blocks::REEDS as u16) << 4);
                    }
                }
            }
        }
    }
}

/// `BlockCactus.canBlockStay` (no solid horizontal neighbour; sand/cactus below).
fn cactus_can_stay(world: &DecorateWorld, x: i32, y: i32, z: i32) -> bool {
    if blocks::material_is_solid(world.get_id(x - 1, y, z))
        || blocks::material_is_solid(world.get_id(x + 1, y, z))
        || blocks::material_is_solid(world.get_id(x, y, z - 1))
        || blocks::material_is_solid(world.get_id(x, y, z + 1))
    {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    below == blocks::CACTUS || below == blocks::SAND
}

/// `WorldGenCactus.generate`.
fn gen_cactus(world: &mut DecorateWorld, rand: &mut JavaRandom, ox: i32, oy: i32, oz: i32) {
    for _ in 0..10 {
        let x = ox + rand.next_int_bound(8) - rand.next_int_bound(8);
        let y = oy + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(x, y, z) {
            let inner = rand.next_int_bound(3) + 1;
            let height = 1 + rand.next_int_bound(inner);
            for i in 0..height {
                if cactus_can_stay(world, x, y, z) {
                    world.set(x, y + i, z, (blocks::CACTUS as u16) << 4);
                }
            }
        }
    }
}

/// `WorldGenWaterlily.generate` (`BlockLilyPad.canPlaceBlockAt`: air above still water).
fn gen_waterlily(world: &mut DecorateWorld, rand: &mut JavaRandom, ox: i32, oy: i32, oz: i32) {
    for _ in 0..10 {
        let x = ox + rand.next_int_bound(8) - rand.next_int_bound(8);
        let y = oy + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(x, y, z) && world.get_id(x, y - 1, z) == blocks::WATER {
            world.set(x, y, z, (blocks::WATERLILY as u16) << 4);
        }
    }
}

/// `WorldGenPumpkin.generate` (on air above grass; FACING = `Plane.HORIZONTAL.random`).
fn gen_pumpkin(world: &mut DecorateWorld, rand: &mut JavaRandom, ox: i32, oy: i32, oz: i32) {
    for _ in 0..64 {
        let x = ox + rand.next_int_bound(8) - rand.next_int_bound(8);
        let y = oy + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(x, y, z) && world.get_id(x, y - 1, z) == blocks::GRASS {
            // `canPlaceBlockAt`: pos replaceable (air) and a solid top surface below (grass).
            if blocks::is_full_block(world.get_id(x, y - 1, z)) {
                // `Plane.HORIZONTAL` = {NORTH, EAST, SOUTH, WEST}; meta = horizontal index.
                let meta = [2u16, 3, 0, 1][rand.next_int_bound(4) as usize];
                world.set(x, y, z, ((blocks::PUMPKIN as u16) << 4) | meta);
            }
        }
    }
}

/// `BlockMushroom.canBlockStay` (mycelium/podzol, or a full block below with light < 13).
fn mushroom_can_stay(world: &DecorateWorld, x: i32, y: i32, z: i32) -> bool {
    if !(0..256).contains(&y) {
        return false;
    }
    let below = world.get(x, y - 1, z);
    let below_id = (below >> 4) as i32;
    if below_id == blocks::MYCELIUM {
        return true;
    }
    if below_id == blocks::DIRT && (below & 15) == blocks::PODZOL_META {
        return true;
    }
    world.get_light(x, y, z) < 13 && blocks::is_full_block(below_id)
}

/// `GeneratorBushFeature.generate` (brown/red mushroom).
fn gen_bush(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    ox: i32,
    oy: i32,
    oz: i32,
    state: u16,
) {
    for _ in 0..64 {
        let x = ox + rand.next_int_bound(8) - rand.next_int_bound(8);
        let y = oy + rand.next_int_bound(4) - rand.next_int_bound(4);
        let z = oz + rand.next_int_bound(8) - rand.next_int_bound(8);
        if world.is_air(x, y, z) && mushroom_can_stay(world, x, y, z) {
            world.set(x, y, z, state);
        }
    }
}

/// `BlockHugeMushroom.EnumType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HugeMushroom {
    NorthWest,
    North,
    NorthEast,
    West,
    Center,
    East,
    SouthWest,
    South,
    SouthEast,
    Stem,
    AllInside,
    AllOutside,
    AllStem,
}

impl HugeMushroom {
    fn meta(self) -> u16 {
        match self {
            HugeMushroom::NorthWest => 1,
            HugeMushroom::North => 2,
            HugeMushroom::NorthEast => 3,
            HugeMushroom::West => 4,
            HugeMushroom::Center => 5,
            HugeMushroom::East => 6,
            HugeMushroom::SouthWest => 7,
            HugeMushroom::South => 8,
            HugeMushroom::SouthEast => 9,
            HugeMushroom::Stem => 10,
            HugeMushroom::AllInside => 0,
            HugeMushroom::AllOutside => 14,
            HugeMushroom::AllStem => 15,
        }
    }

    /// `EnumType.byMetadata` (unknown metadata falls back to `ALL_INSIDE`).
    fn by_metadata(m: i32) -> HugeMushroom {
        match m {
            1 => HugeMushroom::NorthWest,
            2 => HugeMushroom::North,
            3 => HugeMushroom::NorthEast,
            4 => HugeMushroom::West,
            5 => HugeMushroom::Center,
            6 => HugeMushroom::East,
            7 => HugeMushroom::SouthWest,
            8 => HugeMushroom::South,
            9 => HugeMushroom::SouthEast,
            10 => HugeMushroom::Stem,
            14 => HugeMushroom::AllOutside,
            15 => HugeMushroom::AllStem,
            _ => HugeMushroom::AllInside,
        }
    }
}

fn huge_mushroom_state(is_brown: bool, variant: HugeMushroom) -> u16 {
    let id = if is_brown {
        blocks::BROWN_MUSHROOM_BLOCK
    } else {
        blocks::RED_MUSHROOM_BLOCK
    };
    ((id as u16) << 4) | variant.meta()
}

/// `WorldGenBigMushroom.generate` (the no-arg constructor; `is_brown` is the shared `mushroomType`).
fn gen_big_mushroom(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    is_brown: bool,
) {
    let size = rand.next_int_bound(3) + 4;
    if !(y >= 1 && y + size + 1 < 256) {
        return;
    }

    let mut ok = true;
    'scan: for by in y..=(y + 1 + size) {
        let r = if by <= y + 3 { 0 } else { 3 };
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if (0..256).contains(&by) {
                    let id = world.get_id(bx, by, bz);
                    if id != 0 && !blocks::material_is_leaves(id) {
                        ok = false;
                        break 'scan;
                    }
                } else {
                    ok = false;
                    break 'scan;
                }
            }
        }
    }
    if !ok {
        return;
    }

    let below = world.get_id(x, y - 1, z);
    if below != blocks::DIRT && below != blocks::GRASS && below != blocks::MYCELIUM {
        return;
    }

    let cap = if is_brown { y + size } else { y + size - 3 };
    for cy in cap..=(y + size) {
        let mut r = 1;
        if cy < y + size {
            r += 1;
        }
        if is_brown {
            r = 3;
        }
        let x1 = x - r;
        let x2 = x + r;
        let z1 = z - r;
        let z2 = z + r;

        for px in x1..=x2 {
            for pz in z1..=z2 {
                let mut meta = 5;
                if px == x1 {
                    meta -= 1;
                } else if px == x2 {
                    meta += 1;
                }
                if pz == z1 {
                    meta -= 3;
                } else if pz == z2 {
                    meta += 3;
                }
                let mut variant = HugeMushroom::by_metadata(meta);
                if is_brown || cy < y + size {
                    if (px == x1 || px == x2) && (pz == z1 || pz == z2) {
                        continue;
                    }
                    if px == x - (r - 1) && pz == z1 {
                        variant = HugeMushroom::NorthWest;
                    }
                    if px == x1 && pz == z - (r - 1) {
                        variant = HugeMushroom::NorthWest;
                    }
                    if px == x + (r - 1) && pz == z1 {
                        variant = HugeMushroom::NorthEast;
                    }
                    if px == x2 && pz == z - (r - 1) {
                        variant = HugeMushroom::NorthEast;
                    }
                    if px == x - (r - 1) && pz == z2 {
                        variant = HugeMushroom::SouthWest;
                    }
                    if px == x1 && pz == z + (r - 1) {
                        variant = HugeMushroom::SouthWest;
                    }
                    if px == x + (r - 1) && pz == z2 {
                        variant = HugeMushroom::SouthEast;
                    }
                    if px == x2 && pz == z + (r - 1) {
                        variant = HugeMushroom::SouthEast;
                    }
                }
                if variant == HugeMushroom::Center && cy < y + size {
                    variant = HugeMushroom::AllInside;
                }
                if variant != HugeMushroom::AllInside {
                    let id = world.get_id(px, cy, pz);
                    if !blocks::is_full_block(id) {
                        world.set(px, cy, pz, huge_mushroom_state(is_brown, variant));
                    }
                }
            }
        }
    }

    for i in 0..size {
        let id = world.get_id(x, y + i, z);
        if !blocks::is_full_block(id) {
            world.set(
                x,
                y + i,
                z,
                huge_mushroom_state(is_brown, HugeMushroom::Stem),
            );
        }
    }
}

/// `BiomeDecorator.genDecorations`' vegetation section (items after the trees).
#[allow(clippy::too_many_arguments)]
fn plants(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    biome_id: i32,
    cfg: BiomePlants,
    grass_noise: &mut Perlin,
    big_mushroom: &mut BTreeMap<i32, bool>,
) {
    for _ in 0..cfg.big_mushrooms {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.height(bx + x, bz + z);
        let is_brown = match big_mushroom.get(&biome_id) {
            Some(&b) => b,
            None => {
                let b = rand.next_boolean();
                big_mushroom.insert(biome_id, b);
                b
            }
        };
        gen_big_mushroom(world, rand, bx + x, y, bz + z, is_brown);
    }

    for _ in 0..cfg.flowers {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) + 32;
        if h > 0 {
            let y = rand.next_int_bound(h);
            let flower = pick_random_flower(cfg.flower_kind, rand, grass_noise, bx + x, bz + z);
            gen_flowers(world, rand, bx + x, y, bz + z, flower.state());
        }
    }

    for _ in 0..cfg.grass {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            let meta = grass_meta(cfg.grass_kind, rand);
            gen_tall_grass(world, rand, bx + x, y, bz + z, meta);
        }
    }

    for _ in 0..cfg.dead_bush {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_dead_bush(world, rand, bx + x, y, bz + z);
        }
    }

    for _ in 0..cfg.waterlily {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let mut y = rand.next_int_bound(h);
            while y > 0 {
                let down = y - 1;
                if !world.is_air(bx + x, down, bz + z) {
                    break;
                }
                y = down;
            }
            gen_waterlily(world, rand, bx + x, y, bz + z);
        }
    }

    for _ in 0..cfg.mushrooms {
        if rand.next_int_bound(4) == 0 {
            let x = rand.next_int_bound(16) + 8;
            let z = rand.next_int_bound(16) + 8;
            let y = world.height(bx + x, bz + z);
            gen_bush(
                world,
                rand,
                bx + x,
                y,
                bz + z,
                (blocks::BROWN_MUSHROOM as u16) << 4,
            );
        }
        if rand.next_int_bound(8) == 0 {
            let x = rand.next_int_bound(16) + 8;
            let z = rand.next_int_bound(16) + 8;
            let h = world.height(bx + x, bz + z) * 2;
            if h > 0 {
                let y = rand.next_int_bound(h);
                gen_bush(
                    world,
                    rand,
                    bx + x,
                    y,
                    bz + z,
                    (blocks::RED_MUSHROOM as u16) << 4,
                );
            }
        }
    }

    if rand.next_int_bound(4) == 0 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_bush(
                world,
                rand,
                bx + x,
                y,
                bz + z,
                (blocks::BROWN_MUSHROOM as u16) << 4,
            );
        }
    }
    if rand.next_int_bound(8) == 0 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_bush(
                world,
                rand,
                bx + x,
                y,
                bz + z,
                (blocks::RED_MUSHROOM as u16) << 4,
            );
        }
    }

    for _ in 0..cfg.reeds {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_reed(world, rand, bx + x, y, bz + z);
        }
    }
    for _ in 0..10 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_reed(world, rand, bx + x, y, bz + z);
        }
    }

    if rand.next_int_bound(32) == 0 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_pumpkin(world, rand, bx + x, y, bz + z);
        }
    }

    for _ in 0..cfg.cacti {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) * 2;
        if h > 0 {
            let y = rand.next_int_bound(h);
            gen_cactus(world, rand, bx + x, y, bz + z);
        }
    }
}

// ---- per-biome `decorate` overrides ----

/// `BlockDoublePlant.EnumPlantType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DoublePlant {
    Sunflower,
    Syringa,
    Grass,
    Fern,
    Rose,
    Paeonia,
}

impl DoublePlant {
    fn meta(self) -> u16 {
        match self {
            DoublePlant::Sunflower => 0,
            DoublePlant::Syringa => 1,
            DoublePlant::Grass => 2,
            DoublePlant::Fern => 3,
            DoublePlant::Rose => 4,
            DoublePlant::Paeonia => 5,
        }
    }
}

/// The `BiomeGen*` `decorate` code that runs *before* `super.decorate` (`genDecorations`). Plains
/// also mutates the flower/grass counts first.
#[allow(clippy::too_many_arguments)]
fn biome_override_pre(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_id: i32,
    bx: i32,
    bz: i32,
    cfg: &mut BiomePlants,
    grass_noise: &mut Perlin,
    big_tree_height: &mut BTreeMap<i32, i32>,
) {
    match biome_id {
        1 | 129 => plains_pre(world, rand, biome_id, bx, bz, cfg, grass_noise),
        4 | 18 | 27 | 28 | 29 | 132 => forest_pre(world, rand, biome_id, bx, bz, big_tree_height),
        5 | 19 | 30 | 31 | 32 | 33 | 160 | 161 => taiga_pre(world, rand, biome_id, bx, bz),
        35 | 36 => double_plant_loop(world, rand, bx, bz, 7, DoublePlant::Grass),
        140 => snow_spikes_pre(world, rand, bx, bz),
        _ => {}
    }
}

/// The `BiomeGen*` `decorate` code that runs *after* `super.decorate` (`genDecorations`).
fn biome_override_post(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_id: i32,
    bx: i32,
    bz: i32,
) {
    match biome_id {
        2 | 17 => desert_post(world, rand, bx, bz),
        21..=23 => jungle_post(world, rand, bx, bz),
        3 | 20 | 34 | 131 | 162 => hills_post(world, rand, bx, bz),
        _ => {}
    }
}

/// `BiomeGenPlains.decorate`.
fn plains_pre(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_id: i32,
    bx: i32,
    bz: i32,
    cfg: &mut BiomePlants,
    grass_noise: &mut Perlin,
) {
    let v = grass_noise.sample_2d((bx as f64 + 8.0) / 200.0, (bz as f64 + 8.0) / 200.0);
    if v < -0.8 {
        cfg.flowers = 15;
        cfg.grass = 5;
    } else {
        cfg.flowers = 4;
        cfg.grass = 10;
        double_plant_loop(world, rand, bx, bz, 7, DoublePlant::Grass);
    }
    if biome_id == 129 {
        double_plant_loop(world, rand, bx, bz, 10, DoublePlant::Sunflower);
    }
}

/// `BiomeGenForest.decorate` (`field_150632_aF` = 0 forest, 1 flower forest, 2 birch, 3 roofed).
fn forest_pre(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    biome_id: i32,
    bx: i32,
    bz: i32,
    big_tree_height: &mut BTreeMap<i32, i32>,
) {
    let kind = match biome_id {
        132 => 1,
        27 | 28 => 2,
        29 => 3,
        _ => 0,
    };
    if kind == 3 {
        for a in 0..4i32 {
            for b in 0..4i32 {
                let x = a * 4 + 1 + 8 + rand.next_int_bound(3);
                let z = b * 4 + 1 + 8 + rand.next_int_bound(3);
                let y = world.height(bx + x, bz + z);
                if rand.next_int_bound(20) == 0 {
                    let is_brown = rand.next_boolean();
                    gen_big_mushroom(world, rand, bx + x, y, bz + z, is_brown);
                } else {
                    let tree = trees::big_tree_chance(biome_id, rand);
                    let height_limit = big_tree_height.entry(biome_id).or_insert(0);
                    if trees::generate(world, rand, bx + x, y, bz + z, tree, height_limit) {
                        trees::post_generate(world, rand, bx + x, y, bz + z, tree);
                    }
                }
            }
        }
    }

    let mut n = rand.next_int_bound(5) - 3;
    if kind == 1 {
        n += 2;
    }
    for _ in 0..n {
        let plant = match rand.next_int_bound(3) {
            0 => DoublePlant::Syringa,
            1 => DoublePlant::Rose,
            _ => DoublePlant::Paeonia,
        };
        for _ in 0..5 {
            let x = rand.next_int_bound(16) + 8;
            let z = rand.next_int_bound(16) + 8;
            let h = world.height(bx + x, bz + z) + 32;
            let y = rand.next_int_bound(h);
            if gen_double_plant(world, rand, bx + x, y, bz + z, plant) {
                break;
            }
        }
    }
}

/// `BiomeGenTaiga.decorate` (`field_150644_aH`: 1 mega, 2 mega spruce; both grow boulders + ferns).
fn taiga_pre(world: &mut DecorateWorld, rand: &mut JavaRandom, biome_id: i32, bx: i32, bz: i32) {
    let kind = match biome_id {
        32 | 33 => 1,
        160 | 161 => 2,
        _ => 0,
    };
    if kind == 1 || kind == 2 {
        let n = rand.next_int_bound(3);
        for _ in 0..n {
            let x = rand.next_int_bound(16) + 8;
            let z = rand.next_int_bound(16) + 8;
            let y = world.height(bx + x, bz + z);
            gen_block_blob(world, rand, bx + x, y, bz + z);
        }
    }
    double_plant_loop(world, rand, bx, bz, 7, DoublePlant::Fern);
}

/// `BiomeGenSnow.decorate` (`field_150615_aC` = ice-plains-spikes).
fn snow_spikes_pre(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    for _ in 0..3 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.height(bx + x, bz + z);
        gen_ice_spike(world, rand, bx + x, y, bz + z);
    }
    for _ in 0..2 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.height(bx + x, bz + z);
        gen_ice_path(world, rand, bx + x, y, bz + z);
    }
}

/// `BiomeGenDesert.decorate` (the 1/1000 well).
fn desert_post(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    if rand.next_int_bound(1000) == 0 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = world.height(bx + x, bz + z) + 1;
        gen_desert_well(world, bx + x, y, bz + z);
    }
}

/// `BiomeGenJungle.decorate` (a melon then 50 `WorldGenVines`).
fn jungle_post(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    let x = rand.next_int_bound(16) + 8;
    let z = rand.next_int_bound(16) + 8;
    let h = world.height(bx + x, bz + z) * 2;
    let y = rand.next_int_bound(h);
    gen_melon(world, rand, bx + x, y, bz + z);
    for _ in 0..50 {
        let vx = rand.next_int_bound(16) + 8;
        let vz = rand.next_int_bound(16) + 8;
        gen_vines(world, rand, bx + vx, 128, bz + vz);
    }
}

/// `BiomeGenHills.decorate` (emerald ore pockets + monster-egg `WorldGenMinable`).
fn hills_post(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    let n = 3 + rand.next_int_bound(6);
    for _ in 0..n {
        let x = rand.next_int_bound(16);
        let y = rand.next_int_bound(28) + 4;
        let z = rand.next_int_bound(16);
        if world.get_id(bx + x, y, bz + z) == STONE {
            world.set(bx + x, y, bz + z, (blocks::EMERALD_ORE as u16) << 4);
        }
    }
    for _ in 0..7 {
        let x = rand.next_int_bound(16);
        let y = rand.next_int_bound(64);
        let z = rand.next_int_bound(16);
        gen_minable(
            world,
            rand,
            bx + x,
            y,
            bz + z,
            (blocks::MONSTER_EGG as u16) << 4,
            9,
        );
    }
}

/// The `DOUBLE_PLANT_GENERATOR` loop over the `+8` window used by the plains/taiga/savanna
/// overrides (the `getHeight + 32` bound).
fn double_plant_loop(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    bx: i32,
    bz: i32,
    n: i32,
    plant: DoublePlant,
) {
    for _ in 0..n {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let h = world.height(bx + x, bz + z) + 32;
        let y = rand.next_int_bound(h);
        gen_double_plant(world, rand, bx + x, y, bz + z, plant);
    }
}

/// `WorldGenDoublePlant.generate` (returns whether it placed at least one plant).
fn gen_double_plant(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    plant: DoublePlant,
) -> bool {
    let mut placed = false;
    for _ in 0..64 {
        let px = x + rand.next_int_bound(8) - rand.next_int_bound(8);
        let py = y + rand.next_int_bound(4) - rand.next_int_bound(4);
        let pz = z + rand.next_int_bound(8) - rand.next_int_bound(8);
        // The overworld provider's `getHasNoSky()` is false, so the `!getHasNoSky() || y < 254`
        // guard in vanilla is always true here.
        if world.is_air(px, py, pz)
            && world.is_air(px, py + 1, pz)
            && bush_soil(world.get_id(px, py - 1, pz))
        {
            double_plant_place(world, px, py, pz, plant);
            placed = true;
        }
    }
    placed
}

/// `BlockDoublePlant.placeAt`: lower half (the variant metadata) then upper half. The upper half's
/// `getMetaFromState` is `8 | FACING.getHorizontalIndex()` with `FACING` at its default `NORTH`
/// (horizontal index 2).
fn double_plant_place(world: &mut DecorateWorld, x: i32, y: i32, z: i32, plant: DoublePlant) {
    let id = (blocks::DOUBLE_PLANT as u16) << 4;
    world.set(x, y, z, id | plant.meta());
    world.set(x, y + 1, z, id | (8 | 2));
}

/// `WorldGenMelon.generate`.
fn gen_melon(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) {
    for _ in 0..64 {
        let px = x + rand.next_int_bound(8) - rand.next_int_bound(8);
        let py = y + rand.next_int_bound(4) - rand.next_int_bound(4);
        let pz = z + rand.next_int_bound(8) - rand.next_int_bound(8);
        if material_is_replaceable(world.get_id(px, py, pz))
            && world.get_id(px, py - 1, pz) == GRASS
        {
            world.set(px, py, pz, (blocks::MELON as u16) << 4);
        }
    }
}

/// `WorldGenVines.generate`. The jungle override starts it at `y = 128`, so the `y < 128` loop never
/// runs (a vanilla 1.8.9 quirk); kept faithful for reuse.
fn gen_vines(world: &mut DecorateWorld, rand: &mut JavaRandom, mut x: i32, mut y: i32, mut z: i32) {
    while y < 128 {
        if world.is_air(x, y, z) {
            // `EnumFacing.Plane.HORIZONTAL.facings()` = {NORTH, EAST, SOUTH, WEST}; the support
            // block is at `pos.offset(facing.getOpposite())`, and the placed vine gets that face.
            for (dx, dz, bit) in [(0, -1, 4u16), (1, 0, 8), (0, 1, 1), (-1, 0, 2)] {
                let support = world.get_id(x - dx, y, z - dz);
                if blocks::is_full_block(support) && material_blocks_movement(support) {
                    world.set(x, y, z, ((blocks::VINE as u16) << 4) | bit);
                    break;
                }
            }
        } else {
            x += rand.next_int_bound(4) - rand.next_int_bound(4);
            z += rand.next_int_bound(4) - rand.next_int_bound(4);
        }
        y += 1;
    }
}

/// `Block` material `air`/`dirt`/`snow`/`ice` (the ice generators' replaceable set).
#[inline]
fn ice_replaceable(id: i32) -> bool {
    id == 0 || id == blocks::DIRT || id == blocks::SNOW || id == blocks::ICE
}

/// `WorldGenIceSpike.generate`.
fn gen_ice_spike(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, mut y: i32, z: i32) {
    while world.is_air(x, y, z) && y > 2 {
        y -= 1;
    }
    if world.get_id(x, y, z) != blocks::SNOW {
        return;
    }
    y += rand.next_int_bound(4);
    let height = rand.next_int_bound(4) + 7;
    let radius = height / 4 + rand.next_int_bound(2);
    if radius > 1 && rand.next_int_bound(60) == 0 {
        y += 10 + rand.next_int_bound(30);
    }

    for i in 0..height {
        let size = (1.0 - i as f32 / height as f32) * radius as f32;
        let r = ceiling_float_int(size);
        for dx in -r..=r {
            let ex = abs_int(dx) as f32 - 0.25;
            for dz in -r..=r {
                let ez = abs_int(dz) as f32 - 0.25;
                let inner = (dx == 0 && dz == 0) || ex * ex + ez * ez <= size * size;
                // Java short-circuits the `&&`: the `nextFloat` draw only happens when `inner`.
                if inner
                    && ((dx != -r && dx != r && dz != -r && dz != r) || rand.next_float() <= 0.75)
                {
                    let yy = y + i;
                    if ice_replaceable(world.get_id(x + dx, yy, z + dz)) {
                        world.set(x + dx, yy, z + dz, (blocks::PACKED_ICE as u16) << 4);
                    }
                    if i != 0 && r > 1 {
                        let by = y - i;
                        if ice_replaceable(world.get_id(x + dx, by, z + dz)) {
                            world.set(x + dx, by, z + dz, (blocks::PACKED_ICE as u16) << 4);
                        }
                    }
                }
            }
        }
    }

    let base_r = (radius - 1).clamp(0, 1);
    for dx in -base_r..=base_r {
        for dz in -base_r..=base_r {
            let mut py = y - 1;
            let mut budget = 50;
            if abs_int(dx) == 1 && abs_int(dz) == 1 {
                budget = rand.next_int_bound(5);
            }
            while py > 50 {
                let id = world.get_id(x + dx, py, z + dz);
                if id != 0
                    && id != blocks::DIRT
                    && id != blocks::SNOW
                    && id != blocks::ICE
                    && id != blocks::PACKED_ICE
                {
                    break;
                }
                world.set(x + dx, py, z + dz, (blocks::PACKED_ICE as u16) << 4);
                py -= 1;
                budget -= 1;
                if budget <= 0 {
                    py -= rand.next_int_bound(5) + 1;
                    budget = rand.next_int_bound(5);
                }
            }
        }
    }
}

/// `WorldGenIcePath.generate` (`WorldGenIcePath(4)`).
fn gen_ice_path(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, mut y: i32, z: i32) {
    while world.is_air(x, y, z) && y > 2 {
        y -= 1;
    }
    if world.get_id(x, y, z) != blocks::SNOW {
        return;
    }
    let r = rand.next_int_bound(4 - 2) + 2;
    for px in (x - r)..=(x + r) {
        for pz in (z - r)..=(z + r) {
            let dx = px - x;
            let dz = pz - z;
            if dx * dx + dz * dz <= r * r {
                for py in (y - 1)..=(y + 1) {
                    let id = world.get_id(px, py, pz);
                    if id == blocks::DIRT || id == blocks::SNOW || id == blocks::ICE {
                        world.set(px, py, pz, (blocks::PACKED_ICE as u16) << 4);
                    }
                }
            }
        }
    }
}

/// `WorldGenBlockBlob.generate` (`new WorldGenBlockBlob(Blocks.mossy_cobblestone, 0)`).
fn gen_block_blob(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    mut x: i32,
    mut y: i32,
    mut z: i32,
) {
    while y > 3 {
        if !world.is_air(x, y - 1, z) {
            let b = world.get_id(x, y - 1, z);
            if b == blocks::GRASS || b == blocks::DIRT || b == blocks::STONE {
                break;
            }
        }
        y -= 1;
    }
    if y <= 3 {
        return;
    }
    let size = 0; // `WorldGenBlockBlob.field_150544_b`
    for _ in 0..3 {
        let rx = size + rand.next_int_bound(2);
        let ry = size + rand.next_int_bound(2);
        let rz = size + rand.next_int_bound(2);
        let limit = (rx + ry + rz) as f32 * 0.333f32 + 0.5f32;
        for px in (x - rx)..=(x + rx) {
            for py in (y - ry)..=(y + ry) {
                for pz in (z - rz)..=(z + rz) {
                    let dx = (px - x) as f64;
                    let dy = (py - y) as f64;
                    let dz = (pz - z) as f64;
                    if dx * dx + dy * dy + dz * dz <= (limit * limit) as f64 {
                        world.set(px, py, pz, (blocks::MOSSY_COBBLESTONE as u16) << 4);
                    }
                }
            }
        }
        x += -(size + 1) + rand.next_int_bound(2 + size * 2);
        y += -rand.next_int_bound(2);
        z += -(size + 1) + rand.next_int_bound(2 + size * 2);
    }
}

/// `WorldGenDesertWells.generate`.
fn gen_desert_well(world: &mut DecorateWorld, x: i32, mut y: i32, z: i32) {
    while world.is_air(x, y, z) && y > 2 {
        y -= 1;
    }
    if world.get_id(x, y, z) != blocks::SAND || (world.get(x, y, z) & 15) != 0 {
        return;
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if world.is_air(x + dx, y - 1, z + dz) && world.is_air(x + dx, y - 2, z + dz) {
                return;
            }
        }
    }
    let sandstone = (blocks::SANDSTONE as u16) << 4;
    let slab = ((blocks::STONE_SLAB as u16) << 4) | 1; // variant SAND, HALF BOTTOM
    for dy in -1..=0 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                world.set(x + dx, y + dy, z + dz, sandstone);
            }
        }
    }
    let water = (blocks::FLOWING_WATER as u16) << 4;
    world.set(x, y, z, water);
    for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        world.set(x + dx, y, z + dz, water);
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if dx == -2 || dx == 2 || dz == -2 || dz == 2 {
                world.set(x + dx, y + 1, z + dz, sandstone);
            }
        }
    }
    world.set(x + 2, y + 1, z, slab);
    world.set(x - 2, y + 1, z, slab);
    world.set(x, y + 1, z + 2, slab);
    world.set(x, y + 1, z - 2, slab);
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 {
                world.set(x, y + 4, z, sandstone);
            } else {
                world.set(x + dx, y + 4, z + dz, slab);
            }
        }
    }
    for dy in 1..=3 {
        world.set(x - 1, y + dy, z - 1, sandstone);
        world.set(x - 1, y + dy, z + 1, sandstone);
        world.set(x + 1, y + dy, z - 1, sandstone);
        world.set(x + 1, y + dy, z + 1, sandstone);
    }
}

// ---- springs (`WorldGenLiquids`) ----

/// `BiomeDecorator.genDecorations`' springs section (`generateLakes`).
fn springs(world: &mut DecorateWorld, rand: &mut JavaRandom, bx: i32, bz: i32) {
    for _ in 0..50 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let y = rand.next_int_bound(248) + 8;
        if y > 0 {
            let yy = rand.next_int_bound(y);
            gen_liquids(
                world,
                bx + x,
                yy,
                bz + z,
                blocks::FLOWING_WATER,
                blocks::WATER,
                1,
            );
        }
    }
    for _ in 0..20 {
        let x = rand.next_int_bound(16) + 8;
        let z = rand.next_int_bound(16) + 8;
        let a = rand.next_int_bound(240) + 8;
        let b = rand.next_int_bound(a) + 8;
        let y = rand.next_int_bound(b);
        gen_liquids(
            world,
            bx + x,
            y,
            bz + z,
            blocks::FLOWING_LAVA,
            blocks::LAVA,
            2,
        );
    }
}

/// `WorldGenLiquids.generate`: a 3-stone / 1-air pocket spring, then the immediate liquid tick.
///
/// The tick runs on a plain `World` (the oracle), whose `scheduleUpdate` is a no-op, so
/// `BlockDynamicLiquid.updateTick` is a single level-0 step: place the static liquid at `pos`
/// (the stone above/below stops vertical flow) and a flowing liquid at `flow_level` in the one
/// open horizontal neighbour (`flow_level` = 1 for water, 2 for overworld lava).
#[allow(clippy::too_many_arguments)]
fn gen_liquids(
    world: &mut DecorateWorld,
    x: i32,
    y: i32,
    z: i32,
    dynamic_id: i32,
    static_id: i32,
    flow_level: i32,
) -> bool {
    if world.get_id(x, y + 1, z) != blocks::STONE {
        return false;
    }
    if world.get_id(x, y - 1, z) != blocks::STONE {
        return false;
    }
    let center = world.get_id(x, y, z);
    if center != blocks::AIR && center != blocks::STONE {
        return false;
    }
    let stone = (world.get_id(x - 1, y, z) == blocks::STONE) as i32
        + (world.get_id(x + 1, y, z) == blocks::STONE) as i32
        + (world.get_id(x, y, z - 1) == blocks::STONE) as i32
        + (world.get_id(x, y, z + 1) == blocks::STONE) as i32;
    let air = world.is_air(x - 1, y, z) as i32
        + world.is_air(x + 1, y, z) as i32
        + world.is_air(x, y, z - 1) as i32
        + world.is_air(x, y, z + 1) as i32;
    if stone == 3 && air == 1 {
        world.set(x, y, z, (dynamic_id as u16) << 4); // level 0
                                                      // `forceBlockUpdateTick` -> `BlockDynamicLiquid.updateTick` (level 0):
                                                      //   `placeStaticBlock(pos)` writes the static liquid, then the one open horizontal
                                                      //   neighbour receives a flowing block at `flow_level` via flag 3. That set's
                                                      //   neighbour notification calls `onNeighborBlockChange` on every block within a
                                                      //   Manhattan radius 2 of it; `BlockStaticLiquid.updateLiquid` converts any such static
                                                      //   liquid back to its flowing block (same level, update scheduled = no-op), which also
                                                      //   reverts `pos` itself. Net: `pos` stays flowing level 0, the neighbour is flowing at
                                                      //   `flow_level`, and nearby static water/lava becomes flowing.
        let _ = static_id;
        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            if world.is_air(x + dx, y, z + dz) {
                world.set(
                    x + dx,
                    y,
                    z + dz,
                    ((dynamic_id as u16) << 4) | flow_level as u16,
                );
                convert_static_liquids(world, x + dx, y, z + dz);
            }
        }
    }
    true
}

/// `BlockStaticLiquid.onNeighborBlockChange` over the 6 neighbours `notifyNeighborsOfStateChange`
/// notifies: any static liquid among them becomes its flowing block (same level; update = no-op).
fn convert_static_liquids(world: &mut DecorateWorld, cx: i32, cy: i32, cz: i32) {
    for (dx, dy, dz) in [
        (-1, 0, 0),
        (1, 0, 0),
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
    ] {
        let v = world.get(cx + dx, cy + dy, cz + dz);
        match (v >> 4) as i32 {
            blocks::WATER => {
                world.set(
                    cx + dx,
                    cy + dy,
                    cz + dz,
                    ((blocks::FLOWING_WATER as u16) << 4) | (v & 15),
                );
            }
            blocks::LAVA => {
                world.set(
                    cx + dx,
                    cy + dy,
                    cz + dz,
                    ((blocks::FLOWING_LAVA as u16) << 4) | (v & 15),
                );
            }
            _ => {}
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

    /// A flat world filled with one block (`id << 4 | meta`) up to y=63 and air above.
    fn flat_world(block: u16) -> DecorateWorld {
        let mut world = DecorateWorld::new(63);
        let mut blocks = vec![0u16; 65536];
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..64 {
                    blocks[chunk_index(x, y, z)] = block;
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

    /// The per-biome plant counts come from the live game (`BiomeDecorTableOracle`).
    #[test]
    fn biome_plant_table_matches_the_game() {
        let swamp = biome_plants(6);
        assert_eq!(swamp.waterlily, 4);
        assert_eq!(swamp.reeds, 10);
        assert_eq!(swamp.mushrooms, 8);
        assert_eq!(swamp.flower_kind, FlowerKind::Swamp);

        let mushroom_island = biome_plants(14);
        assert_eq!(mushroom_island.big_mushrooms, 1);
        assert_eq!(mushroom_island.flowers, -100);

        let mesa = biome_plants(37);
        assert_eq!(mesa.cacti, 5);
        assert_eq!(mesa.dead_bush, 20);

        assert_eq!(biome_plants(32).grass_kind, GrassKind::Taiga);
        assert_eq!(biome_plants(21).grass_kind, GrassKind::Jungle);
    }

    /// `BlockFlower.EnumFlowerType` -> the block state (`yellow_flower` only for the dandelion).
    #[test]
    fn flower_states_use_the_right_block_and_meta() {
        assert_eq!(
            Flower::Dandelion.state(),
            (blocks::YELLOW_FLOWER as u16) << 4
        );
        assert_eq!(Flower::Poppy.state(), (blocks::RED_FLOWER as u16) << 4);
        assert_eq!(
            Flower::BlueOrchid.state(),
            ((blocks::RED_FLOWER as u16) << 4) | 1
        );
        assert_eq!(
            Flower::OxeyeDaisy.state(),
            ((blocks::RED_FLOWER as u16) << 4) | 8
        );
    }

    /// `BlockMushroom.canBlockStay`: dark + full block below, or mycelium (which bypasses light).
    #[test]
    fn mushroom_light_and_soil_rules() {
        let mut world = stone_world(63);
        assert!(mushroom_can_stay(&world, 3, 20, 7)); // underground, dark, stone below
        assert!(!mushroom_can_stay(&world, 3, 64, 7)); // open sky (light 15), stone below
        world.set(3, 63, 7, (blocks::MYCELIUM as u16) << 4);
        assert!(mushroom_can_stay(&world, 3, 64, 7)); // mycelium bypasses the light rule
    }

    /// The per-biome `treesPerChunk` counts (dumped from the game).
    #[test]
    fn biome_tree_counts_match_the_game() {
        assert_eq!(biome_trees(4), 10); // forest
        assert_eq!(biome_trees(21), 50); // jungle
        assert_eq!(biome_trees(23), 2); // jungle edge
        assert_eq!(biome_trees(1), -999); // plains
        assert_eq!(biome_trees(37), -999); // mesa
        assert_eq!(biome_trees(38), 5); // mesa plateau F
        assert_eq!(biome_trees(132), 6); // flower forest
    }

    /// A 3-stone / 1-air pocket spring stays flowing level 0 and flows into its one open neighbour.
    #[test]
    fn spring_places_flowing_liquid_in_a_pocket() {
        let mut world = stone_world(63);
        world.set(8, 20, 8, 0); // spring position
        world.set(8, 20, 9, 0); // the one open (south) neighbour
        gen_liquids(
            &mut world,
            8,
            20,
            8,
            blocks::FLOWING_WATER,
            blocks::WATER,
            1,
        );
        assert_eq!(world.get(8, 20, 8), (blocks::FLOWING_WATER as u16) << 4);
        assert_eq!(
            world.get(8, 20, 9),
            ((blocks::FLOWING_WATER as u16) << 4) | 1
        );
    }

    /// A plain `BiomeGenMutated` runs the *base* biome's decorator counts but keeps the
    /// `BiomeGenBase` default flower/grass hooks (`BiomeGenMutated` does not override them).
    #[test]
    fn mutated_biomes_use_base_counts_and_default_hooks() {
        assert_eq!(decorate_trees(133), biome_trees(5)); // taiga M
        assert_eq!(decorate_trees(130), -999); // desert M
        let base = biome_plants(5);
        let m = decorate_plants(133);
        assert_eq!(m.grass, base.grass);
        assert_eq!(m.mushrooms, base.mushrooms);
        assert_eq!(m.flower_kind, FlowerKind::Default);
        assert_eq!(m.grass_kind, GrassKind::Default);
        assert_eq!(decorate_plants(130).reeds, 50); // desert's reedsPerChunk
    }

    /// `BlockDoublePlant.placeAt`: lower half carries the variant meta, upper half is
    /// `8 | FACING.getHorizontalIndex()` with the default `FACING = NORTH` (horizontal index 2).
    #[test]
    fn double_plant_writes_variant_lower_and_facing_upper_half() {
        let mut world = flat_world((blocks::GRASS as u16) << 4);
        double_plant_place(&mut world, 8, 64, 8, DoublePlant::Fern);
        assert_eq!(
            world.get(8, 64, 8),
            ((blocks::DOUBLE_PLANT as u16) << 4) | 3
        );
        assert_eq!(
            world.get(8, 65, 8),
            ((blocks::DOUBLE_PLANT as u16) << 4) | (8 | 2)
        );
    }

    /// `WorldGenDesertWells`: a flowing-water centre, a sandstone ring, sand slabs at the four
    /// rim posts and an inner ring of slabs around a sandstone centre at `y + 4`.
    #[test]
    fn desert_well_builds_the_vanilla_layout() {
        let mut world = flat_world((blocks::SAND as u16) << 4);
        gen_desert_well(&mut world, 8, 64, 8);
        let water = (blocks::FLOWING_WATER as u16) << 4;
        let sandstone = (blocks::SANDSTONE as u16) << 4;
        let slab = ((blocks::STONE_SLAB as u16) << 4) | 1; // variant SAND, HALF BOTTOM

        assert_eq!(world.get(8, 63, 8), water);
        assert_eq!(world.get(6, 64, 8), slab);
        assert_eq!(world.get(10, 64, 8), slab);
        assert_eq!(world.get(8, 64, 10), slab);
        assert_eq!(world.get(8, 64, 6), slab);
        assert_eq!(world.get(6, 64, 6), sandstone); // rim corner
        assert_eq!(world.get(8, 67, 8), sandstone); // y+4 centre
        assert_eq!(world.get(7, 67, 7), slab); // y+4 inner ring
        assert_eq!(world.get(7, 64, 7), sandstone); // pillar
    }

    /// `WorldGenIcePath(4)` replaces the snow surface with packed ice.
    #[test]
    fn ice_path_replaces_snow_with_packed_ice() {
        let mut world = flat_world((blocks::SNOW as u16) << 4);
        let mut rand = JavaRandom::new(1);
        gen_ice_path(&mut world, &mut rand, 8, 64, 8);
        assert_eq!(world.get(8, 63, 8), (blocks::PACKED_ICE as u16) << 4);
        assert_eq!(world.get(8, 64, 8), 0); // the air above is not replaced
    }

    /// `WorldGenBlockBlob` places mossy cobblestone around the surface position.
    #[test]
    fn block_blob_places_mossy_cobblestone() {
        let mut world = flat_world((blocks::GRASS as u16) << 4);
        let mut rand = JavaRandom::new(1);
        gen_block_blob(&mut world, &mut rand, 8, 64, 8);
        let mut found = 0;
        for y in 60..66 {
            for x in 4..13 {
                for z in 4..13 {
                    if world.get_id(x, y, z) == blocks::MOSSY_COBBLESTONE {
                        found += 1;
                    }
                }
            }
        }
        assert!(found > 0, "expected at least one mossy cobblestone");
    }
}
