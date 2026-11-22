//! `StructureOceanMonument` (ocean monuments): the registration half.
//!
//! The monument is grid-placed like the scattered feature: `canSpawnStructureAtCoords` re-seeds the
//! world `Random` with `World.setRandomSeed(gx, gz, 10387313)` and averages two `nextInt(27)` offsets,
//! then requires a deep-ocean biome (`WorldChunkManager.getBiomeGenerator`) with only
//! ocean/frozen-ocean/river biomes within 29 blocks (`areBiomesViable`).
//!
//! The *serialized* layout is a single piece: `StructureOceanMonument.StartMonument.func_175789_b`
//! re-seeds the `MapGenBase` stream with the world seed, draws the orientation
//! (`EnumFacing.Plane.HORIZONTAL`), and adds one `MonumentBuilding` whose box is fixed at
//! `(x, 39, z) .. (x + 57, 61, z + 57)`. `MonumentBuilding` keeps its room graph in a *private* list
//! (walked by its `addComponentParts`, not by `StructureStart.generateStructure`), so the room graph
//! is not part of this layout artefact and is deferred to the placement port (see
//! `structure/placement.rs`).
//!
//! Block placement is not ported yet (layout only).

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{BoundingBox, Facing, Piece, PieceKind, StructureStart};
use crate::mapgen;
use crate::terrain::BiomeLookup;

/// `StructureOceanMonument.field_175800_f` (default spacing).
const SPACING: i32 = 32;
/// `StructureOceanMonument.field_175801_g` (default separation).
const SEPARATION: i32 = 5;
/// The salt `canSpawnStructureAtCoords` passes to `World.setRandomSeed`.
const SALT: i64 = 10387313;
/// `StructureOceanMonument.field_175802_d` ids: ocean, deepOcean, river, frozenOcean, frozenRiver.
const MONUMENT_BIOMES: [i32; 5] = [0, 24, 7, 10, 11];
const DEEP_OCEAN: i32 = 24;

/// `StructureOceanMonument`: a `MapGenStructure` for ocean monuments.
pub struct OceanMonument {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
}

impl Default for OceanMonument {
    fn default() -> Self {
        Self::new()
    }
}

impl OceanMonument {
    pub fn new() -> OceanMonument {
        OceanMonument {
            starts: BTreeMap::new(),
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenStructure.recursiveGenerate` over `MapGenBase`'s neighbourhood.
    pub fn generate(
        &mut self,
        world_seed: i64,
        target_x: i32,
        target_z: i32,
        lookup: &mut BiomeLookup,
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
                if can_spawn_at_coords(world_seed, nx, nz, lookup) {
                    starts.insert((nx, nz), start(rand, nx, nz, world_seed));
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

    /// `MapGenStructure.structureMap` size (for tests/diagnostics).
    pub fn start_count(&self) -> usize {
        self.starts.len()
    }
}

/// `World.setRandomSeed(x, z, salt)`.
fn random_seed(world_seed: i64, x: i32, z: i32, salt: i64) -> i64 {
    (x as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((z as i64).wrapping_mul(132897987541))
        .wrapping_add(world_seed)
        .wrapping_add(salt)
}

/// `EnumFacing.Plane.HORIZONTAL.facings()` is `{NORTH, EAST, SOUTH, WEST}`.
fn horizontal_facing(rand: &mut JavaRandom) -> Facing {
    match rand.next_int_bound(4) {
        0 => Facing::North,
        1 => Facing::East,
        2 => Facing::South,
        _ => Facing::West,
    }
}

/// `StructureOceanMonument.canSpawnStructureAtCoords`.
fn can_spawn_at_coords(world_seed: i64, x0: i32, z0: i32, lookup: &mut BiomeLookup) -> bool {
    let mut x = x0;
    let mut z = z0;
    if x < 0 {
        x -= SPACING - 1;
    }
    if z < 0 {
        z -= SPACING - 1;
    }

    let gx = x / SPACING;
    let gz = z / SPACING;
    let mut r = JavaRandom::new(random_seed(world_seed, gx, gz, SALT));
    let span = SPACING - SEPARATION;
    let mut cx = gx * SPACING;
    let mut cz = gz * SPACING;
    cx += (r.next_int_bound(span) + r.next_int_bound(span)) / 2;
    cz += (r.next_int_bound(span) + r.next_int_bound(span)) / 2;

    if x0 != cx || z0 != cz {
        return false;
    }
    let bx = x0 * 16 + 8;
    let bz = z0 * 16 + 8;
    if lookup.index_at(bx, bz) != DEEP_OCEAN {
        return false;
    }
    lookup.are_viable(bx, bz, 29, &MONUMENT_BIOMES)
}

/// `StructureOceanMonument.StartMonument.func_175789_b`: re-seed with the world seed, pick the
/// orientation, and build the single `MonumentBuilding`.
fn start(rand: &mut JavaRandom, chunk_x: i32, chunk_z: i32, world_seed: i64) -> StructureStart {
    rand.set_seed(world_seed);
    let a = rand.next_long();
    let b = rand.next_long();
    rand.set_seed((chunk_x as i64).wrapping_mul(a) ^ (chunk_z as i64).wrapping_mul(b) ^ world_seed);
    // `EnumFacing.Plane.HORIZONTAL.random(rand)` - the only draw that reaches the serialized layout
    // (`MonumentBuilding`'s box is orientation-independent).
    let facing = horizontal_facing(rand);

    let x = chunk_x * 16 + 8 - 29;
    let z = chunk_z * 16 + 8 - 29;
    let pieces = vec![Piece {
        kind: PieceKind::MonumentBuilding,
        coord_base_mode: Some(facing),
        component_type: 0,
        bbox: BoundingBox::new(x, 39, z, x + 58 - 1, 61, z + 58 - 1),
    }];

    StructureStart {
        id: "Monument",
        chunk_x,
        chunk_z,
        bbox: StructureStart::update_bbox(&pieces),
        pieces,
        sizeable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    fn bounds(p: &BoundingBox) -> (i32, i32, i32, i32, i32, i32) {
        (p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z)
    }

    /// Monuments are off in `GenOptions::none()`.
    #[test]
    fn monument_off_registers_nothing() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(77, -50, GenOptions::none());
        assert!(chunk.structures.iter().all(|s| s.id != "Monument"));
    }

    /// Seed 0's monument at chunk (77,-50) - one `OMB` piece, orientation NORTH, box
    /// 1211 39 -821 1268 61 -764 (oracle). The origin chunk reports its own building box.
    #[test]
    fn monument_seed0_chunk_77_neg50() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(77, -50, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Monument")
            .expect("seed 0 (77,-50) reports its monument");
        assert_eq!((start.chunk_x, start.chunk_z), (77, -50));
        assert_eq!(bounds(&start.bbox), (1211, 39, -821, 1268, 61, -764));
        assert_eq!(start.pieces.len(), 1);

        let piece = &start.pieces[0];
        assert_eq!(piece.kind.id(), "OMB");
        assert_eq!(piece.component_type, 0);
        assert_eq!(piece.coord_base_mode, Some(Facing::North));
        assert_eq!(bounds(&piece.bbox), (1211, 39, -821, 1268, 61, -764));
    }

    /// Seed -7's monument at chunk (-28,21) - orientation EAST, box -469 39 315 -412 61 372
    /// (oracle), and its 58x23x58 extent is orientation-independent.
    #[test]
    fn monument_seed_neg7_chunk_neg28_21() {
        let mut gen = TerrainGen::new(-7);
        let chunk = gen.generate_chunk(-28, 21, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Monument")
            .expect("seed -7 (-28,21) reports its monument");
        assert_eq!((start.chunk_x, start.chunk_z), (-28, 21));
        assert_eq!(bounds(&start.bbox), (-469, 39, 315, -412, 61, 372));

        let piece = &start.pieces[0];
        assert_eq!(piece.kind.id(), "OMB");
        assert_eq!(piece.coord_base_mode, Some(Facing::East));
        assert_eq!(bounds(&piece.bbox), (-469, 39, 315, -412, 61, 372));
    }
}
