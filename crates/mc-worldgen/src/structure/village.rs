//! `MapGenVillage` (villages): the registration + layout half.
//!
//! Villages are grid-placed like the scattered feature: `canSpawnStructureAtCoords` re-seeds the
//! world `Random` with `World.setRandomSeed(gx, gz, 10387312)` (two `nextInt(24)` offsets) and
//! requires a plains/desert/savanna biome at the chunk centre (`areBiomesViable(.., 0, ..)`).
//!
//! The layout is `StructureVillagePieces`. A `Start` (a `Well`) draws a weighted piece list
//! (`getStructureVillageWeightedPieceList`), then grows a tree of roads (`Path`) and buildings
//! (`Church`/`Hall`/`House*`/`Well`/`Field*`/`WoodHut`). Each candidate is checked against the 112-block
//! window and `areBiomesViable` around its centre; a rejected piece is discarded (its RNG draw is still
//! consumed). Roads spawn buildings along their length and occasionally branch into more roads.
//!
//! Two lists are drained in random order (`MapGenVillage.Start`): `field_74930_j` (roads) and
//! `field_74932_i` (buildings), roads first. A start is `isSizeableStructure` only when it has more
//! than two non-road pieces.
//!
//! Block placement lives in [`super::village_placement`] (`TerrainGen::populate_village_chunk`).

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{
    component_box, find_intersecting, BoundingBox, Facing, Piece, PieceKind, StructureStart,
    VillageKind, VillagePiece,
};
use crate::mapgen;
use crate::terrain::BiomeLookup;

/// `MapGenVillage.field_82665_g` (default spacing).
const SPACING: i32 = 32;
/// `MapGenVillage.field_82666_h` (default separation).
const SEPARATION: i32 = 8;
/// The salt `canSpawnStructureAtCoords` passes to `World.setRandomSeed`.
const SALT: i64 = 10387312;
/// `MapGenVillage.villageSpawnBiomes` ids: plains, desert, savanna.
const VILLAGE_BIOMES: [i32; 3] = [1, 2, 35];
/// `Block` ids for the `Field1`/`Field2` crop types (`func_151559_a`).
const CROP_CARROTS: i32 = 141;
const CROP_POTATOES: i32 = 142;
const CROP_WHEAT: i32 = 59;

/// `MapGenVillage`: a `MapGenStructure` for villages.
pub struct Village {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
}

impl Default for Village {
    fn default() -> Self {
        Self::new()
    }
}

impl Village {
    pub fn new() -> Village {
        Village {
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
                    starts.insert((nx, nz), start(rand, nx, nz, lookup));
                }
            },
        );
    }

    /// Every *sizeable* registered start whose bounding box meets this chunk (`isSizeableStructure`
    /// excludes villages with <= 2 non-road pieces).
    pub fn starts_intersecting(&self, chunk_x: i32, chunk_z: i32) -> Vec<StructureStart> {
        self.starts
            .values()
            .filter(|s| s.sizeable && s.intersects_chunk(chunk_x, chunk_z))
            .cloned()
            .collect()
    }

    /// Mutable access to every *sizeable* registered start whose box meets the X/Z region (the
    /// `MapGenStructure.generateStructure` walk). Mutation matters: a piece's `HPos` (average ground
    /// level) is cached on its first placement pass.
    pub fn for_each_start_meeting_mut<F: FnMut(&mut StructureStart)>(
        &mut self,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        mut visit: F,
    ) {
        for start in self.starts.values_mut() {
            if start.sizeable && start.bbox.intersects_with_xz(x0, z0, x1, z1) {
                visit(start);
            }
        }
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

/// `MathHelper.getRandomIntegerInRange(rand, min, max)`.
fn random_int_in_range(rand: &mut JavaRandom, min: i32, max: i32) -> i32 {
    min + rand.next_int_bound(max - min + 1)
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

/// `MapGenVillage.canSpawnStructureAtCoords`.
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
    cx += r.next_int_bound(span);
    cz += r.next_int_bound(span);

    if x0 != cx || z0 != cz {
        return false;
    }
    lookup.are_viable(x0 * 16 + 8, z0 * 16 + 8, 0, &VILLAGE_BIOMES)
}

/// `StructureVillagePieces.PieceWeight`.
#[derive(Clone, Copy)]
struct Weight {
    kind: VillageKind,
    weight: i32,
    limit: i32,
    spawned: i32,
}

/// `getStructureVillageWeightedPieceList(rand, terrainType)` (draw order matters).
fn weighted_piece_list(rand: &mut JavaRandom, terrain: i32) -> Vec<Weight> {
    let mut v = vec![
        Weight {
            kind: VillageKind::House4Garden,
            weight: 4,
            limit: random_int_in_range(rand, 2 + terrain, 4 + terrain * 2),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::Church,
            weight: 20,
            limit: random_int_in_range(rand, terrain, 1 + terrain),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::House1,
            weight: 20,
            limit: random_int_in_range(rand, terrain, 2 + terrain),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::WoodHut,
            weight: 3,
            limit: random_int_in_range(rand, 2 + terrain, 5 + terrain * 3),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::Hall,
            weight: 15,
            limit: random_int_in_range(rand, terrain, 2 + terrain),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::Field1,
            weight: 3,
            limit: random_int_in_range(rand, 1 + terrain, 4 + terrain),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::Field2,
            weight: 3,
            limit: random_int_in_range(rand, 2 + terrain, 4 + terrain * 2),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::House2,
            weight: 15,
            limit: random_int_in_range(rand, 0, 1 + terrain),
            spawned: 0,
        },
        Weight {
            kind: VillageKind::House3,
            weight: 8,
            limit: random_int_in_range(rand, terrain, 3 + terrain * 2),
            spawned: 0,
        },
    ];
    v.retain(|w| w.limit != 0);
    v
}

/// `func_75079_a`: sum of weights if any can still spawn, else -1.
fn get_total_weight(list: &[Weight]) -> i32 {
    let mut any = false;
    let mut sum = 0;
    for w in list {
        if w.limit == 0 || w.spawned < w.limit {
            any = true;
        }
        sum += w.weight;
    }
    if any {
        sum
    } else {
        -1
    }
}

/// `PieceWeight.canSpawnMoreVillagePieces*`.
fn can_spawn_weight(w: &Weight) -> bool {
    w.limit == 0 || w.spawned < w.limit
}

/// `Village.canVillageGoDeeper`.
fn can_go_deeper(b: &BoundingBox) -> bool {
    b.min_y > 10
}

/// `MapGenVillage.Start`.
fn start(
    rand: &mut JavaRandom,
    chunk_x: i32,
    chunk_z: i32,
    lookup: &mut BiomeLookup,
) -> StructureStart {
    let terrain_type = 0;
    let weights = weighted_piece_list(rand, terrain_type);

    let bx = (chunk_x << 4) + 2;
    let bz = (chunk_z << 4) + 2;
    let in_desert = {
        let b = lookup.index_at(bx, bz);
        b == 2 || b == 17
    };
    let facing = horizontal_facing(rand);

    let root = make_piece(
        facing,
        0,
        BoundingBox::new(bx, 64, bz, bx + 6 - 1, 78, bz + 6 - 1),
        VillagePiece {
            kind: VillageKind::Start,
            desert: in_desert,
            ..Default::default()
        },
    );
    let mut layout = VillageLayout {
        root_x: bx,
        root_z: bz,
        pieces: vec![root],
        roads: Vec::new(),
        buildings: Vec::new(),
        weights,
        last: None,
        terrain_type,
        in_desert,
    };
    layout.build_component(lookup, rand, 0);
    while !layout.roads.is_empty() || !layout.buildings.is_empty() {
        let index = if layout.roads.is_empty() {
            let i = rand.next_int_bound(layout.buildings.len() as i32) as usize;
            layout.buildings.remove(i)
        } else {
            let i = rand.next_int_bound(layout.roads.len() as i32) as usize;
            layout.roads.remove(i)
        };
        layout.build_component(lookup, rand, index);
    }

    let non_roads = layout
        .pieces
        .iter()
        .filter(|p| match &p.kind {
            PieceKind::Village(v) => !v.kind.is_road(),
            _ => true,
        })
        .count();
    StructureStart {
        id: "Village",
        chunk_x,
        chunk_z,
        bbox: StructureStart::update_bbox(&layout.pieces),
        pieces: layout.pieces,
        sizeable: non_roads > 2,
    }
}

/// Builds a `Village` piece (component type + facing) whose base fields are defaulted.
fn make_piece(facing: Facing, component_type: i32, bbox: BoundingBox, data: VillagePiece) -> Piece {
    Piece {
        kind: PieceKind::Village(data),
        coord_base_mode: Some(facing),
        component_type,
        bbox,
    }
}

macro_rules! village_data {
    ($kind:expr, $desert:expr $(, $field:ident : $value:expr)* $(,)?) => {
        VillagePiece { kind: $kind, desert: $desert, $($field: $value,)* ..Default::default() }
    };
}

/// The mutable state of one village layout.
struct VillageLayout {
    pieces: Vec<Piece>,
    roads: Vec<usize>,
    buildings: Vec<usize>,
    weights: Vec<Weight>,
    last: Option<VillageKind>,
    terrain_type: i32,
    in_desert: bool,
    root_x: i32,
    root_z: i32,
}

impl VillageLayout {
    fn bbox_facing(&self, index: usize) -> (BoundingBox, Facing) {
        let p = &self.pieces[index];
        (p.bbox, p.coord_base_mode.expect("village piece facing"))
    }

    /// `StructureComponent.buildComponent`, dispatched on the piece kind (only `Well`/`Start`/`Path`
    /// override it).
    fn build_component(&mut self, lookup: &mut BiomeLookup, rand: &mut JavaRandom, index: usize) {
        let kind = match &self.pieces[index].kind {
            PieceKind::Village(v) => v.kind,
            _ => return,
        };
        match kind {
            VillageKind::Start | VillageKind::Well => {
                let (b, facing) = self.bbox_facing(index);
                let ct = self.pieces[index].component_type;
                // `Well.buildComponent`: four roads out of the sides.
                self.try_add_path(
                    lookup,
                    rand,
                    b.min_x - 1,
                    b.max_y - 4,
                    b.min_z + 1,
                    Facing::West,
                    ct,
                );
                self.try_add_path(
                    lookup,
                    rand,
                    b.max_x + 1,
                    b.max_y - 4,
                    b.min_z + 1,
                    Facing::East,
                    ct,
                );
                self.try_add_path(
                    lookup,
                    rand,
                    b.min_x + 1,
                    b.max_y - 4,
                    b.min_z - 1,
                    Facing::North,
                    ct,
                );
                self.try_add_path(
                    lookup,
                    rand,
                    b.min_x + 1,
                    b.max_y - 4,
                    b.max_z + 1,
                    Facing::South,
                    ct,
                );
                let _ = facing;
            }
            VillageKind::Path => self.path_build(lookup, rand, index),
            _ => {}
        }
    }

    /// `Path.buildComponent`.
    fn path_build(&mut self, lookup: &mut BiomeLookup, rand: &mut JavaRandom, index: usize) {
        let (b, facing) = self.bbox_facing(index);
        let ct = self.pieces[index].component_type;
        let length = match &self.pieces[index].kind {
            PieceKind::Village(v) => v.length,
            _ => 0,
        };

        let mut found = false;
        let mut pos = rand.next_int_bound(5);
        while pos < length - 8 {
            if let Some(i) = self.next_nn(lookup, rand, index, 0, pos) {
                let cb = self.pieces[i].bbox;
                pos += cb.x_size().max(cb.z_size());
                found = true;
            }
            pos += 2 + rand.next_int_bound(5);
        }
        let mut pos = rand.next_int_bound(5);
        while pos < length - 8 {
            if let Some(i) = self.next_pp(lookup, rand, index, 0, pos) {
                let cb = self.pieces[i].bbox;
                pos += cb.x_size().max(cb.z_size());
                found = true;
            }
            pos += 2 + rand.next_int_bound(5);
        }

        if found && rand.next_int_bound(3) > 0 {
            match facing {
                Facing::North => self.try_add_path(
                    lookup,
                    rand,
                    b.min_x - 1,
                    b.min_y,
                    b.min_z,
                    Facing::West,
                    ct,
                ),
                Facing::South => self.try_add_path(
                    lookup,
                    rand,
                    b.min_x - 1,
                    b.min_y,
                    b.max_z - 2,
                    Facing::West,
                    ct,
                ),
                Facing::West => self.try_add_path(
                    lookup,
                    rand,
                    b.min_x,
                    b.min_y,
                    b.min_z - 1,
                    Facing::North,
                    ct,
                ),
                Facing::East => self.try_add_path(
                    lookup,
                    rand,
                    b.max_x - 2,
                    b.min_y,
                    b.min_z - 1,
                    Facing::North,
                    ct,
                ),
            };
        }
        if found && rand.next_int_bound(3) > 0 {
            match facing {
                Facing::North => self.try_add_path(
                    lookup,
                    rand,
                    b.max_x + 1,
                    b.min_y,
                    b.min_z,
                    Facing::East,
                    ct,
                ),
                Facing::South => self.try_add_path(
                    lookup,
                    rand,
                    b.max_x + 1,
                    b.min_y,
                    b.max_z - 2,
                    Facing::East,
                    ct,
                ),
                Facing::West => self.try_add_path(
                    lookup,
                    rand,
                    b.min_x,
                    b.min_y,
                    b.max_z + 1,
                    Facing::South,
                    ct,
                ),
                Facing::East => self.try_add_path(
                    lookup,
                    rand,
                    b.max_x - 2,
                    b.min_y,
                    b.max_z + 1,
                    Facing::South,
                    ct,
                ),
            };
        }
    }

    /// `getNextComponentNN`.
    fn next_nn(
        &mut self,
        lookup: &mut BiomeLookup,
        rand: &mut JavaRandom,
        from: usize,
        x_off: i32,
        y_off: i32,
    ) -> Option<usize> {
        let (b, facing) = self.bbox_facing(from);
        let ct = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => {
                (b.min_x - 1, b.min_y + x_off, b.min_z + y_off, Facing::West)
            }
            Facing::West | Facing::East => {
                (b.min_x + y_off, b.min_y + x_off, b.min_z - 1, Facing::North)
            }
        };
        self.try_add_building(lookup, rand, x, y, z, f, ct)
    }

    /// `getNextComponentPP`.
    fn next_pp(
        &mut self,
        lookup: &mut BiomeLookup,
        rand: &mut JavaRandom,
        from: usize,
        x_off: i32,
        y_off: i32,
    ) -> Option<usize> {
        let (b, facing) = self.bbox_facing(from);
        let ct = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => {
                (b.max_x + 1, b.min_y + x_off, b.min_z + y_off, Facing::East)
            }
            Facing::West | Facing::East => {
                (b.min_x + y_off, b.min_y + x_off, b.max_z + 1, Facing::South)
            }
        };
        self.try_add_building(lookup, rand, x, y, z, f, ct)
    }

    /// `func_176069_e`: place a path (or reject it via `areBiomesViable`).
    #[allow(clippy::too_many_arguments)]
    fn try_add_path(
        &mut self,
        lookup: &mut BiomeLookup,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<usize> {
        if depth > 3 + self.terrain_type {
            return None;
        }
        if (x - self.root_x).abs() > 112 || (z - self.root_z).abs() > 112 {
            return None;
        }
        let bbox = self.path_bbox(rand, x, y, z, facing)?;
        if !can_go_deeper(&bbox) {
            return None;
        }
        let length = bbox.x_size().max(bbox.z_size());
        let desert = self.in_desert;
        let piece = make_piece(
            facing,
            depth,
            bbox,
            village_data!(VillageKind::Path, desert, length: length),
        );
        if self.viable(lookup, &bbox) {
            let i = self.pieces.len();
            self.pieces.push(piece);
            self.roads.push(i);
            Some(i)
        } else {
            None
        }
    }

    /// `func_176066_d`: place a building (or reject it via `areBiomesViable`).
    #[allow(clippy::too_many_arguments)]
    fn try_add_building(
        &mut self,
        lookup: &mut BiomeLookup,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<usize> {
        if depth > 50 {
            return None;
        }
        if (x - self.root_x).abs() > 112 || (z - self.root_z).abs() > 112 {
            return None;
        }
        let piece = self.weighted_building(lookup, rand, x, y, z, facing, depth + 1)?;
        let bbox = piece.bbox;
        if self.viable(lookup, &bbox) {
            let i = self.pieces.len();
            self.pieces.push(piece);
            self.buildings.push(i);
            Some(i)
        } else {
            None
        }
    }

    /// `func_176067_c`: weighted building selection (5 attempts, then a torch).
    #[allow(clippy::too_many_arguments)]
    fn weighted_building(
        &mut self,
        lookup: &mut BiomeLookup,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let _ = lookup;
        let total = get_total_weight(&self.weights);
        if total <= 0 {
            return None;
        }
        let mut attempts = 0;
        while attempts < 5 {
            attempts += 1;
            let mut roll = rand.next_int_bound(total);
            let mut index = 0;
            loop {
                if index >= self.weights.len() {
                    break;
                }
                let w = self.weights[index];
                roll -= w.weight;
                if roll < 0 {
                    if !can_spawn_weight(&w)
                        || (self.last == Some(w.kind) && self.weights.len() > 1)
                    {
                        break;
                    }
                    if let Some(piece) = self.create_building(w.kind, rand, x, y, z, facing, depth)
                    {
                        self.weights[index].spawned += 1;
                        self.last = Some(w.kind);
                        if !can_spawn_weight(&self.weights[index]) {
                            self.weights.remove(index);
                        }
                        return Some(piece);
                    }
                }
                index += 1;
            }
        }
        self.create_torch(x, y, z, facing, depth)
    }

    /// A building factory (`func_176065_a`'s dispatch). `None` when the box is rejected.
    #[allow(clippy::too_many_arguments)]
    fn create_building(
        &self,
        kind: VillageKind,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let desert = self.in_desert;
        // (sizeX, sizeY, sizeZ) and whether the factory also needs `canVillageGoDeeper`.
        let (sx, sy, sz, check_deeper) = match kind {
            VillageKind::Church => (5, 12, 9, true),
            VillageKind::Field1 => (13, 4, 9, true),
            VillageKind::Field2 => (7, 4, 9, true),
            VillageKind::Hall => (9, 7, 11, true),
            VillageKind::House1 => (9, 9, 6, true),
            VillageKind::House2 => (10, 6, 7, true),
            VillageKind::House3 => (9, 7, 12, true),
            VillageKind::House4Garden => (5, 6, 5, false),
            VillageKind::WoodHut => (4, 6, 5, true),
            _ => return None,
        };
        let bbox = component_box(x, y, z, 0, 0, 0, sx, sy, sz, facing);
        if check_deeper && !can_go_deeper(&bbox) {
            return None;
        }
        if find_intersecting(&self.pieces, &bbox).is_some() {
            return None;
        }
        // Kind-specific constructor draws (after the fit checks).
        let data = match kind {
            VillageKind::Field1 => {
                let mut crops = [0i32; 4];
                for c in &mut crops {
                    *c = crop_id(rand);
                }
                village_data!(kind, desert, crops: crops)
            }
            VillageKind::Field2 => {
                let mut crops = [0i32; 4];
                crops[0] = crop_id(rand);
                crops[1] = crop_id(rand);
                village_data!(kind, desert, crops: crops)
            }
            VillageKind::House4Garden => {
                village_data!(kind, desert, terrace: rand.next_boolean())
            }
            VillageKind::WoodHut => {
                let tall = rand.next_boolean();
                let table = rand.next_int_bound(3);
                village_data!(kind, desert, tall: tall, table: table)
            }
            _ => village_data!(kind, desert),
        };
        Some(make_piece(facing, depth, bbox, data))
    }

    /// The `Torch.func_175856_a` fallback (`func_176067_c`), which draws no RNG.
    fn create_torch(&self, x: i32, y: i32, z: i32, facing: Facing, depth: i32) -> Option<Piece> {
        let bbox = component_box(x, y, z, 0, 0, 0, 3, 4, 2, facing);
        if find_intersecting(&self.pieces, &bbox).is_some() {
            return None;
        }
        let desert = self.in_desert;
        Some(make_piece(
            facing,
            depth,
            bbox,
            village_data!(VillageKind::Torch, desert),
        ))
    }

    /// `Path.func_175848_a`: the first non-intersecting length (`7 * nextInt(3..=5)`, then -7 steps).
    fn path_bbox(
        &self,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
    ) -> Option<BoundingBox> {
        let mut len = 7 * random_int_in_range(rand, 3, 5);
        while len >= 7 {
            let bbox = component_box(x, y, z, 0, 0, 0, 3, 3, len, facing);
            if find_intersecting(&self.pieces, &bbox).is_none() {
                return Some(bbox);
            }
            len -= 7;
        }
        None
    }

    /// `areBiomesViable` around the piece's centre.
    fn viable(&self, lookup: &mut BiomeLookup, bbox: &BoundingBox) -> bool {
        let cx = (bbox.min_x + bbox.max_x) / 2;
        let cz = (bbox.min_z + bbox.max_z) / 2;
        let dx = bbox.max_x - bbox.min_x;
        let dz = bbox.max_z - bbox.min_z;
        let radius = dx.max(dz) / 2 + 4;
        lookup.are_viable(cx, cz, radius, &VILLAGE_BIOMES)
    }
}

/// `func_151559_a` / `func_151560_a`: the crop block id (`nextInt(5)`).
fn crop_id(rand: &mut JavaRandom) -> i32 {
    match rand.next_int_bound(5) {
        0 => CROP_CARROTS,
        1 => CROP_POTATOES,
        _ => CROP_WHEAT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    fn bounds(p: &BoundingBox) -> (i32, i32, i32, i32, i32, i32) {
        (p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z)
    }

    /// Villages are off in `GenOptions::none()`.
    #[test]
    fn village_off_registers_nothing() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(0, 0, GenOptions::none());
        assert!(chunk.structures.iter().all(|s| s.id != "Village"));
    }

    /// Seed 0's village at (-53,-16) - 18 pieces, root `ViStart` facing EAST, first road a length-21
    /// `ViSR` (oracle: BB -867 64 -275 -820 80 -221, root ViStart -846 64 -254 -841 78 -249, O=3).
    #[test]
    fn village_golden_seed0() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-53, -16, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Village")
            .expect("seed 0 village at (-53,-16)");
        assert_eq!((start.chunk_x, start.chunk_z), (-53, -16));
        assert!(start.sizeable);
        assert_eq!(bounds(&start.bbox), (-867, 64, -275, -820, 80, -221));
        assert_eq!(start.pieces.len(), 18);

        let root = &start.pieces[0];
        assert_eq!(root.kind.id(), "ViStart");
        assert_eq!(root.component_type, 0);
        assert_eq!(root.coord_base_mode, Some(Facing::East));
        assert_eq!(bounds(&root.bbox), (-846, 64, -254, -841, 78, -249));

        let path = &start.pieces[1];
        assert_eq!(path.kind.id(), "ViSR");
        assert_eq!(bounds(&path.bbox), (-867, 74, -253, -847, 76, -251));
        assert!(matches!(&path.kind, PieceKind::Village(v) if v.length == 21));
    }

    /// Seed -7's village at (35,1) - 5 pieces, root facing WEST (oracle: BB 541 64 8 567 80 32,
    /// ViStart 562 64 18 567 78 23, O=1). Chunk (35,1)'s own area misses the box, so (34,1) reports it.
    #[test]
    fn village_golden_seed_neg7() {
        let mut gen = TerrainGen::new(-7);
        let chunk = gen.generate_chunk(34, 1, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Village")
            .expect("seed -7 village at (35,1)");
        assert_eq!((start.chunk_x, start.chunk_z), (35, 1));
        assert_eq!(bounds(&start.bbox), (541, 64, 8, 567, 80, 32));
        assert_eq!(start.pieces.len(), 5);
        assert_eq!(start.pieces[0].coord_base_mode, Some(Facing::West));
    }

    /// The weighted piece list draws one limit per class in source order, dropping zero limits.
    #[test]
    fn weighted_piece_list_limits() {
        let mut rand = JavaRandom::new(0);
        let list = weighted_piece_list(&mut rand, 0);
        assert!(!list.is_empty());
        for w in &list {
            assert!(w.limit != 0);
            assert!(w.weight > 0);
        }
        let total: i32 = list.iter().map(|w| w.weight).sum();
        assert_eq!(get_total_weight(&list), total);
    }
}
