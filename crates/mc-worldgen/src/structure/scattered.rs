//! `MapGenScatteredFeature` (desert / jungle temples and swamp huts): the registration half.
//!
//! Unlike the mineshaft, a scattered feature is a *single* `ComponentScatteredFeaturePieces.Feature`
//! piece (its `addComponentParts` builds the whole temple); there is no `buildComponent` recursion.
//! `getStructureStart` picks the piece by the biome at the chunk centre, and each piece's constructor
//! draws one `nextInt(4)` for its orientation.
//!
//! Two details matter for parity:
//!
//! * `canSpawnStructureAtCoords` re-seeds the *world's* `Random` with
//!   `World.setRandomSeed(gx, gz, 14357617)`, a pure function of `(worldSeed, gx, gz)`, then rolls two
//!   `nextInt(24)` offsets - so the placement is a deterministic grid function, independent of the
//!   `MapGenBase` per-chunk stream (except `recursiveGenerate`'s discard `nextInt`).
//! * The biome comes from `getBiomeGenerator` / `getBiomeGenForCoords`, i.e. the biome-index
//!   (`Voronoi`) layer at the block coordinate `(chunk*16 + 8)`. This module is handed a biome lookup
//!   closure so it does not depend on the layer internals.
//!
//! Block placement lives in `structure/placement.rs`: the swamp hut is ported; the desert and jungle
//! pyramids are still layout-only.

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{BoundingBox, Facing, Piece, PieceKind, StructureStart};
use crate::mapgen;

/// `MapGenScatteredFeature.maxDistanceBetweenScatteredFeatures`.
const MAX_DISTANCE: i32 = 32;
/// `MapGenScatteredFeature.minDistanceBetweenScatteredFeatures`.
const MIN_DISTANCE: i32 = 8;
/// The salt `MapGenScatteredFeature.canSpawnStructureAtCoords` passes to `World.setRandomSeed`.
const SALT: i64 = 14357617;

// `BiomeGenBase` ids in `MapGenScatteredFeature.biomelist` (desert/desertHills/jungle/jungleHills/
// swampland) and the `Start` constructor's jungle/swampland/desert classification.
const BIOME_DESERT: i32 = 2;
const BIOME_DESERT_HILLS: i32 = 17;
const BIOME_JUNGLE: i32 = 21;
const BIOME_JUNGLE_HILLS: i32 = 22;
const BIOME_SWAMPLAND: i32 = 6;

fn is_temple_biome(id: i32) -> bool {
    matches!(
        id,
        BIOME_DESERT | BIOME_DESERT_HILLS | BIOME_JUNGLE | BIOME_JUNGLE_HILLS | BIOME_SWAMPLAND
    )
}

/// `MapGenScatteredFeature`: a `MapGenStructure` (registry + `MapGenBase` RNG) for temples.
pub struct ScatteredFeature {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
}

impl Default for ScatteredFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl ScatteredFeature {
    pub fn new() -> ScatteredFeature {
        ScatteredFeature {
            starts: BTreeMap::new(),
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenStructure.recursiveGenerate` over every chunk in `MapGenBase`'s neighbourhood: register
    /// one start per chunk that passes `canSpawnStructureAtCoords`. `biome_at(bx, bz)` resolves the
    /// biome-index biome at a block coordinate (the port of `WorldChunkManager.getBiomeGenerator`).
    pub fn generate(
        &mut self,
        world_seed: i64,
        target_x: i32,
        target_z: i32,
        biome_at: &mut dyn FnMut(i32, i32) -> i32,
    ) {
        let starts = &mut self.starts;
        mapgen::generate(
            world_seed,
            &mut self.rand,
            target_x,
            target_z,
            |rand, nx, nz| {
                if starts.contains_key(&(nx, nz)) {
                    return;
                }
                rand.next_int(); // `MapGenStructure.recursiveGenerate`'s discard draw
                if can_spawn_at_coords(world_seed, nx, nz, biome_at) {
                    starts.insert((nx, nz), start(rand, nx, nz, biome_at));
                }
            },
        );
    }

    /// Every registered start whose bounding box meets this chunk.
    pub fn starts_intersecting(&self, chunk_x: i32, chunk_z: i32) -> Vec<StructureStart> {
        self.starts
            .values()
            .filter(|s| s.intersects_chunk(chunk_x, chunk_z))
            .cloned()
            .collect()
    }

    /// Mutable access to every registered start whose bounding box meets the 16x16 area
    /// `[x0..=x1] x [z0..=z1]` (the `MapGenStructure.generateStructure` walk). Mutation matters:
    /// `func_74935_a` offsets a jungle pyramid / swamp hut's box on its first placement pass.
    pub fn for_each_start_meeting_mut(
        &mut self,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        mut f: impl FnMut(&mut StructureStart),
    ) {
        for start in self.starts.values_mut() {
            if start.sizeable && start.bbox.intersects_with_xz(x0, z0, x1, z1) {
                f(start);
            }
        }
    }

    /// `MapGenStructure.structureMap` size (for tests/diagnostics).
    pub fn start_count(&self) -> usize {
        self.starts.len()
    }
}

/// `World.setRandomSeed(x, z, salt)`: the world-seed salt used by the scattered-feature grid.
fn random_seed(world_seed: i64, x: i32, z: i32, salt: i64) -> i64 {
    (x as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((z as i64).wrapping_mul(132897987541))
        .wrapping_add(world_seed)
        .wrapping_add(salt)
}

/// `MapGenScatteredFeature.canSpawnStructureAtCoords`.
fn can_spawn_at_coords(
    world_seed: i64,
    x0: i32,
    z0: i32,
    biome_at: &mut dyn FnMut(i32, i32) -> i32,
) -> bool {
    let mut x = x0;
    let mut z = z0;
    if x < 0 {
        x -= MAX_DISTANCE - 1;
    }
    if z < 0 {
        z -= MAX_DISTANCE - 1;
    }

    let gx = x / MAX_DISTANCE;
    let gz = z / MAX_DISTANCE;
    let mut r = JavaRandom::new(random_seed(world_seed, gx, gz, SALT));
    let mut cx = gx * MAX_DISTANCE;
    let mut cz = gz * MAX_DISTANCE;
    cx += r.next_int_bound(MAX_DISTANCE - MIN_DISTANCE);
    cz += r.next_int_bound(MAX_DISTANCE - MIN_DISTANCE);

    // The original chunk coordinates (the negative adjustment above is only for the grid lookup).
    x0 == cx && z0 == cz && is_temple_biome(biome_at(x0 * 16 + 8, z0 * 16 + 8))
}

/// `MapGenScatteredFeature.Start`: the one piece, chosen by the centre biome.
fn start(
    rand: &mut JavaRandom,
    chunk_x: i32,
    chunk_z: i32,
    biome_at: &mut dyn FnMut(i32, i32) -> i32,
) -> StructureStart {
    let biome = biome_at(chunk_x * 16 + 8, chunk_z * 16 + 8);
    let mut pieces = Vec::new();
    if biome == BIOME_JUNGLE || biome == BIOME_JUNGLE_HILLS {
        pieces.push(jungle_pyramid(rand, chunk_x * 16, chunk_z * 16));
    } else if biome == BIOME_SWAMPLAND {
        pieces.push(swamp_hut(rand, chunk_x * 16, chunk_z * 16));
    } else if biome == BIOME_DESERT || biome == BIOME_DESERT_HILLS {
        pieces.push(desert_pyramid(rand, chunk_x * 16, chunk_z * 16));
    }

    StructureStart {
        id: "Temple",
        chunk_x,
        chunk_z,
        bbox: StructureStart::update_bbox(&pieces),
        pieces,
        sizeable: true,
    }
}

/// `ComponentScatteredFeaturePieces.Feature`'s constructor: one orientation draw and a box whose X/Z
/// extents swap for an east/west orientation. `y` is always 64 at construction (`HPos` is only set
/// later, during placement).
fn feature_bbox(
    rand: &mut JavaRandom,
    x: i32,
    z: i32,
    size_x: i32,
    size_y: i32,
    size_z: i32,
) -> (Facing, BoundingBox) {
    // `EnumFacing.Plane.HORIZONTAL.facings()` is `{NORTH, EAST, SOUTH, WEST}`.
    let facing = match rand.next_int_bound(4) {
        0 => Facing::North,
        1 => Facing::East,
        2 => Facing::South,
        _ => Facing::West,
    };
    let (dx, dz) = if matches!(facing, Facing::North | Facing::South) {
        (size_x, size_z)
    } else {
        (size_z, size_x)
    };
    let bbox = BoundingBox::new(x, 64, z, x + dx - 1, 64 + size_y - 1, z + dz - 1);
    (facing, bbox)
}

/// `ComponentScatteredFeaturePieces.DesertPyramid(Random, x, z)`.
fn desert_pyramid(rand: &mut JavaRandom, x: i32, z: i32) -> Piece {
    let size = [21, 15, 21];
    let (facing, bbox) = feature_bbox(rand, x, z, size[0], size[1], size[2]);
    Piece {
        kind: PieceKind::TempleDesertPyramid {
            size,
            h_pos: -1,
            chests: [false; 4],
        },
        coord_base_mode: Some(facing),
        component_type: 0,
        bbox,
    }
}

/// `ComponentScatteredFeaturePieces.JunglePyramid(Random, x, z)`.
fn jungle_pyramid(rand: &mut JavaRandom, x: i32, z: i32) -> Piece {
    let size = [12, 10, 15];
    let (facing, bbox) = feature_bbox(rand, x, z, size[0], size[1], size[2]);
    Piece {
        kind: PieceKind::TempleJunglePyramid {
            size,
            h_pos: -1,
            main_chest: false,
            hidden_chest: false,
            trap1: false,
            trap2: false,
        },
        coord_base_mode: Some(facing),
        component_type: 0,
        bbox,
    }
}

/// `ComponentScatteredFeaturePieces.SwampHut(Random, x, z)`.
fn swamp_hut(rand: &mut JavaRandom, x: i32, z: i32) -> Piece {
    let size = [7, 7, 9];
    let (facing, bbox) = feature_bbox(rand, x, z, size[0], size[1], size[2]);
    Piece {
        kind: PieceKind::TempleSwampHut {
            size,
            h_pos: -1,
            witch: false,
        },
        coord_base_mode: Some(facing),
        component_type: 0,
        bbox,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    fn bounds(p: &BoundingBox) -> (i32, i32, i32, i32, i32, i32) {
        (p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z)
    }

    /// The scattered grid is the only thing registering temples: `GenOptions::none()` registers none.
    #[test]
    fn scattered_feature_off_registers_nothing() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(2, 46, GenOptions::none());
        assert!(chunk.structures.is_empty());
    }

    /// Seed 0's swamp hut - start (3,47), orientation EAST, size 7x7x9 (oracle: `TeSH`, O=3,
    /// BB 48 64 752 56 70 758). Reported by the neighbouring chunk (2,46), not by (3,47).
    #[test]
    fn scattered_feature_swamp_hut_seed0() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(2, 46, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Temple")
            .expect("seed 0 (2,46) reports its swamp hut");
        assert_eq!((start.chunk_x, start.chunk_z), (3, 47));
        assert_eq!(bounds(&start.bbox), (48, 64, 752, 56, 70, 758));
        assert_eq!(start.pieces.len(), 1);

        let piece = &start.pieces[0];
        assert_eq!(piece.kind.id(), "TeSH");
        assert_eq!(piece.component_type, 0);
        assert_eq!(piece.coord_base_mode, Some(Facing::East));
        assert_eq!(bounds(&piece.bbox), (48, 64, 752, 56, 70, 758));
        assert!(matches!(
            piece.kind,
            PieceKind::TempleSwampHut {
                size: [7, 7, 9],
                h_pos: -1,
                witch: false,
            }
        ));

        // The origin chunk's own 16x16 area sits south of the box, so it does not report it.
        let origin = gen.generate_chunk(3, 47, GenOptions::vanilla());
        assert!(origin.structures.iter().all(|s| s.id != "Temple"));
    }

    /// Seed -7's desert pyramid - start (4,2), orientation EAST, size 21x15x21 (oracle: `TeDP`,
    /// O=3, BB 64 64 32 84 78 52).
    #[test]
    fn scattered_feature_desert_pyramid_seed_neg7() {
        let mut gen = TerrainGen::new(-7);
        let chunk = gen.generate_chunk(4, 2, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Temple")
            .expect("seed -7 (4,2) reports its desert pyramid");
        assert_eq!((start.chunk_x, start.chunk_z), (4, 2));
        assert_eq!(bounds(&start.bbox), (64, 64, 32, 84, 78, 52));

        let piece = &start.pieces[0];
        assert_eq!(piece.kind.id(), "TeDP");
        assert_eq!(piece.coord_base_mode, Some(Facing::East));
        assert!(matches!(
            piece.kind,
            PieceKind::TempleDesertPyramid {
                size: [21, 15, 21],
                h_pos: -1,
                ..
            }
        ));
    }

    /// Seed 12345's jungle pyramid - start (72,-155), orientation SOUTH, size 12x10x15 (oracle:
    /// `TeJP`, O=0, BB 1152 64 -2480 1163 73 -2466).
    #[test]
    fn scattered_feature_jungle_pyramid_seed12345() {
        let mut gen = TerrainGen::new(12345);
        let chunk = gen.generate_chunk(71, -156, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Temple")
            .expect("seed 12345 (71,-156) reports its jungle pyramid");
        assert_eq!((start.chunk_x, start.chunk_z), (72, -155));
        assert_eq!(bounds(&start.bbox), (1152, 64, -2480, 1163, 73, -2466));

        let piece = &start.pieces[0];
        assert_eq!(piece.kind.id(), "TeJP");
        assert_eq!(piece.coord_base_mode, Some(Facing::South));
        assert!(matches!(
            piece.kind,
            PieceKind::TempleJunglePyramid {
                size: [12, 10, 15],
                h_pos: -1,
                ..
            }
        ));
    }

    /// `canSpawnStructureAtCoords` is a pure function of `(worldSeed, chunk)`: a fresh generator
    /// registers the same start regardless of what was generated before.
    #[test]
    fn scattered_feature_registration_is_order_independent() {
        let mut a = TerrainGen::new(0);
        let mut b = TerrainGen::new(0);
        // Different generation order must not change the registered start.
        let after = b.generate_chunk(100, -50, GenOptions::vanilla());
        let _ = after;
        let a_start = a
            .generate_chunk(2, 46, GenOptions::vanilla())
            .structures
            .into_iter()
            .find(|s| s.id == "Temple")
            .expect("temple");
        let b_start = b
            .generate_chunk(2, 46, GenOptions::vanilla())
            .structures
            .into_iter()
            .find(|s| s.id == "Temple")
            .expect("temple");
        assert_eq!(a_start, b_start);
    }
}
