//! `StructureVillagePieces.addComponentParts` bodies - the `populate` half of a village (the
//! `provideChunk` half is `village.rs`).
//!
//! Ported classes: the shared `Village` base (`func_175847_a` desert material substitution,
//! `getAverageGroundLevel`, `canVillageGoDeeper`, `setBlockState`/`fillWithBlocks`/
//! `replaceAirAndLiquidDownwards` overrides) plus `Well`, `Church`, `Field1`, `Field2`, `Hall`,
//! `House1`, `House2`, `House3`, `House4Garden`, `WoodHut`, `Torch` and `Path`. `Start` extends
//! `Well` and uses its body.
//!
//! **`HPos`.** A piece caches the average ground level on its first placement pass
//! (`field_143015_k`) and offsets its box then; later passes reuse it. The port stores it in
//! `VillagePiece::h_pos` and [`village_adjust_height`] runs before the body.
//!
//! **Block-update scope.** `ItemDoor.placeDoor` calls `notifyNeighborsOfStateChange`, and several
//! placed blocks have non-trivial `onBlockAdded` bodies (torch support, furnace facing, chest
//! facing). Those are block-update semantics, not placement. The oracle's `TestWorld` is a
//! *remote* world (so `Chunk.setBlockState` skips `onBlockAdded`) and no-ops
//! `notifyNeighborsOfStateChange` / `forceBlockUpdateTick`; the port writes the placed states and
//! stops. This is the village analogue of the mineshaft `onBlockAdded` note.

use mc_core::random::JavaRandom;

use super::placement::{
    door_meta_with_offset, facing_from_horizontal, ladder_meta_with_offset,
    stairs_meta_with_offset, ChunkWorld, Component,
};
use super::{BoundingBox, Facing, Piece, PieceKind, VillageKind, VillagePiece};
use crate::chunk::{is_full_block, material_is_air};
use crate::loot::chest::VILLAGE_BLACKSMITH_CHEST_CONTENT;

// Block ids (vanilla 1.8.9).
const AIR: u16 = 0;
const DIRT: u16 = 3 << 4;
const COBBLESTONE: u16 = 4 << 4;
const PLANKS: u16 = 5 << 4;
const FLOWING_WATER: u16 = 8 << 4;
const WATER: u16 = 9 << 4;
const FLOWING_LAVA: u16 = 10 << 4;
const GRAVEL: u16 = 13 << 4;
const LOG: u16 = 17 << 4;
const SANDSTONE: u16 = 24 << 4;
const WOOL: u16 = (35 << 4) | 15; // `EnumDyeColor.WHITE.getDyeDamage()`
const DOUBLE_STONE_SLAB: u16 = 43 << 4;
const STONE_SLAB: u16 = 44 << 4;
const BOOKSHELF: u16 = 47 << 4;
const TORCH: u16 = 50 << 4;
const OAK_STAIRS: u16 = 53 << 4;
const CRAFTING_TABLE: u16 = 58 << 4;
const FARMLAND: u16 = 60 << 4;
const FURNACE: u16 = 61 << 4;
const OAK_DOOR: u16 = 64 << 4;
const LADDER: u16 = 65 << 4;
const STONE_STAIRS: u16 = 67 << 4;
const WOODEN_PRESSURE_PLATE: u16 = 72 << 4;
const OAK_FENCE: u16 = 85 << 4;
const IRON_BARS: u16 = 101 << 4;
const GLASS_PANE: u16 = 102 << 4;
const SANDSTONE_STAIRS: u16 = 128 << 4;

/// `WorldProvider.getAverageGroundLevel()` = `seaLevel + 1` (63 + 1).
const AVERAGE_GROUND_LEVEL: i32 = 64;

/// `Village.func_175847_a`: the desert material substitution, applied to every written state.
pub(crate) fn desert_map(value: u16) -> u16 {
    if value == AIR {
        return value;
    }
    let id = value >> 4;
    let meta = value & 15;
    match id {
        17 | 162 => SANDSTONE,              // log / log2
        4 => SANDSTONE,                     // cobblestone (DEFAULT)
        5 => SANDSTONE | 2,                 // planks -> smooth sandstone
        53 | 67 => SANDSTONE_STAIRS | meta, // oak_stairs / stone_stairs (same FACING)
        13 => SANDSTONE,                    // gravel
        _ => value,
    }
}

/// A village component view: applies the desert substitution on every write.
struct VC<'a> {
    c: &'a Component,
    desert: bool,
}

impl<'a> VC<'a> {
    fn map(&self, value: u16) -> u16 {
        if self.desert {
            desert_map(value)
        } else {
            value
        }
    }

    fn set(&self, world: &mut ChunkWorld, value: u16, x: i32, y: i32, z: i32, chunk: &BoundingBox) {
        self.c
            .set_block_state(world, self.map(value), x, y, z, chunk);
    }

    #[allow(clippy::too_many_arguments)]
    fn fill(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        outer: u16,
        inner: u16,
        always: bool,
    ) {
        self.c.fill_with_blocks(
            world,
            chunk,
            x1,
            y1,
            z1,
            x2,
            y2,
            z2,
            self.map(outer),
            self.map(inner),
            always,
        );
    }

    fn replace_down(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        value: u16,
        x: i32,
        y: i32,
        z: i32,
    ) {
        self.c
            .replace_air_and_liquid_downwards(world, chunk, self.map(value), x, y, z);
    }

    fn get(&self, world: &ChunkWorld, x: i32, y: i32, z: i32, chunk: &BoundingBox) -> u16 {
        self.c.get_block_state_from_pos(world, x, y, z, chunk)
    }

    /// `StructureComponent.clearCurrentPositionBlocksUpwards` (writes air, no substitution).
    fn clear_upwards(&self, world: &mut ChunkWorld, x: i32, y: i32, z: i32, chunk: &BoundingBox) {
        let px = self.c.get_x_with_offset(x, z);
        let mut py = self.c.get_y_with_offset(y);
        let pz = self.c.get_z_with_offset(x, z);
        if !chunk.is_vec_inside(px, py, pz) {
            return;
        }
        while py < 255 && !material_is_air(world.get(px, py, pz)) {
            world.set(px, py, pz, AIR);
            py += 1;
        }
    }

    /// `StructureComponent.placeDoorCurrentPosition` (`ItemDoor.placeDoor`).
    fn door(
        &self,
        world: &mut ChunkWorld,
        x: i32,
        y: i32,
        z: i32,
        horizontal: Facing,
        chunk: &BoundingBox,
    ) {
        let px = self.c.get_x_with_offset(x, z);
        let py = self.c.get_y_with_offset(y);
        let pz = self.c.get_z_with_offset(x, z);
        if !chunk.is_vec_inside(px, py, pz) {
            return;
        }
        let facing = horizontal.rotate_y_ccw();
        let (rx, rz) = facing.rotate_y().offset();
        let (lx, lz) = facing.rotate_y_ccw().offset();
        let normal = |x: i32, y: i32, z: i32| is_full_block((world.get(x, y, z) >> 4) as i32);
        let is_door = |x: i32, y: i32, z: i32| (world.get(x, y, z) >> 4) as i32 == 64;
        let left_door = is_door(px + lx, py, pz + lz) || is_door(px + lx, py + 1, pz + lz);
        let right_door = is_door(px + rx, py, pz + rz) || is_door(px + rx, py + 1, pz + rz);
        let left_n = normal(px + lx, py, pz + lz) as i32 + normal(px + lx, py + 1, pz + lz) as i32;
        let right_n = normal(px + rx, py, pz + rz) as i32 + normal(px + rx, py + 1, pz + rz) as i32;
        let hinge_right = (left_door && !right_door) || right_n > left_n;
        let lower = OAK_DOOR | facing.rotate_y().horizontal_index() as u16;
        let upper = OAK_DOOR | (8 | if hinge_right { 1 } else { 0 });
        world.set(px, py, pz, lower);
        world.set(px, py + 1, pz, upper);
    }

    /// `Village.getAverageGroundLevel`.
    fn average_ground(&self, world: &ChunkWorld, bbox: &BoundingBox, chunk: &BoundingBox) -> i32 {
        let mut sum = 0i64;
        let mut count = 0i64;
        for z in bbox.min_z..=bbox.max_z {
            for x in bbox.min_x..=bbox.max_x {
                if chunk.is_vec_inside(x, 64, z) {
                    sum += world
                        .get_top_solid_or_liquid(x, z)
                        .max(AVERAGE_GROUND_LEVEL) as i64;
                    count += 1;
                }
            }
        }
        if count == 0 {
            -1
        } else {
            (sum / count) as i32
        }
    }
}

/// `Village.addComponentParts`'s `field_143015_k` prologue. Returns false when the piece has no
/// intersection with this pass (vanilla returns `true` without writing).
pub(crate) fn village_adjust_height(
    piece: &mut Piece,
    world: &ChunkWorld,
    chunk: &BoundingBox,
) -> bool {
    let kind = match &piece.kind {
        PieceKind::Village(v) => v.kind,
        _ => return true,
    };
    if kind == VillageKind::Path {
        return true;
    }
    let h_pos = match &piece.kind {
        PieceKind::Village(v) => v.h_pos,
        _ => -1,
    };
    if h_pos >= 0 {
        return true;
    }
    let vc = VC {
        c: &Component::new(piece),
        desert: false,
    };
    let h = vc.average_ground(world, &piece.bbox, chunk);
    if h < 0 {
        return false;
    }
    let dy = if matches!(kind, VillageKind::Start | VillageKind::Well) {
        h - piece.bbox.max_y + 3
    } else {
        h - piece.bbox.max_y + piece.bbox.y_size() - 1
    };
    piece.bbox.offset(0, dy, 0);
    if let PieceKind::Village(v) = &mut piece.kind {
        v.h_pos = h;
    }
    true
}

/// `StructureVillagePieces.Village.addComponentParts`, dispatched on the piece kind.
pub(crate) fn village_parts(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    v: &mut VillagePiece,
) -> bool {
    let vc = VC {
        c: comp,
        desert: v.desert,
    };
    match v.kind {
        VillageKind::Start | VillageKind::Well => well(&vc, world, chunk),
        VillageKind::Church => church(&vc, world, chunk),
        VillageKind::Field1 => field1(&vc, world, rand, chunk, &v.crops),
        VillageKind::Field2 => field2(&vc, world, rand, chunk, &v.crops),
        VillageKind::Hall => hall(&vc, world, chunk),
        VillageKind::House1 => house1(&vc, world, chunk),
        VillageKind::House2 => house2(&vc, world, rand, chunk, &mut v.chest),
        VillageKind::House3 => house3(&vc, world, chunk),
        VillageKind::House4Garden => house4_garden(&vc, world, chunk, v.terrace),
        VillageKind::WoodHut => wood_hut(&vc, world, chunk, v.tall, v.table),
        VillageKind::Torch => torch_piece(&vc, world, chunk),
        VillageKind::Path => path(&vc, world, chunk),
    }
    true
}

/// A stair block state with `getMetadataWithOffset`.
fn stair(comp: &Component, id: u16, meta: i32) -> u16 {
    id | stairs_meta_with_offset(comp.facing, meta) as u16
}

fn torch(facing: Facing) -> u16 {
    // `BlockTorch.getMetaFromState`: EAST 1, WEST 2, SOUTH 3, NORTH 4, UP 5.
    let meta = match facing {
        Facing::East => 1,
        Facing::West => 2,
        Facing::South => 3,
        Facing::North => 4,
    };
    TORCH | meta
}

fn ladder(comp: &Component, index: i32) -> u16 {
    LADDER | ladder_meta_with_offset(comp.facing, index) as u16
}

/// `Well.addComponentParts` (`ViW` / `ViStart`).
fn well(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    vc.fill(
        world,
        chunk,
        1,
        0,
        1,
        4,
        12,
        4,
        COBBLESTONE,
        FLOWING_WATER,
        false,
    );
    vc.set(world, AIR, 2, 12, 2, chunk);
    vc.set(world, AIR, 3, 12, 2, chunk);
    vc.set(world, AIR, 2, 12, 3, chunk);
    vc.set(world, AIR, 3, 12, 3, chunk);
    for x in [1, 4] {
        for z in [1, 4] {
            vc.set(world, OAK_FENCE, x, 13, z, chunk);
            vc.set(world, OAK_FENCE, x, 14, z, chunk);
        }
    }
    vc.fill(
        world,
        chunk,
        1,
        15,
        1,
        4,
        15,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    for x in 0..=5 {
        for z in 0..=5 {
            if x == 0 || x == 5 || z == 0 || z == 5 {
                vc.set(world, GRAVEL, x, 11, z, chunk);
                vc.clear_upwards(world, x, 12, z, chunk);
            }
        }
    }
}

/// `Church.addComponentParts` (`ViST`).
fn church(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 1, 1, 1, 3, 3, 7, AIR, AIR, false);
    vc.fill(world, chunk, 1, 5, 1, 3, 9, 3, AIR, AIR, false);
    vc.fill(
        world,
        chunk,
        1,
        0,
        0,
        3,
        0,
        8,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        1,
        0,
        3,
        10,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        1,
        1,
        0,
        10,
        3,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        4,
        1,
        1,
        4,
        10,
        3,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        0,
        4,
        0,
        4,
        7,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        4,
        0,
        4,
        4,
        4,
        7,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        1,
        8,
        3,
        4,
        8,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        5,
        4,
        3,
        10,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        5,
        5,
        3,
        5,
        7,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        9,
        0,
        4,
        9,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        4,
        0,
        4,
        4,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    for (x, y, z) in [(0, 11, 2), (4, 11, 2), (2, 11, 0), (2, 11, 4)] {
        vc.set(world, COBBLESTONE, x, y, z, chunk);
    }
    for (x, y, z) in [(1, 1, 6), (1, 1, 7), (2, 1, 7), (3, 1, 6), (3, 1, 7)] {
        vc.set(world, COBBLESTONE, x, y, z, chunk);
    }
    vc.set(world, stair(vc.c, STONE_STAIRS, 3), 1, 1, 5, chunk);
    vc.set(world, stair(vc.c, STONE_STAIRS, 3), 2, 1, 6, chunk);
    vc.set(world, stair(vc.c, STONE_STAIRS, 3), 3, 1, 5, chunk);
    vc.set(world, stair(vc.c, STONE_STAIRS, 1), 1, 2, 7, chunk);
    vc.set(world, stair(vc.c, STONE_STAIRS, 0), 3, 2, 7, chunk);
    for (x, y, z) in [
        (0, 2, 2),
        (0, 3, 2),
        (4, 2, 2),
        (4, 3, 2),
        (0, 6, 2),
        (0, 7, 2),
        (4, 6, 2),
        (4, 7, 2),
        (2, 6, 0),
        (2, 7, 0),
        (2, 6, 4),
        (2, 7, 4),
        (0, 3, 6),
        (4, 3, 6),
        (2, 3, 8),
    ] {
        vc.set(world, GLASS_PANE, x, y, z, chunk);
    }
    vc.set(world, torch(f.opposite()), 2, 4, 7, chunk);
    vc.set(world, torch(f.rotate_y()), 1, 4, 6, chunk);
    vc.set(world, torch(f.rotate_y_ccw()), 3, 4, 6, chunk);
    vc.set(world, torch(f), 2, 4, 5, chunk);
    let l = ladder(vc.c, 4);
    for y in 1..=9 {
        vc.set(world, l, 3, y, 3, chunk);
    }
    vc.set(world, AIR, 2, 1, 0, chunk);
    vc.set(world, AIR, 2, 2, 0, chunk);
    vc.door(
        world,
        2,
        1,
        0,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    if material_is_air(vc.get(world, 2, 0, -1, chunk))
        && !material_is_air(vc.get(world, 2, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 2, 0, -1, chunk);
    }
    for z in 0..9 {
        for x in 0..5 {
            vc.clear_upwards(world, x, 12, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// The `Field1`/`Field2` crop columns (one `getRandomIntegerInRange(2, 7)` per position).
fn crop(value: u16, rand: &mut JavaRandom) -> u16 {
    value | (2 + rand.next_int_bound(6)) as u16
}

/// `Field1.addComponentParts` (`ViDF`).
fn field1(
    vc: &VC,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    crops: &[i32; 4],
) {
    let a = (crops[0] as u16) << 4;
    let b = (crops[1] as u16) << 4;
    let c = (crops[2] as u16) << 4;
    let d = (crops[3] as u16) << 4;
    vc.fill(world, chunk, 0, 1, 0, 12, 4, 8, AIR, AIR, false);
    for (x1, x2) in [(1, 2), (4, 5), (7, 8), (10, 11)] {
        vc.fill(world, chunk, x1, 0, 1, x2, 0, 7, FARMLAND, FARMLAND, false);
    }
    for x in [0, 6, 12] {
        vc.fill(world, chunk, x, 0, 0, x, 0, 8, LOG, LOG, false);
    }
    vc.fill(world, chunk, 1, 0, 0, 11, 0, 0, LOG, LOG, false);
    vc.fill(world, chunk, 1, 0, 8, 11, 0, 8, LOG, LOG, false);
    for x in [3, 9] {
        vc.fill(world, chunk, x, 0, 1, x, 0, 7, WATER, WATER, false);
    }
    for z in 1..=7 {
        for (x, value) in [
            (1, a),
            (2, a),
            (4, b),
            (5, b),
            (7, c),
            (8, c),
            (10, d),
            (11, d),
        ] {
            vc.set(world, crop(value, rand), x, 1, z, chunk);
        }
    }
    for z in 0..9 {
        for x in 0..13 {
            vc.clear_upwards(world, x, 4, z, chunk);
            vc.replace_down(world, chunk, DIRT, x, -1, z);
        }
    }
}

/// `Field2.addComponentParts` (`ViF`).
fn field2(
    vc: &VC,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    crops: &[i32; 4],
) {
    let a = (crops[0] as u16) << 4;
    let b = (crops[1] as u16) << 4;
    vc.fill(world, chunk, 0, 1, 0, 6, 4, 8, AIR, AIR, false);
    vc.fill(world, chunk, 1, 0, 1, 2, 0, 7, FARMLAND, FARMLAND, false);
    vc.fill(world, chunk, 4, 0, 1, 5, 0, 7, FARMLAND, FARMLAND, false);
    vc.fill(world, chunk, 0, 0, 0, 0, 0, 8, LOG, LOG, false);
    vc.fill(world, chunk, 6, 0, 0, 6, 0, 8, LOG, LOG, false);
    vc.fill(world, chunk, 1, 0, 0, 5, 0, 0, LOG, LOG, false);
    vc.fill(world, chunk, 1, 0, 8, 5, 0, 8, LOG, LOG, false);
    vc.fill(world, chunk, 3, 0, 1, 3, 0, 7, WATER, WATER, false);
    for z in 1..=7 {
        for (x, value) in [(1, a), (2, a), (4, b), (5, b)] {
            vc.set(world, crop(value, rand), x, 1, z, chunk);
        }
    }
    for z in 0..9 {
        for x in 0..7 {
            vc.clear_upwards(world, x, 4, z, chunk);
            vc.replace_down(world, chunk, DIRT, x, -1, z);
        }
    }
}

/// `Hall.addComponentParts` (`ViPH`).
fn hall(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 1, 1, 1, 7, 4, 4, AIR, AIR, false);
    vc.fill(world, chunk, 2, 1, 6, 8, 4, 10, AIR, AIR, false);
    vc.fill(world, chunk, 2, 0, 6, 8, 0, 10, DIRT, DIRT, false);
    vc.set(world, COBBLESTONE, 6, 0, 6, chunk);
    vc.fill(world, chunk, 2, 1, 6, 2, 1, 10, OAK_FENCE, OAK_FENCE, false);
    vc.fill(world, chunk, 8, 1, 6, 8, 1, 10, OAK_FENCE, OAK_FENCE, false);
    vc.fill(
        world, chunk, 3, 1, 10, 7, 1, 10, OAK_FENCE, OAK_FENCE, false,
    );
    vc.fill(world, chunk, 1, 0, 1, 7, 0, 4, PLANKS, PLANKS, false);
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        0,
        3,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        8,
        0,
        0,
        8,
        3,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        0,
        0,
        7,
        1,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        0,
        5,
        7,
        1,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(world, chunk, 1, 2, 0, 7, 3, 0, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 2, 5, 7, 3, 5, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 4, 1, 8, 4, 1, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 4, 4, 8, 4, 4, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 5, 2, 8, 5, 3, PLANKS, PLANKS, false);
    for (x, y, z) in [(0, 4, 2), (0, 4, 3), (8, 4, 2), (8, 4, 3)] {
        vc.set(world, PLANKS, x, y, z, chunk);
    }
    let s3 = stair(vc.c, OAK_STAIRS, 3);
    let s2 = stair(vc.c, OAK_STAIRS, 2);
    for dy in -1..=2 {
        for x in 0..=8 {
            vc.set(world, s3, x, 4 + dy, dy, chunk);
            vc.set(world, s2, x, 4 + dy, 5 - dy, chunk);
        }
    }
    for (x, y, z) in [(0, 2, 1), (0, 2, 4), (8, 2, 1), (8, 2, 4)] {
        vc.set(world, LOG, x, y, z, chunk);
    }
    for (x, y, z) in [
        (0, 2, 2),
        (0, 2, 3),
        (8, 2, 2),
        (8, 2, 3),
        (2, 2, 5),
        (3, 2, 5),
        (5, 2, 0),
        (6, 2, 5),
    ] {
        vc.set(world, GLASS_PANE, x, y, z, chunk);
    }
    vc.set(world, OAK_FENCE, 2, 1, 3, chunk);
    vc.set(world, WOODEN_PRESSURE_PLATE, 2, 2, 3, chunk);
    vc.set(world, PLANKS, 1, 1, 4, chunk);
    vc.set(world, stair(vc.c, OAK_STAIRS, 3), 2, 1, 4, chunk);
    vc.set(world, stair(vc.c, OAK_STAIRS, 1), 1, 1, 3, chunk);
    vc.fill(
        world,
        chunk,
        5,
        0,
        1,
        7,
        0,
        3,
        DOUBLE_STONE_SLAB,
        DOUBLE_STONE_SLAB,
        false,
    );
    vc.set(world, DOUBLE_STONE_SLAB, 6, 1, 1, chunk);
    vc.set(world, DOUBLE_STONE_SLAB, 6, 1, 2, chunk);
    vc.set(world, AIR, 2, 1, 0, chunk);
    vc.set(world, AIR, 2, 2, 0, chunk);
    vc.set(world, torch(f), 2, 3, 1, chunk);
    vc.door(
        world,
        2,
        1,
        0,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    if material_is_air(vc.get(world, 2, 0, -1, chunk))
        && !material_is_air(vc.get(world, 2, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 2, 0, -1, chunk);
    }
    vc.set(world, AIR, 6, 1, 5, chunk);
    vc.set(world, AIR, 6, 2, 5, chunk);
    vc.set(world, torch(f.opposite()), 6, 3, 4, chunk);
    vc.door(
        world,
        6,
        1,
        5,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    for z in 0..5 {
        for x in 0..9 {
            vc.clear_upwards(world, x, 7, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `House1.addComponentParts` (`ViBH`).
fn house1(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 1, 1, 1, 7, 5, 4, AIR, AIR, false);
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        8,
        0,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        5,
        0,
        8,
        5,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        6,
        1,
        8,
        6,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        7,
        2,
        8,
        7,
        3,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    let s3 = stair(vc.c, OAK_STAIRS, 3);
    let s2 = stair(vc.c, OAK_STAIRS, 2);
    for dy in -1..=2 {
        for x in 0..=8 {
            vc.set(world, s3, x, 6 + dy, dy, chunk);
            vc.set(world, s2, x, 6 + dy, 5 - dy, chunk);
        }
    }
    vc.fill(
        world,
        chunk,
        0,
        1,
        0,
        0,
        1,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        1,
        5,
        8,
        1,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        8,
        1,
        0,
        8,
        1,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        2,
        1,
        0,
        7,
        1,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        2,
        0,
        0,
        4,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        2,
        5,
        0,
        4,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        8,
        2,
        5,
        8,
        4,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        8,
        2,
        0,
        8,
        4,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(world, chunk, 0, 2, 1, 0, 4, 4, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 2, 5, 7, 4, 5, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 8, 2, 1, 8, 4, 4, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 2, 0, 7, 4, 0, PLANKS, PLANKS, false);
    for (x, y, z) in [
        (4, 2, 0),
        (5, 2, 0),
        (6, 2, 0),
        (4, 3, 0),
        (5, 3, 0),
        (6, 3, 0),
        (0, 2, 2),
        (0, 2, 3),
        (0, 3, 2),
        (0, 3, 3),
        (8, 2, 2),
        (8, 2, 3),
        (8, 3, 2),
        (8, 3, 3),
        (2, 2, 5),
        (3, 2, 5),
        (5, 2, 5),
        (6, 2, 5),
    ] {
        vc.set(world, GLASS_PANE, x, y, z, chunk);
    }
    vc.fill(world, chunk, 1, 4, 1, 7, 4, 1, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 4, 4, 7, 4, 4, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 3, 4, 7, 3, 4, BOOKSHELF, BOOKSHELF, false);
    vc.set(world, PLANKS, 7, 1, 4, chunk);
    vc.set(world, stair(vc.c, OAK_STAIRS, 0), 7, 1, 3, chunk);
    for x in [6, 5, 4, 3] {
        vc.set(world, s3, x, 1, 4, chunk);
    }
    vc.set(world, OAK_FENCE, 6, 1, 3, chunk);
    vc.set(world, WOODEN_PRESSURE_PLATE, 6, 2, 3, chunk);
    vc.set(world, OAK_FENCE, 4, 1, 3, chunk);
    vc.set(world, WOODEN_PRESSURE_PLATE, 4, 2, 3, chunk);
    vc.set(world, CRAFTING_TABLE, 7, 1, 1, chunk);
    vc.set(world, AIR, 1, 1, 0, chunk);
    vc.set(world, AIR, 1, 2, 0, chunk);
    vc.door(
        world,
        1,
        1,
        0,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    if material_is_air(vc.get(world, 1, 0, -1, chunk))
        && !material_is_air(vc.get(world, 1, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 1, 0, -1, chunk);
    }
    for z in 0..6 {
        for x in 0..9 {
            vc.clear_upwards(world, x, 9, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `House2.addComponentParts` (`ViS`, the blacksmith).
fn house2(
    vc: &VC,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    has_chest: &mut bool,
) {
    vc.fill(world, chunk, 0, 1, 0, 9, 4, 6, AIR, AIR, false);
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        9,
        0,
        6,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        0,
        4,
        0,
        9,
        4,
        6,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world, chunk, 0, 5, 0, 9, 5, 6, STONE_SLAB, STONE_SLAB, false,
    );
    vc.fill(world, chunk, 1, 5, 1, 8, 5, 5, AIR, AIR, false);
    vc.fill(world, chunk, 1, 1, 0, 2, 3, 0, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 1, 0, 0, 4, 0, LOG, LOG, false);
    vc.fill(world, chunk, 3, 1, 0, 3, 4, 0, LOG, LOG, false);
    vc.fill(world, chunk, 0, 1, 6, 0, 4, 6, LOG, LOG, false);
    vc.set(world, PLANKS, 3, 3, 1, chunk);
    vc.fill(world, chunk, 3, 1, 2, 3, 3, 2, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 4, 1, 3, 5, 3, 3, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 1, 1, 0, 3, 5, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 1, 6, 5, 3, 6, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 5, 1, 0, 5, 3, 0, OAK_FENCE, OAK_FENCE, false);
    vc.fill(world, chunk, 9, 1, 0, 9, 3, 0, OAK_FENCE, OAK_FENCE, false);
    vc.fill(
        world,
        chunk,
        6,
        1,
        4,
        9,
        4,
        6,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.set(world, FLOWING_LAVA, 7, 1, 5, chunk);
    vc.set(world, FLOWING_LAVA, 8, 1, 5, chunk);
    vc.set(world, IRON_BARS, 9, 2, 5, chunk);
    vc.set(world, IRON_BARS, 9, 2, 4, chunk);
    vc.fill(world, chunk, 7, 2, 4, 8, 2, 5, AIR, AIR, false);
    vc.set(world, COBBLESTONE, 6, 1, 3, chunk);
    vc.set(world, FURNACE, 6, 2, 3, chunk);
    vc.set(world, FURNACE, 6, 3, 3, chunk);
    vc.set(world, DOUBLE_STONE_SLAB, 8, 1, 1, chunk);
    for (x, y, z) in [(0, 2, 2), (0, 2, 4), (2, 2, 6), (4, 2, 6)] {
        vc.set(world, GLASS_PANE, x, y, z, chunk);
    }
    vc.set(world, OAK_FENCE, 2, 1, 4, chunk);
    vc.set(world, WOODEN_PRESSURE_PLATE, 2, 2, 4, chunk);
    vc.set(world, PLANKS, 1, 1, 5, chunk);
    vc.set(world, stair(vc.c, OAK_STAIRS, 3), 2, 1, 5, chunk);
    vc.set(world, stair(vc.c, OAK_STAIRS, 1), 1, 1, 4, chunk);
    if !*has_chest {
        let (px, py, pz) = (
            vc.c.get_x_with_offset(5, 5),
            vc.c.get_y_with_offset(1),
            vc.c.get_z_with_offset(5, 5),
        );
        if chunk.is_vec_inside(px, py, pz) {
            *has_chest = true;
            let count = 3 + rand.next_int_bound(6);
            super::placement::temple_chest(
                vc.c,
                world,
                rand,
                chunk,
                5,
                1,
                5,
                &VILLAGE_BLACKSMITH_CHEST_CONTENT,
                count,
            );
        }
    }
    for x in 6..=8 {
        if material_is_air(vc.get(world, x, 0, -1, chunk))
            && !material_is_air(vc.get(world, x, -1, -1, chunk))
        {
            vc.set(world, stair(vc.c, STONE_STAIRS, 3), x, 0, -1, chunk);
        }
    }
    for z in 0..7 {
        for x in 0..10 {
            vc.clear_upwards(world, x, 6, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `House3.addComponentParts` (`ViTRH`).
fn house3(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 1, 1, 1, 7, 4, 4, AIR, AIR, false);
    vc.fill(world, chunk, 2, 1, 6, 8, 4, 10, AIR, AIR, false);
    vc.fill(world, chunk, 2, 0, 5, 8, 0, 10, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 0, 1, 7, 0, 4, PLANKS, PLANKS, false);
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        0,
        3,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        8,
        0,
        0,
        8,
        3,
        10,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        0,
        0,
        7,
        2,
        0,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        1,
        0,
        5,
        2,
        1,
        5,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        2,
        0,
        6,
        2,
        3,
        10,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(
        world,
        chunk,
        3,
        0,
        10,
        7,
        3,
        10,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(world, chunk, 1, 2, 0, 7, 3, 0, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 2, 5, 2, 3, 5, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 4, 1, 8, 4, 1, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 4, 4, 3, 4, 4, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 0, 5, 2, 8, 5, 3, PLANKS, PLANKS, false);
    for (x, y, z) in [(0, 4, 2), (0, 4, 3), (8, 4, 2), (8, 4, 3), (8, 4, 4)] {
        vc.set(world, PLANKS, x, y, z, chunk);
    }
    let s3 = stair(vc.c, OAK_STAIRS, 3);
    let s2 = stair(vc.c, OAK_STAIRS, 2);
    for dy in -1..=2 {
        for x in 0..=8 {
            vc.set(world, s3, x, 4 + dy, dy, chunk);
            if (dy > -1 || x <= 1) && (dy > 0 || x <= 3) && (dy > 1 || x <= 4 || x >= 6) {
                vc.set(world, s2, x, 4 + dy, 5 - dy, chunk);
            }
        }
    }
    vc.fill(world, chunk, 3, 4, 5, 3, 4, 10, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 7, 4, 2, 7, 4, 10, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 4, 5, 4, 4, 5, 10, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 6, 5, 4, 6, 5, 10, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 5, 6, 3, 5, 6, 10, PLANKS, PLANKS, false);
    let s0 = stair(vc.c, OAK_STAIRS, 0);
    for i in (1..=4).rev() {
        vc.set(world, PLANKS, i, 2 + i, 7 - i, chunk);
        for j in (8 - i)..=10 {
            vc.set(world, s0, i, 2 + i, j, chunk);
        }
    }
    let s1 = stair(vc.c, OAK_STAIRS, 1);
    vc.set(world, PLANKS, 6, 6, 3, chunk);
    vc.set(world, PLANKS, 7, 5, 4, chunk);
    vc.set(world, s1, 6, 6, 4, chunk);
    for x in 6..=8 {
        for z in 5..=10 {
            vc.set(world, s1, x, 12 - x, z, chunk);
        }
    }
    for (x, y, z) in [
        (0, 2, 1),
        (0, 2, 4),
        (4, 2, 0),
        (6, 2, 0),
        (8, 2, 1),
        (8, 2, 4),
        (8, 2, 6),
        (8, 2, 9),
        (2, 2, 6),
        (2, 2, 9),
        (4, 4, 10),
        (6, 4, 10),
    ] {
        vc.set(world, LOG, x, y, z, chunk);
    }
    for (x, y, z) in [
        (0, 2, 2),
        (0, 2, 3),
        (5, 2, 0),
        (8, 2, 2),
        (8, 2, 3),
        (8, 2, 7),
        (8, 2, 8),
        (2, 2, 7),
        (2, 2, 8),
        (5, 4, 10),
    ] {
        vc.set(world, GLASS_PANE, x, y, z, chunk);
    }
    vc.set(world, PLANKS, 8, 2, 5, chunk);
    vc.set(world, PLANKS, 5, 5, 10, chunk);
    vc.set(world, AIR, 2, 1, 0, chunk);
    vc.set(world, AIR, 2, 2, 0, chunk);
    vc.set(world, torch(f), 2, 3, 1, chunk);
    vc.door(
        world,
        2,
        1,
        0,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    vc.fill(world, chunk, 1, 0, -1, 3, 2, -1, AIR, AIR, false);
    if material_is_air(vc.get(world, 2, 0, -1, chunk))
        && !material_is_air(vc.get(world, 2, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 2, 0, -1, chunk);
    }
    for z in 0..5 {
        for x in 0..9 {
            vc.clear_upwards(world, x, 7, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
    for z in 5..11 {
        for x in 2..9 {
            vc.clear_upwards(world, x, 7, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `House4Garden.addComponentParts` (`ViSH`).
fn house4_garden(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox, terrace: bool) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        4,
        0,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(world, chunk, 0, 4, 0, 4, 4, 4, LOG, LOG, false);
    vc.fill(world, chunk, 1, 4, 1, 3, 4, 3, PLANKS, PLANKS, false);
    for (x, z) in [(0, 0), (4, 0), (0, 4), (4, 4)] {
        for y in 1..=3 {
            vc.set(world, COBBLESTONE, x, y, z, chunk);
        }
    }
    vc.fill(world, chunk, 0, 1, 1, 0, 3, 3, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 4, 1, 1, 4, 3, 3, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 1, 4, 3, 3, 4, PLANKS, PLANKS, false);
    vc.set(world, GLASS_PANE, 0, 2, 2, chunk);
    vc.set(world, GLASS_PANE, 2, 2, 4, chunk);
    vc.set(world, GLASS_PANE, 4, 2, 2, chunk);
    for (x, y) in [(1, 1), (1, 2), (1, 3), (2, 3), (3, 3), (3, 2), (3, 1)] {
        vc.set(world, PLANKS, x, y, 0, chunk);
    }
    if material_is_air(vc.get(world, 2, 0, -1, chunk))
        && !material_is_air(vc.get(world, 2, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 2, 0, -1, chunk);
    }
    vc.fill(world, chunk, 1, 1, 1, 3, 3, 3, AIR, AIR, false);
    if terrace {
        for (x, z) in [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (0, 4),
            (1, 4),
            (2, 4),
            (3, 4),
            (4, 4),
            (4, 1),
            (4, 2),
            (4, 3),
            (0, 1),
            (0, 2),
            (0, 3),
        ] {
            vc.set(world, OAK_FENCE, x, 5, z, chunk);
        }
        let l = ladder(vc.c, 3);
        for y in 1..=4 {
            vc.set(world, l, 3, y, 3, chunk);
        }
    }
    vc.set(world, torch(f), 2, 3, 1, chunk);
    for z in 0..5 {
        for x in 0..5 {
            vc.clear_upwards(world, x, 6, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `WoodHut.addComponentParts` (`ViSmH`).
fn wood_hut(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox, tall: bool, table: i32) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 1, 1, 1, 3, 5, 4, AIR, AIR, false);
    vc.fill(
        world,
        chunk,
        0,
        0,
        0,
        3,
        0,
        4,
        COBBLESTONE,
        COBBLESTONE,
        false,
    );
    vc.fill(world, chunk, 1, 0, 1, 2, 0, 3, DIRT, DIRT, false);
    if tall {
        vc.fill(world, chunk, 1, 4, 1, 2, 4, 3, LOG, LOG, false);
    } else {
        vc.fill(world, chunk, 1, 5, 1, 2, 5, 3, LOG, LOG, false);
    }
    for (x, z) in [(1, 0), (2, 0), (1, 4), (2, 4)] {
        vc.set(world, LOG, x, 4, z, chunk);
    }
    for z in 1..=3 {
        vc.set(world, LOG, 0, 4, z, chunk);
        vc.set(world, LOG, 3, 4, z, chunk);
    }
    for (x, z) in [(0, 0), (3, 0), (0, 4), (3, 4)] {
        vc.fill(world, chunk, x, 1, z, x, 3, z, LOG, LOG, false);
    }
    vc.fill(world, chunk, 0, 1, 1, 0, 3, 3, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 3, 1, 1, 3, 3, 3, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 1, 0, 2, 3, 0, PLANKS, PLANKS, false);
    vc.fill(world, chunk, 1, 1, 4, 2, 3, 4, PLANKS, PLANKS, false);
    vc.set(world, GLASS_PANE, 0, 2, 2, chunk);
    vc.set(world, GLASS_PANE, 3, 2, 2, chunk);
    if table > 0 {
        vc.set(world, OAK_FENCE, table, 1, 3, chunk);
        vc.set(world, WOODEN_PRESSURE_PLATE, table, 2, 3, chunk);
    }
    vc.set(world, AIR, 1, 1, 0, chunk);
    vc.set(world, AIR, 1, 2, 0, chunk);
    vc.door(
        world,
        1,
        1,
        0,
        facing_from_horizontal(door_meta_with_offset(Some(f), 1)),
        chunk,
    );
    if material_is_air(vc.get(world, 1, 0, -1, chunk))
        && !material_is_air(vc.get(world, 1, -1, -1, chunk))
    {
        vc.set(world, stair(vc.c, STONE_STAIRS, 3), 1, 0, -1, chunk);
    }
    for z in 0..5 {
        for x in 0..4 {
            vc.clear_upwards(world, x, 6, z, chunk);
            vc.replace_down(world, chunk, COBBLESTONE, x, -1, z);
        }
    }
}

/// `Torch.addComponentParts` (`ViL`).
fn torch_piece(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let f = vc.c.facing.expect("village facing");
    vc.fill(world, chunk, 0, 0, 0, 2, 3, 1, AIR, AIR, false);
    for y in 0..=2 {
        vc.set(world, OAK_FENCE, 1, y, 0, chunk);
    }
    vc.set(world, WOOL, 1, 3, 0, chunk);
    let east_or_north = f == Facing::East || f == Facing::North;
    vc.set(
        world,
        torch(f.rotate_y()),
        if east_or_north { 2 } else { 0 },
        3,
        0,
        chunk,
    );
    vc.set(world, torch(f), 1, 3, 1, chunk);
    vc.set(
        world,
        torch(f.rotate_y_ccw()),
        if east_or_north { 0 } else { 2 },
        3,
        0,
        chunk,
    );
    vc.set(world, torch(f.opposite()), 1, 3, -1, chunk);
}

/// `Path.addComponentParts` (`ViSR`): gravel on the terrain surface, cobblestone below.
fn path(vc: &VC, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let gravel = vc.map(GRAVEL);
    let cobble = vc.map(COBBLESTONE);
    let bb = vc.c.bbox;
    for x in bb.min_x..=bb.max_x {
        for z in bb.min_z..=bb.max_z {
            if chunk.is_vec_inside(x, 64, z) {
                let top = world.get_top_solid_or_liquid(x, z) - 1;
                world.set(x, top, z, gravel);
                world.set(x, top - 1, z, cobble);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainGen;

    /// The `populate_village_chunk` driver wires the village pieces into a chunk (seed 0's village
    /// at (-53,-16)); the parity harness pins the exact blocks.
    #[test]
    fn village_placement_writes_structure_blocks() {
        let mut gen = TerrainGen::new(0);
        let blocks = gen.populate_village_chunk(-53, -16);
        assert!(blocks.iter().any(|&v| (v >> 4) == 4), "cobblestone missing");
        assert!(blocks.iter().any(|&v| (v >> 4) == 85), "fence missing");
    }

    #[test]
    fn desert_map_matches_vanilla() {
        assert_eq!(desert_map(LOG), SANDSTONE);
        assert_eq!(desert_map(COBBLESTONE), SANDSTONE);
        assert_eq!(desert_map(PLANKS), SANDSTONE | 2);
        assert_eq!(desert_map(GRAVEL), SANDSTONE);
        assert_eq!(desert_map(OAK_STAIRS | 2), SANDSTONE_STAIRS | 2);
        assert_eq!(desert_map(STONE_STAIRS | 3), SANDSTONE_STAIRS | 3);
        assert_eq!(desert_map(TORCH | 2), TORCH | 2);
        assert_eq!(desert_map(AIR), AIR);
    }
}
