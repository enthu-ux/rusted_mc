//! `MapGenNetherBridge` (nether fortresses): the registration + layout half.
//!
//! Unlike the other generators this one runs in the *nether* (`ChunkProviderHell`), not the overworld,
//! so it is a standalone generator rather than a field of [`crate::terrain::TerrainGen`]. It needs no
//! biome lookup: `canSpawnStructureAtCoords` re-seeds the `MapGenBase` stream with
//! `(chunk >> 4) ^ ((chunk >> 4) << 4) ^ worldSeed` and rolls a fixed pattern.
//!
//! The layout is the `StructureNetherBridgePieces` weighted piece graph. A `Start` (a `Crossing3`)
//! carries the primary/secondary `PieceWeight` lists; `getNextComponent{Normal,X,Z}` place a child via
//! a weighted pick (`func_175871_a`, 5 attempts, falling back to an `End`), and the root's pending
//! list is drained in random order (`MapGenNetherBridge.Start`). Finally `setRandomHeight(48, 70)`
//! drops the structure to a random height.
//!
//! Vanilla quirk replicated: when a child lands outside the 112-block window, `func_175870_a` builds
//! an `End` piece purely for its RNG draw and *discards* it (it is never added to the components).
//!
//! Block placement is not ported yet (layout only).

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{
    component_box, find_intersecting, BoundingBox, Facing, NetherKind, Piece, PieceKind,
    StructureStart,
};
use crate::mapgen;

/// `StructureNetherBridgePieces.PieceWeight`.
#[derive(Clone, Copy)]
struct Weight {
    kind: NetherKind,
    weight: i32,
    /// `field_78824_d` - 0 means unlimited.
    limit: i32,
    /// `field_78825_e` - may be chosen again even if it is the previous piece.
    repeat: bool,
    /// `field_78827_c`.
    spawned: i32,
}

const PRIMARY: [Weight; 6] = [
    Weight {
        kind: NetherKind::Straight,
        weight: 30,
        limit: 0,
        repeat: true,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Crossing3,
        weight: 10,
        limit: 4,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Crossing,
        weight: 10,
        limit: 4,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Stairs,
        weight: 10,
        limit: 3,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Throne,
        weight: 5,
        limit: 2,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Entrance,
        weight: 5,
        limit: 1,
        repeat: false,
        spawned: 0,
    },
];

const SECONDARY: [Weight; 7] = [
    Weight {
        kind: NetherKind::Corridor5,
        weight: 25,
        limit: 0,
        repeat: true,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Crossing2,
        weight: 15,
        limit: 5,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Corridor2,
        weight: 5,
        limit: 10,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Corridor,
        weight: 5,
        limit: 10,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Corridor3,
        weight: 10,
        limit: 3,
        repeat: true,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::Corridor4,
        weight: 7,
        limit: 2,
        repeat: false,
        spawned: 0,
    },
    Weight {
        kind: NetherKind::NetherStalkRoom,
        weight: 5,
        limit: 2,
        repeat: false,
        spawned: 0,
    },
];

/// `MapGenNetherBridge`: a `MapGenStructure` for nether fortresses.
pub struct NetherBridge {
    starts: BTreeMap<(i32, i32), StructureStart>,
    rand: JavaRandom,
}

impl Default for NetherBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl NetherBridge {
    pub fn new() -> NetherBridge {
        NetherBridge {
            starts: BTreeMap::new(),
            rand: JavaRandom::new(0),
        }
    }

    /// `MapGenStructure.recursiveGenerate` over `MapGenBase`'s neighbourhood.
    pub fn generate(&mut self, world_seed: i64, target_x: i32, target_z: i32) {
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
                if can_spawn_at_coords(rand, world_seed, nx, nz) {
                    starts.insert((nx, nz), start(rand, nx, nz));
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

/// `EnumFacing.Plane.HORIZONTAL.facings()` is `{NORTH, EAST, SOUTH, WEST}`.
fn horizontal_facing(rand: &mut JavaRandom) -> Facing {
    match rand.next_int_bound(4) {
        0 => Facing::North,
        1 => Facing::East,
        2 => Facing::South,
        _ => Facing::West,
    }
}

/// `MapGenNetherBridge.canSpawnStructureAtCoords`. Re-seeds `rand` (the `MapGenBase` stream) with
/// `(chunk >> 4) ^ ((chunk >> 4) << 4) ^ worldSeed`.
fn can_spawn_at_coords(rand: &mut JavaRandom, world_seed: i64, x: i32, z: i32) -> bool {
    let rx = x >> 4;
    let rz = z >> 4;
    rand.set_seed((rx as i64) ^ ((rz as i64) << 4) ^ world_seed);
    rand.next_int();
    if rand.next_int_bound(3) != 0 {
        return false;
    }
    // The second `nextInt(8)` is drawn only when the first coordinate matched.
    x == (rx << 4) + 4 + rand.next_int_bound(8) && z == (rz << 4) + 4 + rand.next_int_bound(8)
}

/// `MapGenNetherBridge.Start`: the root `Crossing3` and its drained pending list.
fn start(rand: &mut JavaRandom, chunk_x: i32, chunk_z: i32) -> StructureStart {
    let bx = (chunk_x << 4) + 2;
    let bz = (chunk_z << 4) + 2;
    let facing = horizontal_facing(rand);
    let root = Piece {
        kind: PieceKind::Nether {
            kind: NetherKind::Start,
            chest: false,
            seed: 0,
            has_spawner: false,
        },
        coord_base_mode: Some(facing),
        component_type: 0,
        bbox: BoundingBox::new(bx, 64, bz, bx + 19 - 1, 73, bz + 19 - 1),
    };

    let mut layout = Layout::new(root);
    layout.build_component(0, rand);
    while !layout.pending.is_empty() {
        let i = rand.next_int_bound(layout.pending.len() as i32) as usize;
        let index = layout.pending.remove(i);
        layout.build_component(index, rand);
    }

    let mut start = StructureStart {
        id: "Fortress",
        chunk_x,
        chunk_z,
        bbox: StructureStart::update_bbox(&layout.pieces),
        pieces: layout.pieces,
        sizeable: true,
    };
    start.set_random_height(rand, 48, 70);
    start
}

/// The mutable state of one fortress layout (vanilla threads it through `Start`'s fields + the two
/// component lists).
struct Layout {
    pieces: Vec<Piece>,
    pending: Vec<usize>,
    primary: Vec<Weight>,
    secondary: Vec<Weight>,
    last: Option<NetherKind>,
    root_x: i32,
    root_z: i32,
}

impl Layout {
    fn new(root: Piece) -> Layout {
        Layout {
            root_x: root.bbox.min_x,
            root_z: root.bbox.min_z,
            pieces: vec![root],
            pending: Vec::new(),
            primary: PRIMARY.to_vec(),
            secondary: SECONDARY.to_vec(),
            last: None,
        }
    }

    fn bbox_facing(&self, index: usize) -> (BoundingBox, Facing) {
        let p = &self.pieces[index];
        (p.bbox, p.coord_base_mode.expect("nether piece facing"))
    }

    /// `StructureComponent.buildComponent`, dispatched on the piece kind.
    fn build_component(&mut self, index: usize, rand: &mut JavaRandom) {
        let kind = match &self.pieces[index].kind {
            PieceKind::Nether { kind, .. } => *kind,
            _ => return,
        };
        match kind {
            NetherKind::Start | NetherKind::Crossing3 => {
                self.next_normal(index, rand, 8, 3, false);
                self.next_x(index, rand, 3, 8, false);
                self.next_z(index, rand, 3, 8, false);
            }
            NetherKind::Straight => self.next_normal(index, rand, 1, 3, false),
            NetherKind::Crossing => {
                self.next_normal(index, rand, 2, 0, false);
                self.next_x(index, rand, 0, 2, false);
                self.next_z(index, rand, 0, 2, false);
            }
            NetherKind::Corridor => self.next_x(index, rand, 0, 1, true),
            NetherKind::Corridor2 => self.next_z(index, rand, 0, 1, true),
            NetherKind::Corridor3 => self.next_normal(index, rand, 1, 0, true),
            NetherKind::Corridor4 => {
                let (_, facing) = self.bbox_facing(index);
                let x_off = if facing == Facing::West || facing == Facing::North {
                    5
                } else {
                    1
                };
                // The two `nextInt(8)` draws are evaluated in argument order, so the X draw (and
                // its whole subtree) happens before the Z draw.
                let x_above = rand.next_int_bound(8) > 0;
                self.next_x(index, rand, 0, x_off, x_above);
                let z_above = rand.next_int_bound(8) > 0;
                self.next_z(index, rand, 0, x_off, z_above);
            }
            NetherKind::Corridor5 => self.next_normal(index, rand, 1, 0, true),
            NetherKind::Crossing2 => {
                self.next_normal(index, rand, 1, 0, true);
                self.next_x(index, rand, 0, 1, true);
                self.next_z(index, rand, 0, 1, true);
            }
            NetherKind::Entrance => self.next_normal(index, rand, 5, 3, true),
            NetherKind::NetherStalkRoom => {
                self.next_normal(index, rand, 5, 3, true);
                self.next_normal(index, rand, 5, 11, true);
            }
            NetherKind::Stairs => self.next_z(index, rand, 6, 2, false),
            NetherKind::End | NetherKind::Throne => {}
        }
    }

    /// `getNextComponentNormal`.
    fn next_normal(
        &mut self,
        from: usize,
        rand: &mut JavaRandom,
        x_off: i32,
        y_off: i32,
        above: bool,
    ) {
        let (bbox, facing) = self.bbox_facing(from);
        let depth = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North => (
                bbox.min_x + x_off,
                bbox.min_y + y_off,
                bbox.min_z - 1,
                facing,
            ),
            Facing::South => (
                bbox.min_x + x_off,
                bbox.min_y + y_off,
                bbox.max_z + 1,
                facing,
            ),
            Facing::West => (
                bbox.min_x - 1,
                bbox.min_y + y_off,
                bbox.min_z + x_off,
                facing,
            ),
            Facing::East => (
                bbox.max_x + 1,
                bbox.min_y + y_off,
                bbox.min_z + x_off,
                facing,
            ),
        };
        self.grow(rand, x, y, z, f, depth, above);
    }

    /// `getNextComponentX`.
    fn next_x(&mut self, from: usize, rand: &mut JavaRandom, x_off: i32, y_off: i32, above: bool) {
        let (bbox, facing) = self.bbox_facing(from);
        let depth = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => (
                bbox.min_x - 1,
                bbox.min_y + x_off,
                bbox.min_z + y_off,
                Facing::West,
            ),
            Facing::West | Facing::East => (
                bbox.min_x + y_off,
                bbox.min_y + x_off,
                bbox.min_z - 1,
                Facing::North,
            ),
        };
        self.grow(rand, x, y, z, f, depth, above);
    }

    /// `getNextComponentZ`.
    fn next_z(&mut self, from: usize, rand: &mut JavaRandom, x_off: i32, y_off: i32, above: bool) {
        let (bbox, facing) = self.bbox_facing(from);
        let depth = self.pieces[from].component_type;
        let (x, y, z, f) = match facing {
            Facing::North | Facing::South => (
                bbox.max_x + 1,
                bbox.min_y + x_off,
                bbox.min_z + y_off,
                Facing::East,
            ),
            Facing::West | Facing::East => (
                bbox.min_x + y_off,
                bbox.min_y + x_off,
                bbox.max_z + 1,
                Facing::South,
            ),
        };
        self.grow(rand, x, y, z, f, depth, above);
    }

    /// `func_175870_a`.
    #[allow(clippy::too_many_arguments)]
    fn grow(
        &mut self,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
        above: bool,
    ) {
        if (x - self.root_x).abs() <= 112 && (z - self.root_z).abs() <= 112 {
            let child = self.weighted_pick(above, rand, x, y, z, facing, depth + 1);
            if let Some(piece) = child {
                let index = self.pieces.len();
                self.pieces.push(piece);
                self.pending.push(index);
            }
        } else {
            // Vanilla constructs an `End` here purely for its RNG draw and discards it.
            let _ = self.create_piece(NetherKind::End, rand, x, y, z, facing, depth);
        }
    }

    /// `Piece.func_175871_a`: weighted selection (5 attempts, then an `End`).
    #[allow(clippy::too_many_arguments)]
    fn weighted_pick(
        &mut self,
        above: bool,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        let list = if above {
            &self.secondary
        } else {
            &self.primary
        };
        let total = get_total_weight(list);
        let can = total > 0 && depth <= 30;
        let mut attempts = 0;
        while attempts < 5 && can {
            attempts += 1;
            let mut roll = rand.next_int_bound(total);
            let mut index = 0;
            loop {
                let len = if above {
                    self.secondary.len()
                } else {
                    self.primary.len()
                };
                if index >= len {
                    break;
                }
                let w = if above {
                    self.secondary[index]
                } else {
                    self.primary[index]
                };
                roll -= w.weight;
                if roll < 0 {
                    if !can_spawn_weight(&w) || (self.last == Some(w.kind) && !w.repeat) {
                        break;
                    }
                    if let Some(piece) = self.create_piece(w.kind, rand, x, y, z, facing, depth) {
                        let list = if above {
                            &mut self.secondary
                        } else {
                            &mut self.primary
                        };
                        list[index].spawned += 1;
                        self.last = Some(w.kind);
                        if !can_spawn_weight(&list[index]) {
                            list.remove(index);
                        }
                        return Some(piece);
                    }
                }
                index += 1;
            }
        }
        self.create_piece(NetherKind::End, rand, x, y, z, facing, depth)
    }

    /// `func_175887_b` + the piece factories: build a child piece (`isAboveGround` + disjoint), or
    /// `None` when it does not fit.
    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        &self,
        kind: NetherKind,
        rand: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: Facing,
        depth: i32,
    ) -> Option<Piece> {
        // (offsetX, offsetY, offsetZ, sizeX, sizeY, sizeZ)
        let (ox, oy, oz, sx, sy, sz) = match kind {
            NetherKind::Straight => (-1, -3, 0, 5, 10, 19),
            NetherKind::Crossing3 | NetherKind::Start => (-8, -3, 0, 19, 10, 19),
            NetherKind::Crossing => (-2, 0, 0, 7, 9, 7),
            NetherKind::Stairs => (-2, 0, 0, 7, 11, 7),
            NetherKind::Throne => (-2, 0, 0, 7, 8, 9),
            NetherKind::Entrance | NetherKind::NetherStalkRoom => (-5, -3, 0, 13, 14, 13),
            NetherKind::Corridor5
            | NetherKind::Crossing2
            | NetherKind::Corridor2
            | NetherKind::Corridor => (-1, 0, 0, 5, 7, 5),
            NetherKind::Corridor3 => (-1, -7, 0, 5, 14, 10),
            NetherKind::Corridor4 => (-3, 0, 0, 9, 7, 9),
            NetherKind::End => (-1, -3, 0, 5, 10, 8),
        };
        let bbox = component_box(x, y, z, ox, oy, oz, sx, sy, sz, facing);
        if bbox.min_y <= 10 || find_intersecting(&self.pieces, &bbox).is_some() {
            return None;
        }
        // Kind-specific constructor draws (after the fit checks).
        let (chest, seed) = match kind {
            NetherKind::Corridor | NetherKind::Corridor2 => (rand.next_int_bound(3) == 0, 0),
            NetherKind::End => (false, rand.next_int()),
            _ => (false, 0),
        };
        Some(Piece {
            kind: PieceKind::Nether {
                kind,
                chest,
                seed,
                // `Throne.hasSpawner` is only set during block placement.
                has_spawner: false,
            },
            coord_base_mode: Some(facing),
            component_type: depth,
            bbox,
        })
    }
}

/// `getTotalWeight`: the sum of the weights if any can still spawn, else -1.
fn get_total_weight(list: &[Weight]) -> i32 {
    let mut any = false;
    let mut sum = 0;
    for w in list {
        if w.limit > 0 && w.spawned < w.limit {
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

/// `func_78822_a` / `func_78823_a`: `limit == 0 || spawned < limit`.
fn can_spawn_weight(w: &Weight) -> bool {
    w.limit == 0 || w.spawned < w.limit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    fn bounds(p: &BoundingBox) -> (i32, i32, i32, i32, i32, i32) {
        (p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z)
    }

    /// Seed 0's fortress at chunk (-37,213) - 169 pieces, root `NeStart` facing EAST, and the whole
    /// structure dropped to `setRandomHeight(48,70)` (oracle: BB -709 48 3306 -520 73 3471).
    #[test]
    fn nether_bridge_golden_seed0() {
        let mut nb = NetherBridge::new();
        nb.generate(0, -37, 213);
        let start = nb
            .starts_intersecting(-37, 213)
            .into_iter()
            .find(|s| s.chunk_x == -37 && s.chunk_z == 213)
            .expect("seed 0 fortress at (-37,213)");
        assert_eq!(start.id, "Fortress");
        assert_eq!(bounds(&start.bbox), (-709, 48, 3306, -520, 73, 3471));
        assert_eq!(start.pieces.len(), 169);

        let root = &start.pieces[0];
        assert_eq!(root.kind.id(), "NeStart");
        assert_eq!(root.component_type, 0);
        assert_eq!(root.coord_base_mode, Some(Facing::East));
        assert_eq!(bounds(&root.bbox), (-590, 48, 3410, -572, 57, 3428));
    }

    /// Seed 0's fortress at chunk (91,-7) - root `NeStart` facing NORTH (oracle: pieces 153,
    /// BB 1367 48 -218 1547 72 10).
    #[test]
    fn nether_bridge_golden_seed0_second() {
        let mut nb = NetherBridge::new();
        nb.generate(0, 91, -7);
        let start = nb
            .starts_intersecting(91, -7)
            .into_iter()
            .find(|s| s.chunk_x == 91 && s.chunk_z == -7)
            .expect("seed 0 fortress at (91,-7)");
        assert_eq!(bounds(&start.bbox), (1367, 48, -218, 1547, 72, 10));
        assert_eq!(start.pieces.len(), 153);
        assert_eq!(start.pieces[0].coord_base_mode, Some(Facing::North));
    }

    /// The generator is deterministic per (seed, chunk).
    #[test]
    fn nether_bridge_layout_is_deterministic() {
        let mut a = NetherBridge::new();
        let mut b = NetherBridge::new();
        a.generate(0, -37, 213);
        b.generate(0, -37, 213);
        assert_eq!(
            a.starts_intersecting(-37, 213),
            b.starts_intersecting(-37, 213)
        );
    }

    /// `TerrainGen` (the overworld generator) never registers fortresses.
    #[test]
    fn overworld_has_no_fortresses() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(6, 12, GenOptions::vanilla());
        assert!(chunk.structures.iter().all(|s| s.id != "Fortress"));
    }
}
