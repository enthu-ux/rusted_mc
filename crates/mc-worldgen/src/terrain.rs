//! Port of the 1.8.9 overworld base terrain (`ChunkProviderGenerate`): the density field that
//! fills a chunk with stone/water, then per-biome surface replacement.
//!
//! Covers `setBlocksInChunk` + `func_147423_a` (shape), `replaceBlocksForBiome`, and the biome
//! terrain generators: `BiomeGenBase.generateBiomeTerrain`, `BiomeGenHills`, `BiomeGenTaiga`,
//! `BiomeGenSwamp`, `BiomeGenSavanna.Mutated`, and `BiomeGenMesa` (`genTerrainBlocks` +
//! `func_150619_a` + `func_180629_a`). Caves/ravines/structures are out of scope.
//!
//! Default `WorldType.DEFAULT` settings (`ChunkProviderSettings.Factory` defaults). Block states
//! are stamped as `id << 4 | meta`, indexed `x << 12 | z << 8 | y` (vanilla's quirks included).

use crate::cave::{Caves, Ravines};
use crate::chunk::{self, GeneratedChunk};
use crate::decorate::{self, DecorateOptions, DecorateSettings, DecorateWorld};
use crate::layer::BiomeLayers;
use crate::noise::{Octaves, Perlin};
use crate::structure::mineshaft::Mineshaft;
use crate::structure::monument::OceanMonument;
use crate::structure::placement::{self, ChunkWorld};
use crate::structure::scattered::ScatteredFeature;
use crate::structure::stronghold::Stronghold;
use crate::structure::village::Village;
use mc_core::random::JavaRandom;
use std::f64::consts::PI;

// ChunkProviderSettings defaults (the subset the density field uses).
const COORDINATE_SCALE: f32 = 684.412;
const HEIGHT_SCALE: f32 = 684.412;
const UPPER_LIMIT_SCALE: f32 = 512.0;
const LOWER_LIMIT_SCALE: f32 = 512.0;
const DEPTH_NOISE_SCALE_X: f32 = 200.0;
const DEPTH_NOISE_SCALE_Z: f32 = 200.0;
const DEPTH_NOISE_SCALE_EXPONENT: f32 = 0.5;
const MAIN_NOISE_SCALE_X: f32 = 80.0;
const MAIN_NOISE_SCALE_Y: f32 = 160.0;
const MAIN_NOISE_SCALE_Z: f32 = 80.0;
const BASE_SIZE: f32 = 8.5;
const STRETCH_Y: f32 = 12.0;
const BIOME_DEPTH_WEIGHT: f32 = 1.0;
const BIOME_DEPTH_OFFSET: f32 = 0.0;
const BIOME_SCALE_WEIGHT: f32 = 1.0;
const BIOME_SCALE_OFFSET: f32 = 0.0;
const SEA_LEVEL: i32 = 63;

// Block states, `id << 4 | meta`.
const BEDROCK: u16 = 7 << 4;
const STONE: u16 = 1 << 4;
const WATER: u16 = 9 << 4;
const ICE: u16 = 79 << 4;
const GRAVEL: u16 = 13 << 4;
const GRASS: u16 = 2 << 4;
const DIRT: u16 = 3 << 4;
const COARSE_DIRT: u16 = (3 << 4) | 1;
const PODZOL: u16 = (3 << 4) | 2;
const SANDSTONE: u16 = 24 << 4;
const RED_SANDSTONE: u16 = 179 << 4;
const HARDENED_CLAY: u16 = 172 << 4;
const WATERLILY: u16 = 111 << 4;
const CLAY_ORANGE: u16 = (159 << 4) | 1; // EnumDyeColor.ORANGE
const CLAY_YELLOW: u16 = (159 << 4) | 4;
const CLAY_BROWN: u16 = (159 << 4) | 12;
const CLAY_RED: u16 = (159 << 4) | 14;
const CLAY_WHITE: u16 = (159 << 4) | 0;
const CLAY_SILVER: u16 = (159 << 4) | 8;

#[inline]
fn get(primer: &[u16], x: i32, y: i32, z: i32) -> u16 {
    primer[((x << 12) | (z << 8) | y) as usize]
}

#[inline]
fn set(primer: &mut [u16], x: i32, y: i32, z: i32, v: u16) {
    primer[((x << 12) | (z << 8) | y) as usize] = v;
}

/// `BiomeGenBase.{minHeight,maxHeight}` for every registered id (base + mutation), taken from the
/// live game. Unregistered ids fall back to a harmless default.
fn biome_height(id: i32) -> (f32, f32) {
    match id {
        0 => (-1.0, 0.1),
        1 => (0.125, 0.05),
        2 => (0.125, 0.05),
        3 => (1.0, 0.5),
        4 => (0.1, 0.2),
        5 => (0.2, 0.2),
        6 => (-0.2, 0.1),
        7 => (-0.5, 0.0),
        8 => (0.1, 0.2),
        9 => (0.1, 0.2),
        10 => (-1.0, 0.1),
        11 => (-0.5, 0.0),
        12 => (0.125, 0.05),
        13 => (0.45, 0.3),
        14 => (0.2, 0.3),
        15 => (0.0, 0.025),
        16 => (0.0, 0.025),
        17 => (0.45, 0.3),
        18 => (0.45, 0.3),
        19 => (0.45, 0.3),
        20 => (0.8, 0.3),
        21 => (0.1, 0.2),
        22 => (0.45, 0.3),
        23 => (0.1, 0.2),
        24 => (-1.8, 0.1),
        25 => (0.1, 0.8),
        26 => (0.0, 0.025),
        27 => (0.1, 0.2),
        28 => (0.45, 0.3),
        29 => (0.1, 0.2),
        30 => (0.2, 0.2),
        31 => (0.45, 0.3),
        32 => (0.2, 0.2),
        33 => (0.45, 0.3),
        34 => (1.0, 0.5),
        35 => (0.125, 0.05),
        36 => (1.5, 0.025),
        37 => (0.1, 0.2),
        38 => (1.5, 0.025),
        39 => (1.5, 0.025),
        129 => (0.125, 0.05),
        130 => (0.225, 0.25),
        131 => (1.0, 0.5),
        132 => (0.1, 0.4),
        133 => (0.3, 0.4),
        134 => (-0.1, 0.3),
        140 => (0.425, 0.450_000_02),
        149 => (0.2, 0.4),
        151 => (0.2, 0.4),
        155 => (0.2, 0.4),
        156 => (0.55, 0.5),
        157 => (0.2, 0.4),
        158 => (0.3, 0.4),
        160 => (0.2, 0.2),
        161 => (0.2, 0.2),
        162 => (1.0, 0.5),
        163 => (0.3625, 1.225),
        164 => (1.05, 1.212_500_1),
        165 => (0.1, 0.2),
        166 => (0.45, 0.3),
        167 => (0.45, 0.3),
        _ => (0.1, 0.2),
    }
}

/// `BiomeGenBase.temperature`, exact per id (mutations may override their base's value).
fn biome_temp(id: i32) -> f32 {
    match id {
        0 => 0.5,
        1 => 0.8,
        2 => 2.0,
        3 => 0.2,
        4 => 0.7,
        5 => 0.25,
        6 => 0.8,
        7 => 0.5,
        8 => 2.0,
        9 => 0.5,
        10 => 0.0,
        11 => 0.0,
        12 => 0.0,
        13 => 0.0,
        14 => 0.9,
        15 => 0.9,
        16 => 0.8,
        17 => 2.0,
        18 => 0.7,
        19 => 0.25,
        20 => 0.2,
        21 => 0.95,
        22 => 0.95,
        23 => 0.95,
        24 => 0.5,
        25 => 0.2,
        26 => 0.05,
        27 => 0.6,
        28 => 0.6,
        29 => 0.7,
        30 => -0.5,
        31 => -0.5,
        32 => 0.3,
        33 => 0.3,
        34 => 0.2,
        35 => 1.2,
        36 => 1.0,
        37 => 2.0,
        38 => 2.0,
        39 => 2.0,
        129 => 0.8,
        130 => 2.0,
        131 => 0.2,
        132 => 0.7,
        133 => 0.25,
        134 => 0.8,
        140 => 0.0,
        149 => 0.95,
        151 => 0.95,
        155 => 0.6,
        156 => 0.6,
        157 => 0.7,
        158 => -0.5,
        160 => 0.25,
        161 => 0.25,
        162 => 0.2,
        163 => 1.1,
        164 => 1.0,
        165 => 2.0,
        166 => 2.0,
        167 => 2.0,
        _ => 0.5,
    }
}

/// `BiomeGenBase.{topBlock,fillerBlock}` as `id << 4 | meta`, per registered id.
pub(crate) fn biome_top_filler(id: i32) -> (u16, u16) {
    match id {
        2 | 16 | 17 | 26 => (12 << 4, 12 << 4), // sand
        9 => (DIRT, DIRT),
        14 | 15 => (110 << 4, DIRT), // mycelium
        25 => (STONE, STONE),
        37 | 38 | 39 | 165 | 166 | 167 => ((12 << 4) | 1, 159 << 4), // red sand / clay
        140 => (80 << 4, DIRT),                                      // snow block
        _ => (GRASS, DIRT),
    }
}

/// Which of the biome terrain generators applies to a biome id.
enum TerrainKind {
    Default,
    Hills(i32),
    Taiga(i32),
    Swamp,
    SavannaMutated,
    Mesa { a_h: bool, a_i: bool },
    Delegate(i32),
}

fn terrain_kind(id: i32) -> TerrainKind {
    match id {
        3 => TerrainKind::Hills(0),
        20 => TerrainKind::Hills(1),
        34 => TerrainKind::Hills(1),
        131 => TerrainKind::Hills(2),
        162 => TerrainKind::Hills(2),
        5 | 19 | 30 | 31 => TerrainKind::Taiga(0),
        32 | 33 => TerrainKind::Taiga(1),
        160 | 161 => TerrainKind::Taiga(2),
        6 => TerrainKind::Swamp,
        134 => TerrainKind::Delegate(6),
        163 | 164 => TerrainKind::SavannaMutated,
        37 => TerrainKind::Mesa {
            a_h: false,
            a_i: false,
        },
        38 => TerrainKind::Mesa {
            a_h: false,
            a_i: true,
        },
        39 => TerrainKind::Mesa {
            a_h: false,
            a_i: false,
        },
        165 => TerrainKind::Mesa {
            a_h: true,
            a_i: false,
        },
        166 => TerrainKind::Mesa {
            a_h: false,
            a_i: true,
        },
        167 => TerrainKind::Mesa {
            a_h: false,
            a_i: false,
        },
        130 => TerrainKind::Delegate(2),
        133 => TerrainKind::Delegate(5),
        149 => TerrainKind::Delegate(21),
        151 => TerrainKind::Delegate(23),
        155 => TerrainKind::Delegate(27),
        156 => TerrainKind::Delegate(28),
        157 => TerrainKind::Delegate(29),
        158 => TerrainKind::Delegate(30),
        _ => TerrainKind::Default,
    }
}

/// `MathHelper.denormalizeClamp`.
fn denormalize_clamp(lower: f64, upper: f64, alpha: f64) -> f64 {
    if alpha < 0.0 {
        lower
    } else if alpha > 1.0 {
        upper
    } else {
        lower + (upper - lower) * alpha
    }
}

/// `MathHelper.sqrt_float`.
fn sqrt_float(v: f32) -> f32 {
    (v as f64).sqrt() as f32
}

/// `Math.round(double)`.
fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i64 as i32
}

/// `WorldChunkManager.getBiomeGenerator(x, z, ..)`: the biome-index (`Voronoi`) layer's biome at a
/// block coordinate. The 16x16 block cache only rounds the coordinate down and indexes `x + z*16`.
fn biome_index_at(voronoi: &mut crate::layer::Layer, bx: i32, bz: i32) -> i32 {
    let cx = bx & !15;
    let cz = bz & !15;
    let ints = voronoi.get_ints(cx, cz, 16, 16);
    ints[((bx & 15) + (bz & 15) * 16) as usize]
}

/// `WorldChunkManager`'s two biome queries the structure generators use: the biome-index (`Voronoi`)
/// layer for `getBiomeGenerator`, and the generation (`genBiomes`) layer for `areBiomesViable`.
pub struct BiomeLookup<'a> {
    /// `var5[1]` - `biomeIndexLayer` (`getBiomeGenerator`).
    pub voronoi: &'a mut crate::layer::Layer,
    /// `var5[0]` - `genBiomes` (`areBiomesViable` / `findBiomePosition`).
    pub biome: &'a mut crate::layer::Layer,
}

impl BiomeLookup<'_> {
    /// `WorldChunkManager.getBiomeGenerator(x, z, ..)` at a block coordinate.
    pub fn index_at(&mut self, bx: i32, bz: i32) -> i32 {
        biome_index_at(self.voronoi, bx, bz)
    }

    /// `WorldChunkManager.areBiomesViable(x, z, radius, allowed)`: every generation-layer biome in
    /// the `((x±radius)>>2)` grid must be in `allowed`.
    pub fn are_viable(&mut self, x: i32, z: i32, radius: i32, allowed: &[i32]) -> bool {
        let c0 = (x - radius) >> 2;
        let d0 = (z - radius) >> 2;
        let c1 = (x + radius) >> 2;
        let d1 = (z + radius) >> 2;
        let w = c1 - c0 + 1;
        let h = d1 - d0 + 1;
        let ints = self.biome.get_ints(c0, d0, w, h);
        ints.iter().all(|id| allowed.contains(id))
    }

    /// `WorldChunkManager.findBiomePosition(x, z, radius, allowed, rand)`: reservoir sampling over
    /// the generation-layer biomes in the `((x±radius)>>2)` grid, returning the block position
    /// `(bx, 0, bz)` or `None`. `rand` is the caller's (world-seed) `Random`.
    pub fn find_biome_position(
        &mut self,
        x: i32,
        z: i32,
        radius: i32,
        allowed: &[i32],
        rand: &mut mc_core::random::JavaRandom,
    ) -> Option<(i32, i32)> {
        let c0 = (x - radius) >> 2;
        let d0 = (z - radius) >> 2;
        let c1 = (x + radius) >> 2;
        let d1 = (z + radius) >> 2;
        let w = c1 - c0 + 1;
        let h = d1 - d0 + 1;
        let ints = self.biome.get_ints(c0, d0, w, h);
        let mut pos: Option<(i32, i32)> = None;
        let mut count = 0;
        for (i, &id) in ints.iter().enumerate() {
            let i = i as i32;
            let px = (c0 + i % w) << 2;
            let pz = (d0 + i / w) << 2;
            if allowed.contains(&id) && (pos.is_none() || rand.next_int_bound(count + 1) == 0) {
                pos = Some((px, pz));
                count += 1;
            }
        }
        pos
    }
}

/// `MapGenStronghold.field_151546_e`: every registered `BiomeGenBase` with `minHeight > 0` (the
/// stronghold placement candidates). Ids are base `0..=39` plus the registered mutations.
pub const STRONGHOLD_BIOMES: &[i32] = &[
    1, 2, 3, 4, 5, 8, 9, 12, 13, 14, 17, 18, 19, 20, 21, 22, 23, 25, 27, 28, 29, 30, 31, 32, 33,
    34, 35, 36, 37, 38, 39, 129, 130, 131, 132, 133, 140, 149, 151, 155, 156, 157, 158, 160, 161,
    162, 163, 164, 165, 166, 167,
];

/// `BiomeGenMesa.func_150619_a` — builds the 64-entry colored-clay table (its own `Perlin` offset
/// noise is created by the caller from the same `Random`).
fn build_clay_table(r: &mut JavaRandom) -> [u16; 64] {
    let mut a = [HARDENED_CLAY; 64];

    let mut i = 0i32;
    while i < 64 {
        i += r.next_int_bound(5) + 1;
        if i < 64 {
            a[i as usize] = CLAY_ORANGE;
        }
        i += 1; // the vanilla for-loop's update clause
    }

    let n = r.next_int_bound(4) + 2;
    for _ in 0..n {
        let len = r.next_int_bound(3) + 1;
        let start = r.next_int_bound(64);
        let mut k = 0;
        while start + k < 64 && k < len {
            a[(start + k) as usize] = CLAY_YELLOW;
            k += 1;
        }
    }

    let n = r.next_int_bound(4) + 2;
    for _ in 0..n {
        let len = r.next_int_bound(3) + 2;
        let start = r.next_int_bound(64);
        let mut k = 0;
        while start + k < 64 && k < len {
            a[(start + k) as usize] = CLAY_BROWN;
            k += 1;
        }
    }

    let n = r.next_int_bound(4) + 2;
    for _ in 0..n {
        let len = r.next_int_bound(3) + 1;
        let start = r.next_int_bound(64);
        let mut k = 0;
        while start + k < 64 && k < len {
            a[(start + k) as usize] = CLAY_RED;
            k += 1;
        }
    }

    let n = r.next_int_bound(3) + 3;
    let mut pos = 0i32;
    for _ in 0..n {
        pos += r.next_int_bound(16) + 4;
        let mut k = 0;
        while pos + k < 64 && k < 1 {
            a[(pos + k) as usize] = CLAY_WHITE;
            if pos + k > 1 && r.next_boolean() {
                a[(pos + k - 1) as usize] = CLAY_SILVER;
            }
            if pos + k < 63 && r.next_boolean() {
                a[(pos + k + 1) as usize] = CLAY_SILVER;
            }
            k += 1;
        }
    }

    a
}

/// One overworld `ChunkProviderGenerate`.
pub struct TerrainGen {
    seed: i64,
    caves: Caves,
    ravines: Ravines,
    mineshafts: Mineshaft,
    scattered: ScatteredFeature,
    monuments: OceanMonument,
    village: Village,
    stronghold: Stronghold,
    lower_gen: Octaves,
    upper_gen: Octaves,
    main_gen: Octaves,
    field_147430_m: Perlin,
    noise_gen6: Octaves,
    #[allow(dead_code)]
    noise_gen5: Octaves,
    #[allow(dead_code)]
    mob_spawner_noise: Octaves,
    parabolic: [f32; 25],
    biomes: [i32; 100],
    q: Vec<f64>,
    main: Vec<f64>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    depth: Vec<f64>,
    stone_noise: Vec<f64>,
    grass_color_noise: Perlin,
    clay: [u16; 64],
    clay_noise: Perlin,
    mesa_e: Perlin,
    mesa_f: Perlin,
    rand: JavaRandom,
    layers: BiomeLayers,
    /// `WorldGenBigMushroom.mushroomType` shared per biome (brown = true), keyed by biome id.
    big_mushroom_state: std::collections::BTreeMap<i32, bool>,
    /// `WorldGenBigTree.heightLimit` shared per biome, keyed by biome id.
    big_tree_state: std::collections::BTreeMap<i32, i32>,
}

impl TerrainGen {
    /// Builds the noise stack in vanilla order, consuming the seed's `Random` draws as the game
    /// does. `BiomeGenBase`'s static `GRASS_COLOR_NOISE` is `Perlin(Random(2345), 1)`; Mesa's
    /// per-world noise is cached on first use (a fresh world starts with the cached seed 0, so its
    /// `aE`/`aF` come from `Random(0)`).
    pub fn new(seed: i64) -> TerrainGen {
        let mut rand = JavaRandom::new(seed);
        let lower_gen = Octaves::new(&mut rand, 16);
        let upper_gen = Octaves::new(&mut rand, 16);
        let main_gen = Octaves::new(&mut rand, 8);
        let field_147430_m = Perlin::new(&mut rand, 4);
        let noise_gen5 = Octaves::new(&mut rand, 10);
        let noise_gen6 = Octaves::new(&mut rand, 16);
        let mob_spawner_noise = Octaves::new(&mut rand, 8);

        let mut parabolic = [0.0f32; 25];
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                parabolic[(dx + 2 + (dz + 2) * 5) as usize] =
                    10.0 / sqrt_float((dx * dx + dz * dz) as f32 + 0.2);
            }
        }

        let grass_color_noise = Perlin::new(&mut JavaRandom::new(2345), 1);

        // BiomeGenMesa: `func_150619_a(seed)` builds the clay table + offset noise from `Random(seed)`;
        // the cached seed starts at 0, so `aE`/`aF` use `Random(0)`.
        let mut r1 = JavaRandom::new(seed);
        let clay_noise = Perlin::new(&mut r1, 1);
        let clay = build_clay_table(&mut r1);
        let mut r0 = JavaRandom::new(0);
        let mesa_e = Perlin::new(&mut r0, 4);
        let mesa_f = Perlin::new(&mut r0, 1);

        TerrainGen {
            seed,
            caves: Caves::new(),
            ravines: Ravines::new(),
            mineshafts: Mineshaft::new(),
            scattered: ScatteredFeature::new(),
            monuments: OceanMonument::new(),
            village: Village::new(),
            stronghold: Stronghold::new(),
            lower_gen,
            upper_gen,
            main_gen,
            field_147430_m,
            noise_gen6,
            noise_gen5,
            mob_spawner_noise,
            parabolic,
            biomes: [0; 100],
            q: vec![0.0; 825],
            main: vec![0.0; 825],
            lower: vec![0.0; 825],
            upper: vec![0.0; 825],
            depth: vec![0.0; 25],
            stone_noise: vec![0.0; 256],
            grass_color_noise,
            clay,
            clay_noise,
            mesa_e,
            mesa_f,
            rand,
            layers: BiomeLayers::new(seed),
            big_mushroom_state: std::collections::BTreeMap::new(),
            big_tree_state: std::collections::BTreeMap::new(),
        }
    }
}

/// Which passes of `ChunkProviderGenerate.provideChunk` run after the surface replacement.
/// Decoration (`populate`) is not ported yet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GenOptions {
    pub caves: bool,
    pub ravines: bool,
    /// `useMineShafts && mapFeaturesEnabled` - registers mineshaft starts (their blocks are written
    /// later, in `populate`).
    pub mineshafts: bool,
    /// `useScatteredFeatures? && mapFeaturesEnabled` - registers desert/jungle temple and swamp-hut
    /// starts (their blocks are written later, in `populate`).
    pub scattered_features: bool,
    /// `useOceanMonuments? && mapFeaturesEnabled` - registers ocean-monument starts.
    pub monuments: bool,
    /// `useVillages? && mapFeaturesEnabled` - registers village starts.
    pub villages: bool,
    /// `useStrongholds? && mapFeaturesEnabled` - registers stronghold starts.
    pub strongholds: bool,
}

impl GenOptions {
    /// `useCaves = false`, `useRavines = false`, structures off (base terrain only).
    pub fn none() -> GenOptions {
        GenOptions {
            caves: false,
            ravines: false,
            mineshafts: false,
            scattered_features: false,
            monuments: false,
            villages: false,
            strongholds: false,
        }
    }

    /// `ChunkProviderSettings.Factory` defaults with `mapFeaturesEnabled = true`.
    pub fn vanilla() -> GenOptions {
        GenOptions {
            caves: true,
            ravines: true,
            mineshafts: true,
            scattered_features: true,
            monuments: true,
            villages: true,
            strongholds: true,
        }
    }
}

impl Default for GenOptions {
    fn default() -> GenOptions {
        GenOptions::vanilla()
    }
}

impl TerrainGen {
    /// The base terrain of one chunk: `setBlocksInChunk` + `replaceBlocksForBiome`
    /// (`provideChunk` with caves, ravines and structures off).
    pub fn generate_base_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<u16> {
        self.base_and_surface(chunk_x, chunk_z).0
    }

    /// `ChunkProviderGenerate.provideChunk`: base terrain, surface replacement, the selected
    /// carving passes, structure registration, then the canonical chunk artefacts (biome array +
    /// height map).
    pub fn generate_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        opts: GenOptions,
    ) -> GeneratedChunk {
        let (mut primer, biome_ids) = self.base_and_surface(chunk_x, chunk_z);
        if opts.caves {
            let _span = mc_perf::span("worldgen.caves");
            self.caves
                .generate(self.seed, chunk_x, chunk_z, &mut primer, &biome_ids);
        }
        if opts.ravines {
            let _span = mc_perf::span("worldgen.ravines");
            self.ravines
                .generate(self.seed, chunk_x, chunk_z, &mut primer, &biome_ids);
        }
        if opts.mineshafts {
            let _span = mc_perf::span("worldgen.mineshafts");
            self.mineshafts
                .generate(self.seed, chunk_x, chunk_z, SEA_LEVEL);
        }
        if opts.scattered_features || opts.monuments || opts.villages || opts.strongholds {
            let seed = self.seed;
            let TerrainGen {
                scattered,
                monuments,
                village,
                stronghold,
                layers,
                ..
            } = self;
            let mut lookup = crate::terrain::BiomeLookup {
                voronoi: &mut layers.voronoi,
                biome: &mut layers.biome,
            };
            if opts.scattered_features {
                let _span = mc_perf::span("worldgen.temples");
                scattered.generate(seed, chunk_x, chunk_z, &mut |bx, bz| {
                    lookup.index_at(bx, bz)
                });
            }
            if opts.monuments {
                let _span = mc_perf::span("worldgen.monuments");
                monuments.generate(seed, chunk_x, chunk_z, &mut lookup);
            }
            if opts.villages {
                let _span = mc_perf::span("worldgen.villages");
                village.generate(seed, chunk_x, chunk_z, &mut lookup);
            }
            if opts.strongholds {
                let _span = mc_perf::span("worldgen.strongholds");
                stronghold.generate(seed, chunk_x, chunk_z, &mut lookup);
            }
        }

        let _span = mc_perf::span("worldgen.artifacts");
        let height_map = chunk::height_map(&primer);
        let biomes = chunk::biome_array(&biome_ids);
        let mut structures = self.mineshafts.starts_intersecting(chunk_x, chunk_z);
        if opts.scattered_features {
            structures.extend(self.scattered.starts_intersecting(chunk_x, chunk_z));
        }
        if opts.monuments {
            structures.extend(self.monuments.starts_intersecting(chunk_x, chunk_z));
        }
        if opts.villages {
            structures.extend(self.village.starts_intersecting(chunk_x, chunk_z));
        }
        if opts.strongholds {
            structures.extend(self.stronghold.starts_intersecting(chunk_x, chunk_z));
        }
        GeneratedChunk {
            blocks: primer,
            biomes,
            height_map,
            structures,
        }
    }

    /// `ChunkProviderGenerate.populate`'s mineshaft passes for one chunk: the target chunk's blocks
    /// after the four covering populate passes write their pieces.
    ///
    /// Vanilla's `generateStructure` box is offset by 8 ([`placement::structure_box`]), so it spans two
    /// chunks per axis; the target chunk has terrain supplied for the surrounding 3x3 (which also
    /// registers every start the passes can meet) and is written by the four chunks whose boxes cover
    /// it. The boxes are disjoint, but piece state carries across passes, so they run in a fixed order
    /// ((cx-1,cz-1), (cx-1,cz), (cx,cz-1), (cx,cz)); vanilla's is chunk-load order.
    pub fn populate_mineshaft_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<u16> {
        let mut world = ChunkWorld::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                let chunk = self.generate_chunk(chunk_x + dx, chunk_z + dz, GenOptions::vanilla());
                world.insert_chunk(chunk_x + dx, chunk_z + dz, chunk.blocks);
            }
        }

        for a in [chunk_x - 1, chunk_x] {
            for b in [chunk_z - 1, chunk_z] {
                let mut rand = placement::populate_rand(self.seed, a, b);
                let bbox = placement::structure_box(a, b);
                let x0 = (a << 4) + 8;
                let z0 = (b << 4) + 8;
                self.mineshafts
                    .for_each_start_meeting_mut(x0, z0, x0 + 15, z0 + 15, |start| {
                        placement::generate_structure(start, &mut world, &mut rand, &bbox);
                    });
            }
        }

        world.chunk_blocks(chunk_x, chunk_z).to_vec()
    }

    /// `ChunkProviderGenerate.populate`'s decoration: the target chunk's blocks after the four
    /// covering `BiomeDecorator.decorate` passes (the `+8`-offset window, see `crate::decorate`).
    ///
    /// Vanilla's pass order is chunk-load order; the port uses a fixed order
    /// ((cx-1,cz-1), (cx-1,cz), (cx,cz-1), (cx,cz)) and corpora keep order-independent fixtures.
    pub fn decorate_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        opts: DecorateOptions,
    ) -> Vec<u16> {
        let mut world = DecorateWorld::new(SEA_LEVEL);
        let mut biomes = std::collections::BTreeMap::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                let chunk = self.generate_chunk(chunk_x + dx, chunk_z + dz, GenOptions::vanilla());
                biomes.insert((chunk_x + dx, chunk_z + dz), chunk.biomes);
                world.insert_chunk(chunk_x + dx, chunk_z + dz, chunk.blocks);
            }
        }

        let settings = DecorateSettings::default();
        for a in [chunk_x - 1, chunk_x] {
            for b in [chunk_z - 1, chunk_z] {
                let mut rand = placement::populate_rand(self.seed, a, b);
                // `getBiomeGenForCoords(field.add(16, 0, 16))`: the generation biome at the corner,
                // i.e. local (0, 0) of the next chunk (`biome_array` index `x + z * 16`).
                let biome_id = biomes[&(a + 1, b + 1)][0] as i32;
                decorate::decorate(
                    &mut world,
                    &mut rand,
                    a * 16,
                    b * 16,
                    biome_id,
                    &settings,
                    opts,
                    &mut self.grass_color_noise,
                    &mut self.big_mushroom_state,
                    &mut self.big_tree_state,
                );
            }
        }

        world.chunk_blocks(chunk_x, chunk_z).to_vec()
    }

    /// `provideChunk` steps 1-4, returning the primer and the generation biome array.
    fn base_and_surface(&mut self, chunk_x: i32, chunk_z: i32) -> (Vec<u16>, Vec<i32>) {
        let _span = mc_perf::span("worldgen.chunk");
        self.rand
            .set_seed(chunk_x as i64 * 341873128712 + chunk_z as i64 * 132897987541);
        let mut primer = self.set_blocks_in_chunk(chunk_x, chunk_z);
        let biomes = self
            .layers
            .voronoi
            .get_ints(chunk_x * 16, chunk_z * 16, 16, 16);
        self.replace_blocks_for_biome(chunk_x, chunk_z, &mut primer, &biomes);
        (primer, biomes)
    }

    /// `ChunkProviderGenerate.replaceBlocksForBiome`.
    fn replace_blocks_for_biome(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        primer: &mut [u16],
        biomes: &[i32],
    ) {
        let _span = mc_perf::span("worldgen.surface");
        self.field_147430_m.generate_default(
            &mut self.stone_noise,
            (chunk_x * 16) as f64,
            (chunk_z * 16) as f64,
            16,
            16,
            0.03125 * 2.0,
            0.03125 * 2.0,
            1.0,
        );

        for a in 0..16i32 {
            for b in 0..16i32 {
                let idx = (b + a * 16) as usize;
                let biome = biomes[idx];
                let sn = self.stone_noise[idx];
                self.gen_terrain(biome, chunk_x * 16 + a, chunk_z * 16 + b, sn, primer);
            }
        }
    }

    fn gen_terrain(&mut self, biome: i32, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let temp = biome_temp(biome);
        match terrain_kind(biome) {
            TerrainKind::Default => {
                let (top, filler) = biome_top_filler(biome);
                self.generate_biome_terrain(temp, x, z, sn, top, filler, primer);
            }
            TerrainKind::Hills(h) => self.gen_hills(h, temp, x, z, sn, primer),
            TerrainKind::Taiga(h) => self.gen_taiga(h, temp, x, z, sn, primer),
            TerrainKind::Swamp => self.gen_swamp(temp, x, z, sn, primer),
            TerrainKind::SavannaMutated => self.gen_savanna_mutated(temp, x, z, sn, primer),
            TerrainKind::Mesa { a_h, a_i } => self.gen_mesa(a_h, a_i, x, z, sn, primer),
            TerrainKind::Delegate(base) => self.gen_terrain(base, x, z, sn, primer),
        }
    }

    /// `BiomeGenHills.genTerrainBlocks`.
    fn gen_hills(&mut self, h: i32, temp: f32, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let (top, filler) = if (sn < -1.0 || sn > 2.0) && h == 2 {
            (GRAVEL, GRAVEL)
        } else if sn > 1.0 && h != 1 {
            (STONE, STONE)
        } else {
            (GRASS, DIRT)
        };
        self.generate_biome_terrain(temp, x, z, sn, top, filler, primer);
    }

    /// `BiomeGenTaiga.genTerrainBlocks`.
    fn gen_taiga(&mut self, h: i32, temp: f32, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let mut top = GRASS;
        if h == 1 || h == 2 {
            if sn > 1.75 {
                top = COARSE_DIRT;
            } else if sn > -0.95 {
                top = PODZOL;
            }
        }
        self.generate_biome_terrain(temp, x, z, sn, top, DIRT, primer);
    }

    /// `BiomeGenSavanna.Mutated.genTerrainBlocks`.
    fn gen_savanna_mutated(&mut self, temp: f32, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let (top, filler) = if sn > 1.75 {
            (STONE, STONE)
        } else if sn > -0.5 {
            (COARSE_DIRT, DIRT)
        } else {
            (GRASS, DIRT)
        };
        self.generate_biome_terrain(temp, x, z, sn, top, filler, primer);
    }

    /// `BiomeGenSwamp.genTerrainBlocks`.
    fn gen_swamp(&mut self, temp: f32, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let v = self
            .grass_color_noise
            .sample_2d(x as f64 * 0.25, z as f64 * 0.25);
        if v > 0.0 {
            let xin = x & 15;
            let zin = z & 15;
            for y in (0..=255i32).rev() {
                if get(primer, zin, y, xin) != 0 {
                    if y == 62 && (get(primer, zin, y, xin) >> 4) != 9 {
                        set(primer, zin, y, xin, WATER);
                        if v < 0.12 {
                            set(primer, zin, y + 1, xin, WATERLILY);
                        }
                    }
                    break;
                }
            }
        }
        self.generate_biome_terrain(temp, x, z, sn, GRASS, DIRT, primer);
    }

    /// `BiomeGenBase.generateBiomeTerrain`.
    #[allow(clippy::too_many_arguments)]
    fn generate_biome_terrain(
        &mut self,
        temp: f32,
        x: i32,
        z: i32,
        sn: f64,
        top0: u16,
        filler0: u16,
        primer: &mut [u16],
    ) {
        let sea = SEA_LEVEL;
        let mut top = top0;
        let mut filler = filler0;
        let mut k = -1i32;
        let depth = (sn / 3.0 + 3.0 + self.rand.next_double() * 0.25) as i32;
        let xin = x & 15;
        let zin = z & 15;

        for y in (0..=255i32).rev() {
            if y <= self.rand.next_int_bound(5) {
                set(primer, zin, y, xin, BEDROCK);
            } else if get(primer, zin, y, xin) == 0 {
                k = -1;
            } else if (get(primer, zin, y, xin) >> 4) == 1 {
                if k == -1 {
                    if depth <= 0 {
                        top = 0;
                        filler = STONE;
                    } else if y >= sea - 4 && y <= sea + 1 {
                        top = top0;
                        filler = filler0;
                    }
                    if y < sea && top == 0 {
                        top = if temp < 0.15 { ICE } else { WATER };
                    }
                    k = depth;
                    if y >= sea - 1 {
                        set(primer, zin, y, xin, top);
                    } else if y < sea - 7 - depth {
                        top = 0;
                        filler = STONE;
                        set(primer, zin, y, xin, GRAVEL);
                    } else {
                        set(primer, zin, y, xin, filler);
                    }
                } else if k > 0 {
                    k -= 1;
                    set(primer, zin, y, xin, filler);
                    if k == 0 && (filler >> 4) == 12 {
                        k = self.rand.next_int_bound(4) + 0.max(y - 63);
                        filler = if (filler & 15) == 1 {
                            RED_SANDSTONE
                        } else {
                            SANDSTONE
                        };
                    }
                }
            }
        }
    }

    /// `BiomeGenMesa.genTerrainBlocks`.
    fn gen_mesa(&mut self, a_h: bool, a_i: bool, x: i32, z: i32, sn: f64, primer: &mut [u16]) {
        let mut var22 = 0.0f64;
        if a_h {
            let v10 = (x & -16) + (z & 15);
            let v11 = (z & -16) + (x & 15);
            let v12 = sn
                .abs()
                .min(self.mesa_e.sample_2d(v10 as f64 * 0.25, v11 as f64 * 0.25));
            if v12 > 0.0 {
                let v14 = 0.001953125f64;
                let v16 = self
                    .mesa_f
                    .sample_2d(v10 as f64 * v14, v11 as f64 * v14)
                    .abs();
                var22 = v12 * v12 * 2.5;
                let v18 = (v16 * 50.0).ceil() + 14.0;
                if var22 > v18 {
                    var22 = v18;
                }
                var22 += 64.0;
            }
        }

        let xin = x & 15;
        let zin = z & 15;
        let sea = SEA_LEVEL;
        let mut band = 159 << 4;
        let mut filler = 159 << 4;
        let depth = (sn / 3.0 + 3.0 + self.rand.next_double() * 0.25) as i32;
        let v28 = (sn / 3.0 * PI).cos() > 0.0;
        let mut var17 = -1i32;
        let mut var29 = false;

        for y in (0..=255i32).rev() {
            if get(primer, zin, y, xin) == 0 && y < var22 as i32 {
                set(primer, zin, y, xin, STONE);
            }
            if y <= self.rand.next_int_bound(5) {
                set(primer, zin, y, xin, BEDROCK);
            } else if get(primer, zin, y, xin) == 0 {
                var17 = -1;
            } else if (get(primer, zin, y, xin) >> 4) == 1 {
                if var17 == -1 {
                    var29 = false;
                    if depth <= 0 {
                        band = 0;
                        filler = STONE;
                    } else if y >= sea - 4 && y <= sea + 1 {
                        band = 159 << 4;
                        filler = 159 << 4;
                    }
                    if y < sea && band == 0 {
                        band = WATER;
                    }
                    var17 = depth + 0.max(y - sea);
                    if y >= sea - 1 {
                        if !a_i || y <= 86 + depth * 2 {
                            if y > sea + 3 + depth {
                                let v21 = if y < 64 || y > 127 {
                                    CLAY_ORANGE
                                } else if v28 {
                                    HARDENED_CLAY
                                } else {
                                    self.func_180629_a(x, y, z)
                                };
                                set(primer, zin, y, xin, v21);
                            } else {
                                set(primer, zin, y, xin, (12 << 4) | 1);
                                var29 = true;
                            }
                        } else if v28 {
                            set(primer, zin, y, xin, COARSE_DIRT);
                        } else {
                            set(primer, zin, y, xin, GRASS);
                        }
                    } else {
                        set(primer, zin, y, xin, filler);
                        if (filler >> 4) == 159 {
                            set(primer, zin, y, xin, CLAY_ORANGE);
                        }
                    }
                } else if var17 > 0 {
                    var17 -= 1;
                    if var29 {
                        set(primer, zin, y, xin, CLAY_ORANGE);
                    } else {
                        let c = self.func_180629_a(x, y, z);
                        set(primer, zin, y, xin, c);
                    }
                }
            }
        }
    }

    /// `BiomeGenMesa.func_180629_a`.
    fn func_180629_a(&self, x: i32, y: i32, z: i32) -> u16 {
        let _ = z;
        let off = java_round(
            self.clay_noise
                .sample_2d(x as f64 / 512.0, x as f64 / 512.0)
                * 2.0,
        );
        self.clay[((y + off + 64) % 64) as usize]
    }

    /// `ChunkProviderGenerate.func_147423_a` — the density field for one chunk.
    fn func_147423_a(&mut self, x: i32, y: i32, z: i32) {
        let coord = COORDINATE_SCALE;
        let height = HEIGHT_SCALE;

        self.noise_gen6.generate(
            &mut self.depth,
            x,
            10,
            z,
            5,
            1,
            5,
            DEPTH_NOISE_SCALE_X as f64,
            DEPTH_NOISE_SCALE_EXPONENT as f64,
            DEPTH_NOISE_SCALE_Z as f64,
        );
        self.main_gen.generate(
            &mut self.main,
            x,
            y,
            z,
            5,
            33,
            5,
            (coord / MAIN_NOISE_SCALE_X) as f64,
            (height / MAIN_NOISE_SCALE_Y) as f64,
            (coord / MAIN_NOISE_SCALE_Z) as f64,
        );
        self.lower_gen.generate(
            &mut self.lower,
            x,
            y,
            z,
            5,
            33,
            5,
            coord as f64,
            height as f64,
            coord as f64,
        );
        self.upper_gen.generate(
            &mut self.upper,
            x,
            y,
            z,
            5,
            33,
            5,
            coord as f64,
            height as f64,
            coord as f64,
        );

        let mut q_idx = 0usize;
        let mut d_idx = 0usize;
        for i in 0..5i32 {
            for j in 0..5i32 {
                let mut depth_sum = 0.0f32;
                let mut scale_sum = 0.0f32;
                let mut weight_sum = 0.0f32;
                let base_id = self.biomes[(i + 2 + (j + 2) * 10) as usize];
                let base_min = biome_height(base_id).0;

                for a in -2i32..=2 {
                    for b in -2i32..=2 {
                        let bid = self.biomes[(i + a + 2 + (j + b + 2) * 10) as usize];
                        let (min_h, max_h) = biome_height(bid);
                        let depth = BIOME_DEPTH_OFFSET + min_h * BIOME_DEPTH_WEIGHT;
                        let scale = BIOME_SCALE_OFFSET + max_h * BIOME_SCALE_WEIGHT;

                        let mut weight =
                            self.parabolic[(a + 2 + (b + 2) * 5) as usize] / (depth + 2.0);
                        if min_h > base_min {
                            weight /= 2.0;
                        }

                        scale_sum += scale * weight;
                        depth_sum += depth * weight;
                        weight_sum += weight;
                    }
                }

                let scale_avg = scale_sum / weight_sum;
                let depth_avg = depth_sum / weight_sum;
                let scale_final = scale_avg * 0.9 + 0.1;
                let depth_final = (depth_avg * 4.0 - 1.0) / 8.0;

                let mut d = self.depth[d_idx] / 8000.0;
                d_idx += 1;
                if d < 0.0 {
                    d = -d * 0.3;
                }
                d = d * 3.0 - 2.0;
                if d < 0.0 {
                    d /= 2.0;
                    if d < -1.0 {
                        d = -1.0;
                    }
                    d /= 1.4;
                    d /= 2.0;
                } else {
                    if d > 1.0 {
                        d = 1.0;
                    }
                    d /= 8.0;
                }

                let mut var47 = depth_final as f64;
                var47 += d * 0.2;
                var47 = var47 * BASE_SIZE as f64 / 8.0;
                let var21 = BASE_SIZE as f64 + var47 * 4.0;

                for k in 0..33i32 {
                    let mut var24 =
                        (k as f64 - var21) * STRETCH_Y as f64 * 128.0 / 256.0 / scale_final as f64;
                    if var24 < 0.0 {
                        var24 *= 4.0;
                    }

                    let var26 = self.lower[q_idx] / LOWER_LIMIT_SCALE as f64;
                    let var28 = self.upper[q_idx] / UPPER_LIMIT_SCALE as f64;
                    let var30 = (self.main[q_idx] / 10.0 + 1.0) / 2.0;
                    let mut var32 = denormalize_clamp(var26, var28, var30) - var24;
                    if k > 29 {
                        let t = (k as f64 - 29.0) / 3.0f64;
                        var32 = var32 * (1.0 - t) + -10.0 * t;
                    }
                    self.q[q_idx] = var32;
                    q_idx += 1;
                }
            }
        }
    }

    /// `ChunkProviderGenerate.setBlocksInChunk` — the stone/water/air shape.
    pub fn set_blocks_in_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<u16> {
        let _span = mc_perf::span("worldgen.shape");
        let ids = self
            .layers
            .biome
            .get_ints(chunk_x * 4 - 2, chunk_z * 4 - 2, 10, 10);
        self.biomes.copy_from_slice(&ids);
        self.func_147423_a(chunk_x * 4, 0, chunk_z * 4);

        let mut primer = vec![0u16; 65536];
        for a in 0..4 {
            let v5 = a * 5;
            let v6 = (a + 1) * 5;
            for b in 0..4 {
                let v8 = ((v5 + b) * 33) as usize;
                let v9 = ((v5 + b + 1) * 33) as usize;
                let v10 = ((v6 + b) * 33) as usize;
                let v11 = ((v6 + b + 1) * 33) as usize;

                for c in 0..32 {
                    let v13 = 0.125f64;
                    let mut v15 = self.q[v8 + c];
                    let mut v17 = self.q[v9 + c];
                    let mut v19 = self.q[v10 + c];
                    let mut v21 = self.q[v11 + c];
                    let v23 = (self.q[v8 + c + 1] - v15) * v13;
                    let v25 = (self.q[v9 + c + 1] - v17) * v13;
                    let v27 = (self.q[v10 + c + 1] - v19) * v13;
                    let v29 = (self.q[v11 + c + 1] - v21) * v13;

                    for d in 0..8 {
                        let v32 = 0.25f64;
                        let mut v34 = v15;
                        let mut v36 = v17;
                        let v38 = (v19 - v15) * v32;
                        let v40 = (v21 - v17) * v32;

                        for e in 0..4 {
                            let v43 = 0.25f64;
                            let mut v45 = v34;
                            let v47 = (v36 - v34) * v43;
                            v45 -= v47;

                            for f in 0..4 {
                                v45 += v47;
                                let px = (a * 4 + e) as i32;
                                let py = (c * 8 + d) as i32;
                                let pz = (b * 4 + f) as i32;
                                if v45 > 0.0 {
                                    set(&mut primer, px, py, pz, STONE);
                                } else if py < SEA_LEVEL {
                                    set(&mut primer, px, py, pz, WATER);
                                }
                            }

                            v34 += v38;
                            v36 += v40;
                        }

                        v15 += v23;
                        v17 += v25;
                        v19 += v27;
                        v21 += v29;
                    }
                }
            }
        }
        primer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden shape values for seed 0, chunk (0, 0), column (x=0, z=0), from the oracle.
    #[test]
    fn terrain_shape_golden_seed0() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.set_blocks_in_chunk(0, 0);
        assert_eq!(primer.len(), 65536);
        let at = |y: usize| primer[y]; // x=0, z=0 → index = y
        assert_eq!(at(0), STONE);
        assert_eq!(at(63), STONE);
        assert_eq!(at(100), 0); // air
    }

    /// Golden surface values for seed 0, chunk (-48, -48) (Mesa), column (x=0, z=0): bedrock at 0,
    /// orange clay band at 63, water at 64 — from the oracle.
    #[test]
    fn terrain_surface_golden_seed0_mesa() {
        let mut gen = TerrainGen::new(0);
        let primer = gen.generate_base_chunk(-48, -48);
        let at = |y: usize| primer[y];
        assert_eq!(at(0), BEDROCK);
        assert_eq!(at(61), CLAY_ORANGE);
        assert_eq!(at(62), WATER);
    }
}
