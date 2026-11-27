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
//!    walks the registered starts whose (offset) bounding box meets the chunk and writes their pieces
//!    into the world (see `placement.rs`).
//!
//! `MapGenStructure.structureMap` is a `HashMap` in vanilla, so its iteration order must never leak
//! into anything observable - the dump sorts by chunk coordinate.

pub mod mineshaft;
pub mod monument;
pub mod monument_placement;
pub mod netherbridge;
pub mod netherbridge_placement;
pub mod placement;
pub mod scattered;
pub mod stronghold;
pub mod stronghold_placement;
pub mod village;
pub mod village_placement;

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

    /// `EnumFacing.getIndex()` (DOWN=0, UP=1, NORTH=2, SOUTH=3, WEST=4, EAST=5).
    pub fn index(self) -> i32 {
        match self {
            Facing::North => 2,
            Facing::South => 3,
            Facing::West => 4,
            Facing::East => 5,
        }
    }

    /// `EnumFacing.getOpposite()`.
    pub fn opposite(self) -> Facing {
        match self {
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::West => Facing::East,
            Facing::East => Facing::West,
        }
    }

    /// `EnumFacing.rotateY()` (clockwise seen from above).
    pub fn rotate_y(self) -> Facing {
        match self {
            Facing::North => Facing::East,
            Facing::East => Facing::South,
            Facing::South => Facing::West,
            Facing::West => Facing::North,
        }
    }

    /// `EnumFacing.rotateYCCW()` (counter-clockwise seen from above).
    pub fn rotate_y_ccw(self) -> Facing {
        match self {
            Facing::North => Facing::West,
            Facing::West => Facing::South,
            Facing::South => Facing::East,
            Facing::East => Facing::North,
        }
    }

    /// `getFrontOffsetX`/`getFrontOffsetZ` of a horizontal facing.
    pub fn offset(self) -> (i32, i32) {
        match self {
            Facing::North => (0, -1),
            Facing::South => (0, 1),
            Facing::West => (-1, 0),
            Facing::East => (1, 0),
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

/// The `StructureNetherBridgePieces` piece classes (the `MapGenNetherBridge` fortress).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetherKind {
    Straight,
    Crossing3,
    Crossing,
    Stairs,
    Throne,
    Entrance,
    Corridor5,
    Crossing2,
    Corridor2,
    Corridor,
    Corridor3,
    Corridor4,
    NetherStalkRoom,
    Start,
    End,
}

impl NetherKind {
    /// The `MapGenStructureIO` component name.
    pub fn id(self) -> &'static str {
        match self {
            NetherKind::Straight => "NeBS",
            NetherKind::Crossing3 => "NeBCr",
            NetherKind::Crossing => "NeRC",
            NetherKind::Stairs => "NeSR",
            NetherKind::Throne => "NeMT",
            NetherKind::Entrance => "NeCE",
            NetherKind::Corridor5 => "NeSC",
            NetherKind::Crossing2 => "NeSCSC",
            NetherKind::Corridor2 => "NeSCRT",
            NetherKind::Corridor => "NeSCLT",
            NetherKind::Corridor3 => "NeCCS",
            NetherKind::Corridor4 => "NeCTB",
            NetherKind::NetherStalkRoom => "NeCSR",
            NetherKind::Start => "NeStart",
            NetherKind::End => "NeBEF",
        }
    }
}

/// The `StructureVillagePieces` piece classes (the `MapGenVillage` village).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VillageKind {
    Well,
    Start,
    Torch,
    Church,
    Hall,
    House1,
    House2,
    House3,
    House4Garden,
    WoodHut,
    Field1,
    Field2,
    Path,
}

impl VillageKind {
    /// The `MapGenStructureIO` component name.
    pub fn id(self) -> &'static str {
        match self {
            VillageKind::Well => "ViW",
            VillageKind::Start => "ViStart",
            VillageKind::Torch => "ViL",
            VillageKind::Church => "ViST",
            VillageKind::Hall => "ViPH",
            VillageKind::House1 => "ViBH",
            VillageKind::House2 => "ViS",
            VillageKind::House3 => "ViTRH",
            VillageKind::House4Garden => "ViSH",
            VillageKind::WoodHut => "ViSmH",
            VillageKind::Field1 => "ViDF",
            VillageKind::Field2 => "ViF",
            VillageKind::Path => "ViSR",
        }
    }

    /// `StructureVillagePieces.Road` subclasses (only `Path`); `MapGenVillage.Start` counts these
    /// separately when deciding `isSizeableStructure`.
    pub fn is_road(self) -> bool {
        matches!(self, VillageKind::Path)
    }
}

/// A `StructureVillagePieces.Village` piece with its kind-specific serialized fields. `h_pos`
/// (`HPos`) is -1 and `v_count` (`VCount`) is 0 at layout time (both are set during block placement).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VillagePiece {
    pub kind: VillageKind,
    pub h_pos: i32,
    pub v_count: i32,
    pub desert: bool,
    /// `Field1`/`Field2` crop block ids (`CA`..`CD`; unused entries 0).
    pub crops: [i32; 4],
    pub chest: bool,
    pub terrace: bool,
    pub tall: bool,
    pub table: i32,
    pub length: i32,
}

/// The `StructureStrongholdPieces` piece classes (the `MapGenStronghold` stronghold).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StrongholdKind {
    Stairs2,
    Straight,
    Prison,
    LeftTurn,
    RightTurn,
    RoomCrossing,
    StairsStraight,
    Stairs,
    Crossing,
    ChestCorridor,
    Library,
    PortalRoom,
    Corridor,
}

impl StrongholdKind {
    /// The `MapGenStructureIO` component name.
    pub fn id(self) -> &'static str {
        match self {
            StrongholdKind::Stairs2 => "SHStart",
            StrongholdKind::Straight => "SHS",
            StrongholdKind::Prison => "SHPH",
            StrongholdKind::LeftTurn => "SHLT",
            StrongholdKind::RightTurn => "SHRT",
            StrongholdKind::RoomCrossing => "SHRC",
            StrongholdKind::StairsStraight => "SHSSD",
            StrongholdKind::Stairs => "SHSD",
            StrongholdKind::Crossing => "SH5C",
            StrongholdKind::ChestCorridor => "SHCC",
            StrongholdKind::Library => "SHLi",
            StrongholdKind::PortalRoom => "SHPR",
            StrongholdKind::Corridor => "SHFC",
        }
    }
}

/// `StructureStrongholdPieces.Stronghold.Door` (`EntryDoor`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Door {
    Opening,
    WoodDoor,
    Grates,
    IronDoor,
}

impl Door {
    /// The enum constant name, as written to NBT.
    pub fn name(self) -> &'static str {
        match self {
            Door::Opening => "OPENING",
            Door::WoodDoor => "WOOD_DOOR",
            Door::Grates => "GRATES",
            Door::IronDoor => "IRON_DOOR",
        }
    }
}

/// A `StructureStrongholdPieces.Stronghold` piece: the base `EntryDoor` plus the kind-specific
/// serialized fields.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StrongholdPiece {
    pub kind: StrongholdKind,
    pub door: Door,
    /// `Corridor` (`Steps`).
    pub steps: i32,
    /// `ChestCorridor` (`Chest`).
    pub chest: bool,
    /// `Crossing` (`leftLow`/`leftHigh`/`rightLow`/`rightHigh`).
    pub left_low: bool,
    pub left_high: bool,
    pub right_low: bool,
    pub right_high: bool,
    /// `Library` (`Tall`).
    pub tall: bool,
    /// `PortalRoom` (`Mob`).
    pub has_spawner: bool,
    /// `RoomCrossing` (`Type`).
    pub room_type: i32,
    /// `Stairs`/`Stairs2` (`Source`).
    pub source: bool,
    /// `Straight` (`Left`/`Right`).
    pub expands_x: bool,
    pub expands_z: bool,
}

impl Default for StrongholdPiece {
    fn default() -> Self {
        StrongholdPiece {
            kind: StrongholdKind::Straight,
            door: Door::Opening,
            steps: 0,
            chest: false,
            left_low: false,
            left_high: false,
            right_low: false,
            right_high: false,
            tall: false,
            has_spawner: false,
            room_type: 0,
            source: false,
            expands_x: false,
            expands_z: false,
        }
    }
}

impl Default for VillagePiece {
    fn default() -> Self {
        VillagePiece {
            kind: VillageKind::Well,
            h_pos: -1,
            v_count: 0,
            desert: false,
            crops: [0; 4],
            chest: false,
            terrace: false,
            tall: false,
            table: 0,
            length: 0,
        }
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
    /// `ComponentScatteredFeaturePieces.DesertPyramid` (`TeDP`).
    TempleDesertPyramid {
        size: [i32; 3],
        h_pos: i32,
        chests: [bool; 4],
    },
    /// `ComponentScatteredFeaturePieces.JunglePyramid` (`TeJP`).
    TempleJunglePyramid {
        size: [i32; 3],
        h_pos: i32,
        main_chest: bool,
        hidden_chest: bool,
        trap1: bool,
        trap2: bool,
    },
    /// `ComponentScatteredFeaturePieces.SwampHut` (`TeSH`).
    TempleSwampHut {
        size: [i32; 3],
        h_pos: i32,
        witch: bool,
    },
    /// `StructureOceanMonumentPieces.MonumentBuilding` (`OMB`). The monument's room pieces live in
    /// the building's own list (not the start's components), so only this one piece is serialized.
    MonumentBuilding,
    /// A `StructureNetherBridgePieces` fortress piece. `chest` (`Chest`), `seed` (`Seed`) and
    /// `has_spawner` (`Mob`) are the kind-specific serialized fields.
    Nether {
        kind: NetherKind,
        chest: bool,
        seed: i32,
        has_spawner: bool,
    },
    /// A `StructureVillagePieces.Village` piece.
    Village(VillagePiece),
    /// A `StructureStrongholdPieces.Stronghold` piece.
    Stronghold(StrongholdPiece),
}

impl PieceKind {
    /// The `MapGenStructureIO` name, as written to NBT as `id`.
    pub fn id(&self) -> &'static str {
        match self {
            PieceKind::MineshaftCorridor { .. } => "MSCorridor",
            PieceKind::MineshaftCross { .. } => "MSCrossing",
            PieceKind::MineshaftRoom { .. } => "MSRoom",
            PieceKind::MineshaftStairs => "MSStairs",
            PieceKind::TempleDesertPyramid { .. } => "TeDP",
            PieceKind::TempleJunglePyramid { .. } => "TeJP",
            PieceKind::TempleSwampHut { .. } => "TeSH",
            PieceKind::MonumentBuilding => "OMB",
            PieceKind::Nether { kind, .. } => kind.id(),
            PieceKind::Village(v) => v.kind.id(),
            PieceKind::Stronghold(s) => s.kind.id(),
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
    /// `StructureStart.isSizeableStructure()` - false for a village with <= 2 non-road pieces
    /// (such a start is never placed).
    pub sizeable: bool,
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
    pub(crate) fn mark_available_height(
        &mut self,
        rand: &mut JavaRandom,
        sea_level: i32,
        height: i32,
    ) {
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

    /// `StructureStart.setRandomHeight` (`MapGenNetherBridge.Start`): offset the whole structure so
    /// its `minY` lands in `[min, max]`, drawing at most one `nextInt`.
    pub(crate) fn set_random_height(&mut self, rand: &mut JavaRandom, min: i32, max: i32) {
        let span = max - min + 1 - self.bbox.y_size();
        let base = if span > 1 {
            min + rand.next_int_bound(span)
        } else {
            min
        };

        let dy = base - self.bbox.min_y;
        self.bbox.offset(0, dy, 0);
        for piece in &mut self.pieces {
            piece.bbox.offset(0, dy, 0);
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

/// `StructureBoundingBox.getComponentToAddBoundingBox`.
#[allow(clippy::too_many_arguments)]
pub fn component_box(
    x: i32,
    y: i32,
    z: i32,
    off_x: i32,
    off_y: i32,
    off_z: i32,
    size_x: i32,
    size_y: i32,
    size_z: i32,
    facing: Facing,
) -> BoundingBox {
    match facing {
        Facing::North => BoundingBox::new(
            x + off_x,
            y + off_y,
            z - size_z + 1 + off_z,
            x + size_x - 1 + off_x,
            y + size_y - 1 + off_y,
            z + off_z,
        ),
        Facing::South => BoundingBox::new(
            x + off_x,
            y + off_y,
            z + off_z,
            x + size_x - 1 + off_x,
            y + size_y - 1 + off_y,
            z + size_z - 1 + off_z,
        ),
        Facing::West => BoundingBox::new(
            x - size_z + 1 + off_z,
            y + off_y,
            z + off_x,
            x + off_z,
            y + size_y - 1 + off_y,
            z + size_x - 1 + off_x,
        ),
        Facing::East => BoundingBox::new(
            x + off_z,
            y + off_y,
            z + off_x,
            x + size_z - 1 + off_z,
            y + size_y - 1 + off_y,
            z + size_x - 1 + off_x,
        ),
    }
}
