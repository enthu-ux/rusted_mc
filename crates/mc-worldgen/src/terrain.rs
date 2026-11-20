//! Port of the 1.8.9 overworld base-terrain *shape*: the density noise that fills a chunk with
//! stone / water / air before the surface blocks are applied.
//!
//! Covers `ChunkProviderGenerate.setBlocksInChunk` + `func_147423_a` (the terrain density field)
//! and the `GenLayer` biome fetch it depends on. The per-biome surface replacement
//! (`ChunkProviderGenerate.replaceBlocksForBiome` / `BiomeGenBase.generateBiomeTerrain`, incl.
//! Mesa's custom generator) is the next increment.
//!
//! Default `WorldType.DEFAULT` settings (`ChunkProviderSettings.Factory` defaults; cave/ravine
//! flags do not affect the shape). Block states are stamped as `id << 4 | meta` (meta 0 here).

use crate::layer::BiomeLayers;
use crate::noise::{Octaves, Perlin};
use mc_core::random::JavaRandom;

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

// Block ids (meta 0 here).
const STONE: u16 = 1;
const WATER: u16 = 9; // `Blocks.water` (useLavaOceans = false)

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

/// One overworld `ChunkProviderGenerate` density generator (shape only).
pub struct TerrainGen {
    /// `field_147431_j` (lower limit), 16 octaves.
    lower_gen: Octaves,
    /// `field_147432_k` (upper limit), 16 octaves.
    upper_gen: Octaves,
    /// `field_147429_l` (main), 8 octaves.
    main_gen: Octaves,
    noise_gen6: Octaves,
    /// Kept for the surface increment (`field_147430_m` / `noiseGen5` / `mobSpawnerNoise`).
    #[allow(dead_code)]
    field_147430_m: Perlin,
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
    layers: BiomeLayers,
}

impl TerrainGen {
    /// Builds the noise stack in vanilla order (`ChunkProviderGenerate`'s constructor), consuming
    /// the seed's `Random` draws exactly as the game does.
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

        TerrainGen {
            lower_gen,
            upper_gen,
            main_gen,
            noise_gen6,
            field_147430_m,
            noise_gen5,
            mob_spawner_noise,
            parabolic,
            biomes: [0; 100],
            q: vec![0.0; 825],
            main: vec![0.0; 825],
            lower: vec![0.0; 825],
            upper: vec![0.0; 825],
            depth: vec![0.0; 25],
            layers: BiomeLayers::new(seed),
        }
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
                let mut depth_sum = 0.0f32; // var11 (sum of depth * weight)
                let mut scale_sum = 0.0f32; // var10 (sum of scale * weight)
                let mut weight_sum = 0.0f32; // var12
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

    /// `ChunkProviderGenerate.setBlocksInChunk`, returning the primer as `id << 4 | meta` indexed by
    /// `x << 12 | z << 8 | y`.
    pub fn set_blocks_in_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<u16> {
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
                                let x = a * 4 + e;
                                let y = c * 8 + d;
                                let z = b * 4 + f;
                                let idx = ((x << 12) | (z << 8) | y) as usize;
                                if v45 > 0.0 {
                                    primer[idx] = STONE << 4;
                                } else if (y as i32) < SEA_LEVEL {
                                    primer[idx] = WATER << 4;
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
        assert_eq!(at(0), STONE << 4);
        assert_eq!(at(63), STONE << 4);
        assert_eq!(at(100), 0); // air
    }
}
