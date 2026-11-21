//! 1.8.9 structure generation: `MapGenStructure`'s registry, the shared
//! `StructureStart` / `StructureComponent` / `StructureBoundingBox` model, and the piece kinds.
//!
//! Two phases, and they use *different* RNG streams:
//!
//! 1. **Registration** (`provideChunk` -> `MapGenBase.generate` -> `MapGenStructure.recursiveGenerate`):
//!    for every chunk in the 17x17 neighbourhood, if its structure start is not registered yet,
//!    draw a discard `rand.nextInt()`, ask `canSpawnStructureAtCoords`, and (if it says yes) build a
//!    `StructureStart` - which lays out the whole piece tree with that neighbour's stream. Nothing is
//!    written to the chunk here; the blocks come in phase 2.
//! 2. **Placement** (`populate` -> `MapGenStructure.generateStructure` -> `StructureStart.generateStructure`):
//!    walks the registered starts whose bounding box meets the chunk and writes their pieces into the
//!    world (see `handoff.md`; not ported yet).
//!
//! `MapGenStructure.structureMap` is a `HashMap` in vanilla, so its iteration order must never leak
//! into anything observable - the dump sorts by chunk coordinate.

pub mod mineshaft;

use mc_core::random::JavaRandom;

/// `EnumFacing`'s four horizontals, in `EnumFacing.getHorizontalIndex()` order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Facing {
    North,
    South,
    West,
    East,
}

impl Facing {
    /// `EnumFacing.getHorizontalIndex()`, as written to NBT as `O` / `D`. Not the declaration
    /// order: vanilla declares `NORTH(2, 3, 2)`, `SOUTH(3, 2, 0)`, `WEST(4, 5, 1)`, `EAST(5, 4, 3)`
    /// (index, opposite, horizontalIndex, ...).
    pub fn horizontal_index(self) -> i32 {
        match self {
            Facing::South => 0,
            Facing::West => 1,
            Facing::North => 2,
            Facing::East => 3,
        }
    }
}

/// `StructureBoundingBox` (inclusive on both ends).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BoundingBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl BoundingBox {
    pub fn new(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> BoundingBox {
        BoundingBox {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// `StructureBoundingBox.getNewBoundingBox()` - the empty box `StructureStart` expands from.
    pub fn empty() -> BoundingBox {
        BoundingBox::new(i32::MAX, i32::MAX, i32::MAX, i32::MIN, i32::MIN, i32::MIN)
    }

    /// `intersectsWith(StructureBoundingBox)` (all three axes).
    pub fn intersects_with(&self, other: &BoundingBox) -> bool {
        self.max_x >= other.min_x
            && self.min_x <= other.max_x
            && self.max_z >= other.min_z
            && self.min_z <= other.max_z
            && self.max_y >= other.min_y
            && self.min_y <= other.max_y
    }

    /// `intersectsWith(int, int, int, int)` - the X/Z-only form used by `generateStructure`.
    pub fn intersects_with_xz(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        self.max_x >= x0 && self.min_x <= x1 && self.max_z >= z0 && self.min_z <= z1
    }

    /// `expandTo`.
    pub fn expand_to(&mut self, other: &BoundingBox) {
        self.min_x = self.min_x.min(other.min_x);
        self.min_y = self.min_y.min(other.min_y);
        self.min_z = self.min_z.min(other.min_z);
        self.max_x = self.max_x.max(other.max_x);
        self.max_y = self.max_y.max(other.max_y);
        self.max_z = self.max_z.max(other.max_z);
    }

    /// `offset`.
    pub fn offset(&mut self, x: i32, y: i32, z: i32) {
        self.min_x += x;
        self.min_y += y;
        self.min_z += z;
        self.max_x += x;
        self.max_y += y;
        self.max_z += z;
    }

    /// `isVecInside`.
    pub fn is_vec_inside(&self, x: i32, y: i32, z: i32) -> bool {
        x >= self.min_x
            && x <= self.max_x
            && z >= self.min_z
            && z <= self.max_z
            && y >= self.min_y
            && y <= self.max_y
    }

    /// `getXSize`.
    pub fn x_size(&self) -> i32 {
        self.max_x - self.min_x + 1
    }

    /// `getYSize`.
    pub fn y_size(&self) -> i32 {
        self.max_y - self.min_y + 1
    }

    /// `getZSize`.
    pub fn z_size(&self) -> i32 {
        self.max_z - self.min_z + 1
    }
}

/// The kind-specific state of a `StructureComponent`. Only the fields vanilla uses (and serializes)
/// are kept; `MineshaftRoom`'s linked boxes are runtime-only, used by its block placement.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PieceKind {
    /// `StructureMineshaftPieces.Corridor`.
    MineshaftCorridor {
        has_rails: bool,
        has_spiders: bool,
        spawner_placed: bool,
        section_count: i32,
    },
    /// `StructureMineshaftPieces.Cross`. Its constructor stores the direction in its own
    /// `corridorDirection` field and never sets `coordBaseMode`, so `O` is always -1 for a cross.
    MineshaftCross {
        direction: Facing,
        multiple_floors: bool,
    },
    /// `StructureMineshaftPieces.Room`.
    MineshaftRoom { linked_rooms: Vec<BoundingBox> },
    /// `StructureMineshaftPieces.Stairs`.
    MineshaftStairs,
}

impl PieceKind {
    /// The `MapGenStructureIO` name, as written to NBT as `id`.
    pub fn id(&self) -> &'static str {
        match self {
            PieceKind::MineshaftCorridor { .. } => "MSCorridor",
            PieceKind::MineshaftCross { .. } => "MSCrossing",
            PieceKind::MineshaftRoom { .. } => "MSRoom",
            PieceKind::MineshaftStairs => "MSStairs",
        }
    }
}

/// A `StructureComponent`: one piece of a structure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Piece {
    pub kind: PieceKind,
    /// `coordBaseMode` (`null` for the root `Room`, which is placed without rotation).
    pub coord_base_mode: Option<Facing>,
    /// `componentType` - the recursion depth, which caps `buildComponent` at 8.
    pub component_type: i32,
    pub bbox: BoundingBox,
}

/// A `StructureStart` (one structure, with its whole piece tree).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StructureStart {
    /// `MapGenStructureIO.getStructureStartName`.
    pub id: &'static str,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub bbox: BoundingBox,
    pub pieces: Vec<Piece>,
}

impl StructureStart {
    /// `updateBoundingBox`: the box covering every piece.
    pub fn update_bbox(pieces: &[Piece]) -> BoundingBox {
        let mut bbox = BoundingBox::empty();
        for p in pieces {
            bbox.expand_to(&p.bbox);
        }
        bbox
    }

    /// `markAvailableHeight`: raise the whole structure so its top sits `height` blocks under sea
    /// level, drawing at most one `nextInt` (and only when it is short enough to need lifting).
    fn mark_available_height(&mut self, rand: &mut JavaRandom, sea_level: i32, height: i32) {
        let span = sea_level - height;
        let mut size = self.bbox.y_size() + 1;
        if size < span {
            size += rand.next_int_bound(span - size);
        }

        let dy = size - self.bbox.max_y;
        self.bbox.offset(0, dy, 0);
        for piece in &mut self.pieces {
            // `StructureComponent.func_181138_a`
            piece.bbox.offset(0, dy, 0);
            if let PieceKind::MineshaftRoom { linked_rooms } = &mut piece.kind {
                for linked in linked_rooms.iter_mut() {
                    linked.offset(0, dy, 0);
                }
            }
        }
    }

    /// The starts whose box meets the 16x16 chunk area `generateStructure` would be given.
    pub fn intersects_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        let x = (chunk_x << 4) + 8;
        let z = (chunk_z << 4) + 8;
        self.bbox.intersects_with_xz(x, z, x + 15, z + 15)
    }
}

/// `StructureComponent.findIntersecting`.
pub fn find_intersecting(pieces: &[Piece], bbox: &BoundingBox) -> Option<usize> {
    pieces.iter().position(|p| p.bbox.intersects_with(bbox))
}
