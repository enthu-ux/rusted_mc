//! `StructureNetherBridgePieces.addComponentParts` bodies - the `populate` half of a nether
//! fortress (the `provideChunk` half is `netherbridge.rs`).
//!
//! Ported classes: `StructureNetherBridgePieces.{Straight, Crossing3, Crossing, Stairs, Throne,
//! Entrance, Corridor5, Crossing2, Corridor2, Corridor, Corridor3, Corridor4, NetherStalkRoom,
//! End}`. `Start` extends `Crossing3` and uses the same body.
//!
//! Block states are vanilla 1.8.9 ids: nether brick `112`, nether brick fence `113`, nether brick
//! stairs `114`, soul sand `88`, nether wart `115`, mob spawner `52`, flowing lava `10`.
//!
//! **Block-update scope.** `Entrance.addComponentParts` calls
//! `World.forceBlockUpdateTick(Blocks.flowing_lava, ..)`, which in vanilla immediately runs
//! `BlockDynamicLiquid.updateTick` (the lava flows). That is block-update semantics, not placement
//! (cf. `handoff.md`'s mineshaft `onBlockAdded` note), so the port writes the lava and stops; the
//! oracle neutralises `forceBlockUpdateTick`. The nether pieces never call
//! `isLiquidInStructureBoundingBox`, so no piece is ever dropped and placement is order-independent
//! (once each pass has its own RNG state; see `netherbridge::hell_populate_rand`).

use mc_core::random::JavaRandom;

use super::placement::{stairs_meta_with_offset, temple_chest, ChunkWorld, Component};
use super::{BoundingBox, NetherKind};
use crate::loot::chest::NETHER_BRIDGE_CHEST_CONTENT;

const AIR: u16 = 0;
const NETHER_BRICK: u16 = 112 << 4;
const NETHER_BRICK_FENCE: u16 = 113 << 4;
const NETHER_BRICK_STAIRS: u16 = 114 << 4;
const SOUL_SAND: u16 = 88 << 4;
const NETHER_WART: u16 = 115 << 4;
const MOB_SPAWNER: u16 = 52 << 4;
const FLOWING_LAVA: u16 = 10 << 4;

/// `StructureComponent.getMetadataWithOffset(Blocks.nether_brick_stairs, meta)`.
fn stairs(comp: &Component, meta: i32) -> u16 {
    NETHER_BRICK_STAIRS | (stairs_meta_with_offset(comp.facing, meta) as u16)
}

/// `StructureComponent.addComponentParts` for a nether fortress piece. Returns vanilla's `true`
/// (the nether bodies never signal "remove piece").
#[allow(clippy::too_many_arguments)]
pub(crate) fn nether_parts(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    kind: NetherKind,
    chest: &mut bool,
    seed: i32,
    has_spawner: &mut bool,
) -> bool {
    match kind {
        NetherKind::Straight => straight(comp, world, chunk),
        NetherKind::Crossing3 | NetherKind::Start => crossing3(comp, world, chunk),
        NetherKind::Crossing => crossing(comp, world, chunk),
        NetherKind::Stairs => stairs_room(comp, world, chunk),
        NetherKind::Throne => throne(comp, world, chunk, has_spawner),
        NetherKind::Entrance => entrance(comp, world, chunk),
        NetherKind::Corridor5 => corridor5(comp, world, chunk),
        NetherKind::Crossing2 => crossing2(comp, world, chunk),
        NetherKind::Corridor2 => corridor2(comp, world, rand, chunk, chest),
        NetherKind::Corridor => corridor(comp, world, rand, chunk, chest),
        NetherKind::Corridor3 => corridor3(comp, world, chunk),
        NetherKind::Corridor4 => corridor4(comp, world, chunk),
        NetherKind::NetherStalkRoom => nether_stalk_room(comp, world, chunk),
        NetherKind::End => end(comp, world, chunk, seed),
    }
    true
}

/// `Corridor.addComponentParts` (`NeSCLT`).
fn corridor(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    chest: &mut bool,
) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        4,
        1,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 4, 5, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        4,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        1,
        4,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        3,
        4,
        4,
        3,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        0,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        4,
        3,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        4,
        1,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        3,
        3,
        4,
        3,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK,
        false,
    );
    if *chest {
        let (px, py, pz) = (
            comp.get_x_with_offset(3, 3),
            comp.get_y_with_offset(2),
            comp.get_z_with_offset(3, 3),
        );
        if chunk.is_vec_inside(px, py, pz) {
            *chest = false;
            let count = 2 + rand.next_int_bound(4);
            temple_chest(
                comp,
                world,
                rand,
                chunk,
                3,
                2,
                3,
                &NETHER_BRIDGE_CHEST_CONTENT,
                count,
            );
        }
    }
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        0,
        4,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        for z in 0..=4 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Corridor2.addComponentParts` (`NeSCRT`).
fn corridor2(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    chest: &mut bool,
) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        4,
        1,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 4, 5, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        0,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        1,
        0,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        3,
        0,
        4,
        3,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        4,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        2,
        4,
        4,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        4,
        1,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        3,
        3,
        4,
        3,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK,
        false,
    );
    if *chest {
        let (px, py, pz) = (
            comp.get_x_with_offset(1, 3),
            comp.get_y_with_offset(2),
            comp.get_z_with_offset(1, 3),
        );
        if chunk.is_vec_inside(px, py, pz) {
            *chest = false;
            let count = 2 + rand.next_int_bound(4);
            temple_chest(
                comp,
                world,
                rand,
                chunk,
                1,
                2,
                3,
                &NETHER_BRIDGE_CHEST_CONTENT,
                count,
            );
        }
    }
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        0,
        4,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        for z in 0..=4 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Corridor3.addComponentParts` (`NeCCS`).
fn corridor3(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    let stair = stairs(comp, 2);
    for i in 0..=9 {
        let a = 1.max(7 - i);
        let b = (a + 5).max(14 - i).min(13);
        let z = i;
        comp.fill_with_blocks(
            world,
            chunk,
            0,
            0,
            z,
            4,
            a,
            z,
            NETHER_BRICK,
            NETHER_BRICK,
            false,
        );
        comp.fill_with_blocks(world, chunk, 1, a + 1, z, 3, b - 1, z, AIR, AIR, false);
        if i <= 6 {
            comp.set_block_state(world, stair, 1, a + 1, z, chunk);
            comp.set_block_state(world, stair, 2, a + 1, z, chunk);
            comp.set_block_state(world, stair, 3, a + 1, z, chunk);
        }
        comp.fill_with_blocks(
            world,
            chunk,
            0,
            b,
            z,
            4,
            b,
            z,
            NETHER_BRICK,
            NETHER_BRICK,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            0,
            a + 1,
            z,
            0,
            b - 1,
            z,
            NETHER_BRICK,
            NETHER_BRICK,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            4,
            a + 1,
            z,
            4,
            b - 1,
            z,
            NETHER_BRICK,
            NETHER_BRICK,
            false,
        );
        if i & 1 == 0 {
            comp.fill_with_blocks(
                world,
                chunk,
                0,
                a + 2,
                z,
                0,
                a + 3,
                z,
                NETHER_BRICK_FENCE,
                NETHER_BRICK_FENCE,
                false,
            );
            comp.fill_with_blocks(
                world,
                chunk,
                4,
                a + 2,
                z,
                4,
                a + 3,
                z,
                NETHER_BRICK_FENCE,
                NETHER_BRICK_FENCE,
                false,
            );
        }
        for x in 0..=4 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Corridor4.addComponentParts` (`NeCTB`).
fn corridor4(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        8,
        1,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 8, 5, 8, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        0,
        8,
        6,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        2,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        2,
        0,
        8,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        0,
        1,
        4,
        0,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        3,
        0,
        7,
        4,
        0,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        4,
        8,
        2,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 1, 1, 4, 2, 2, 4, AIR, AIR, false);
    comp.fill_with_blocks(world, chunk, 6, 1, 4, 7, 2, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        8,
        8,
        3,
        8,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        6,
        0,
        3,
        7,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        3,
        6,
        8,
        3,
        7,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        4,
        0,
        5,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        3,
        4,
        8,
        5,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        5,
        2,
        5,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        3,
        5,
        7,
        5,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        4,
        5,
        1,
        5,
        5,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        4,
        5,
        7,
        5,
        5,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    // `for (var4 = 0; var4 <= 5; var4++) for (var5 = 0; var5 <= 8; var5++)` -> x = var5, z = var4.
    for z in 0..=5 {
        for x in 0..=8 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Corridor5.addComponentParts` (`NeSC`).
fn corridor5(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        4,
        1,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 4, 5, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        0,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        4,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        1,
        0,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        3,
        0,
        4,
        3,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        1,
        4,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        3,
        4,
        4,
        3,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        0,
        4,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        for z in 0..=4 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Crossing.addComponentParts` (`NeRC`).
fn crossing(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        6,
        1,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 6, 7, 6, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        1,
        6,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        6,
        1,
        6,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        2,
        0,
        6,
        6,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        2,
        6,
        6,
        6,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        0,
        6,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        5,
        0,
        6,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        2,
        0,
        6,
        6,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        2,
        5,
        6,
        6,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        6,
        0,
        4,
        6,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        0,
        4,
        5,
        0,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        6,
        6,
        4,
        6,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        6,
        4,
        5,
        6,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        2,
        0,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        2,
        0,
        5,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        6,
        2,
        6,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        5,
        2,
        6,
        5,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    for x in 0..=6 {
        for z in 0..=6 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Crossing2.addComponentParts` (`NeSCSC`).
fn crossing2(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        4,
        1,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 4, 5, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        0,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        4,
        5,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        4,
        0,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        4,
        4,
        5,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        0,
        4,
        6,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        for z in 0..=4 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Crossing3.addComponentParts` (`NeBCr`; also `Start`, which extends it).
fn crossing3(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        3,
        0,
        11,
        4,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        7,
        18,
        4,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 8, 5, 0, 10, 7, 18, AIR, AIR, false);
    comp.fill_with_blocks(world, chunk, 0, 5, 8, 18, 7, 10, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        5,
        0,
        7,
        5,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        5,
        11,
        7,
        5,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        0,
        11,
        5,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        11,
        11,
        5,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        7,
        7,
        5,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        7,
        18,
        5,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        11,
        7,
        5,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        11,
        18,
        5,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        2,
        0,
        11,
        2,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        2,
        13,
        11,
        2,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        0,
        0,
        11,
        1,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        0,
        15,
        11,
        1,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 7..=11 {
        for z in 0..=2 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, 18 - z);
        }
    }
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        7,
        5,
        2,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        13,
        2,
        7,
        18,
        2,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        7,
        3,
        1,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        15,
        0,
        7,
        18,
        1,
        11,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=2 {
        for z in 7..=11 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, 18 - x, -1, z);
        }
    }
}

/// `End.addComponentParts` (`NeBEF`), which uses its own `Random(seed)`.
fn end(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox, seed: i32) {
    let mut rand = JavaRandom::new(seed as i64);
    for x in 0..=4 {
        for y in 3..=4 {
            let d = rand.next_int_bound(8);
            comp.fill_with_blocks(
                world,
                chunk,
                x,
                y,
                0,
                x,
                y,
                d,
                NETHER_BRICK,
                NETHER_BRICK,
                false,
            );
        }
    }
    let d = rand.next_int_bound(8);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        0,
        0,
        5,
        d,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    let d = rand.next_int_bound(8);
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        5,
        0,
        4,
        5,
        d,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        let d = rand.next_int_bound(5);
        comp.fill_with_blocks(
            world,
            chunk,
            x,
            2,
            0,
            x,
            2,
            d,
            NETHER_BRICK,
            NETHER_BRICK,
            false,
        );
    }
    for x in 0..=4 {
        for y in 0..=1 {
            let d = rand.next_int_bound(3);
            comp.fill_with_blocks(
                world,
                chunk,
                x,
                y,
                0,
                x,
                y,
                d,
                NETHER_BRICK,
                NETHER_BRICK,
                false,
            );
        }
    }
}

/// `Entrance.addComponentParts` (`NeCE`).
fn entrance(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        0,
        12,
        4,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 5, 0, 12, 13, 12, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        0,
        1,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        0,
        12,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        11,
        4,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        11,
        10,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        9,
        11,
        7,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        0,
        4,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        0,
        10,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        9,
        0,
        7,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        11,
        2,
        10,
        12,
        10,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        8,
        0,
        7,
        8,
        0,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    let mut i = 1;
    while i <= 11 {
        comp.fill_with_blocks(
            world,
            chunk,
            i,
            10,
            0,
            i,
            11,
            0,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            i,
            10,
            12,
            i,
            11,
            12,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            0,
            10,
            i,
            0,
            11,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            12,
            10,
            i,
            12,
            11,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.set_block_state(world, NETHER_BRICK, i, 13, 0, chunk);
        comp.set_block_state(world, NETHER_BRICK, i, 13, 12, chunk);
        comp.set_block_state(world, NETHER_BRICK, 0, 13, i, chunk);
        comp.set_block_state(world, NETHER_BRICK, 12, 13, i, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, i + 1, 13, 0, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, i + 1, 13, 12, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, i + 1, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, 12, 13, i + 1, chunk);
        i += 2;
    }
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 0, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 12, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 0, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 12, 13, 0, chunk);

    let mut i = 3;
    while i <= 9 {
        comp.fill_with_blocks(
            world,
            chunk,
            1,
            7,
            i,
            1,
            8,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            11,
            7,
            i,
            11,
            8,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        i += 2;
    }

    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        8,
        2,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        4,
        12,
        2,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        0,
        0,
        8,
        1,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        0,
        9,
        8,
        1,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        4,
        3,
        1,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        9,
        0,
        4,
        12,
        1,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 4..=8 {
        for z in 0..=2 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, 12 - z);
        }
    }
    for x in 0..=2 {
        for z in 4..=8 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, 12 - x, -1, z);
        }
    }
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        5,
        5,
        7,
        5,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 6, 1, 6, 6, 4, 6, AIR, AIR, false);
    comp.set_block_state(world, NETHER_BRICK, 6, 0, 6, chunk);
    comp.set_block_state(world, FLOWING_LAVA, 6, 5, 6, chunk);
    // `World.forceBlockUpdateTick(Blocks.flowing_lava, ..)` is block-update semantics: the port
    // writes the lava and stops (see the module docs).
}

/// `NetherStalkRoom.addComponentParts` (`NeCSR`).
fn nether_stalk_room(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        0,
        12,
        4,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 5, 0, 12, 13, 12, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        0,
        1,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        11,
        5,
        0,
        12,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        11,
        4,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        11,
        10,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        9,
        11,
        7,
        12,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        0,
        4,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        0,
        10,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        9,
        0,
        7,
        12,
        1,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        11,
        2,
        10,
        12,
        10,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );

    let mut i = 1;
    while i <= 11 {
        comp.fill_with_blocks(
            world,
            chunk,
            i,
            10,
            0,
            i,
            11,
            0,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            i,
            10,
            12,
            i,
            11,
            12,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            0,
            10,
            i,
            0,
            11,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            12,
            10,
            i,
            12,
            11,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.set_block_state(world, NETHER_BRICK, i, 13, 0, chunk);
        comp.set_block_state(world, NETHER_BRICK, i, 13, 12, chunk);
        comp.set_block_state(world, NETHER_BRICK, 0, 13, i, chunk);
        comp.set_block_state(world, NETHER_BRICK, 12, 13, i, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, i + 1, 13, 0, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, i + 1, 13, 12, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, i + 1, chunk);
        comp.set_block_state(world, NETHER_BRICK_FENCE, 12, 13, i + 1, chunk);
        i += 2;
    }
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 0, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 12, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 0, 13, 0, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 12, 13, 0, chunk);

    let mut i = 3;
    while i <= 9 {
        comp.fill_with_blocks(
            world,
            chunk,
            1,
            7,
            i,
            1,
            8,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            11,
            7,
            i,
            11,
            8,
            i,
            NETHER_BRICK_FENCE,
            NETHER_BRICK_FENCE,
            false,
        );
        i += 2;
    }

    let top = stairs(comp, 3);
    for i in 0..=6 {
        let z = i + 4;
        for x in 5..=7 {
            comp.set_block_state(world, top, x, 5 + i, z, chunk);
        }
        if (5..=8).contains(&z) {
            comp.fill_with_blocks(
                world,
                chunk,
                5,
                5,
                z,
                7,
                i + 4,
                z,
                NETHER_BRICK,
                NETHER_BRICK,
                false,
            );
        } else if (9..=10).contains(&z) {
            comp.fill_with_blocks(
                world,
                chunk,
                5,
                8,
                z,
                7,
                i + 4,
                z,
                NETHER_BRICK,
                NETHER_BRICK,
                false,
            );
        }
        if i >= 1 {
            comp.fill_with_blocks(world, chunk, 5, 6 + i, z, 7, 9 + i, z, AIR, AIR, false);
        }
    }
    for x in 5..=7 {
        comp.set_block_state(world, top, x, 12, 11, chunk);
    }
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        6,
        7,
        5,
        7,
        7,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        7,
        6,
        7,
        7,
        7,
        7,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(world, chunk, 5, 13, 12, 7, 13, 12, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        2,
        3,
        5,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        9,
        3,
        5,
        10,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        4,
        2,
        5,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        9,
        5,
        2,
        10,
        5,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        9,
        5,
        9,
        10,
        5,
        10,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        10,
        5,
        4,
        10,
        5,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    let s0 = stairs(comp, 0);
    let s1 = stairs(comp, 1);
    comp.set_block_state(world, s1, 4, 5, 2, chunk);
    comp.set_block_state(world, s1, 4, 5, 3, chunk);
    comp.set_block_state(world, s1, 4, 5, 9, chunk);
    comp.set_block_state(world, s1, 4, 5, 10, chunk);
    comp.set_block_state(world, s0, 8, 5, 2, chunk);
    comp.set_block_state(world, s0, 8, 5, 3, chunk);
    comp.set_block_state(world, s0, 8, 5, 9, chunk);
    comp.set_block_state(world, s0, 8, 5, 10, chunk);
    comp.fill_with_blocks(world, chunk, 3, 4, 4, 4, 4, 8, SOUL_SAND, SOUL_SAND, false);
    comp.fill_with_blocks(world, chunk, 8, 4, 4, 9, 4, 8, SOUL_SAND, SOUL_SAND, false);
    comp.fill_with_blocks(
        world,
        chunk,
        3,
        5,
        4,
        4,
        5,
        8,
        NETHER_WART,
        NETHER_WART,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        8,
        5,
        4,
        9,
        5,
        8,
        NETHER_WART,
        NETHER_WART,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        0,
        8,
        2,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        4,
        12,
        2,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        0,
        0,
        8,
        1,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        0,
        9,
        8,
        1,
        12,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        4,
        3,
        1,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        9,
        0,
        4,
        12,
        1,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 4..=8 {
        for z in 0..=2 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, 12 - z);
        }
    }
    for x in 0..=2 {
        for z in 4..=8 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, 12 - x, -1, z);
        }
    }
}

/// `Stairs.addComponentParts` (`NeSR`).
fn stairs_room(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        6,
        1,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 6, 10, 6, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        1,
        8,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        2,
        0,
        6,
        8,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        1,
        0,
        8,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        2,
        1,
        6,
        8,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        2,
        6,
        5,
        8,
        6,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        2,
        0,
        5,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        3,
        2,
        6,
        5,
        2,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        3,
        4,
        6,
        5,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.set_block_state(world, NETHER_BRICK, 5, 2, 5, chunk);
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        2,
        5,
        4,
        3,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        3,
        2,
        5,
        3,
        4,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        2,
        5,
        2,
        5,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        2,
        5,
        1,
        6,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        7,
        1,
        5,
        7,
        4,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 6, 8, 2, 6, 8, 4, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        6,
        0,
        4,
        8,
        0,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        5,
        0,
        4,
        5,
        0,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    for x in 0..=6 {
        for z in 0..=6 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

/// `Straight.addComponentParts` (`NeBS`).
fn straight(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        0,
        4,
        4,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(world, chunk, 1, 5, 0, 3, 7, 18, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        0,
        0,
        5,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        5,
        0,
        4,
        5,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        0,
        4,
        2,
        5,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        2,
        13,
        4,
        2,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        0,
        4,
        1,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        0,
        15,
        4,
        1,
        18,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    for x in 0..=4 {
        for z in 0..=2 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, 18 - z);
        }
    }
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        1,
        1,
        0,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        4,
        0,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        3,
        14,
        0,
        4,
        14,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        1,
        17,
        0,
        4,
        17,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        1,
        1,
        4,
        4,
        1,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        4,
        4,
        4,
        4,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        3,
        14,
        4,
        4,
        14,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        4,
        1,
        17,
        4,
        4,
        17,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
}

/// `Throne.addComponentParts` (`NeMT`), including the blaze spawner.
fn throne(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox, has_spawner: &mut bool) {
    comp.fill_with_blocks(world, chunk, 0, 2, 0, 6, 7, 7, AIR, AIR, false);
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        0,
        0,
        5,
        1,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        2,
        1,
        5,
        2,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        3,
        2,
        5,
        3,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        4,
        3,
        5,
        4,
        7,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        2,
        0,
        1,
        4,
        2,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        2,
        0,
        5,
        4,
        2,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        5,
        2,
        1,
        5,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        5,
        5,
        2,
        5,
        5,
        3,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        5,
        3,
        0,
        5,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        5,
        3,
        6,
        5,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        5,
        8,
        5,
        5,
        8,
        NETHER_BRICK,
        NETHER_BRICK,
        false,
    );
    comp.set_block_state(world, NETHER_BRICK_FENCE, 1, 6, 3, chunk);
    comp.set_block_state(world, NETHER_BRICK_FENCE, 5, 6, 3, chunk);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        6,
        3,
        0,
        6,
        8,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        6,
        3,
        6,
        6,
        8,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        1,
        6,
        8,
        5,
        7,
        8,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        2,
        8,
        8,
        4,
        8,
        8,
        NETHER_BRICK_FENCE,
        NETHER_BRICK_FENCE,
        false,
    );
    if !*has_spawner {
        let (px, py, pz) = (
            comp.get_x_with_offset(3, 5),
            comp.get_y_with_offset(5),
            comp.get_z_with_offset(3, 5),
        );
        if chunk.is_vec_inside(px, py, pz) {
            *has_spawner = true;
            comp.set_block_state(world, MOB_SPAWNER, 3, 5, 5, chunk);
        }
    }
    for x in 0..=6 {
        for z in 0..=6 {
            comp.replace_air_and_liquid_downwards(world, chunk, NETHER_BRICK, x, -1, z);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::netherbridge::NetherBridge;

    /// Seed 0's fortress at (-37,213): at least one piece kind writes nether brick, and the
    /// `Throne` spawner / `Entrance` lava / corridor chest paths are exercised across the corpus.
    #[test]
    fn nether_parts_write_blocks() {
        let mut bridge = NetherBridge::new();
        bridge.generate(0, -37, 213);
        let starts = bridge.starts_intersecting(-37, 213);
        assert!(!starts.is_empty());
        let mut world = ChunkWorld::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.insert_chunk(-37 + dx, 213 + dz, vec![0u16; 16 * 16 * 256]);
            }
        }
        let mut rand = JavaRandom::new(0);
        let bbox = crate::structure::placement::structure_box(-37, 213);
        let mut wrote = false;
        for mut start in starts {
            crate::structure::placement::generate_structure(
                &mut start, &mut world, &mut rand, &bbox,
            );
            wrote |= world
                .chunk_blocks(-37, 213)
                .iter()
                .any(|&v| (v >> 4) == 112);
        }
        assert!(wrote, "the fortress pieces must write nether brick");
    }
}
