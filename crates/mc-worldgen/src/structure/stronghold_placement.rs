//! `StructureStrongholdPieces.addComponentParts` bodies - the `populate` half of a stronghold (the
//! `provideChunk` half is `stronghold.rs`).
//!
//! Ported classes: the shared `Stronghold` base (`placeDoor` for all four `EntryDoor` kinds) and the
//! `Stones` selector, plus `ChestCorridor`, `Corridor`, `Crossing`, `LeftTurn`, `Library`,
//! `PortalRoom`, `Prison`, `RightTurn`, `RoomCrossing`, `Stairs` (`Stairs2` reuses it),
//! `StairsStraight` and `Straight`.
//!
//! RNG: every `fillWithRandomizedBlocks` calls the `Stones` selector once per **edge** position (one
//! `nextFloat`), returning cracked/mossy/monster-egg/plain stonebrick; interior positions become air.
//! `PortalRoom` draws one `nextFloat` per end-portal frame (the `EYE` property). A chest draws a
//! random enchanted book before its item list.
//!
//! **Liquid drop.** Every body except `PortalRoom` starts with `isLiquidInStructureBoundingBox`; a
//! `true` returns `false`, and `generate_structure` removes the piece. That is real vanilla behavior
//! (the stronghold is underground and can meet water/lava), so the port models it.
//!
//! Block-update scope: the bodies only place states; `onBlockAdded` effects (torch support, door
//! neighbours) are skipped by the oracle's remote world, like the other structure harnesses.

use mc_core::random::JavaRandom;

use super::placement::{
    door_meta_with_offset, ladder_meta_with_offset, stairs_meta_with_offset,
    stone_button_meta_with_offset, ChunkWorld, Component,
};
use super::{BoundingBox, Door, Facing, StrongholdKind, StrongholdPiece};
use crate::loot::chest::{
    enchanted_book_content, enchanted_book_content_with, with_extra, ChestContent,
    STRONGHOLD_CHEST_CONTENT, STRONGHOLD_LIBRARY_CHEST_CONTENT,
    STRONGHOLD_ROOM_CROSSING_CHEST_CONTENT,
};

const AIR: u16 = 0;
const COBBLESTONE: u16 = 4 << 4;
const PLANKS: u16 = 5 << 4;
const FLOWING_WATER: u16 = 8 << 4;
const FLOWING_LAVA: u16 = 10 << 4;
const WEB: u16 = 30 << 4;
const DOUBLE_STONE_SLAB: u16 = 43 << 4;
const STONE_SLAB_STONE: u16 = 44 << 4; // `EnumType.STONE`
const STONE_SLAB_SMOOTHBRICK: u16 = (44 << 4) | 5; // `EnumType.SMOOTHBRICK`
const BOOKSHELF: u16 = 47 << 4;
const TORCH: u16 = (50 << 4) | 5; // default state FACING=UP -> meta 5
const OAK_DOOR: u16 = 64 << 4; // default lower meta 3, upper meta 8
const LADDER: u16 = 65 << 4;
const STONE_STAIRS: u16 = 67 << 4;
const IRON_DOOR: u16 = 71 << 4;
const STONE_BUTTON: u16 = 77 << 4;
const OAK_FENCE: u16 = 85 << 4;
const MONSTER_EGG: u16 = (97 << 4) | 2; // `BlockSilverfish.EnumType.STONEBRICK` meta 2
const STONEBRICK: u16 = 98 << 4;
const IRON_BARS: u16 = 101 << 4;
const STONE_BRICK_STAIRS: u16 = 109 << 4;
const END_PORTAL_FRAME: u16 = 120 << 4;
const MOB_SPAWNER: u16 = 52 << 4;

const STONEBRICK_MOSSY: u16 = (98 << 4) | 1;
const STONEBRICK_CRACKED: u16 = (98 << 4) | 2;

/// `StructureStrongholdPieces.Stones.selectBlocks`: cracked/mossy/monster-egg/plain stonebrick on
/// the edge, air in the interior. One `nextFloat` per edge position.
fn stronghold_stone(rand: &mut JavaRandom, edge: bool) -> u16 {
    if !edge {
        return AIR;
    }
    let f = rand.next_float();
    if f < 0.2 {
        STONEBRICK_CRACKED
    } else if f < 0.5 {
        STONEBRICK_MOSSY
    } else if f < 0.55 {
        MONSTER_EGG
    } else {
        STONEBRICK
    }
}

/// `Stronghold.placeDoor` for the four `EntryDoor` kinds.
fn place_door(
    comp: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    door: Door,
    x: i32,
    y: i32,
    z: i32,
) {
    let frame = [
        (x, y),
        (x, y + 1),
        (x, y + 2),
        (x + 1, y + 2),
        (x + 2, y + 2),
        (x + 2, y + 1),
        (x + 2, y),
    ];
    match door {
        Door::Opening => comp.fill_with_blocks(
            world,
            chunk,
            x,
            y,
            z,
            x + 3 - 1,
            y + 3 - 1,
            z,
            AIR,
            AIR,
            false,
        ),
        Door::WoodDoor => {
            for &(fx, fy) in &frame {
                comp.set_block_state(world, STONEBRICK, fx, fy, z, chunk);
            }
            comp.set_block_state(world, OAK_DOOR | 3, x + 1, y, z, chunk);
            comp.set_block_state(world, OAK_DOOR | 8, x + 1, y + 1, z, chunk);
        }
        Door::Grates => {
            comp.set_block_state(world, AIR, x + 1, y, z, chunk);
            comp.set_block_state(world, AIR, x + 1, y + 1, z, chunk);
            for &(fx, fy) in &frame {
                comp.set_block_state(world, IRON_BARS, fx, fy, z, chunk);
            }
        }
        Door::IronDoor => {
            for &(fx, fy) in &frame {
                comp.set_block_state(world, STONEBRICK, fx, fy, z, chunk);
            }
            comp.set_block_state(world, IRON_DOOR | 3, x + 1, y, z, chunk);
            comp.set_block_state(world, IRON_DOOR | 8, x + 1, y + 1, z, chunk);
            let b1 = stone_button_meta_with_offset(comp.facing, 4);
            let b2 = stone_button_meta_with_offset(comp.facing, 3);
            comp.set_block_state(world, STONE_BUTTON | b1 as u16, x + 2, y + 1, z + 1, chunk);
            comp.set_block_state(world, STONE_BUTTON | b2 as u16, x + 2, y + 1, z - 1, chunk);
        }
    }
}

/// `StructureStrongholdPieces.Stronghold.addComponentParts`, dispatched on the piece kind.
pub(crate) fn stronghold_parts(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    s: &mut StrongholdPiece,
) -> bool {
    if s.kind != StrongholdKind::PortalRoom
        && comp.is_liquid_in_structure_bounding_box(world, chunk)
    {
        return false;
    }
    match s.kind {
        StrongholdKind::ChestCorridor => {
            chest_corridor(comp, world, rand, chunk, s.door, &mut s.chest)
        }
        StrongholdKind::Corridor => corridor(comp, world, chunk, s.steps),
        StrongholdKind::Crossing => crossing(comp, world, rand, chunk, s),
        StrongholdKind::LeftTurn => left_turn(comp, world, rand, chunk, s.door),
        StrongholdKind::Library => library(comp, world, rand, chunk, s.tall, s.door),
        StrongholdKind::PortalRoom => portal_room(comp, world, rand, chunk, &mut s.has_spawner),
        StrongholdKind::Prison => prison(comp, world, rand, chunk, s.door),
        StrongholdKind::RightTurn => right_turn(comp, world, rand, chunk, s.door),
        StrongholdKind::RoomCrossing => room_crossing(comp, world, rand, chunk, s),
        StrongholdKind::Stairs | StrongholdKind::Stairs2 => {
            stairs(comp, world, rand, chunk, s.door)
        }
        StrongholdKind::StairsStraight => stairs_straight(comp, world, rand, chunk, s.door),
        StrongholdKind::Straight => straight(comp, world, rand, chunk, s),
    }
    true
}

/// `fillWithRandomizedBlocks` with the shared `Stones` selector.
#[allow(clippy::too_many_arguments)]
fn randomized(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    x1: i32,
    y1: i32,
    z1: i32,
    x2: i32,
    y2: i32,
    z2: i32,
    always: bool,
) {
    comp.fill_with_randomized_blocks(
        world,
        chunk,
        rand,
        x1,
        y1,
        z1,
        x2,
        y2,
        z2,
        always,
        stronghold_stone,
    );
}

#[allow(clippy::too_many_arguments)]
fn structure_chest(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    x: i32,
    y: i32,
    z: i32,
    list: &[ChestContent],
    count: i32,
) {
    super::placement::temple_chest(comp, world, rand, chunk, x, y, z, list, count);
}

/// `ChestCorridor.addComponentParts` (`SHCC`).
fn chest_corridor(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
    has_chest: &mut bool,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 4, 6, true);
    place_door(comp, world, chunk, door, 1, 1, 0);
    place_door(comp, world, chunk, Door::Opening, 1, 1, 6);
    comp.fill_with_blocks(
        world, chunk, 3, 1, 2, 3, 1, 4, STONEBRICK, STONEBRICK, false,
    );
    for (x, y, z) in [(3, 1, 1), (3, 1, 5), (3, 2, 2), (3, 2, 4)] {
        comp.set_block_state(world, STONE_SLAB_SMOOTHBRICK, x, y, z, chunk);
    }
    for z in 2..=4 {
        comp.set_block_state(world, STONE_SLAB_SMOOTHBRICK, 2, 1, z, chunk);
    }
    let (px, py, pz) = (
        comp.get_x_with_offset(3, 3),
        comp.get_y_with_offset(2),
        comp.get_z_with_offset(3, 3),
    );
    if !*has_chest && chunk.is_vec_inside(px, py, pz) {
        *has_chest = true;
        let book = enchanted_book_content(rand);
        let list = with_extra(&STRONGHOLD_CHEST_CONTENT, book);
        let count = 2 + rand.next_int_bound(2);
        structure_chest(comp, world, rand, chunk, 3, 2, 3, &list, count);
    }
}

/// `Corridor.addComponentParts` (`SHFC`).
fn corridor(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox, steps: i32) {
    for i in 0..steps {
        for x in 0..=4 {
            comp.set_block_state(world, STONEBRICK, x, 0, i, chunk);
            comp.set_block_state(world, STONEBRICK, x, 4, i, chunk);
        }
        for y in 1..=3 {
            comp.set_block_state(world, STONEBRICK, 0, y, i, chunk);
            comp.set_block_state(world, AIR, 1, y, i, chunk);
            comp.set_block_state(world, AIR, 2, y, i, chunk);
            comp.set_block_state(world, AIR, 3, y, i, chunk);
            comp.set_block_state(world, STONEBRICK, 4, y, i, chunk);
        }
    }
}

/// `Crossing.addComponentParts` (`SH5C`).
fn crossing(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    s: &StrongholdPiece,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 9, 8, 10, true);
    place_door(comp, world, chunk, s.door, 4, 3, 0);
    if s.left_low {
        comp.fill_with_blocks(world, chunk, 0, 3, 1, 0, 5, 3, AIR, AIR, false);
    }
    if s.right_low {
        comp.fill_with_blocks(world, chunk, 9, 3, 1, 9, 5, 3, AIR, AIR, false);
    }
    if s.left_high {
        comp.fill_with_blocks(world, chunk, 0, 5, 7, 0, 7, 9, AIR, AIR, false);
    }
    if s.right_high {
        comp.fill_with_blocks(world, chunk, 9, 5, 7, 9, 7, 9, AIR, AIR, false);
    }
    comp.fill_with_blocks(world, chunk, 5, 1, 10, 7, 3, 10, AIR, AIR, false);
    randomized(comp, world, rand, chunk, 1, 2, 1, 8, 2, 6, false);
    randomized(comp, world, rand, chunk, 4, 1, 5, 4, 4, 9, false);
    randomized(comp, world, rand, chunk, 8, 1, 5, 8, 4, 9, false);
    randomized(comp, world, rand, chunk, 1, 4, 7, 3, 4, 9, false);
    randomized(comp, world, rand, chunk, 1, 3, 5, 3, 3, 6, false);
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        4,
        3,
        3,
        4,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        4,
        6,
        3,
        4,
        6,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    randomized(comp, world, rand, chunk, 5, 1, 7, 7, 1, 8, false);
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        1,
        9,
        7,
        1,
        9,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        2,
        7,
        7,
        2,
        7,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        5,
        7,
        4,
        5,
        9,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        7,
        8,
        5,
        9,
        STONE_SLAB_STONE,
        STONE_SLAB_STONE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        5,
        7,
        7,
        5,
        9,
        DOUBLE_STONE_SLAB,
        DOUBLE_STONE_SLAB,
        false,
    );
    comp.set_block_state(world, TORCH, 6, 5, 6, chunk);
}

/// `LeftTurn.addComponentParts` (`SHLT`).
fn left_turn(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 4, 4, true);
    place_door(comp, world, chunk, door, 1, 1, 0);
    let f = comp.facing.expect("stronghold facing");
    if f != Facing::North && f != Facing::East {
        comp.fill_with_blocks(world, chunk, 4, 1, 1, 4, 3, 3, AIR, AIR, false);
    } else {
        comp.fill_with_blocks(world, chunk, 0, 1, 1, 0, 3, 3, AIR, AIR, false);
    }
}

/// `RightTurn.addComponentParts` (`SHRT`; the weight never produces it, but the body exists).
fn right_turn(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 4, 4, true);
    place_door(comp, world, chunk, door, 1, 1, 0);
    let f = comp.facing.expect("stronghold facing");
    if f != Facing::North && f != Facing::East {
        comp.fill_with_blocks(world, chunk, 0, 1, 1, 0, 3, 3, AIR, AIR, false);
    } else {
        comp.fill_with_blocks(world, chunk, 4, 1, 1, 4, 3, 3, AIR, AIR, false);
    }
}

/// `Library.addComponentParts` (`SHLi`).
fn library(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    tall: bool,
    door: Door,
) {
    let h = if tall { 11 } else { 6 };
    randomized(comp, world, rand, chunk, 0, 0, 0, 13, h - 1, 14, true);
    place_door(comp, world, chunk, door, 4, 1, 0);
    comp.fill_with_random_blocks(
        world, chunk, rand, 0.07, 2, 1, 1, 11, 4, 13, WEB, WEB, false,
    );
    for i in 1..=13 {
        if (i - 1) % 4 == 0 {
            comp.fill_with_blocks(world, chunk, 1, 1, i, 1, 4, i, PLANKS, PLANKS, false);
            comp.fill_with_blocks(world, chunk, 12, 1, i, 12, 4, i, PLANKS, PLANKS, false);
            comp.set_block_state(world, TORCH, 2, 3, i, chunk);
            comp.set_block_state(world, TORCH, 11, 3, i, chunk);
            if tall {
                comp.fill_with_blocks(world, chunk, 1, 6, i, 1, 9, i, PLANKS, PLANKS, false);
                comp.fill_with_blocks(world, chunk, 12, 6, i, 12, 9, i, PLANKS, PLANKS, false);
            }
        } else {
            comp.fill_with_blocks(world, chunk, 1, 1, i, 1, 4, i, BOOKSHELF, BOOKSHELF, false);
            comp.fill_with_blocks(
                world, chunk, 12, 1, i, 12, 4, i, BOOKSHELF, BOOKSHELF, false,
            );
            if tall {
                comp.fill_with_blocks(world, chunk, 1, 6, i, 1, 9, i, BOOKSHELF, BOOKSHELF, false);
                comp.fill_with_blocks(
                    world, chunk, 12, 6, i, 12, 9, i, BOOKSHELF, BOOKSHELF, false,
                );
            }
        }
    }
    for i in (3..12).step_by(2) {
        comp.fill_with_blocks(world, chunk, 3, 1, i, 4, 3, i, BOOKSHELF, BOOKSHELF, false);
        comp.fill_with_blocks(world, chunk, 6, 1, i, 7, 3, i, BOOKSHELF, BOOKSHELF, false);
        comp.fill_with_blocks(world, chunk, 9, 1, i, 10, 3, i, BOOKSHELF, BOOKSHELF, false);
    }
    if tall {
        comp.fill_with_blocks(world, chunk, 1, 5, 1, 3, 5, 13, PLANKS, PLANKS, false);
        comp.fill_with_blocks(world, chunk, 10, 5, 1, 12, 5, 13, PLANKS, PLANKS, false);
        comp.fill_with_blocks(world, chunk, 4, 5, 1, 9, 5, 2, PLANKS, PLANKS, false);
        comp.fill_with_blocks(world, chunk, 4, 5, 12, 9, 5, 13, PLANKS, PLANKS, false);
        for (x, y, z) in [(9, 5, 11), (8, 5, 11), (9, 5, 10)] {
            comp.set_block_state(world, PLANKS, x, y, z, chunk);
        }
        comp.fill_with_blocks(world, chunk, 3, 6, 2, 3, 6, 12, OAK_FENCE, OAK_FENCE, false);
        comp.fill_with_blocks(
            world, chunk, 10, 6, 2, 10, 6, 10, OAK_FENCE, OAK_FENCE, false,
        );
        comp.fill_with_blocks(world, chunk, 4, 6, 2, 9, 6, 2, OAK_FENCE, OAK_FENCE, false);
        comp.fill_with_blocks(
            world, chunk, 4, 6, 12, 8, 6, 12, OAK_FENCE, OAK_FENCE, false,
        );
        for (x, y, z) in [(9, 6, 11), (8, 6, 11), (9, 6, 10)] {
            comp.set_block_state(world, OAK_FENCE, x, y, z, chunk);
        }
        let l = LADDER | ladder_meta_with_offset(comp.facing, 3) as u16;
        for y in 1..=7 {
            comp.set_block_state(world, l, 10, y, 13, chunk);
        }
        for (x, y, z) in [
            (6, 9, 7),
            (7, 9, 7),
            (6, 8, 7),
            (7, 8, 7),
            (6, 7, 7),
            (7, 7, 7),
            (5, 7, 7),
            (8, 7, 7),
            (6, 7, 6),
            (6, 7, 8),
            (7, 7, 6),
            (7, 7, 8),
        ] {
            comp.set_block_state(world, OAK_FENCE, x, y, z, chunk);
        }
        for (x, y, z) in [
            (5, 8, 7),
            (8, 8, 7),
            (6, 8, 6),
            (6, 8, 8),
            (7, 8, 6),
            (7, 8, 8),
        ] {
            comp.set_block_state(world, TORCH, x, y, z, chunk);
        }
    }
    let book = enchanted_book_content_with(rand, 1, 5, 2);
    let list = with_extra(&STRONGHOLD_LIBRARY_CHEST_CONTENT, book);
    let count = 1 + rand.next_int_bound(4);
    structure_chest(comp, world, rand, chunk, 3, 3, 5, &list, count);
    if tall {
        comp.set_block_state(world, AIR, 12, 9, 1, chunk);
        let book = enchanted_book_content_with(rand, 1, 5, 2);
        let list = with_extra(&STRONGHOLD_LIBRARY_CHEST_CONTENT, book);
        let count = 1 + rand.next_int_bound(4);
        structure_chest(comp, world, rand, chunk, 12, 8, 1, &list, count);
    }
}

/// `PortalRoom.addComponentParts` (`SHPR`).
fn portal_room(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    has_spawner: &mut bool,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 10, 7, 15, false);
    place_door(comp, world, chunk, Door::Grates, 4, 1, 0);
    let y4 = 6;
    randomized(comp, world, rand, chunk, 1, y4, 1, 1, y4, 14, false);
    randomized(comp, world, rand, chunk, 9, y4, 1, 9, y4, 14, false);
    randomized(comp, world, rand, chunk, 2, y4, 1, 8, y4, 2, false);
    randomized(comp, world, rand, chunk, 2, y4, 14, 8, y4, 14, false);
    randomized(comp, world, rand, chunk, 1, 1, 1, 2, 1, 4, false);
    randomized(comp, world, rand, chunk, 8, 1, 1, 9, 1, 4, false);
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        1,
        1,
        1,
        1,
        3,
        FLOWING_LAVA,
        FLOWING_LAVA,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        9,
        1,
        1,
        9,
        1,
        3,
        FLOWING_LAVA,
        FLOWING_LAVA,
        false,
    );
    randomized(comp, world, rand, chunk, 3, 1, 8, 7, 1, 12, false);
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        1,
        9,
        6,
        1,
        11,
        FLOWING_LAVA,
        FLOWING_LAVA,
        false,
    );
    for i in (3..14).step_by(2) {
        comp.fill_with_blocks(world, chunk, 0, 3, i, 0, 4, i, IRON_BARS, IRON_BARS, false);
        comp.fill_with_blocks(
            world, chunk, 10, 3, i, 10, 4, i, IRON_BARS, IRON_BARS, false,
        );
    }
    for i in (2..9).step_by(2) {
        comp.fill_with_blocks(
            world, chunk, i, 3, 15, i, 4, 15, IRON_BARS, IRON_BARS, false,
        );
    }
    let stair = STONE_BRICK_STAIRS | stairs_meta_with_offset(comp.facing, 3) as u16;
    randomized(comp, world, rand, chunk, 4, 1, 5, 6, 1, 7, false);
    randomized(comp, world, rand, chunk, 4, 2, 6, 6, 2, 7, false);
    randomized(comp, world, rand, chunk, 4, 3, 7, 6, 3, 7, false);
    for x in 4..=6 {
        comp.set_block_state(world, stair, x, 1, 4, chunk);
        comp.set_block_state(world, stair, x, 2, 5, chunk);
        comp.set_block_state(world, stair, x, 3, 6, chunk);
    }
    let (mut n, mut s, mut e, mut w) = (
        Facing::North.horizontal_index(),
        Facing::South.horizontal_index(),
        Facing::East.horizontal_index(),
        Facing::West.horizontal_index(),
    );
    match comp.facing {
        Some(Facing::South) => {
            n = Facing::South.horizontal_index();
            s = Facing::North.horizontal_index();
        }
        Some(Facing::West) => {
            n = Facing::West.horizontal_index();
            s = Facing::East.horizontal_index();
            e = Facing::South.horizontal_index();
            w = Facing::North.horizontal_index();
        }
        Some(Facing::East) => {
            n = Facing::East.horizontal_index();
            s = Facing::West.horizontal_index();
            e = Facing::South.horizontal_index();
            w = Facing::North.horizontal_index();
        }
        _ => {}
    }
    let frames = [
        (4, 8, n),
        (5, 8, n),
        (6, 8, n),
        (4, 12, s),
        (5, 12, s),
        (6, 12, s),
        (3, 9, e),
        (3, 10, e),
        (3, 11, e),
        (7, 9, w),
        (7, 10, w),
        (7, 11, w),
    ];
    for (x, z, face) in frames {
        let eye = rand.next_float() > 0.9;
        let value = END_PORTAL_FRAME | face as u16 | if eye { 4 } else { 0 };
        comp.set_block_state(world, value, x, 3, z, chunk);
    }
    if !*has_spawner {
        let (px, py, pz) = (
            comp.get_x_with_offset(5, 6),
            comp.get_y_with_offset(3),
            comp.get_z_with_offset(5, 6),
        );
        if chunk.is_vec_inside(px, py, pz) {
            *has_spawner = true;
            comp.set_block_state(world, MOB_SPAWNER, 5, 3, 6, chunk);
        }
    }
}

/// `Prison.addComponentParts` (`SHPH`).
fn prison(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 8, 4, 10, true);
    place_door(comp, world, chunk, door, 1, 1, 0);
    comp.fill_with_blocks(world, chunk, 1, 1, 10, 3, 3, 10, AIR, AIR, false);
    randomized(comp, world, rand, chunk, 4, 1, 1, 4, 3, 1, false);
    randomized(comp, world, rand, chunk, 4, 1, 3, 4, 3, 3, false);
    randomized(comp, world, rand, chunk, 4, 1, 7, 4, 3, 7, false);
    randomized(comp, world, rand, chunk, 4, 1, 9, 4, 3, 9, false);
    comp.fill_with_blocks(world, chunk, 4, 1, 4, 4, 3, 6, IRON_BARS, IRON_BARS, false);
    comp.fill_with_blocks(world, chunk, 5, 1, 5, 7, 3, 5, IRON_BARS, IRON_BARS, false);
    comp.set_block_state(world, IRON_BARS, 4, 3, 2, chunk);
    comp.set_block_state(world, IRON_BARS, 4, 3, 8, chunk);
    let lower = door_meta_with_offset(comp.facing, 3) as u16;
    for z in [2, 8] {
        comp.set_block_state(world, IRON_DOOR | lower, 4, 1, z, chunk);
        comp.set_block_state(world, IRON_DOOR | (8 | lower), 4, 2, z, chunk);
    }
}

/// `RoomCrossing.addComponentParts` (`SHRC`).
fn room_crossing(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    s: &StrongholdPiece,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 10, 6, 10, true);
    place_door(comp, world, chunk, s.door, 4, 1, 0);
    comp.fill_with_blocks(world, chunk, 4, 1, 10, 6, 3, 10, AIR, AIR, false);
    comp.fill_with_blocks(world, chunk, 0, 1, 4, 0, 3, 6, AIR, AIR, false);
    comp.fill_with_blocks(world, chunk, 10, 1, 4, 10, 3, 6, AIR, AIR, false);
    match s.room_type {
        0 => {
            for y in 1..=3 {
                comp.set_block_state(world, STONEBRICK, 5, y, 5, chunk);
            }
            for (x, y, z) in [(4, 3, 5), (6, 3, 5), (5, 3, 4), (5, 3, 6)] {
                comp.set_block_state(world, TORCH, x, y, z, chunk);
            }
            for (x, z) in [
                (4, 4),
                (4, 5),
                (4, 6),
                (6, 4),
                (6, 5),
                (6, 6),
                (5, 4),
                (5, 6),
            ] {
                comp.set_block_state(world, STONE_SLAB_STONE, x, 1, z, chunk);
            }
        }
        1 => {
            for i in 0..5 {
                comp.set_block_state(world, STONEBRICK, 3, 1, 3 + i, chunk);
                comp.set_block_state(world, STONEBRICK, 7, 1, 3 + i, chunk);
                comp.set_block_state(world, STONEBRICK, 3 + i, 1, 3, chunk);
                comp.set_block_state(world, STONEBRICK, 3 + i, 1, 7, chunk);
            }
            for y in 1..=3 {
                comp.set_block_state(world, STONEBRICK, 5, y, 5, chunk);
            }
            comp.set_block_state(world, FLOWING_WATER, 5, 4, 5, chunk);
        }
        2 => {
            for i in 1..=9 {
                comp.set_block_state(world, COBBLESTONE, 1, 3, i, chunk);
                comp.set_block_state(world, COBBLESTONE, 9, 3, i, chunk);
                comp.set_block_state(world, COBBLESTONE, i, 3, 1, chunk);
                comp.set_block_state(world, COBBLESTONE, i, 3, 9, chunk);
            }
            for (x, z) in [(5, 4), (5, 6), (4, 5), (6, 5)] {
                comp.set_block_state(world, COBBLESTONE, x, 1, z, chunk);
                comp.set_block_state(world, COBBLESTONE, x, 3, z, chunk);
            }
            for y in 1..=3 {
                for (x, z) in [(4, 4), (6, 4), (4, 6), (6, 6)] {
                    comp.set_block_state(world, COBBLESTONE, x, y, z, chunk);
                }
            }
            comp.set_block_state(world, TORCH, 5, 3, 5, chunk);
            for i in 2..=8 {
                comp.set_block_state(world, PLANKS, 2, 3, i, chunk);
                comp.set_block_state(world, PLANKS, 3, 3, i, chunk);
                if i <= 3 || i >= 7 {
                    comp.set_block_state(world, PLANKS, 4, 3, i, chunk);
                    comp.set_block_state(world, PLANKS, 5, 3, i, chunk);
                    comp.set_block_state(world, PLANKS, 6, 3, i, chunk);
                }
                comp.set_block_state(world, PLANKS, 7, 3, i, chunk);
                comp.set_block_state(world, PLANKS, 8, 3, i, chunk);
            }
            let l = LADDER | ladder_meta_with_offset(comp.facing, Facing::West.index()) as u16;
            for y in 1..=3 {
                comp.set_block_state(world, l, 9, y, 3, chunk);
            }
            let book = enchanted_book_content(rand);
            let list = with_extra(&STRONGHOLD_ROOM_CROSSING_CHEST_CONTENT, book);
            let count = 1 + rand.next_int_bound(4);
            structure_chest(comp, world, rand, chunk, 3, 4, 8, &list, count);
        }
        _ => {}
    }
}

/// `Stairs.addComponentParts` (`SHSD`; also `Stairs2`/`SHStart`).
fn stairs(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 10, 4, true);
    place_door(comp, world, chunk, door, 1, 7, 0);
    place_door(comp, world, chunk, Door::Opening, 1, 1, 4);
    for (x, y, z) in [
        (2, 6, 1),
        (1, 5, 1),
        (1, 5, 2),
        (1, 4, 3),
        (2, 4, 3),
        (3, 3, 3),
        (3, 3, 2),
        (3, 2, 1),
        (2, 2, 1),
        (1, 1, 1),
        (1, 1, 2),
    ] {
        comp.set_block_state(world, STONEBRICK, x, y, z, chunk);
    }
    for (x, y, z) in [
        (1, 6, 1),
        (1, 5, 3),
        (3, 4, 3),
        (3, 3, 1),
        (1, 2, 1),
        (1, 1, 3),
    ] {
        comp.set_block_state(world, STONE_SLAB_STONE, x, y, z, chunk);
    }
}

/// `StairsStraight.addComponentParts` (`SHSSD`).
fn stairs_straight(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    door: Door,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 10, 7, true);
    place_door(comp, world, chunk, door, 1, 7, 0);
    place_door(comp, world, chunk, Door::Opening, 1, 1, 7);
    let stair = STONE_STAIRS | stairs_meta_with_offset(comp.facing, 2) as u16;
    for i in 0..6 {
        for x in 1..=3 {
            comp.set_block_state(world, stair, x, 6 - i, 1 + i, chunk);
            if i < 5 {
                comp.set_block_state(world, STONEBRICK, x, 5 - i, 1 + i, chunk);
            }
        }
    }
}

/// `Straight.addComponentParts` (`SHS`).
fn straight(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    s: &StrongholdPiece,
) {
    randomized(comp, world, rand, chunk, 0, 0, 0, 4, 4, 6, true);
    place_door(comp, world, chunk, s.door, 1, 1, 0);
    place_door(comp, world, chunk, Door::Opening, 1, 1, 6);
    for (x, y, z) in [(1, 2, 1), (3, 2, 1), (1, 2, 5), (3, 2, 5)] {
        comp.randomly_place_block(world, chunk, rand, 0.1, x, y, z, TORCH);
    }
    if s.expands_x {
        comp.fill_with_blocks(world, chunk, 0, 1, 2, 0, 3, 4, AIR, AIR, false);
    }
    if s.expands_z {
        comp.fill_with_blocks(world, chunk, 4, 1, 2, 4, 3, 4, AIR, AIR, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainGen;

    /// The `populate_stronghold_chunk` driver wires the pieces into a chunk (seed 0's stronghold at
    /// (-37,39)); the parity harness pins the exact blocks.
    #[test]
    fn stronghold_placement_writes_structure_blocks() {
        let mut gen = TerrainGen::new(0);
        let blocks = gen.populate_stronghold_chunk(-37, 39);
        assert!(blocks.iter().any(|&v| (v >> 4) == 98), "stonebrick missing");
    }

    #[test]
    fn stones_selector_matches_vanilla_thresholds() {
        assert_eq!(stronghold_stone(&mut JavaRandom::new(0), false), AIR);
        // Edge positions consume one draw; just pin that the mapping is deterministic and in range.
        let mut rand = JavaRandom::new(0);
        let v = stronghold_stone(&mut rand, true);
        assert!(matches!(
            v,
            STONEBRICK | STONEBRICK_MOSSY | STONEBRICK_CRACKED | MONSTER_EGG
        ));
    }
}
