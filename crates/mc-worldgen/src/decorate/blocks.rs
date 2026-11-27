//! Block ids and `Material`/`Block` predicates the decoration generators need.
//!
//! The ids are the 1.8.9 registry ids; the material flags are dumped from the live game
//! (`oracle/java/src/MaterialTableOracle.java`) and the light opacity reuses
//! `chunk::light_opacity` (`oracle/java/src/BlockTableOracle.java`).

pub use crate::chunk::{is_full_block, light_opacity};

// Block ids (`Block.getBlockById`).
pub const AIR: i32 = 0;
pub const STONE: i32 = 1;
pub const GRASS: i32 = 2;
pub const DIRT: i32 = 3;
pub const COBBLESTONE: i32 = 4;
pub const FLOWING_WATER: i32 = 8;
pub const WATER: i32 = 9;
pub const FLOWING_LAVA: i32 = 10;
pub const LAVA: i32 = 11;
pub const SAND: i32 = 12;
pub const GRAVEL: i32 = 13;
pub const GOLD_ORE: i32 = 14;
pub const IRON_ORE: i32 = 15;
pub const COAL_ORE: i32 = 16;
pub const LOG: i32 = 17;
pub const LEAVES: i32 = 18;
pub const LAPIS_ORE: i32 = 21;
pub const SANDSTONE: i32 = 24;
pub const TALLGRASS: i32 = 31;
pub const DEADBUSH: i32 = 32;
pub const YELLOW_FLOWER: i32 = 37;
pub const RED_FLOWER: i32 = 38;
pub const BROWN_MUSHROOM: i32 = 39;
pub const RED_MUSHROOM: i32 = 40;
pub const STONE_SLAB: i32 = 44;
pub const MOSSY_COBBLESTONE: i32 = 48;
pub const REDSTONE_ORE: i32 = 73;
pub const DIAMOND_ORE: i32 = 56;
pub const FARMLAND: i32 = 60;
pub const ICE: i32 = 79;
pub const SNOW: i32 = 80;
/// `Blocks.snow_layer` (the thin layer; distinct from the snow block id 80).
pub const SNOW_LAYER: i32 = 78;
pub const CACTUS: i32 = 81;
pub const CLAY: i32 = 82;
pub const REEDS: i32 = 83;
pub const PUMPKIN: i32 = 86;
pub const MONSTER_EGG: i32 = 97;
pub const BROWN_MUSHROOM_BLOCK: i32 = 99;
pub const RED_MUSHROOM_BLOCK: i32 = 100;
pub const MELON: i32 = 103;
pub const VINE: i32 = 106;
pub const MYCELIUM: i32 = 110;
pub const WATERLILY: i32 = 111;
pub const COCOA: i32 = 127;
pub const EMERALD_ORE: i32 = 129;
pub const STAINED_HARDENED_CLAY: i32 = 159;
pub const LEAVES2: i32 = 161;
pub const LOG2: i32 = 162;
pub const HARDENED_CLAY: i32 = 172;
pub const PACKED_ICE: i32 = 174;
pub const DOUBLE_PLANT: i32 = 175;

/// `BlockDirt.EnumType.PODZOL` metadata.
pub const PODZOL_META: u16 = 2;

/// `Material.air` (`Block.getMaterial() == Material.air` = block id 0).
#[inline]
pub fn material_is_air(id: i32) -> bool {
    id == AIR
}

/// `Material.isLiquid()` (water and lava, still and flowing).
#[inline]
pub fn material_is_liquid(id: i32) -> bool {
    (FLOWING_WATER..=LAVA).contains(&id)
}

/// `Material.water` (still/flowing water).
#[inline]
pub fn material_is_water(id: i32) -> bool {
    id == FLOWING_WATER || id == WATER
}

/// `Material.leaves` (`Blocks.leaves`/`leaves2`).
#[inline]
pub fn material_is_leaves(id: i32) -> bool {
    id == LEAVES || id == LEAVES2
}

/// `Material.blocksMovement()` (dumped; the `pass`-material ids).
#[inline]
pub fn material_blocks_movement(id: i32) -> bool {
    !matches!(
        id,
        0 | 6
            | 8
            | 9
            | 10
            | 11
            | 27
            | 28
            | 30
            | 31
            | 32
            | 37
            | 38
            | 39
            | 40
            | 50
            | 51
            | 55
            | 59
            | 65
            | 66
            | 69
            | 75
            | 76
            | 77
            | 78
            | 83
            | 90
            | 93
            | 94
            | 104
            | 105
            | 106
            | 111
            | 115
            | 127
            | 131
            | 132
            | 140
            | 141
            | 142
            | 143
            | 144
            | 149
            | 150
            | 157
            | 171
            | 175
    )
}

/// `Material.isSolid()` (dumped; distinct from `blocksMovement` only for `web` (30)).
#[inline]
pub fn material_is_solid(id: i32) -> bool {
    id != 30 && material_blocks_movement(id)
}

/// `Material.isReplaceable()` (dumped; air, liquids, plants, fire, snow layer, vine, double plant).
#[inline]
pub fn material_is_replaceable(id: i32) -> bool {
    matches!(id, 0 | 8 | 9 | 10 | 11 | 31 | 32 | 51 | 78 | 106 | 175)
}
