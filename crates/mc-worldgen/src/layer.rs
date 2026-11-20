//! Bit-exact ports of the 1.8.9 biome-layer stack (`net.minecraft.world.gen.layer.*`)
//! for the default overworld (`WorldType.DEFAULT`; biomeSize = riverSize = 4), plus the
//! `BiomeGenBase` metadata the layers read.
//!
//! Ported from `GenLayer` + `GenLayer{Island,FuzzyZoom,Zoom,AddIsland,RemoveTooMuchOcean,
//! AddSnow,Edge,AddMushroomIsland,DeepOcean,Biome,BiomeEdge,Hills,RareBiome,RiverInit,River,
//! Smooth,Shore,RiverMix,VoronoiZoom}` and `IntCache`.
//!
//! The layer graph from `GenLayer.initializeAllBiomeGenerators` is a DAG (the biome chain is
//! reused by the Voronoi layer). We build it as an owned tree and duplicate the shared node:
//! a layer's only mutable state (`chunkSeed`) is reset every cell, and `worldGenSeed` is a pure
//! function of the seed and the node's own `baseSeed`, so duplication is behaviourally identical.
//! `IntCache` reuse is likewise unobservable (every layer fully writes its output), so each call
//! simply allocates.

const K_MUL: i64 = 6364136223846793005;
const K_ADD: i64 = 1442695040888963407;

use std::fmt;

// ---------------------------------------------------------------------------------------------
// Biome metadata (the subset of BiomeGenBase the layers query)
// ---------------------------------------------------------------------------------------------

const OCEAN: i32 = 0;
const PLAINS: i32 = 1;
const DESERT: i32 = 2;
const EXTREME_HILLS: i32 = 3;
const FOREST: i32 = 4;
const TAIGA: i32 = 5;
const SWAMPLAND: i32 = 6;
const RIVER: i32 = 7;
const ICE_PLAINS: i32 = 12;
const MUSHROOM_ISLAND: i32 = 14;
const MUSHROOM_ISLAND_SHORE: i32 = 15;
const BEACH: i32 = 16;
const JUNGLE: i32 = 21;
const JUNGLE_EDGE: i32 = 23;
const DEEP_OCEAN: i32 = 24;
const STONE_BEACH: i32 = 25;
const COLD_BEACH: i32 = 26;
const MESA: i32 = 37;
const MESA_PLATEAU_F: i32 = 38;
const MESA_PLATEAU: i32 = 39;

/// The effective `getBiomeClass()` of a biome (`BiomeGenMutated` delegates to its base).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BiomeClass {
    Ocean,
    Plains,
    Desert,
    Hills,
    Forest,
    Taiga,
    Swamp,
    River,
    Hell,
    End,
    Snow,
    MushroomIsland,
    Beach,
    Jungle,
    Savanna,
    Mesa,
    StoneBeach,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TempCategory {
    Ocean,
    Cold,
    Medium,
    Warm,
}

#[derive(Clone, Copy)]
pub struct BiomeInfo {
    pub class: BiomeClass,
    pub temperature: f32,
    pub snowy: bool,
}

// Index = biome id 0..=39; (class, temperature, enableSnow). Taken from BiomeGenBase's
// static registrations plus each biome class' constructor. Kept as tuples so rustfmt keeps
// it one-row-per-biome.
const BIOMES: [(BiomeClass, f32, bool); 40] = [
    (BiomeClass::Ocean, 0.5, false),          // 0 ocean
    (BiomeClass::Plains, 0.8, false),         // 1 plains
    (BiomeClass::Desert, 2.0, false),         // 2 desert
    (BiomeClass::Hills, 0.2, false),          // 3 extremeHills
    (BiomeClass::Forest, 0.7, false),         // 4 forest
    (BiomeClass::Taiga, 0.25, false),         // 5 taiga
    (BiomeClass::Swamp, 0.8, false),          // 6 swampland
    (BiomeClass::River, 0.5, false),          // 7 river
    (BiomeClass::Hell, 2.0, false),           // 8 hell
    (BiomeClass::End, 0.5, false),            // 9 sky
    (BiomeClass::Ocean, 0.0, true),           // 10 frozenOcean
    (BiomeClass::River, 0.0, true),           // 11 frozenRiver
    (BiomeClass::Snow, 0.0, true),            // 12 icePlains
    (BiomeClass::Snow, 0.0, true),            // 13 iceMountains
    (BiomeClass::MushroomIsland, 0.9, false), // 14 mushroomIsland
    (BiomeClass::MushroomIsland, 0.9, false), // 15 mushroomIslandShore
    (BiomeClass::Beach, 0.8, false),          // 16 beach
    (BiomeClass::Desert, 2.0, false),         // 17 desertHills
    (BiomeClass::Forest, 0.7, false),         // 18 forestHills
    (BiomeClass::Taiga, 0.25, false),         // 19 taigaHills
    (BiomeClass::Hills, 0.2, false),          // 20 extremeHillsEdge
    (BiomeClass::Jungle, 0.95, false),        // 21 jungle
    (BiomeClass::Jungle, 0.95, false),        // 22 jungleHills
    (BiomeClass::Jungle, 0.95, false),        // 23 jungleEdge
    (BiomeClass::Ocean, 0.5, false),          // 24 deepOcean
    (BiomeClass::StoneBeach, 0.2, false),     // 25 stoneBeach
    (BiomeClass::Beach, 0.05, true),          // 26 coldBeach
    (BiomeClass::Forest, 0.6, false),         // 27 birchForest
    (BiomeClass::Forest, 0.6, false),         // 28 birchForestHills
    (BiomeClass::Forest, 0.7, false),         // 29 roofedForest
    (BiomeClass::Taiga, -0.5, true),          // 30 coldTaiga
    (BiomeClass::Taiga, -0.5, true),          // 31 coldTaigaHills
    (BiomeClass::Taiga, 0.3, false),          // 32 megaTaiga
    (BiomeClass::Taiga, 0.3, false),          // 33 megaTaigaHills
    (BiomeClass::Hills, 0.2, false),          // 34 extremeHillsPlus
    (BiomeClass::Savanna, 1.2, false),        // 35 savanna
    (BiomeClass::Savanna, 1.0, false),        // 36 savannaPlateau
    (BiomeClass::Mesa, 2.0, false),           // 37 mesa
    (BiomeClass::Mesa, 2.0, false),           // 38 mesaPlateau_F
    (BiomeClass::Mesa, 2.0, false),           // 39 mesaPlateau
];

/// Base id for a mutation id (`createMutation` / `createMutatedBiome`), if it exists.
fn mutation_base(id: i32) -> Option<i32> {
    Some(match id {
        129 => PLAINS,
        130 => DESERT,
        131 => EXTREME_HILLS,
        132 => FOREST,
        133 => TAIGA,
        134 => SWAMPLAND,
        140 => ICE_PLAINS,
        149 => JUNGLE,
        151 => JUNGLE_EDGE,
        155 => 27,
        156 => 28,
        157 => 29,
        158 => 30,
        160 => 32,
        161 => 33,
        162 => 34,
        163 => 35,
        164 => 36,
        165 => MESA,
        166 => MESA_PLATEAU_F,
        167 => MESA_PLATEAU,
        _ => return None,
    })
}

/// `BiomeGenBase.getBiome(id)` as a property lookup (mutations report their base's properties).
pub fn biome_info(id: i32) -> Option<BiomeInfo> {
    let (class, temperature, snowy) = if (0..=39).contains(&id) {
        BIOMES[id as usize]
    } else {
        BIOMES[mutation_base(id)? as usize]
    };
    Some(BiomeInfo {
        class,
        temperature,
        snowy,
    })
}

/// `BiomeGenBase.getBiome(id) != null`.
fn biome_exists(id: i32) -> bool {
    biome_info(id).is_some()
}

/// `GenLayer.isBiomeOceanic`.
fn is_biome_oceanic(id: i32) -> bool {
    id == OCEAN || id == DEEP_OCEAN || id == 10 // 10 = frozenOcean
}

/// `BiomeGenBase.getTempCategory()` (ocean-class biomes are always `OCEAN`).
fn temp_category(id: i32) -> TempCategory {
    match biome_info(id) {
        Some(info) if info.class == BiomeClass::Ocean => TempCategory::Ocean,
        Some(info) => {
            if info.temperature < 0.2 {
                TempCategory::Cold
            } else if info.temperature < 1.0 {
                TempCategory::Medium
            } else {
                TempCategory::Warm
            }
        }
        None => TempCategory::Medium,
    }
}

/// `GenLayer.biomesEqualOrMesaPlateau`.
fn biomes_equal_or_mesa_plateau(a: i32, b: i32) -> bool {
    if a == b {
        return true;
    }
    if a != MESA_PLATEAU_F && a != MESA_PLATEAU {
        match (biome_info(a), biome_info(b)) {
            (Some(x), Some(y)) => x.class == y.class,
            _ => false,
        }
    } else {
        b == MESA_PLATEAU_F || b == MESA_PLATEAU
    }
}

// ---------------------------------------------------------------------------------------------
// The layer tree
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeMode {
    CoolWarm,
    HeatIce,
    Special,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Island,
    FuzzyZoom,
    Zoom,
    AddIsland,
    RemoveTooMuchOcean,
    AddSnow,
    Edge(EdgeMode),
    AddMushroomIsland,
    DeepOcean,
    Biome,
    BiomeEdge,
    Hills,
    RareBiome,
    RiverInit,
    River,
    Smooth,
    Shore,
    RiverMix,
    VoronoiZoom,
}

/// One `GenLayer`. `a` is the vanilla `parent`; `b` is the extra chain used by `Hills`
/// (`field_151628_d`) and `RiverMix` (`riverPatternGeneratorChain`).
#[derive(Clone)]
pub struct Layer {
    base_seed: i64,
    world_gen_seed: i64,
    chunk_seed: i64,
    kind: Kind,
    a: Option<Box<Layer>>,
    b: Option<Box<Layer>>,
}

impl fmt::Debug for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layer")
            .field("base_seed", &self.base_seed)
            .field("world_gen_seed", &self.world_gen_seed)
            .finish()
    }
}

fn mix_base_seed(k: i64) -> i64 {
    let mut s = k;
    for _ in 0..3 {
        s = s.wrapping_mul(s.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        s = s.wrapping_add(k);
    }
    s
}

impl Layer {
    fn new(kind: Kind, base_const: i64, a: Option<Box<Layer>>, b: Option<Box<Layer>>) -> Layer {
        Layer {
            base_seed: mix_base_seed(base_const),
            world_gen_seed: 0,
            chunk_seed: 0,
            kind,
            a,
            b,
        }
    }

    fn leaf(kind: Kind, base_const: i64) -> Layer {
        Layer::new(kind, base_const, None, None)
    }

    fn unary(kind: Kind, base_const: i64, a: Layer) -> Layer {
        Layer::new(kind, base_const, Some(Box::new(a)), None)
    }

    /// `GenLayer.initWorldGenSeed`. `RiverMix` seeds both chains; `Hills` does *not* seed its
    /// `field_151628_d` (matching vanilla, where that node keeps `worldGenSeed = 0`).
    fn init_world_gen_seed(&mut self, seed: i64) {
        match self.kind {
            Kind::RiverMix => {
                self.a.as_mut().unwrap().init_world_gen_seed(seed);
                self.b.as_mut().unwrap().init_world_gen_seed(seed);
            }
            _ => {
                if let Some(a) = self.a.as_mut() {
                    a.init_world_gen_seed(seed);
                }
            }
        }
        self.world_gen_seed = seed;
        for _ in 0..3 {
            self.world_gen_seed = self
                .world_gen_seed
                .wrapping_mul(self.world_gen_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
            self.world_gen_seed = self.world_gen_seed.wrapping_add(self.base_seed);
        }
    }

    fn init_chunk_seed(&mut self, x: i64, z: i64) {
        self.chunk_seed = self.world_gen_seed;
        self.chunk_seed = self
            .chunk_seed
            .wrapping_mul(self.chunk_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        self.chunk_seed = self.chunk_seed.wrapping_add(x);
        self.chunk_seed = self
            .chunk_seed
            .wrapping_mul(self.chunk_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        self.chunk_seed = self.chunk_seed.wrapping_add(z);
        self.chunk_seed = self
            .chunk_seed
            .wrapping_mul(self.chunk_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        self.chunk_seed = self.chunk_seed.wrapping_add(x);
        self.chunk_seed = self
            .chunk_seed
            .wrapping_mul(self.chunk_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        self.chunk_seed = self.chunk_seed.wrapping_add(z);
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        let mut v = ((self.chunk_seed >> 24) % bound as i64) as i32;
        if v < 0 {
            v += bound;
        }
        self.chunk_seed = self
            .chunk_seed
            .wrapping_mul(self.chunk_seed.wrapping_mul(K_MUL).wrapping_add(K_ADD));
        self.chunk_seed = self.chunk_seed.wrapping_add(self.world_gen_seed);
        v
    }

    fn select_random(&mut self, args: &[i32]) -> i32 {
        let i = self.next_int(args.len() as i32);
        args[i as usize]
    }

    fn select_mode_or_random(&mut self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        if b == c && c == d {
            b
        } else if a == b && a == c {
            a
        } else if a == b && a == d {
            a
        } else if a == c && a == d {
            a
        } else if a == b && c != d {
            a
        } else if a == c && b != d {
            a
        } else if a == d && b != c {
            a
        } else if b == c && a != d {
            b
        } else if b == d && a != c {
            b
        } else if c == d && a != b {
            c
        } else {
            self.select_random(&[a, b, c, d])
        }
    }

    /// `GenLayer.getInts`.
    pub fn get_ints(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        match self.kind {
            Kind::Island => self.ints_island(x, z, w, h),
            Kind::FuzzyZoom => self.ints_zoom(x, z, w, h, true),
            Kind::Zoom => self.ints_zoom(x, z, w, h, false),
            Kind::AddIsland => self.ints_add_island(x, z, w, h),
            Kind::RemoveTooMuchOcean => self.ints_remove_too_much_ocean(x, z, w, h),
            Kind::AddSnow => self.ints_add_snow(x, z, w, h),
            Kind::Edge(mode) => self.ints_edge(x, z, w, h, mode),
            Kind::AddMushroomIsland => self.ints_add_mushroom_island(x, z, w, h),
            Kind::DeepOcean => self.ints_deep_ocean(x, z, w, h),
            Kind::Biome => self.ints_biome(x, z, w, h),
            Kind::BiomeEdge => self.ints_biome_edge(x, z, w, h),
            Kind::Hills => self.ints_hills(x, z, w, h),
            Kind::RareBiome => self.ints_rare_biome(x, z, w, h),
            Kind::RiverInit => self.ints_river_init(x, z, w, h),
            Kind::River => self.ints_river(x, z, w, h),
            Kind::Smooth => self.ints_smooth(x, z, w, h),
            Kind::Shore => self.ints_shore(x, z, w, h),
            Kind::RiverMix => self.ints_river_mix(x, z, w, h),
            Kind::VoronoiZoom => self.ints_voronoi_zoom(x, z, w, h),
        }
    }

    fn parent(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        self.a.as_mut().unwrap().get_ints(x, z, w, h)
    }

    fn parent2(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        self.b.as_mut().unwrap().get_ints(x, z, w, h)
    }

    fn ints_island(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            for i in 0..w {
                self.init_chunk_seed((x + i) as i64, (z + j) as i64);
                out[(i + j * w) as usize] = if self.next_int(10) == 0 { 1 } else { 0 };
            }
        }
        if x > -w && x <= 0 && z > -h && z <= 0 {
            out[(-x + -z * w) as usize] = 1;
        }
        out
    }

    fn ints_zoom(&mut self, x: i32, z: i32, w: i32, h: i32, fuzzy: bool) -> Vec<i32> {
        let x2 = x >> 1;
        let z2 = z >> 1;
        let w2 = (w >> 1) + 2;
        let h2 = (h >> 1) + 2;
        let p = self.parent(x2, z2, w2, h2);
        let w3 = (w2 - 1) << 1;
        let h3 = (h2 - 1) << 1;
        let mut mid = vec![0i32; (w3 * h3) as usize];

        for row in 0..h2 - 1 {
            let mut o = (row << 1) * w3;
            let mut c0 = p[((row) * w2) as usize];
            let mut c1 = p[((row + 1) * w2) as usize];
            let mut i = 0;
            while i < w2 - 1 {
                self.init_chunk_seed(((i + x2) << 1) as i64, ((row + z2) << 1) as i64);
                let r = p[(i + 1 + row * w2) as usize];
                let s = p[(i + 1 + (row + 1) * w2) as usize];
                mid[o as usize] = c0;
                mid[(o + w3) as usize] = self.select_random(&[c0, c1]);
                o += 1;
                mid[o as usize] = self.select_random(&[c0, r]);
                mid[(o + w3) as usize] = if fuzzy {
                    self.select_random(&[c0, r, c1, s])
                } else {
                    self.select_mode_or_random(c0, r, c1, s)
                };
                o += 1;
                c0 = r;
                c1 = s;
                i += 1;
            }
        }

        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            let src = ((j + (z & 1)) * w3 + (x & 1)) as usize;
            for i in 0..w {
                out[(i + j * w) as usize] = mid[src + i as usize];
            }
        }
        out
    }

    fn ints_add_island(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                let c0 = p[(i + j * pw) as usize];
                let c1 = p[(i + 2 + j * pw) as usize];
                let c2 = p[(i + (j + 2) * pw) as usize];
                let c3 = p[(i + 2 + (j + 2) * pw) as usize];
                let mid = p[(i + 1 + (j + 1) * pw) as usize];
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                if mid != 0 || c0 == 0 && c1 == 0 && c2 == 0 && c3 == 0 {
                    if mid > 0 && (c0 == 0 || c1 == 0 || c2 == 0 || c3 == 0) {
                        if self.next_int(5) == 0 {
                            out[idx] = if mid == 4 { 4 } else { 0 };
                        } else {
                            out[idx] = mid;
                        }
                    } else {
                        out[idx] = mid;
                    }
                } else {
                    let mut chosen = 1;
                    let mut n = 1;
                    if c0 != 0 {
                        let r = self.next_int(n);
                        n += 1;
                        if r == 0 {
                            chosen = c0;
                        }
                    }
                    if c1 != 0 {
                        let r = self.next_int(n);
                        n += 1;
                        if r == 0 {
                            chosen = c1;
                        }
                    }
                    if c2 != 0 {
                        let r = self.next_int(n);
                        n += 1;
                        if r == 0 {
                            chosen = c2;
                        }
                    }
                    if c3 != 0 {
                        let r = self.next_int(n);
                        if r == 0 {
                            chosen = c3;
                        }
                    }
                    if self.next_int(3) == 0 {
                        out[idx] = chosen;
                    } else if chosen == 4 {
                        out[idx] = 4;
                    } else {
                        out[idx] = 0;
                    }
                }
            }
        }
        out
    }

    fn ints_remove_too_much_ocean(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                let top = p[(i + 1 + j * pw) as usize];
                let right = p[(i + 2 + (j + 1) * pw) as usize];
                let left = p[(i + (j + 1) * pw) as usize];
                let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                let mid = p[(i + 1 + (j + 1) * pw) as usize];
                out[idx] = mid;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                if mid == 0
                    && top == 0
                    && right == 0
                    && left == 0
                    && bottom == 0
                    && self.next_int(2) == 0
                {
                    out[idx] = 1;
                }
            }
        }
        out
    }

    fn ints_add_snow(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                let mid = p[(i + 1 + (j + 1) * pw) as usize];
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                if mid == 0 {
                    out[idx] = 0;
                } else {
                    let r = self.next_int(6);
                    out[idx] = if r == 0 {
                        4
                    } else if r <= 1 {
                        3
                    } else {
                        1
                    };
                }
            }
        }
        out
    }

    fn ints_edge(&mut self, x: i32, z: i32, w: i32, h: i32, mode: EdgeMode) -> Vec<i32> {
        match mode {
            EdgeMode::CoolWarm => self.ints_edge_cool_warm(x, z, w, h),
            EdgeMode::HeatIce => self.ints_edge_heat_ice(x, z, w, h),
            EdgeMode::Special => self.ints_edge_special(x, z, w, h),
        }
    }

    fn ints_edge_cool_warm(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let mut mid = p[(i + 1 + (j + 1) * pw) as usize];
                if mid == 1 {
                    let top = p[(i + 1 + j * pw) as usize];
                    let right = p[(i + 2 + (j + 1) * pw) as usize];
                    let left = p[(i + (j + 1) * pw) as usize];
                    let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                    let cool = top == 3 || right == 3 || left == 3 || bottom == 3;
                    let warm = top == 4 || right == 4 || left == 4 || bottom == 4;
                    if cool || warm {
                        mid = 2;
                    }
                }
                out[(i + j * w) as usize] = mid;
            }
        }
        out
    }

    fn ints_edge_heat_ice(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let mut mid = p[(i + 1 + (j + 1) * pw) as usize];
                if mid == 4 {
                    let top = p[(i + 1 + j * pw) as usize];
                    let right = p[(i + 2 + (j + 1) * pw) as usize];
                    let left = p[(i + (j + 1) * pw) as usize];
                    let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                    let ice = top == 2 || right == 2 || left == 2 || bottom == 2;
                    let heat = top == 1 || right == 1 || left == 1 || bottom == 1;
                    if heat || ice {
                        mid = 3;
                    }
                }
                out[(i + j * w) as usize] = mid;
            }
        }
        out
    }

    fn ints_edge_special(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let mut out = self.parent(x, z, w, h);
        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let mut v = out[idx];
                if v != 0 && self.next_int(13) == 0 {
                    v |= ((1 + self.next_int(15)) << 8) & 3840;
                }
                out[idx] = v;
            }
        }
        out
    }

    fn ints_add_mushroom_island(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                let c0 = p[(i + j * pw) as usize];
                let c1 = p[(i + 2 + j * pw) as usize];
                let c2 = p[(i + (j + 2) * pw) as usize];
                let c3 = p[(i + 2 + (j + 2) * pw) as usize];
                let mid = p[(i + 1 + (j + 1) * pw) as usize];
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                out[idx] = if mid == 0
                    && c0 == 0
                    && c1 == 0
                    && c2 == 0
                    && c3 == 0
                    && self.next_int(100) == 0
                {
                    MUSHROOM_ISLAND
                } else {
                    mid
                };
            }
        }
        out
    }

    fn ints_deep_ocean(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let top = p[(i + 1 + j * pw) as usize];
                let right = p[(i + 2 + (j + 1) * pw) as usize];
                let left = p[(i + (j + 1) * pw) as usize];
                let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                let mid = p[(i + 1 + (j + 1) * pw) as usize];
                let mut zeros = 0;
                if top == 0 {
                    zeros += 1;
                }
                if right == 0 {
                    zeros += 1;
                }
                if left == 0 {
                    zeros += 1;
                }
                if bottom == 0 {
                    zeros += 1;
                }
                out[(i + j * w) as usize] = if mid == 0 && zeros > 3 {
                    DEEP_OCEAN
                } else {
                    mid
                };
            }
        }
        out
    }

    fn ints_biome(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        // DEFAULT world type (not DEFAULT_1_1 / CUSTOMIZED).
        const COOL: [i32; 6] = [DESERT, DESERT, DESERT, 35, 35, PLAINS];
        const WARM: [i32; 6] = [FOREST, 29, EXTREME_HILLS, PLAINS, 27, SWAMPLAND];
        const MEDIUM: [i32; 4] = [FOREST, EXTREME_HILLS, TAIGA, PLAINS];
        const COLD: [i32; 4] = [ICE_PLAINS, ICE_PLAINS, ICE_PLAINS, 30];

        let p = self.parent(x, z, w, h);
        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let mut b = p[idx];
                let variant = (b & 3840) >> 8;
                b &= -3841;
                out[idx] = if is_biome_oceanic(b) {
                    b
                } else if b == MUSHROOM_ISLAND {
                    b
                } else if b == 1 {
                    if variant > 0 {
                        if self.next_int(3) == 0 {
                            MESA_PLATEAU
                        } else {
                            MESA_PLATEAU_F
                        }
                    } else {
                        COOL[self.next_int(6) as usize]
                    }
                } else if b == 2 {
                    if variant > 0 {
                        JUNGLE
                    } else {
                        WARM[self.next_int(6) as usize]
                    }
                } else if b == 3 {
                    if variant > 0 {
                        32
                    } else {
                        MEDIUM[self.next_int(4) as usize]
                    }
                } else if b == 4 {
                    COLD[self.next_int(4) as usize]
                } else {
                    MUSHROOM_ISLAND
                };
            }
        }
        out
    }

    fn ints_biome_edge(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let b = p[(i + 1 + (j + 1) * pw) as usize];
                let handled =
                    self.replace_biome_edge_if_necessary(
                        &p,
                        &mut out,
                        i,
                        j,
                        w,
                        b,
                        EXTREME_HILLS,
                        20,
                    ) | self.replace_biome_edge(&p, &mut out, i, j, w, b, MESA_PLATEAU_F, MESA)
                        | self.replace_biome_edge(&p, &mut out, i, j, w, b, MESA_PLATEAU, MESA)
                        | self.replace_biome_edge(&p, &mut out, i, j, w, b, 32, TAIGA);
                if !handled {
                    let idx = (i + j * w) as usize;
                    if b == DESERT {
                        let top = p[(i + 1 + j * pw) as usize];
                        let right = p[(i + 2 + (j + 1) * pw) as usize];
                        let left = p[(i + (j + 1) * pw) as usize];
                        let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                        out[idx] = if top != ICE_PLAINS
                            && right != ICE_PLAINS
                            && left != ICE_PLAINS
                            && bottom != ICE_PLAINS
                        {
                            b
                        } else {
                            34
                        };
                    } else if b == SWAMPLAND {
                        let n0 = p[(i + 1 + j * pw) as usize];
                        let n1 = p[(i + 2 + (j + 1) * pw) as usize];
                        let n2 = p[(i + (j + 1) * pw) as usize];
                        let n3 = p[(i + 1 + (j + 2) * pw) as usize];
                        if n0 == DESERT
                            || n1 == DESERT
                            || n2 == DESERT
                            || n3 == DESERT
                            || n0 == 30
                            || n1 == 30
                            || n2 == 30
                            || n3 == 30
                            || n0 == ICE_PLAINS
                            || n1 == ICE_PLAINS
                            || n2 == ICE_PLAINS
                            || n3 == ICE_PLAINS
                        {
                            out[idx] = PLAINS;
                        } else if n0 != JUNGLE && n3 != JUNGLE && n1 != JUNGLE && n2 != JUNGLE {
                            out[idx] = b;
                        } else {
                            out[idx] = JUNGLE_EDGE;
                        }
                    } else {
                        out[idx] = b;
                    }
                }
            }
        }
        out
    }

    fn replace_biome_edge_if_necessary(
        &mut self,
        p: &[i32],
        out: &mut [i32],
        i: i32,
        j: i32,
        w: i32,
        b: i32,
        target: i32,
        replacement: i32,
    ) -> bool {
        if !biomes_equal_or_mesa_plateau(b, target) {
            return false;
        }
        let pw = w + 2;
        let top = p[(i + 1 + j * pw) as usize];
        let right = p[(i + 2 + (j + 1) * pw) as usize];
        let left = p[(i + (j + 1) * pw) as usize];
        let bottom = p[(i + 1 + (j + 2) * pw) as usize];
        if can_biomes_be_neighbors(top, target)
            && can_biomes_be_neighbors(right, target)
            && can_biomes_be_neighbors(left, target)
            && can_biomes_be_neighbors(bottom, target)
        {
            out[(i + j * w) as usize] = b;
        } else {
            out[(i + j * w) as usize] = replacement;
        }
        true
    }

    fn replace_biome_edge(
        &mut self,
        p: &[i32],
        out: &mut [i32],
        i: i32,
        j: i32,
        w: i32,
        b: i32,
        target: i32,
        replacement: i32,
    ) -> bool {
        if b != target {
            return false;
        }
        let pw = w + 2;
        let top = p[(i + 1 + j * pw) as usize];
        let right = p[(i + 2 + (j + 1) * pw) as usize];
        let left = p[(i + (j + 1) * pw) as usize];
        let bottom = p[(i + 1 + (j + 2) * pw) as usize];
        if biomes_equal_or_mesa_plateau(top, target)
            && biomes_equal_or_mesa_plateau(right, target)
            && biomes_equal_or_mesa_plateau(left, target)
            && biomes_equal_or_mesa_plateau(bottom, target)
        {
            out[(i + j * w) as usize] = b;
        } else {
            out[(i + j * w) as usize] = replacement;
        }
        true
    }

    fn ints_hills(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let r = self.parent2(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let b = p[(i + 1 + (j + 1) * pw) as usize];
                let riv = r[(i + 1 + (j + 1) * pw) as usize];
                let is_m = (riv - 2) % 29 == 0;

                if b != 0 && riv >= 2 && (riv - 2) % 29 == 1 && b < 128 {
                    out[idx] = if biome_exists(b + 128) { b + 128 } else { b };
                    continue;
                }
                if self.next_int(3) != 0 && !is_m {
                    out[idx] = b;
                    continue;
                }

                let mut nb = b;
                if b == DESERT {
                    nb = 17;
                } else if b == FOREST {
                    nb = 18;
                } else if b == 27 {
                    nb = 28;
                } else if b == 29 {
                    nb = PLAINS;
                } else if b == TAIGA {
                    nb = 19;
                } else if b == 32 {
                    nb = 33;
                } else if b == 30 {
                    nb = 31;
                } else if b == PLAINS {
                    nb = if self.next_int(3) == 0 { 18 } else { FOREST };
                } else if b == ICE_PLAINS {
                    nb = 13;
                } else if b == JUNGLE {
                    nb = 22;
                } else if b == OCEAN {
                    nb = DEEP_OCEAN;
                } else if b == EXTREME_HILLS {
                    nb = 34;
                } else if b == 35 {
                    nb = 36;
                } else if biomes_equal_or_mesa_plateau(b, MESA_PLATEAU_F) {
                    nb = MESA;
                } else if b == DEEP_OCEAN && self.next_int(3) == 0 {
                    nb = if self.next_int(2) == 0 {
                        PLAINS
                    } else {
                        FOREST
                    };
                }

                if is_m && nb != b {
                    if biome_exists(nb + 128) {
                        nb += 128;
                    } else {
                        nb = b;
                    }
                }

                if nb == b {
                    out[idx] = b;
                } else {
                    let top = p[(i + 1 + j * pw) as usize];
                    let right = p[(i + 2 + (j + 1) * pw) as usize];
                    let left = p[(i + (j + 1) * pw) as usize];
                    let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                    let mut equal = 0;
                    if biomes_equal_or_mesa_plateau(top, b) {
                        equal += 1;
                    }
                    if biomes_equal_or_mesa_plateau(right, b) {
                        equal += 1;
                    }
                    if biomes_equal_or_mesa_plateau(left, b) {
                        equal += 1;
                    }
                    if biomes_equal_or_mesa_plateau(bottom, b) {
                        equal += 1;
                    }
                    out[idx] = if equal >= 3 { nb } else { b };
                }
            }
        }
        out
    }

    fn ints_rare_biome(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let b = p[(i + 1 + (j + 1) * pw) as usize];
                out[idx] = if self.next_int(57) == 0 && b == PLAINS {
                    PLAINS + 128
                } else {
                    b
                };
            }
        }
        out
    }

    fn ints_river_init(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let p = self.parent(x, z, w, h);
        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                out[idx] = if p[idx] > 0 {
                    self.next_int(299999) + 2
                } else {
                    0
                };
            }
        }
        out
    }

    fn ints_river(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            for i in 0..w {
                let l = river_value(p[(i + (j + 1) * pw) as usize]);
                let r = river_value(p[(i + 2 + (j + 1) * pw) as usize]);
                let t = river_value(p[(i + 1 + j * pw) as usize]);
                let b = river_value(p[(i + 1 + (j + 2) * pw) as usize]);
                let m = river_value(p[(i + 1 + (j + 1) * pw) as usize]);
                out[(i + j * w) as usize] = if m == l && m == t && m == r && m == b {
                    -1
                } else {
                    RIVER
                };
            }
        }
        out
    }

    fn ints_smooth(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let l = p[(i + (j + 1) * pw) as usize];
                let r = p[(i + 2 + (j + 1) * pw) as usize];
                let t = p[(i + 1 + j * pw) as usize];
                let b = p[(i + 1 + (j + 2) * pw) as usize];
                let mut m = p[(i + 1 + (j + 1) * pw) as usize];
                if l == r && t == b {
                    self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                    m = if self.next_int(2) == 0 { l } else { t };
                } else {
                    if l == r {
                        m = l;
                    }
                    if t == b {
                        m = t;
                    }
                }
                out[(i + j * w) as usize] = m;
            }
        }
        out
    }

    fn ints_shore(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let pw = w + 2;
        let p = self.parent(x - 1, z - 1, pw, h + 2);
        let mut out = vec![0i32; (w * h) as usize];

        for j in 0..h {
            for i in 0..w {
                let idx = (i + j * w) as usize;
                self.init_chunk_seed((i + x) as i64, (j + z) as i64);
                let b = p[(i + 1 + (j + 1) * pw) as usize];
                let top = p[(i + 1 + j * pw) as usize];
                let right = p[(i + 2 + (j + 1) * pw) as usize];
                let left = p[(i + (j + 1) * pw) as usize];
                let bottom = p[(i + 1 + (j + 2) * pw) as usize];
                let jungle = biome_info(b)
                    .map(|x| x.class == BiomeClass::Jungle)
                    .unwrap_or(false);
                let snowy = biome_info(b).map(|x| x.snowy).unwrap_or(false);

                if b == MUSHROOM_ISLAND {
                    out[idx] = if top != OCEAN && right != OCEAN && left != OCEAN && bottom != OCEAN
                    {
                        b
                    } else {
                        MUSHROOM_ISLAND_SHORE
                    };
                } else if jungle {
                    if !shore_jungle(top)
                        || !shore_jungle(right)
                        || !shore_jungle(left)
                        || !shore_jungle(bottom)
                    {
                        out[idx] = JUNGLE_EDGE;
                    } else if !is_biome_oceanic(top)
                        && !is_biome_oceanic(right)
                        && !is_biome_oceanic(left)
                        && !is_biome_oceanic(bottom)
                    {
                        out[idx] = b;
                    } else {
                        out[idx] = BEACH;
                    }
                } else if b == EXTREME_HILLS || b == 34 || b == 20 {
                    self.shore_beach(&p, &mut out, i, j, w, b, STONE_BEACH);
                } else if snowy {
                    self.shore_beach(&p, &mut out, i, j, w, b, COLD_BEACH);
                } else if b == MESA || b == MESA_PLATEAU_F {
                    if is_biome_oceanic(top)
                        || is_biome_oceanic(right)
                        || is_biome_oceanic(left)
                        || is_biome_oceanic(bottom)
                    {
                        out[idx] = b;
                    } else if shore_mesa(top)
                        && shore_mesa(right)
                        && shore_mesa(left)
                        && shore_mesa(bottom)
                    {
                        out[idx] = b;
                    } else {
                        out[idx] = DESERT;
                    }
                } else if b != OCEAN && b != DEEP_OCEAN && b != RIVER && b != SWAMPLAND {
                    if !is_biome_oceanic(top)
                        && !is_biome_oceanic(right)
                        && !is_biome_oceanic(left)
                        && !is_biome_oceanic(bottom)
                    {
                        out[idx] = b;
                    } else {
                        out[idx] = BEACH;
                    }
                } else {
                    out[idx] = b;
                }
            }
        }
        out
    }

    fn shore_beach(
        &mut self,
        p: &[i32],
        out: &mut [i32],
        i: i32,
        j: i32,
        w: i32,
        b: i32,
        replacement: i32,
    ) {
        if is_biome_oceanic(b) {
            out[(i + j * w) as usize] = b;
            return;
        }
        let pw = w + 2;
        let top = p[(i + 1 + j * pw) as usize];
        let right = p[(i + 2 + (j + 1) * pw) as usize];
        let left = p[(i + (j + 1) * pw) as usize];
        let bottom = p[(i + 1 + (j + 2) * pw) as usize];
        out[(i + j * w) as usize] = if !is_biome_oceanic(top)
            && !is_biome_oceanic(right)
            && !is_biome_oceanic(left)
            && !is_biome_oceanic(bottom)
        {
            b
        } else {
            replacement
        };
    }

    fn ints_river_mix(&mut self, x: i32, z: i32, w: i32, h: i32) -> Vec<i32> {
        let p = self.parent(x, z, w, h);
        let r = self.parent2(x, z, w, h);
        let mut out = vec![0i32; (w * h) as usize];
        for k in 0..(w * h) as usize {
            let b = p[k];
            out[k] = if b == OCEAN || b == DEEP_OCEAN {
                b
            } else if r[k] == RIVER {
                if b == ICE_PLAINS {
                    11 // frozenRiver
                } else if b != MUSHROOM_ISLAND && b != MUSHROOM_ISLAND_SHORE {
                    r[k] & 0xFF
                } else {
                    MUSHROOM_ISLAND_SHORE
                }
            } else {
                b
            };
        }
        out
    }

    fn ints_voronoi_zoom(&mut self, x_in: i32, z_in: i32, w: i32, h: i32) -> Vec<i32> {
        let x = x_in - 2;
        let z = z_in - 2;
        let px = x >> 2;
        let pz = z >> 2;
        let pw = (w >> 2) + 2;
        let ph = (h >> 2) + 2;
        let p = self.parent(px, pz, pw, ph);
        let w3 = (pw - 1) << 2;
        let h3 = (ph - 1) << 2;
        let mut mid = vec![0i32; (w3 * h3) as usize];

        for row in 0..ph - 1 {
            let mut left_col = 0i32;
            let mut top_left = p[((row) * pw) as usize];
            let mut bot_left = p[((row + 1) * pw) as usize];

            while left_col < pw - 1 {
                self.init_chunk_seed(((left_col + px) << 2) as i64, ((row + pz) << 2) as i64);
                let da_x = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6;
                let da_y = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6;
                self.init_chunk_seed(((left_col + px + 1) << 2) as i64, ((row + pz) << 2) as i64);
                let db_x = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6 + 4.0;
                let db_y = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6;
                self.init_chunk_seed(((left_col + px) << 2) as i64, ((row + pz + 1) << 2) as i64);
                let dc_x = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6;
                let dc_y = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6 + 4.0;
                self.init_chunk_seed(
                    ((left_col + px + 1) << 2) as i64,
                    ((row + pz + 1) << 2) as i64,
                );
                let dd_x = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6 + 4.0;
                let dd_y = (self.next_int(1024) as f64 / 1024.0 - 0.5) * 3.6 + 4.0;

                let top_right = p[(left_col + 1 + row * pw) as usize] & 0xFF;
                let bot_right = p[(left_col + 1 + (row + 1) * pw) as usize] & 0xFF;

                for fy in 0..4 {
                    let base = (((row << 2) + fy) * w3 + (left_col << 2)) as usize;
                    for fx in 0..4 {
                        let d0 = (fy as f64 - da_y) * (fy as f64 - da_y)
                            + (fx as f64 - da_x) * (fx as f64 - da_x);
                        let d1 = (fy as f64 - db_y) * (fy as f64 - db_y)
                            + (fx as f64 - db_x) * (fx as f64 - db_x);
                        let d2 = (fy as f64 - dc_y) * (fy as f64 - dc_y)
                            + (fx as f64 - dc_x) * (fx as f64 - dc_x);
                        let d3 = (fy as f64 - dd_y) * (fy as f64 - dd_y)
                            + (fx as f64 - dd_x) * (fx as f64 - dd_x);
                        mid[base + fx as usize] = if d0 < d1 && d0 < d2 && d0 < d3 {
                            top_left
                        } else if d1 < d0 && d1 < d2 && d1 < d3 {
                            top_right
                        } else if d2 < d0 && d2 < d1 && d2 < d3 {
                            bot_left
                        } else {
                            bot_right
                        };
                    }
                }

                top_left = top_right;
                bot_left = bot_right;
                left_col += 1;
            }
        }

        let mut out = vec![0i32; (w * h) as usize];
        for j in 0..h {
            let src = ((j + (z & 3)) * w3 + (x & 3)) as usize;
            for i in 0..w {
                out[(i + j * w) as usize] = mid[src + i as usize];
            }
        }
        out
    }
}

/// `GenLayerRiver.func_151630_c`.
fn river_value(v: i32) -> i32 {
    if v >= 2 {
        2 + (v & 1)
    } else {
        v
    }
}

/// `GenLayerBiomeEdge.canBiomesBeNeighbors`.
fn can_biomes_be_neighbors(a: i32, b: i32) -> bool {
    if biomes_equal_or_mesa_plateau(a, b) {
        return true;
    }
    match (biome_info(a), biome_info(b)) {
        (Some(_), Some(_)) => {
            let ca = temp_category(a);
            let cb = temp_category(b);
            ca == cb || ca == TempCategory::Medium || cb == TempCategory::Medium
        }
        _ => false,
    }
}

/// `GenLayerShore.func_151631_c`.
fn shore_jungle(v: i32) -> bool {
    if biome_info(v)
        .map(|x| x.class == BiomeClass::Jungle)
        .unwrap_or(false)
    {
        return true;
    }
    v == JUNGLE_EDGE || v == JUNGLE || v == 22 || v == FOREST || v == TAIGA || is_biome_oceanic(v)
}

/// `GenLayerShore.func_151633_d`.
fn shore_mesa(v: i32) -> bool {
    biome_info(v)
        .map(|x| x.class == BiomeClass::Mesa)
        .unwrap_or(false)
}

fn magnify(base: i64, mut layer: Layer, count: i32) -> Layer {
    for i in 0..count {
        layer = Layer::unary(Kind::Zoom, base + i as i64, layer);
    }
    layer
}

/// The three layers returned by `GenLayer.initializeAllBiomeGenerators` (default overworld).
pub struct BiomeLayers {
    /// `var5[0]` — `genBiomes`.
    pub biome: Layer,
    /// `var5[1]` — `biomeIndexLayer` (Voronoi).
    pub voronoi: Layer,
}

impl BiomeLayers {
    /// Port of `GenLayer.initializeAllBiomeGenerators(seed, WorldType.DEFAULT, "")`.
    pub fn new(seed: i64) -> BiomeLayers {
        let mut v = Layer::leaf(Kind::Island, 1);
        v = Layer::unary(Kind::FuzzyZoom, 2000, v);
        v = Layer::unary(Kind::AddIsland, 1, v);
        v = Layer::unary(Kind::Zoom, 2001, v);
        v = Layer::unary(Kind::AddIsland, 2, v);
        v = Layer::unary(Kind::AddIsland, 50, v);
        v = Layer::unary(Kind::AddIsland, 70, v);
        v = Layer::unary(Kind::RemoveTooMuchOcean, 2, v);
        v = Layer::unary(Kind::AddSnow, 2, v);
        v = Layer::unary(Kind::AddIsland, 3, v);
        v = Layer::unary(Kind::Edge(EdgeMode::CoolWarm), 2, v);
        v = Layer::unary(Kind::Edge(EdgeMode::HeatIce), 2, v);
        v = Layer::unary(Kind::Edge(EdgeMode::Special), 3, v);
        v = Layer::unary(Kind::Zoom, 2002, v);
        v = Layer::unary(Kind::Zoom, 2003, v);
        v = Layer::unary(Kind::AddIsland, 4, v);
        v = Layer::unary(Kind::AddMushroomIsland, 5, v);
        v = Layer::unary(Kind::DeepOcean, 4, v);
        v = magnify(1000, v, 0);

        let biome_size = 4;
        let river_size = 4;

        // var4 == v (the shared base chain). Copies are made where vanilla shares nodes.
        let mut river = Layer::unary(Kind::RiverInit, 100, v.clone());

        // var10 = magnify(1000, var8_RiverInit, 2). In vanilla its two Zoom nodes are never
        // seeded (`GenLayerHills` does not recurse `field_151628_d`), but the RiverInit beneath
        // them is shared with the river chain and *is* seeded. We mirror that: pre-seed the
        // inner chain, then wrap it in two (unseeded) Zooms.
        let mut hills_river_inner = river.clone();
        hills_river_inner.init_world_gen_seed(seed);
        let hills_river = magnify(1000, hills_river_inner, 2);

        let mut biome = Layer::unary(Kind::Biome, 200, v.clone());
        biome = magnify(1000, biome, 2);
        biome = Layer::unary(Kind::BiomeEdge, 1000, biome);
        biome = Layer::new(
            Kind::Hills,
            1000,
            Some(Box::new(biome)),
            Some(Box::new(hills_river)),
        );

        river = magnify(1000, river, 2);
        river = magnify(1000, river, river_size);
        river = Layer::unary(Kind::River, 1, river);
        river = Layer::unary(Kind::Smooth, 1000, river);

        biome = Layer::unary(Kind::RareBiome, 1001, biome);

        for i in 0..biome_size {
            biome = Layer::unary(Kind::Zoom, 1000 + i as i64, biome);
            if i == 0 {
                biome = Layer::unary(Kind::AddIsland, 3, biome);
            }
            if i == 1 || biome_size == 1 {
                biome = Layer::unary(Kind::Shore, 1000, biome);
            }
        }
        biome = Layer::unary(Kind::Smooth, 1000, biome);
        biome = Layer::new(
            Kind::RiverMix,
            100,
            Some(Box::new(biome)),
            Some(Box::new(river)),
        );

        let mut voronoi = Layer::unary(Kind::VoronoiZoom, 10, biome.clone());

        biome.init_world_gen_seed(seed);
        voronoi.init_world_gen_seed(seed);

        BiomeLayers { biome, voronoi }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_info_matches_class_and_mutation() {
        assert_eq!(biome_info(0).unwrap().class, BiomeClass::Ocean);
        assert_eq!(biome_info(10).unwrap().class, BiomeClass::Ocean);
        // Mutation 132 (forest M) delegates to forest.
        assert_eq!(biome_info(132).unwrap().class, BiomeClass::Forest);
        assert!(biome_info(40).is_none());
        assert!(biome_info(128).is_none());
        assert!(biome_exists(39));
        assert!(!biome_exists(50));
    }

    #[test]
    fn mesa_plateau_equality() {
        assert!(biomes_equal_or_mesa_plateau(MESA_PLATEAU_F, MESA_PLATEAU));
        assert!(biomes_equal_or_mesa_plateau(MESA, MESA_PLATEAU_F));
        assert!(!biomes_equal_or_mesa_plateau(MESA, DESERT));
    }

    /// First 8 Voronoi (`biomeIndexLayer`) ids for seed 0 at chunk (-37, 210), from the oracle.
    #[test]
    fn voronoi_golden_seed0() {
        let mut layers = BiomeLayers::new(0);
        let ids = layers.voronoi.get_ints(-37, 210, 16, 16);
        assert_eq!(&ids[..8], &[0, 0, 7, 7, 7, 7, 16, 16]);
    }

    /// The biome (`genBiomes`) layer for the same area begins as ocean.
    #[test]
    fn biome_golden_seed0() {
        let mut layers = BiomeLayers::new(0);
        let ids = layers.biome.get_ints(-37, 210, 16, 16);
        assert_eq!(&ids[..8], &[0, 0, 0, 0, 0, 0, 0, 0]);
    }
}
