//! `MapGenMineshaft` and the `StructureMineshaftPieces` layout half (piece selection, bounding
//! boxes and the `buildComponent` recursion). The pieces' block placement (`addComponentParts`) is
//! a later increment; see `structure/mod.rs`.

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{find_intersecting, BoundingBox, Facing, Piece, PieceKind, StructureStart};
use crate::mapgen;

/// `MapGenMineshaft.field_82673_e`.
const CHANCE: f64 = 0.004;
/// `StructureMineshaftStart`'s `markAvailableHeight(world, rand, 10)`.
const AVAILABLE_HEIGHT: i32 = 10;
/// `StructureMineshaftPieces.func_175890_b`'s recursion cap (piece depth).
const MAX_DEPTH: i32 = 8;

/// `MapGenMineshaft`: a `MapGenStructure` (registry + `MapGenBase` RNG) for mineshafts.
pub struct Mineshaft {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
}

impl Default for Mineshaft {
    fn default() -> Self {
        Self::new()
    }
}

impl Mineshaft {
    pub fn new() -> Mineshaft {
        Mineshaft {
            starts: BTreeMap::new(),
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenStructure.recursiveGenerate` for every chunk in `MapGenBase`'s neighbourhood: register
    /// a start per chunk that passes `canSpawnStructureAtCoords` (nothing is written to the chunk).
    pub fn generate(&mut self, world_seed: i64, target_x: i32, target_z: i32, sea_level: i32) {
        let starts = &mut self.starts;
        mapgen::generate(
            world_seed,
            &mut self.rand,
            target_x,
            target_z,
            |rand, nx, nz| {
                // `initializeStructureData` restores a saved map first; a fresh world starts empty, and
                // an already-registered chunk is skipped *before* any draw.
                if starts.contains_key(&(nx, nz)) {
                    return;
                }
                rand.next_int(); // `MapGenStructure.recursiveGenerate`'s discard draw
                if can_spawn_at_coords(rand, nx, nz) {
                    starts.insert((nx, nz), start(rand, nx, nz, sea_level));
                }
            },
        );
    }

    /// Every registered start whose bounding box meets this chunk - the set
    /// `MapGenStructure.generateStructure` would place while populating it.
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

/// `MapGenMineshaft.canSpawnStructureAtCoords`.
fn can_spawn_at_coords(rand: &mut JavaRandom, chunk_x: i32, chunk_z: i32) -> bool {
    rand.next_double() < CHANCE && rand.next_int_bound(80) < chunk_x.abs().max(chunk_z.abs())
}

/// `StructureMineshaftStart`: the root `Room` at the chunk's centre, then its piece tree.
fn start(rand: &mut JavaRandom, chunk_x: i32, chunk_z: i32, sea_level: i32) -> StructureStart {
    let origin_x = (chunk_x << 4) + 2;
    let origin_z = (chunk_z << 4) + 2;
    // Argument order matters: the three `nextInt(6)` draws are X, Y, Z.
    let max_x = origin_x + 7 + rand.next_int_bound(6);
    let max_y = 54 + rand.next_int_bound(6);
    let max_z = origin_z + 7 + rand.next_int_bound(6);
    let room_bbox = BoundingBox::new(origin_x, 50, origin_z, max_x, max_y, max_z);

    let mut pieces = vec![Piece {
        kind: PieceKind::MineshaftRoom {
            linked_rooms: Vec::new(),
        },
        coord_base_mode: None,
        component_type: 0,
        bbox: room_bbox,
    }];

    // `room.buildComponent(room, components, rand)` - the root is its own `var1`.
    build_component(0, &mut pieces, rand, room_bbox);

    let mut start = StructureStart {
        id: "Mineshaft",
        chunk_x,
        chunk_z,
        bbox: StructureStart::update_bbox(&pieces),
        pieces,
    };
    start.mark_available_height(rand, sea_level, AVAILABLE_HEIGHT);
    start
}

/// `StructureMineshaftPieces.func_175890_b`: create, append and immediately recurse into a child.
///
/// Vanilla forwards its `var1` (the *root* component, because every piece passes its own parameter
/// straight down rather than `this`) to both the +/-80 window test and the child's
/// `buildComponent`, so `parent_bbox` is the root `Room`'s box here too.
#[allow(clippy::too_many_arguments)]
fn add_piece(
    parent_bbox: BoundingBox,
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    facing: Facing,
    depth: i32,
) -> Option<usize> {
    if depth > MAX_DEPTH {
        return None;
    }
    if (x - parent_bbox.min_x).abs() > 80 || (z - parent_bbox.min_z).abs() > 80 {
        return None;
    }

    let piece = create_piece(pieces, rand, x, y, z, facing, depth + 1)?;
    let index = pieces.len();
    pieces.push(piece);
    build_component(index, pieces, rand, parent_bbox);
    Some(index)
}

/// `StructureMineshaftPieces.func_175892_a`: roll a piece type and size it.
fn create_piece(
    pieces: &[Piece],
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    facing: Facing,
    component_type: i32,
) -> Option<Piece> {
    let roll = rand.next_int_bound(100);
    if roll >= 80 {
        if let Some(bbox) = cross_bbox(pieces, rand, x, y, z, facing) {
            return Some(Piece {
                kind: PieceKind::MineshaftCross {
                    direction: facing,
                    multiple_floors: bbox.y_size() > 3,
                },
                // `Cross`'s constructor never assigns `coordBaseMode`.
                coord_base_mode: None,
                component_type,
                bbox,
            });
        }
    } else if roll >= 70 {
        if let Some(bbox) = stairs_bbox(pieces, x, y, z, facing) {
            return Some(Piece {
                kind: PieceKind::MineshaftStairs,
                coord_base_mode: Some(facing),
                component_type,
                bbox,
            });
        }
    } else if let Some(bbox) = corridor_bbox(pieces, rand, x, y, z, facing) {
        // The `Corridor` constructor draws after the box is chosen.
        let has_rails = rand.next_int_bound(3) == 0;
        let has_spiders = !has_rails && rand.next_int_bound(23) == 0;
        let section_count = if facing == Facing::North || facing == Facing::South {
            bbox.z_size() / 5
        } else {
            bbox.x_size() / 5
        };
        return Some(Piece {
            kind: PieceKind::MineshaftCorridor {
                has_rails,
                has_spiders,
                spawner_placed: false,
                section_count,
            },
            coord_base_mode: Some(facing),
            component_type,
            bbox,
        });
    }

    None
}

/// `StructureMineshaftPieces.Corridor.func_175814_a`.
fn corridor_bbox(
    pieces: &[Piece],
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    facing: Facing,
) -> Option<BoundingBox> {
    let mut bbox = BoundingBox::new(x, y, z, x, y + 2, z);

    // `for (var7 = nextInt(3) + 2; var7 > 0; var7--)`
    let mut sections = rand.next_int_bound(3) + 2;
    while sections > 0 {
        let len = sections * 5;
        match facing {
            Facing::North => {
                bbox.max_x = x + 2;
                bbox.min_z = z - (len - 1);
            }
            Facing::South => {
                bbox.max_x = x + 2;
                bbox.max_z = z + (len - 1);
            }
            Facing::West => {
                bbox.min_x = x - (len - 1);
                bbox.max_z = z + 2;
            }
            Facing::East => {
                bbox.max_x = x + (len - 1);
                bbox.max_z = z + 2;
            }
        }
        if find_intersecting(pieces, &bbox).is_none() {
            break;
        }
        sections -= 1;
    }

    if sections > 0 {
        Some(bbox)
    } else {
        None
    }
}

/// `StructureMineshaftPieces.Cross.func_175813_a`.
fn cross_bbox(
    pieces: &[Piece],
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    facing: Facing,
) -> Option<BoundingBox> {
    let mut bbox = BoundingBox::new(x, y, z, x, y + 2, z);
    if rand.next_int_bound(4) == 0 {
        bbox.max_y += 4;
    }

    match facing {
        Facing::North => {
            bbox.min_x = x - 1;
            bbox.max_x = x + 3;
            bbox.min_z = z - 4;
        }
        Facing::South => {
            bbox.min_x = x - 1;
            bbox.max_x = x + 3;
            bbox.max_z = z + 4;
        }
        Facing::West => {
            bbox.min_x = x - 4;
            bbox.min_z = z - 1;
            bbox.max_z = z + 3;
        }
        Facing::East => {
            bbox.max_x = x + 4;
            bbox.min_z = z - 1;
            bbox.max_z = z + 3;
        }
    }

    if find_intersecting(pieces, &bbox).is_some() {
        None
    } else {
        Some(bbox)
    }
}

/// `StructureMineshaftPieces.Stairs.func_175812_a` (no RNG).
fn stairs_bbox(pieces: &[Piece], x: i32, y: i32, z: i32, facing: Facing) -> Option<BoundingBox> {
    let mut bbox = BoundingBox::new(x, y - 5, z, x, y + 2, z);
    match facing {
        Facing::North => {
            bbox.max_x = x + 2;
            bbox.min_z = z - 8;
        }
        Facing::South => {
            bbox.max_x = x + 2;
            bbox.max_z = z + 8;
        }
        Facing::West => {
            bbox.min_x = x - 8;
            bbox.max_z = z + 2;
        }
        Facing::East => {
            bbox.max_x = x + 8;
            bbox.max_z = z + 2;
        }
    }

    if find_intersecting(pieces, &bbox).is_some() {
        None
    } else {
        Some(bbox)
    }
}

/// `StructureComponent.buildComponent`, dispatched on the piece's kind.
fn build_component(
    index: usize,
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    parent_bbox: BoundingBox,
) {
    let (kind, facing, component_type, bbox) = {
        let p = &pieces[index];
        (p.kind.clone(), p.coord_base_mode, p.component_type, p.bbox)
    };

    match kind {
        PieceKind::MineshaftCorridor { .. } => corridor_build(
            pieces,
            rand,
            bbox,
            facing.expect("corridor facing"),
            component_type,
            parent_bbox,
        ),
        PieceKind::MineshaftCross {
            direction,
            multiple_floors,
        } => cross_build(
            pieces,
            rand,
            bbox,
            direction,
            multiple_floors,
            component_type,
            parent_bbox,
        ),
        PieceKind::MineshaftRoom { .. } => {
            room_build(index, pieces, rand, bbox, component_type, parent_bbox)
        }
        PieceKind::MineshaftStairs => stairs_build(
            pieces,
            rand,
            bbox,
            facing.expect("stairs facing"),
            component_type,
            parent_bbox,
        ),
    }
}

/// `Corridor.buildComponent`. Vanilla draws the exit position inside the call's argument list; the
/// draws are hoisted here, in the same order, because Rust cannot borrow `rand` twice.
fn corridor_build(
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    bbox: BoundingBox,
    facing: Facing,
    depth: i32,
    parent_bbox: BoundingBox,
) {
    let roll = rand.next_int_bound(4);
    let (x, z, child_facing) = match (facing, roll) {
        (Facing::North, 0..=1) => (bbox.min_x, bbox.min_z - 1, Facing::North),
        (Facing::North, 2) => (bbox.min_x - 1, bbox.min_z, Facing::West),
        (Facing::North, _) => (bbox.max_x + 1, bbox.min_z, Facing::East),
        (Facing::South, 0..=1) => (bbox.min_x, bbox.max_z + 1, Facing::South),
        (Facing::South, 2) => (bbox.min_x - 1, bbox.max_z - 3, Facing::West),
        (Facing::South, _) => (bbox.max_x + 1, bbox.max_z - 3, Facing::East),
        (Facing::West, 0..=1) => (bbox.min_x - 1, bbox.min_z, Facing::West),
        (Facing::West, 2) => (bbox.min_x, bbox.min_z - 1, Facing::North),
        (Facing::West, _) => (bbox.min_x, bbox.max_z + 1, Facing::South),
        (Facing::East, 0..=1) => (bbox.max_x + 1, bbox.min_z, Facing::East),
        (Facing::East, 2) => (bbox.max_x - 3, bbox.min_z - 1, Facing::North),
        (Facing::East, _) => (bbox.max_x - 3, bbox.max_z + 1, Facing::South),
    };
    let y = bbox.min_y - 1 + rand.next_int_bound(3);
    add_piece(parent_bbox, pieces, rand, x, y, z, child_facing, depth);

    if depth < 8 {
        if facing == Facing::North || facing == Facing::South {
            let mut z = bbox.min_z + 3;
            while z + 3 <= bbox.max_z {
                let roll = rand.next_int_bound(5);
                if roll == 0 {
                    add_piece(
                        parent_bbox,
                        pieces,
                        rand,
                        bbox.min_x - 1,
                        bbox.min_y,
                        z,
                        Facing::West,
                        depth + 1,
                    );
                } else if roll == 1 {
                    add_piece(
                        parent_bbox,
                        pieces,
                        rand,
                        bbox.max_x + 1,
                        bbox.min_y,
                        z,
                        Facing::East,
                        depth + 1,
                    );
                }
                z += 5;
            }
        } else {
            let mut x = bbox.min_x + 3;
            while x + 3 <= bbox.max_x {
                let roll = rand.next_int_bound(5);
                if roll == 0 {
                    add_piece(
                        parent_bbox,
                        pieces,
                        rand,
                        x,
                        bbox.min_y,
                        bbox.min_z - 1,
                        Facing::North,
                        depth + 1,
                    );
                } else if roll == 1 {
                    add_piece(
                        parent_bbox,
                        pieces,
                        rand,
                        x,
                        bbox.min_y,
                        bbox.max_z + 1,
                        Facing::South,
                        depth + 1,
                    );
                }
                x += 5;
            }
        }
    }
}

/// `Cross.buildComponent`.
#[allow(clippy::too_many_arguments)]
fn cross_build(
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    bbox: BoundingBox,
    facing: Facing,
    multiple_floors: bool,
    depth: i32,
    parent_bbox: BoundingBox,
) {
    match facing {
        Facing::North => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.min_z - 1,
                Facing::North,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x - 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::West,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.max_x + 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::East,
                depth,
            );
        }
        Facing::South => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.max_z + 1,
                Facing::South,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x - 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::West,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.max_x + 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::East,
                depth,
            );
        }
        Facing::West => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.min_z - 1,
                Facing::North,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.max_z + 1,
                Facing::South,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x - 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::West,
                depth,
            );
        }
        Facing::East => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.min_z - 1,
                Facing::North,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y,
                bbox.max_z + 1,
                Facing::South,
                depth,
            );
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.max_x + 1,
                bbox.min_y,
                bbox.min_z + 1,
                Facing::East,
                depth,
            );
        }
    }

    if multiple_floors {
        if rand.next_boolean() {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y + 4,
                bbox.min_z - 1,
                Facing::North,
                depth,
            );
        }
        if rand.next_boolean() {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x - 1,
                bbox.min_y + 4,
                bbox.min_z + 1,
                Facing::West,
                depth,
            );
        }
        if rand.next_boolean() {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.max_x + 1,
                bbox.min_y + 4,
                bbox.min_z + 1,
                Facing::East,
                depth,
            );
        }
        if rand.next_boolean() {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x + 1,
                bbox.min_y + 4,
                bbox.max_z + 1,
                Facing::South,
                depth,
            );
        }
    }
}

/// `Room.buildComponent`: four sweeps (north, south, west, east) that step by
/// `rand.nextInt(size)` and link each accepted child back to the room.
fn room_build(
    index: usize,
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    bbox: BoundingBox,
    depth: i32,
    parent_bbox: BoundingBox,
) {
    let mut y_span = bbox.y_size() - 3 - 1;
    if y_span <= 0 {
        y_span = 1;
    }

    let mut pos = 0;
    while pos < bbox.x_size() {
        pos += rand.next_int_bound(bbox.x_size());
        if pos + 3 > bbox.x_size() {
            break;
        }
        let y = bbox.min_y + rand.next_int_bound(y_span) + 1;
        if let Some(child) = add_piece(
            parent_bbox,
            pieces,
            rand,
            bbox.min_x + pos,
            y,
            bbox.min_z - 1,
            Facing::North,
            depth,
        ) {
            let cb = pieces[child].bbox;
            link_room(
                pieces,
                index,
                BoundingBox::new(
                    cb.min_x,
                    cb.min_y,
                    bbox.min_z,
                    cb.max_x,
                    cb.max_y,
                    bbox.min_z + 1,
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < bbox.x_size() {
        pos += rand.next_int_bound(bbox.x_size());
        if pos + 3 > bbox.x_size() {
            break;
        }
        let y = bbox.min_y + rand.next_int_bound(y_span) + 1;
        if let Some(child) = add_piece(
            parent_bbox,
            pieces,
            rand,
            bbox.min_x + pos,
            y,
            bbox.max_z + 1,
            Facing::South,
            depth,
        ) {
            let cb = pieces[child].bbox;
            link_room(
                pieces,
                index,
                BoundingBox::new(
                    cb.min_x,
                    cb.min_y,
                    bbox.max_z - 1,
                    cb.max_x,
                    cb.max_y,
                    bbox.max_z,
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < bbox.z_size() {
        pos += rand.next_int_bound(bbox.z_size());
        if pos + 3 > bbox.z_size() {
            break;
        }
        let y = bbox.min_y + rand.next_int_bound(y_span) + 1;
        if let Some(child) = add_piece(
            parent_bbox,
            pieces,
            rand,
            bbox.min_x - 1,
            y,
            bbox.min_z + pos,
            Facing::West,
            depth,
        ) {
            let cb = pieces[child].bbox;
            link_room(
                pieces,
                index,
                BoundingBox::new(
                    bbox.min_x,
                    cb.min_y,
                    cb.min_z,
                    bbox.min_x + 1,
                    cb.max_y,
                    cb.max_z,
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < bbox.z_size() {
        pos += rand.next_int_bound(bbox.z_size());
        if pos + 3 > bbox.z_size() {
            break;
        }
        let y = bbox.min_y + rand.next_int_bound(y_span) + 1;
        if let Some(child) = add_piece(
            parent_bbox,
            pieces,
            rand,
            bbox.max_x + 1,
            y,
            bbox.min_z + pos,
            Facing::East,
            depth,
        ) {
            let cb = pieces[child].bbox;
            link_room(
                pieces,
                index,
                BoundingBox::new(
                    bbox.max_x - 1,
                    cb.min_y,
                    cb.min_z,
                    bbox.max_x,
                    cb.max_y,
                    cb.max_z,
                ),
            );
        }
        pos += 4;
    }
}

/// `Room.roomsLinkedToTheRoom.add(...)`.
fn link_room(pieces: &mut [Piece], index: usize, linked: BoundingBox) {
    if let PieceKind::MineshaftRoom { linked_rooms } = &mut pieces[index].kind {
        linked_rooms.push(linked);
    }
}

/// `Stairs.buildComponent`.
fn stairs_build(
    pieces: &mut Vec<Piece>,
    rand: &mut JavaRandom,
    bbox: BoundingBox,
    facing: Facing,
    depth: i32,
    parent_bbox: BoundingBox,
) {
    match facing {
        Facing::North => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x,
                bbox.min_y,
                bbox.min_z - 1,
                Facing::North,
                depth,
            );
        }
        Facing::South => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x,
                bbox.min_y,
                bbox.max_z + 1,
                Facing::South,
                depth,
            );
        }
        Facing::West => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.min_x - 1,
                bbox.min_y,
                bbox.min_z,
                Facing::West,
                depth,
            );
        }
        Facing::East => {
            add_piece(
                parent_bbox,
                pieces,
                rand,
                bbox.max_x + 1,
                bbox.min_y,
                bbox.min_z,
                Facing::East,
                depth,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    fn bounds(p: &BoundingBox) -> (i32, i32, i32, i32, i32, i32) {
        (p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z)
    }

    /// The root Room's placement and the size of the tree it grows (oracle: seed 0, chunk (6, 12)).
    #[test]
    fn mineshaft_layout_seed0_chunk_6_12() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(6, 12, GenOptions::vanilla());

        assert_eq!(chunk.structures.len(), 1);
        let start = &chunk.structures[0];
        assert_eq!(start.id, "Mineshaft");
        assert_eq!((start.chunk_x, start.chunk_z), (6, 12));
        assert_eq!(bounds(&start.bbox), (59, 24, 129, 178, 50, 274));
        assert_eq!(start.pieces.len(), 187);

        let rooms = start
            .pieces
            .iter()
            .filter(|p| matches!(p.kind, PieceKind::MineshaftRoom { .. }))
            .count();
        let corridors = start
            .pieces
            .iter()
            .filter(|p| matches!(p.kind, PieceKind::MineshaftCorridor { .. }))
            .count();
        let crosses = start
            .pieces
            .iter()
            .filter(|p| matches!(p.kind, PieceKind::MineshaftCross { .. }))
            .count();
        let stairs = start
            .pieces
            .iter()
            .filter(|p| matches!(p.kind, PieceKind::MineshaftStairs))
            .count();
        assert_eq!((rooms, corridors, crosses, stairs), (1, 130, 43, 13));
    }

    /// The root Room has a null `coordBaseMode` (`O` -1) and links its three entrances
    /// (oracle: seed 0, chunk (100, -50)).
    #[test]
    fn mineshaft_room_entrances() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(100, -50, GenOptions::vanilla());
        let room = &chunk.structures[0].pieces[0];

        assert_eq!(bounds(&room.bbox), (1522, 40, -718, 1531, 44, -706));
        assert_eq!(room.coord_base_mode, None);
        assert_eq!(room.component_type, 0);

        let PieceKind::MineshaftRoom { linked_rooms } = &room.kind else {
            panic!("the root piece is a Room");
        };
        assert_eq!(linked_rooms.len(), 3);
        assert_eq!(bounds(&linked_rooms[0]), (1528, 41, -707, 1530, 43, -706));
        assert_eq!(bounds(&linked_rooms[1]), (1522, 41, -713, 1523, 43, -709));
        assert_eq!(bounds(&linked_rooms[2]), (1530, 41, -713, 1531, 43, -711));
    }

    /// The Room's first child: a south corridor with rails (oracle: seed 0, chunk (100, -50)).
    #[test]
    fn mineshaft_first_child_corridor() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(100, -50, GenOptions::vanilla());
        let piece = &chunk.structures[0].pieces[1];

        assert_eq!(bounds(&piece.bbox), (1528, 41, -705, 1530, 43, -686));
        assert_eq!(piece.component_type, 1);
        assert_eq!(piece.coord_base_mode, Some(Facing::South));
        assert!(matches!(
            piece.kind,
            PieceKind::MineshaftCorridor {
                has_rails: true,
                has_spiders: false,
                spawner_placed: false,
                section_count: 4,
            }
        ));
    }

    /// A multi-floor cross: `O` is -1 (its constructor never sets `coordBaseMode`) while its own
    /// direction is south (oracle: seed 0, chunk (100, -50), GD 8, D=0 tf=1).
    #[test]
    fn mineshaft_multi_floor_cross() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(100, -50, GenOptions::vanilla());
        let piece = chunk.structures[0]
            .pieces
            .iter()
            .find(|p| {
                matches!(
                    p.kind,
                    PieceKind::MineshaftCross {
                        multiple_floors: true,
                        ..
                    }
                )
            })
            .expect("a multi-floor cross");

        assert_eq!(bounds(&piece.bbox), (1549, 39, -680, 1553, 45, -676));
        assert_eq!(piece.component_type, 8);
        assert_eq!(piece.coord_base_mode, None);
        assert!(matches!(
            piece.kind,
            PieceKind::MineshaftCross {
                direction: Facing::South,
                ..
            }
        ));
    }

    /// A corridor with a spider spawner is one without rails (oracle: seed 0, chunk (100, -50),
    /// GD 5, O=3, Num=4 hr=0 sc=1).
    #[test]
    fn mineshaft_corridor_with_spiders() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(100, -50, GenOptions::vanilla());
        let piece = chunk.structures[0]
            .pieces
            .iter()
            .find(|p| {
                matches!(
                    p.kind,
                    PieceKind::MineshaftCorridor {
                        has_spiders: true,
                        ..
                    }
                )
            })
            .or_else(|| {
                chunk.structures[0].pieces.iter().find(|p| {
                    matches!(
                        p.kind,
                        PieceKind::MineshaftCorridor {
                            spawner_placed: false,
                            has_rails: false,
                            ..
                        }
                    )
                })
            })
            .expect("a corridor");

        // The first corridor with spiders in the tree is at this box (the search order is piece
        // order, so this is stable).
        assert_eq!(bounds(&piece.bbox), (1560, 39, -670, 1579, 41, -668));
        assert_eq!(piece.component_type, 5);
        assert_eq!(piece.coord_base_mode, Some(Facing::East));
        assert!(matches!(
            piece.kind,
            PieceKind::MineshaftCorridor {
                has_rails: false,
                has_spiders: true,
                section_count: 4,
                ..
            }
        ));
    }

    /// `EnumFacing.getHorizontalIndex()` is not the declaration order.
    #[test]
    fn facing_horizontal_indices() {
        assert_eq!(Facing::South.horizontal_index(), 0);
        assert_eq!(Facing::West.horizontal_index(), 1);
        assert_eq!(Facing::North.horizontal_index(), 2);
        assert_eq!(Facing::East.horizontal_index(), 3);
    }

    /// A chunk reports starts that originate in its neighbours, and the origin chunk reports the
    /// same start (oracle: seed 0, chunk (100, -50)'s start originates at (95, -45)).
    #[test]
    fn mineshaft_reported_from_a_neighbour_chunk() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(100, -50, GenOptions::vanilla());

        assert_eq!(chunk.structures.len(), 1);
        assert_eq!(
            (chunk.structures[0].chunk_x, chunk.structures[0].chunk_z),
            (95, -45)
        );

        let origin = gen.generate_chunk(95, -45, GenOptions::vanilla());
        assert_eq!(origin.structures.len(), 1);
        assert_eq!(origin.structures[0].bbox, chunk.structures[0].bbox);
    }

    /// A chunk can hold several structures, reported in chunk-coordinate order (oracle: seed 0,
    /// chunk (-49, 223) contains two).
    #[test]
    fn mineshaft_reports_multiple_starts() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-49, 223, GenOptions::vanilla());

        assert_eq!(chunk.structures.len(), 2);
        assert_eq!(
            (chunk.structures[0].chunk_x, chunk.structures[0].chunk_z),
            (-49, 223)
        );
        assert_eq!(
            (chunk.structures[1].chunk_x, chunk.structures[1].chunk_z),
            (-47, 222)
        );
        assert_eq!(chunk.structures[1].pieces.len(), 167);
    }

    /// Structures are off in `GenOptions::none()`.
    #[test]
    fn mineshafts_off_registers_nothing() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(6, 12, GenOptions::none());
        assert!(chunk.structures.is_empty());
    }
}
