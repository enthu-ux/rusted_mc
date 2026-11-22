//! `MapGenStronghold` (strongholds): the registration + layout half.
//!
//! Stronghold placement is not grid-local: on first use, `canSpawnStructureAtCoords` seeds a
//! `Random(worldSeed)`, walks a spiral (`(1.25*spread + nextDouble()) * 32*spread` at 120° steps),
//! and refines each of the 3 ring positions to the nearest valid biome via
//! `WorldChunkManager.findBiomePosition(.., 112, strongholdBiomes, rand)`. A chunk spawns a start iff
//! it is one of those 3 chunks.
//!
//! The layout is `StructureStrongholdPieces`: a `Stairs2` root draws a weighted piece
//! (`prepareStructurePieces` + `func_175955_b`, 5 attempts, `Corridor` fallback) and grows a tree via
//! `getNextComponent{Normal,X,Z}`. `getStructureStart` re-rolls the whole start until a `PortalRoom`
//! was created. Each piece draws an `EntryDoor` (`getRandomDoor`) except `PortalRoom`/`Corridor`.
//!
//! Vanilla quirk replicated: the `RightTurn` weight dispatches through `LeftTurn.func_175867_a`, which
//! constructs a **`LeftTurn`** (so `RightTurn` pieces are never produced).
//!
//! Block placement is not ported yet (layout only).

use std::collections::BTreeMap;
use std::f64::consts::PI;

use mc_core::random::JavaRandom;

use super::{
    component_box, find_intersecting, BoundingBox, Door, Facing, Piece, PieceKind, StrongholdKind,
    StrongholdPiece, StructureStart,
};
use crate::mapgen;
use crate::terrain::{BiomeLookup, STRONGHOLD_BIOMES};

/// `MapGenStronghold.field_82671_h` (default distance).
const DISTANCE: f64 = 32.0;

/// `StructureStrongholdPieces.pieceWeightArray`.
#[derive(Clone, Copy)]
struct Weight {
    kind: StrongholdKind,
    weight: i32,
    limit: i32,
    spawned: i32,
}

const fn w(kind: StrongholdKind, weight: i32, limit: i32) -> Weight {
    Weight {
        kind,
        weight,
        limit,
        spawned: 0,
    }
}

const PIECE_WEIGHTS: [Weight; 11] = [
    w(StrongholdKind::Straight, 40, 0),
    w(StrongholdKind::Prison, 5, 5),
    w(StrongholdKind::LeftTurn, 20, 0),
    w(StrongholdKind::RightTurn, 20, 0),
    w(StrongholdKind::RoomCrossing, 10, 6),
    w(StrongholdKind::StairsStraight, 5, 5),
    w(StrongholdKind::Stairs, 5, 5),
    w(StrongholdKind::Crossing, 5, 4),
    w(StrongholdKind::ChestCorridor, 5, 4),
    w(StrongholdKind::Library, 10, 2),
    w(StrongholdKind::PortalRoom, 20, 1),
];

/// `MapGenStronghold`: a `MapGenStructure` for strongholds.
pub struct Stronghold {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
    structure_coords: Option<[(i32, i32); 3]>,
}

impl Default for Stronghold {
    fn default() -> Self {
        Self::new()
    }
}

impl Stronghold {
    pub fn new() -> Stronghold {
        Stronghold {
            starts: BTreeMap::new(),
            rand: JavaRandom::new(0),
            structure_coords: None,
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
        let Stronghold {
            starts,
            rand,
            structure_coords,
        } = self;
        mapgen::generate(world_seed, rand, target_x, target_z, |rand, nx, nz| {
            if starts.contains_key(&(nx, nz)) {
                return;
            }
            rand.next_int(); // `MapGenStructure.recursiveGenerate`'s discard draw
            if can_spawn_at_coords(world_seed, nx, nz, lookup, structure_coords) {
                starts.insert((nx, nz), start(rand, nx, nz));
            }
        });
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

/// `Math.round(double)`.
fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i64 as i32
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

/// `Stronghold.getRandomDoor`.
fn random_door(rand: &mut JavaRandom) -> Door {
    match rand.next_int_bound(5) {
        0 | 1 => Door::Opening,
        2 => Door::WoodDoor,
        3 => Door::Grates,
        _ => Door::IronDoor,
    }
}

/// `Village`-style depth gate for stronghold pieces (`minY > 10`).
fn can_go_deeper(b: &BoundingBox) -> bool {
    b.min_y > 10
}

/// `MapGenStronghold.canSpawnStructureAtCoords` - the lazily computed 3 ring positions.
fn can_spawn_at_coords(
    world_seed: i64,
    x: i32,
    z: i32,
    lookup: &mut BiomeLookup,
    coords: &mut Option<[(i32, i32); 3]>,
) -> bool {
    if coords.is_none() {
        let mut r = JavaRandom::new(world_seed);
        let mut angle = r.next_double() * PI * 2.0;
        let spread = 1i32;
        let mut arr = [(0i32, 0i32); 3];
        for slot in &mut arr {
            let dist = (1.25 * spread as f64 + r.next_double()) * (DISTANCE * spread as f64);
            let mut cx = java_round(angle.cos() * dist);
            let mut cz = java_round(angle.sin() * dist);
            if let Some((px, pz)) = lookup.find_biome_position(
                (cx << 4) + 8,
                (cz << 4) + 8,
                112,
                STRONGHOLD_BIOMES,
                &mut r,
            ) {
                cx = px >> 4;
                cz = pz >> 4;
            }
            *slot = (cx, cz);
            angle += (PI * 2.0) * spread as f64 / 3.0;
        }
        *coords = Some(arr);
    }
    let c = coords.unwrap();
    c.iter().any(|&(cx, cz)| cx == x && cz == z)
}

/// `MapGenStronghold.Start` + the re-roll loop until a `PortalRoom` exists.
fn start(rand: &mut JavaRandom, chunk_x: i32, chunk_z: i32) -> StructureStart {
    let bx = (chunk_x << 4) + 2;
    let bz = (chunk_z << 4) + 2;
    loop {
        let facing = horizontal_facing(rand);
        let root = make_piece(
            facing,
            0,
            BoundingBox::new(bx, 64, bz, bx + 5 - 1, 74, bz + 5 - 1),
            StrongholdPiece {
                kind: StrongholdKind::Stairs2,
                door: Door::Opening,
                source: true,
                ..Default::default()
            },
        );
        let mut layout = StrongholdLayout::new(bx, bz);
        layout.pieces.push(root);
        layout.build_component(rand, 0);
        while !layout.pending.is_empty() {
            let i = rand.next_int_bound(layout.pending.len() as i32) as usize;
            let index = layout.pending.remove(i);
            layout.build_component(rand, index);
        }
        if layout.portal_room.is_some() {
            let mut start = StructureStart {
                id: "Stronghold",
                chunk_x,
                chunk_z,
                bbox: StructureStart::update_bbox(&layout.pieces),
                pieces: layout.pieces,
                sizeable: true,
            };
            start.mark_available_height(rand, 63, 10);
            return start;
        }
    }
}

/// Builds a `Stronghold` piece.
fn make_piece(
    facing: Facing,
    component_type: i32,
    bbox: BoundingBox,
    data: StrongholdPiece,
) -> Piece {
    Piece {
        kind: PieceKind::Stronghold(data),
        coord_base_mode: Some(facing),
        component_type,
        bbox,
    }
}

/// The mutable state of one stronghold layout (vanilla's static `structurePieceList` etc., per Start).
struct StrongholdLayout {
    pieces: Vec<Piece>,
    pending: Vec<usize>,
    weights: Vec<Weight>,
    last: Option<StrongholdKind>,
    strong_component_type: Option<StrongholdKind>,
    portal_room: Option<usize>,
    root_x: i32,
    root_z: i32,
}

impl StrongholdLayout {
    fn new(root_x: i32, root_z: i32) -> StrongholdLayout {
        StrongholdLayout {
            pieces: Vec::new(),
            pending: Vec::new(),
            weights: PIECE_WEIGHTS.to_vec(),
            last: None,
            strong_component_type: None,
            portal_room: None,
            root_x,
            root_z,
        }
    }

    fn facing(&self, index: usize) -> Facing {
        self.pieces[index]
            .coord_base_mode
            .expect("stronghold piece facing")
    }

    /// `StructureComponent.buildComponent`, dispatched on the piece kind.
    fn build_component(&mut self, rand: &mut JavaRandom, index: usize) {
        let (kind, data) = match &self.pieces[index].kind {
            PieceKind::Stronghold(s) => (s.kind, s.clone()),
            _ => return,
        };
        match kind {
            StrongholdKind::Stairs2 | StrongholdKind::Stairs => {
                if data.source {
                    self.strong_component_type = Some(StrongholdKind::Crossing);
                }
                self.next_normal(index, rand, 1, 1);
            }
            StrongholdKind::Straight => {
                self.next_normal(index, rand, 1, 1);
                if data.expands_x {
                    self.next_x(index, rand, 1, 2);
                }
                if data.expands_z {
                    self.next_z(index, rand, 1, 2);
                }
            }
            StrongholdKind::Prison => self.next_normal(index, rand, 1, 1),
            StrongholdKind::LeftTurn => {
                let f = self.facing(index);
                if f != Facing::North && f != Facing::East {
                    self.next_z(index, rand, 1, 1);
                } else {
                    self.next_x(index, rand, 1, 1);
                }
            }
            StrongholdKind::RightTurn => {
                let f = self.facing(index);
                if f != Facing::North && f != Facing::East {
                    self.next_x(index, rand, 1, 1);
                } else {
                    self.next_z(index, rand, 1, 1);
                }
            }
            StrongholdKind::RoomCrossing => {
                self.next_normal(index, rand, 4, 1);
                self.next_x(index, rand, 1, 4);
                self.next_z(index, rand, 1, 4);
            }
            StrongholdKind::StairsStraight => self.next_normal(index, rand, 1, 1),
            StrongholdKind::Crossing => {
                let f = self.facing(index);
                let (mut v4, mut v5) = (3, 5);
                if f == Facing::West || f == Facing::North {
                    v4 = 8 - 3;
                    v5 = 8 - 5;
                }
                self.next_normal(index, rand, 5, 1);
                if data.left_low {
                    self.next_x(index, rand, v4, 1);
                }
                if data.left_high {
                    self.next_x(index, rand, v5, 7);
                }
                if data.right_low {
                    self.next_z(index, rand, v4, 1);
                }
                if data.right_high {
                    self.next_z(index, rand, v5, 7);
                }
            }
            StrongholdKind::ChestCorridor => self.next_normal(index, rand, 1, 1),
            // `PortalRoom.buildComponent` records itself on the start.
            StrongholdKind::PortalRoom => self.portal_room = Some(index),
            StrongholdKind::Library | StrongholdKind::Corridor => {}
        }
    }

    fn next_normal(&mut self, from: usize, rand: &mut JavaRandom, x_off: i32, y_off: i32) {
        let b = self.pieces[from].bbox;
        let facing = self.facing(from);
        let ct = self.pieces[from].component_type;
        let (x, y, z) = match facing {
            Facing::North => (b.min_x + x_off, b.min_y + y_off, b.min_z - 1),
            Facing::South => (b.min_x + x_off, b.min_y + y_off, b.max_z + 1),
            Facing::West => (b.min_x - 1, b.min_y + y_off, b.min_z + x_off),
            Facing::East => (b.max_x + 1, b.min_y + y_off, b.min_z + x_off),
        };
        self.try_add(rand, x, y, z, facing, ct);
    }

    fn next_x(&mut self, from: usize, rand: &mut JavaRandom, x_off: i32, y_off: i32) {
        let b = self.pieces[from].bbox;
        let facing = self.facing(from);
        let ct = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => {
                (b.min_x - 1, b.min_y + x_off, b.min_z + y_off, Facing::West)
            }
            Facing::West | Facing::East => {
                (b.min_x + y_off, b.min_y + x_off, b.min_z - 1, Facing::North)
            }
        };
        self.try_add(rand, x, y, z, f, ct);
    }

    fn next_z(&mut self, from: usize, rand: &mut JavaRandom, x_off: i32, y_off: i32) {
        let b = self.pieces[from].bbox;
        let facing = self.facing(from);
        let ct = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => {
                (b.max_x + 1, b.min_y + x_off, b.min_z + y_off, Facing::East)
            }
            Facing::West | Facing::East => {
                (b.min_x + y_off, b.min_y + x_off, b.max_z + 1, Facing::South)
            }
        };
        self.try_add(rand, x, y, z, f, ct);
    }

    /// `func_175953_c`.
    fn try_add(
        &mut self,
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
        let piece = self.weighted_piece(rand, x, y, z, facing, depth + 1)?;
        let i = self.pieces.len();
        self.pieces.push(piece);
        self.pending.push(i);
        Some(i)
    }

    /// `func_175955_b`: the weighted piece selection (with the one-shot `strongComponentType`).
    fn weighted_piece(
        &mut self,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let mut any = false;
        let mut total = 0;
        for w in &self.weights {
            if w.limit > 0 && w.spawned < w.limit {
                any = true;
            }
            total += w.weight;
        }
        if !any {
            return None;
        }

        if let Some(kind) = self.strong_component_type.take() {
            if let Some(piece) = self.create_piece(kind, rand, x, y, z, facing, depth) {
                return Some(piece);
            }
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
                    if !can_spawn_of_type(&w, depth) || self.last == Some(w.kind) {
                        break;
                    }
                    if let Some(piece) = self.create_piece(w.kind, rand, x, y, z, facing, depth) {
                        self.weights[index].spawned += 1;
                        self.last = Some(w.kind);
                        if !can_spawn_more(&self.weights[index]) {
                            self.weights.remove(index);
                        }
                        return Some(piece);
                    }
                }
                index += 1;
            }
        }

        self.create_corridor(rand, x, y, z, facing, depth)
    }

    /// `func_175954_a` (the class dispatch) + the piece factories. `None` when the box is rejected.
    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        &self,
        kind: StrongholdKind,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        // Vanilla quirk: `RightTurn` dispatches through `LeftTurn.func_175867_a`, which constructs a
        // `LeftTurn` (so a `RightTurn` piece is never produced).
        if kind == StrongholdKind::RightTurn {
            return self.create_piece(StrongholdKind::LeftTurn, rand, x, y, z, facing, depth);
        }
        if kind == StrongholdKind::Library {
            return self.create_library(rand, x, y, z, facing, depth);
        }
        if kind == StrongholdKind::Corridor {
            return self.create_corridor(rand, x, y, z, facing, depth);
        }

        let (ox, oy, oz, sx, sy, sz) = match kind {
            StrongholdKind::Straight => (-1, -1, 0, 5, 5, 7),
            StrongholdKind::Prison => (-1, -1, 0, 9, 5, 11),
            StrongholdKind::LeftTurn => (-1, -1, 0, 5, 5, 5),
            StrongholdKind::RoomCrossing => (-4, -1, 0, 11, 7, 11),
            StrongholdKind::StairsStraight => (-1, -7, 0, 5, 11, 8),
            StrongholdKind::Stairs => (-1, -7, 0, 5, 11, 5),
            StrongholdKind::Crossing => (-4, -3, 0, 10, 9, 11),
            StrongholdKind::ChestCorridor => (-1, -1, 0, 5, 5, 7),
            StrongholdKind::PortalRoom => (-4, -1, 0, 11, 8, 16),
            _ => return None,
        };
        let bbox = component_box(x, y, z, ox, oy, oz, sx, sy, sz, facing);
        if !can_go_deeper(&bbox) || find_intersecting(&self.pieces, &bbox).is_some() {
            return None;
        }
        let data = match kind {
            StrongholdKind::PortalRoom => StrongholdPiece {
                kind,
                door: Door::Opening,
                ..Default::default()
            },
            StrongholdKind::Straight => {
                let door = random_door(rand);
                let expands_x = rand.next_int_bound(2) == 0;
                let expands_z = rand.next_int_bound(2) == 0;
                StrongholdPiece {
                    kind,
                    door,
                    expands_x,
                    expands_z,
                    ..Default::default()
                }
            }
            StrongholdKind::Crossing => {
                let door = random_door(rand);
                let left_low = rand.next_boolean();
                let left_high = rand.next_boolean();
                let right_low = rand.next_boolean();
                let right_high = rand.next_int_bound(3) > 0;
                StrongholdPiece {
                    kind,
                    door,
                    left_low,
                    left_high,
                    right_low,
                    right_high,
                    ..Default::default()
                }
            }
            StrongholdKind::RoomCrossing => {
                let door = random_door(rand);
                let room_type = rand.next_int_bound(5);
                StrongholdPiece {
                    kind,
                    door,
                    room_type,
                    ..Default::default()
                }
            }
            _ => StrongholdPiece {
                kind,
                door: random_door(rand),
                ..Default::default()
            },
        };
        Some(make_piece(facing, depth, bbox, data))
    }

    /// `Library.func_175864_a`: try the tall (14x11x15) box, then the short (14x6x15) one.
    fn create_library(
        &self,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let mut bbox = component_box(x, y, z, -4, -1, 0, 14, 11, 15, facing);
        if !can_go_deeper(&bbox) || find_intersecting(&self.pieces, &bbox).is_some() {
            bbox = component_box(x, y, z, -4, -1, 0, 14, 6, 15, facing);
            if !can_go_deeper(&bbox) || find_intersecting(&self.pieces, &bbox).is_some() {
                return None;
            }
        }
        let door = random_door(rand);
        let tall = bbox.y_size() > 6;
        Some(make_piece(
            facing,
            depth,
            bbox,
            StrongholdPiece {
                kind: StrongholdKind::Library,
                door,
                tall,
                ..Default::default()
            },
        ))
    }

    /// `Corridor.func_175869_a` + the `Corridor` constructor (the fallback piece).
    fn create_corridor(
        &self,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let _ = rand;
        let bbox = component_box(x, y, z, -1, -1, 0, 5, 5, 4, facing);
        let inter = find_intersecting(&self.pieces, &bbox)?;
        if self.pieces[inter].bbox.min_y == bbox.min_y {
            for len in (1..=3).rev() {
                let shrunk = component_box(x, y, z, -1, -1, 0, 5, 5, len - 1, facing);
                if !self.pieces[inter].bbox.intersects_with(&shrunk) {
                    let full = component_box(x, y, z, -1, -1, 0, 5, 5, len, facing);
                    if full.min_y <= 1 {
                        return None;
                    }
                    let steps = if facing == Facing::North || facing == Facing::South {
                        full.z_size()
                    } else {
                        full.x_size()
                    };
                    return Some(make_piece(
                        facing,
                        depth,
                        full,
                        StrongholdPiece {
                            kind: StrongholdKind::Corridor,
                            door: Door::Opening,
                            steps,
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        None
    }
}

/// `canSpawnMoreStructuresOfType` (Library/PortalRoom add a depth requirement).
fn can_spawn_of_type(w: &Weight, depth: i32) -> bool {
    match w.kind {
        StrongholdKind::Library => can_spawn_more(w) && depth > 4,
        StrongholdKind::PortalRoom => can_spawn_more(w) && depth > 5,
        _ => can_spawn_more(w),
    }
}

/// `canSpawnMoreStructures`.
fn can_spawn_more(w: &Weight) -> bool {
    w.limit == 0 || w.spawned < w.limit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    /// Strongholds are off in `GenOptions::none()`.
    #[test]
    fn stronghold_off_registers_nothing() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-9, -41, GenOptions::none());
        assert!(chunk.structures.iter().all(|s| s.id != "Stronghold"));
    }

    /// The generator is deterministic per (seed, chunk).
    #[test]
    fn stronghold_is_deterministic() {
        let mut a = TerrainGen::new(0);
        let mut b = TerrainGen::new(0);
        let sa: Vec<_> = a
            .generate_chunk(-9, -41, GenOptions::vanilla())
            .structures
            .into_iter()
            .filter(|s| s.id == "Stronghold")
            .collect();
        let sb: Vec<_> = b
            .generate_chunk(-9, -41, GenOptions::vanilla())
            .structures
            .into_iter()
            .filter(|s| s.id == "Stronghold")
            .collect();
        assert!(!sa.is_empty());
        assert_eq!(sa, sb);
    }

    /// Seed 0's stronghold at (-9,-41) - 267 pieces, root `SHStart` facing NORTH; the whole start
    /// is dropped by `markAvailableHeight` (oracle: BB -248 17 -742 -72 51 -599, root
    /// -142 41 -654 -138 51 -650, O=2).
    #[test]
    fn stronghold_golden_seed0() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-9, -41, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Stronghold")
            .expect("seed 0 stronghold at (-9,-41)");
        assert_eq!((start.chunk_x, start.chunk_z), (-9, -41));
        assert_eq!(
            (
                start.bbox.min_x,
                start.bbox.min_y,
                start.bbox.min_z,
                start.bbox.max_x,
                start.bbox.max_y,
                start.bbox.max_z
            ),
            (-248, 17, -742, -72, 51, -599)
        );
        assert_eq!(start.pieces.len(), 267);
        assert!(start.pieces.iter().any(|p| p.kind.id() == "SHPR"));

        let root = &start.pieces[0];
        assert_eq!(root.kind.id(), "SHStart");
        assert_eq!(root.coord_base_mode, Some(Facing::North));
        assert_eq!(root.component_type, 0);
    }

    /// Seed -7's stronghold at (-44,-34) (oracle: BB -736 2 -570 -629 44 -490, 105 pieces).
    #[test]
    fn stronghold_golden_seed_neg7() {
        let mut gen = TerrainGen::new(-7);
        let chunk = gen.generate_chunk(-44, -34, GenOptions::vanilla());
        let start = chunk
            .structures
            .iter()
            .find(|s| s.id == "Stronghold")
            .expect("seed -7 stronghold at (-44,-34)");
        assert_eq!((start.chunk_x, start.chunk_z), (-44, -34));
        assert_eq!(
            (
                start.bbox.min_x,
                start.bbox.min_y,
                start.bbox.min_z,
                start.bbox.max_x,
                start.bbox.max_y,
                start.bbox.max_z
            ),
            (-736, 2, -570, -629, 44, -490)
        );
        assert_eq!(start.pieces.len(), 105);
    }
}
