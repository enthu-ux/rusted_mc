//! The `WorldGen*` tree generators used by `BiomeDecorator.genDecorations`' tree loop.
//!
//! Ported from the decompiled `net.minecraft.world.gen.feature` classes: `WorldGenTrees`,
//! `WorldGenBigTree`, `WorldGenForest`, `WorldGenTaiga1`, `WorldGenTaiga2`, `WorldGenSavannaTree`,
//! `WorldGenCanopyTree`, `WorldGenSwamp`, `WorldGenShrub`, `WorldGenHugeTrees` (+ `WorldGenMegaJungle`,
//! `WorldGenMegaPineTree`) and `WorldGenAbstractTree`'s helpers. The biome `genBigTreeChance` hooks
//! select the generator and draw any per-construction RNG (`4 + nextInt(7)` for jungle trees).
//!
//! `WorldGenBigTree` is **stateful**: its `heightLimit` is a field on the shared per-biome
//! `worldGeneratorBigTree` instance, set on the first `generate` (or by `validTreeLocation`) and reused.
//! The caller threads that per-biome `heightLimit` through `generate`.

use mc_core::jmath::{abs_int, cos, floor_double, floor_float, sin, sqrt_float};
use mc_core::random::JavaRandom;

use super::blocks;
use super::DecorateWorld;

// `BlockPlanks.EnumType` metadata.
const OAK: u8 = 0;
const SPRUCE: u8 = 1;
const BIRCH: u8 = 2;
const JUNGLE: u8 = 3;
const ACACIA: u8 = 4;
const DARK_OAK: u8 = 5;

/// `BlockLog.EnumAxis` (the two low bits of the metadata).
const AXIS_Y: u8 = 0;
const AXIS_X: u8 = 4;
const AXIS_Z: u8 = 8;

/// `BlockVine` metadata bits (`UP` is not serialised).
const VINE_SOUTH: u8 = 1;
const VINE_WEST: u8 = 2;
const VINE_NORTH: u8 = 4;
const VINE_EAST: u8 = 8;

/// The generator `BiomeGenBase.genBigTreeChance` selects, with its per-instance parameters.
#[derive(Clone, Copy, Debug)]
pub enum TreeKind {
    /// `WorldGenTrees` - oak (or a custom variant + vines).
    Trees {
        min_height: i32,
        wood: u8,
        leaves: u8,
        vines: bool,
    },
    /// `WorldGenForest` (birch; `extra` = `useExtraRandomHeight`).
    Forest {
        extra: bool,
    },
    /// `WorldGenBigTree` (the shared, stateful instance).
    BigTree,
    Taiga1,
    Taiga2,
    Savanna,
    Canopy,
    Swamp,
    /// `WorldGenShrub`.
    Shrub {
        wood: u8,
        leaves: u8,
    },
    MegaJungle,
    MegaPine {
        use_base_height: bool,
    },
}

/// `BlockLog.getStateFromMeta` (variant + axis).
fn log_state(variant: u8, axis: u8) -> u16 {
    let id = if variant < 4 {
        blocks::LOG
    } else {
        blocks::LOG2
    };
    let base = if variant < 4 { variant } else { variant - 4 };
    ((id as u16) << 4) | (base as u16) | (axis as u16)
}

/// The tree leaf state: `CHECK_DECAY = false`, `DECAYABLE = true` -> meta = variant.
fn leaves_state(variant: u8) -> u16 {
    let id = if variant < 4 {
        blocks::LEAVES
    } else {
        blocks::LEAVES2
    };
    let base = if variant < 4 { variant } else { variant - 4 };
    ((id as u16) << 4) | (base as u16)
}

fn vine_state(bits: u8) -> u16 {
    ((blocks::VINE as u16) << 4) | (bits as u16)
}

fn cocoa_state(age: i32, horizontal_index: u8) -> u16 {
    ((blocks::COCOA as u16) << 4) | (horizontal_index as u16) | ((age as u16) << 2)
}

/// `WorldGenAbstractTree.func_150523_a` (a block the tree may replace/overwrite).
fn leaf_check(id: i32) -> bool {
    id == blocks::AIR
        || blocks::material_is_leaves(id)
        || id == blocks::GRASS
        || id == blocks::DIRT
        || id == blocks::LOG
        || id == blocks::LOG2
        || id == 6 // sapling
        || id == blocks::VINE
}

/// `WorldGenAbstractTree.func_175921_a`: turn the block below the trunk into dirt (unless already dirt).
fn set_dirt(world: &mut DecorateWorld, x: i32, y: i32, z: i32) {
    if world.get_id(x, y, z) != blocks::DIRT {
        world.set(x, y, z, (blocks::DIRT as u16) << 4);
    }
}

/// `BiomeGenBase.genBigTreeChance` (dispatching on the biome's class/parameters).
pub fn big_tree_chance(biome_id: i32, rand: &mut JavaRandom) -> TreeKind {
    match biome_id {
        4 | 18 => forest_chance(0, rand),
        27 | 28 => forest_chance(2, rand),
        29 => forest_chance(3, rand),
        132 => forest_chance(1, rand),
        5 | 19 | 30 | 31 => taiga_chance(0, rand),
        32 | 33 => taiga_chance(1, rand),
        160 | 161 => taiga_chance(2, rand),
        21 | 22 => jungle_chance(false, rand),
        23 => jungle_chance(true, rand),
        6 => TreeKind::Swamp,
        35 | 36 | 163 | 164 => savanna_chance(rand),
        12 | 13 | 140 => TreeKind::Taiga2,
        37 | 38 | 39 | 165 | 166 | 167 => TreeKind::Trees {
            min_height: 4,
            wood: OAK,
            leaves: OAK,
            vines: false,
        },
        3 | 20 | 34 | 131 | 162 => hills_chance(rand),
        _ => base_chance(rand),
    }
}

fn base_chance(rand: &mut JavaRandom) -> TreeKind {
    if rand.next_int_bound(10) == 0 {
        TreeKind::BigTree
    } else {
        TreeKind::Trees {
            min_height: 4,
            wood: OAK,
            leaves: OAK,
            vines: false,
        }
    }
}

fn forest_chance(kind: i32, rand: &mut JavaRandom) -> TreeKind {
    if kind == 3 && rand.next_int_bound(3) > 0 {
        TreeKind::Canopy
    } else if kind != 2 && rand.next_int_bound(5) != 0 {
        TreeKind::Trees {
            min_height: 4,
            wood: OAK,
            leaves: OAK,
            vines: false,
        }
    } else {
        TreeKind::Forest { extra: false }
    }
}

fn taiga_chance(kind: i32, rand: &mut JavaRandom) -> TreeKind {
    if (kind == 1 || kind == 2) && rand.next_int_bound(3) == 0 {
        if kind != 2 && rand.next_int_bound(13) != 0 {
            TreeKind::MegaPine {
                use_base_height: false,
            }
        } else {
            TreeKind::MegaPine {
                use_base_height: true,
            }
        }
    } else if rand.next_int_bound(3) == 0 {
        TreeKind::Taiga1
    } else {
        TreeKind::Taiga2
    }
}

fn jungle_chance(edge: bool, rand: &mut JavaRandom) -> TreeKind {
    if rand.next_int_bound(10) == 0 {
        TreeKind::BigTree
    } else if rand.next_int_bound(2) == 0 {
        TreeKind::Shrub {
            wood: JUNGLE,
            leaves: OAK,
        }
    } else if !edge && rand.next_int_bound(3) == 0 {
        TreeKind::MegaJungle
    } else {
        TreeKind::Trees {
            min_height: 4 + rand.next_int_bound(7),
            wood: JUNGLE,
            leaves: JUNGLE,
            vines: true,
        }
    }
}

fn hills_chance(rand: &mut JavaRandom) -> TreeKind {
    if rand.next_int_bound(3) > 0 {
        TreeKind::Taiga2
    } else {
        base_chance(rand)
    }
}

fn savanna_chance(rand: &mut JavaRandom) -> TreeKind {
    if rand.next_int_bound(5) > 0 {
        TreeKind::Savanna
    } else {
        TreeKind::Trees {
            min_height: 4,
            wood: OAK,
            leaves: OAK,
            vines: false,
        }
    }
}

/// Runs the selected generator at `(x, y, z)`. Returns whether it placed a tree.
pub fn generate(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    kind: TreeKind,
    big_tree_height_limit: &mut i32,
) -> bool {
    match kind {
        TreeKind::Trees {
            min_height,
            wood,
            leaves,
            vines,
        } => gen_trees(world, rand, x, y, z, min_height, wood, leaves, vines),
        TreeKind::Forest { extra } => gen_forest(world, rand, x, y, z, extra),
        TreeKind::BigTree => gen_big_tree(world, rand, x, y, z, big_tree_height_limit),
        TreeKind::Taiga1 => gen_taiga1(world, rand, x, y, z),
        TreeKind::Taiga2 => gen_taiga2(world, rand, x, y, z),
        TreeKind::Savanna => gen_savanna(world, rand, x, y, z),
        TreeKind::Canopy => gen_canopy(world, rand, x, y, z),
        TreeKind::Swamp => gen_swamp(world, rand, x, y, z),
        TreeKind::Shrub { wood, leaves } => gen_shrub(world, rand, x, y, z, wood, leaves),
        TreeKind::MegaJungle => gen_mega_jungle(world, rand, x, y, z),
        TreeKind::MegaPine { use_base_height } => {
            gen_mega_pine(world, rand, x, y, z, use_base_height)
        }
    }
}

/// `WorldGenAbstractTree.func_180711_a` after a successful generate (only the mega pine does work).
pub fn post_generate(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    kind: TreeKind,
) {
    if let TreeKind::MegaPine { .. } = kind {
        mega_pine_post(world, rand, x, y, z);
    }
}

// ---- WorldGenTrees ----

#[allow(clippy::too_many_arguments)]
fn gen_trees(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    min_height: i32,
    wood: u8,
    leaves: u8,
    vines_grow: bool,
) -> bool {
    let height = rand.next_int_bound(3) + min_height;
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }

    for by in y..=(y + 1 + height) {
        let mut r = 1;
        if by == y {
            r = 0;
        }
        if by >= y + 1 + height - 2 {
            r = 2;
        }
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) || !leaf_check(world.get_id(bx, by, bz)) {
                    ok = false;
                }
            }
        }
    }
    if !ok {
        return false;
    }

    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT || below == blocks::FARMLAND)
        || y >= 256 - height - 1
    {
        return false;
    }
    set_dirt(world, x, y - 1, z);

    let wood_state = log_state(wood, AXIS_Y);
    let leaf_state = leaves_state(leaves);

    for by in (y - 3 + height)..=(y + height) {
        let d = by - (y + height);
        let leaf_r = 1 - d / 2;
        for bx in (x - leaf_r)..=(x + leaf_r) {
            let dx = bx - x;
            for bz in (z - leaf_r)..=(z + leaf_r) {
                let dz = bz - z;
                if dx.abs() != leaf_r
                    || dz.abs() != leaf_r
                    || (rand.next_int_bound(2) != 0 && d != 0)
                {
                    let id = world.get_id(bx, by, bz);
                    if id == blocks::AIR || blocks::material_is_leaves(id) || id == blocks::VINE {
                        world.set(bx, by, bz, leaf_state);
                    }
                }
            }
        }
    }

    for i in 0..height {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR || blocks::material_is_leaves(id) || id == blocks::VINE {
            world.set(x, y + i, z, wood_state);
            if vines_grow && i > 0 {
                if rand.next_int_bound(3) > 0 && world.is_air(x - 1, y + i, z) {
                    world.set(x - 1, y + i, z, vine_state(VINE_EAST));
                }
                if rand.next_int_bound(3) > 0 && world.is_air(x + 1, y + i, z) {
                    world.set(x + 1, y + i, z, vine_state(VINE_WEST));
                }
                if rand.next_int_bound(3) > 0 && world.is_air(x, y + i, z - 1) {
                    world.set(x, y + i, z - 1, vine_state(VINE_SOUTH));
                }
                if rand.next_int_bound(3) > 0 && world.is_air(x, y + i, z + 1) {
                    world.set(x, y + i, z + 1, vine_state(VINE_NORTH));
                }
            }
        }
    }

    if vines_grow {
        for by in (y - 3 + height)..=(y + height) {
            let d = by - (y + height);
            let r = 2 - d / 2;
            for bx in (x - r)..=(x + r) {
                for bz in (z - r)..=(z + r) {
                    if blocks::material_is_leaves(world.get_id(bx, by, bz)) {
                        if rand.next_int_bound(4) == 0
                            && world.get_id(bx - 1, by, bz) == blocks::AIR
                        {
                            vine_down(world, bx - 1, by, bz, VINE_EAST);
                        }
                        if rand.next_int_bound(4) == 0
                            && world.get_id(bx + 1, by, bz) == blocks::AIR
                        {
                            vine_down(world, bx + 1, by, bz, VINE_WEST);
                        }
                        if rand.next_int_bound(4) == 0
                            && world.get_id(bx, by, bz - 1) == blocks::AIR
                        {
                            vine_down(world, bx, by, bz - 1, VINE_SOUTH);
                        }
                        if rand.next_int_bound(4) == 0
                            && world.get_id(bx, by, bz + 1) == blocks::AIR
                        {
                            vine_down(world, bx, by, bz + 1, VINE_NORTH);
                        }
                    }
                }
            }
        }

        if rand.next_int_bound(5) == 0 && height > 5 {
            for i in 0..2 {
                // `Plane.HORIZONTAL` = {NORTH, EAST, SOUTH, WEST}; `getMetaFromState` uses the
                // horizontal index, and the cocoa sits on the opposite side of the trunk.
                for (dx, dz, opp_dx, opp_dz, hindex) in [
                    (0, -1, 0, 1, 2u8), // NORTH
                    (1, 0, -1, 0, 3),   // EAST
                    (0, 1, 0, -1, 0),   // SOUTH
                    (-1, 0, 1, 0, 1),   // WEST
                ] {
                    let _ = (dx, dz);
                    if rand.next_int_bound(4 - i) == 0 {
                        let age = rand.next_int_bound(3);
                        world.set(
                            x + opp_dx,
                            y + height - 5 + i,
                            z + opp_dz,
                            cocoa_state(age, hindex),
                        );
                    }
                }
            }
        }
    }

    true
}

fn vine_down(world: &mut DecorateWorld, x: i32, y: i32, z: i32, bits: u8) {
    world.set(x, y, z, vine_state(bits));
    let mut remaining = 4;
    let mut yy = y - 1;
    while world.is_air(x, yy, z) && remaining > 0 {
        world.set(x, yy, z, vine_state(bits));
        remaining -= 1;
        yy -= 1;
    }
}

// ---- WorldGenForest (birch) ----

fn gen_forest(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    extra: bool,
) -> bool {
    let mut height = rand.next_int_bound(3) + 5;
    if extra {
        height += rand.next_int_bound(7);
    }
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }
    for by in y..=(y + 1 + height) {
        let mut r = 1;
        if by == y {
            r = 0;
        }
        if by >= y + 1 + height - 2 {
            r = 2;
        }
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) || !leaf_check(world.get_id(bx, by, bz)) {
                    ok = false;
                }
            }
        }
    }
    if !ok {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT || below == blocks::FARMLAND)
        || y >= 256 - height - 1
    {
        return false;
    }
    set_dirt(world, x, y - 1, z);
    let wood = log_state(BIRCH, AXIS_Y);
    let leaf = leaves_state(BIRCH);
    for by in (y - 3 + height)..=(y + height) {
        let d = by - (y + height);
        let r = 1 - d / 2;
        for bx in (x - r)..=(x + r) {
            let dx = bx - x;
            for bz in (z - r)..=(z + r) {
                let dz = bz - z;
                if dx.abs() != r || dz.abs() != r || (rand.next_int_bound(2) != 0 && d != 0) {
                    let id = world.get_id(bx, by, bz);
                    if id == blocks::AIR || blocks::material_is_leaves(id) {
                        world.set(bx, by, bz, leaf);
                    }
                }
            }
        }
    }
    for i in 0..height {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(x, y + i, z, wood);
        }
    }
    true
}

// ---- WorldGenTaiga1 ----

fn gen_taiga1(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) -> bool {
    let height = rand.next_int_bound(5) + 7;
    let trunk = height - rand.next_int_bound(2) - 3;
    let rest = height - trunk;
    let leaf_r = 1 + rand.next_int_bound(rest + 1);
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }

    let mut by = y;
    while by <= y + 1 + height && ok {
        let r = if by - y < trunk { 0 } else { leaf_r };
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) || !leaf_check(world.get_id(bx, by, bz)) {
                    ok = false;
                }
            }
        }
        by += 1;
    }
    if !ok {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT) || y >= 256 - height - 1 {
        return false;
    }
    set_dirt(world, x, y - 1, z);

    let wood = log_state(SPRUCE, AXIS_Y);
    let leaf = leaves_state(SPRUCE);
    let mut r = 0i32;
    let mut by = y + height;
    while by >= y + trunk {
        for bx in (x - r)..=(x + r) {
            let dx = bx - x;
            for bz in (z - r)..=(z + r) {
                let dz = bz - z;
                if dx.abs() != r || dz.abs() != r || r <= 0 {
                    if !blocks::is_full_block(world.get_id(bx, by, bz)) {
                        world.set(bx, by, bz, leaf);
                    }
                }
            }
        }
        if r >= 1 && by == y + trunk + 1 {
            r -= 1;
        } else if r < leaf_r {
            r += 1;
        }
        by -= 1;
    }
    for i in 0..height - 1 {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(x, y + i, z, wood);
        }
    }
    true
}

// ---- WorldGenTaiga2 ----

fn gen_taiga2(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) -> bool {
    let height = rand.next_int_bound(4) + 6;
    let trunk = 1 + rand.next_int_bound(2);
    let rest = height - trunk;
    let leaf_r = 2 + rand.next_int_bound(2);
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }
    for by in y..=(y + 1 + height) {
        let r = if by - y < trunk { 0 } else { leaf_r };
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) {
                    ok = false;
                } else {
                    let id = world.get_id(bx, by, bz);
                    if id != blocks::AIR && !blocks::material_is_leaves(id) {
                        ok = false;
                    }
                }
            }
        }
    }
    if !ok {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT || below == blocks::FARMLAND)
        || y >= 256 - height - 1
    {
        return false;
    }
    set_dirt(world, x, y - 1, z);

    let wood = log_state(SPRUCE, AXIS_Y);
    let leaf = leaves_state(SPRUCE);
    let mut r = rand.next_int_bound(2);
    let mut dr = 1;
    let mut max = 0;
    for i in 0..=rest {
        let by = y + height - i;
        for bx in (x - r)..=(x + r) {
            let dx = bx - x;
            for bz in (z - r)..=(z + r) {
                let dz = bz - z;
                if dx.abs() != r || dz.abs() != r || r <= 0 {
                    if !blocks::is_full_block(world.get_id(bx, by, bz)) {
                        world.set(bx, by, bz, leaf);
                    }
                }
            }
        }
        if r >= dr {
            r = max;
            max = 1;
            dr += 1;
            if dr > leaf_r {
                dr = leaf_r;
            }
        } else {
            r += 1;
        }
    }
    let top = rand.next_int_bound(3);
    for i in 0..height - top {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(x, y + i, z, wood);
        }
    }
    true
}

// ---- WorldGenSavannaTree (acacia, log2/leaves2) ----

fn gen_savanna(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) -> bool {
    let height = rand.next_int_bound(3) + rand.next_int_bound(3) + 5;
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }
    for by in y..=(y + 1 + height) {
        let mut r = 1;
        if by == y {
            r = 0;
        }
        if by >= y + 1 + height - 2 {
            r = 2;
        }
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) || !leaf_check(world.get_id(bx, by, bz)) {
                    ok = false;
                }
            }
        }
    }
    if !ok {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT) || y >= 256 - height - 1 {
        return false;
    }
    set_dirt(world, x, y - 1, z);

    let wood = log_state(ACACIA, AXIS_Y);
    let leaf = leaves_state(ACACIA);
    // `Plane.HORIZONTAL.random`: {NORTH, EAST, SOUTH, WEST}.
    let branch = rand.next_int_bound(4);
    let (bdx, bdz) = [(0, -1), (1, 0), (0, 1), (-1, 0)][branch as usize];
    let branch_at = height - rand.next_int_bound(4) - 1;
    let mut branch_len = 3 - rand.next_int_bound(3);
    let mut tx = x;
    let mut tz = z;
    let mut last = 0;

    for i in 0..height {
        let by = y + i;
        if i >= branch_at && branch_len > 0 {
            tx += bdx;
            tz += bdz;
            branch_len -= 1;
        }
        let pos = (tx, by, tz);
        let id = world.get_id(pos.0, pos.1, pos.2);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(tx, by, tz, wood);
            last = by;
        }
    }

    let top_x = tx;
    let top_z = tz;
    let leaf_at = |world: &mut DecorateWorld, px: i32, py: i32, pz: i32| {
        let id = world.get_id(px, py, pz);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(px, py, pz, leaf);
        }
    };
    for dx in -3i32..=3 {
        for dz in -3i32..=3 {
            if dx.abs() != 3 || dz.abs() != 3 {
                leaf_at(world, top_x + dx, last, top_z + dz);
            }
        }
    }
    let cy = last + 1;
    for dx in -1..=1 {
        for dz in -1..=1 {
            leaf_at(world, top_x + dx, cy, top_z + dz);
        }
    }
    leaf_at(world, top_x + 2, cy, top_z);
    leaf_at(world, top_x - 2, cy, top_z);
    leaf_at(world, top_x, cy, top_z + 2);
    leaf_at(world, top_x, cy, top_z - 2);

    // Second branch.
    let bx0 = x;
    let bz0 = z;
    let branch2 = rand.next_int_bound(4);
    let (b2dx, b2dz) = [(0, -1), (1, 0), (0, 1), (-1, 0)][branch2 as usize];
    if branch2 != branch {
        let b2_at = branch_at - rand.next_int_bound(2) - 1;
        let mut b2_len = 1 + rand.next_int_bound(3);
        let mut tx = bx0;
        let mut tz = bz0;
        let mut last2 = 0;
        let mut i = b2_at;
        while i < height && b2_len > 0 {
            if i >= 1 {
                let by = y + i;
                tx += b2dx;
                tz += b2dz;
                let id = world.get_id(tx, by, tz);
                if id == blocks::AIR || blocks::material_is_leaves(id) {
                    world.set(tx, by, tz, wood);
                    last2 = by;
                }
            }
            i += 1;
            b2_len -= 1;
        }
        if last2 > 0 {
            let tx2 = tx;
            let tz2 = tz;
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    if dx.abs() != 2 || dz.abs() != 2 {
                        leaf_at(world, tx2 + dx, last2, tz2 + dz);
                    }
                }
            }
            let cy2 = last2 + 1;
            for dx in -1..=1 {
                for dz in -1..=1 {
                    leaf_at(world, tx2 + dx, cy2, tz2 + dz);
                }
            }
        }
    }
    true
}

// ---- WorldGenCanopyTree (dark oak, log2/leaves2) ----

fn gen_canopy(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) -> bool {
    let height = rand.next_int_bound(3) + rand.next_int_bound(2) + 6;
    if !(y >= 1 && y + height + 1 < 256) {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if below != blocks::GRASS && below != blocks::DIRT {
        return false;
    }
    if !canopy_valid(world, x, y, z, height) {
        return false;
    }
    set_dirt(world, x, y - 1, z);
    set_dirt(world, x + 1, y - 1, z);
    set_dirt(world, x, y - 1, z + 1);
    set_dirt(world, x + 1, y - 1, z + 1);

    let wood = log_state(DARK_OAK, AXIS_Y);
    let leaf = leaves_state(DARK_OAK);
    let dir = rand.next_int_bound(4);
    let (dx, dz) = [(0, -1), (1, 0), (0, 1), (-1, 0)][dir as usize];
    let bend_at = height - rand.next_int_bound(4);
    let mut bend_len = 2 - rand.next_int_bound(3);
    let mut tx = x;
    let mut tz = z;
    let top_y = y + height - 1;

    for i in 0..height {
        if i >= bend_at && bend_len > 0 {
            tx += dx;
            tz += dz;
            bend_len -= 1;
        }
        let by = y + i;
        let id = world.get_id(tx, by, tz);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            canopy_log(world, tx, by, tz, wood);
            canopy_log(world, tx + 1, by, tz, wood);
            canopy_log(world, tx, by, tz + 1, wood);
            canopy_log(world, tx + 1, by, tz + 1, wood);
        }
    }

    for dx in -2..=0 {
        for dz in -2..=0 {
            canopy_leaf(world, tx + dx, top_y - 1, tz + dz, leaf);
            canopy_leaf(world, 1 + tx - dx, top_y - 1, tz + dz, leaf);
            canopy_leaf(world, tx + dx, top_y - 1, 1 + tz - dz, leaf);
            canopy_leaf(world, 1 + tx - dx, top_y - 1, 1 + tz - dz, leaf);
            if (dx > -2 || dz > -1) && (dx != -1 || dz != -2) {
                canopy_leaf(world, tx + dx, top_y, tz + dz, leaf);
                canopy_leaf(world, 1 + tx - dx, top_y, tz + dz, leaf);
                canopy_leaf(world, tx + dx, top_y, 1 + tz - dz, leaf);
                canopy_leaf(world, 1 + tx - dx, top_y, 1 + tz - dz, leaf);
            }
        }
    }

    if rand.next_boolean() {
        canopy_leaf(world, tx, top_y + 2, tz, leaf);
        canopy_leaf(world, tx + 1, top_y + 2, tz, leaf);
        canopy_leaf(world, tx + 1, top_y + 2, tz + 1, leaf);
        canopy_leaf(world, tx, top_y + 2, tz + 1, leaf);
    }

    for dx in -3i32..=4 {
        for dz in -3i32..=4 {
            if (dx != -3 || dz != -3)
                && (dx != -3 || dz != 4)
                && (dx != 4 || dz != -3)
                && (dx != 4 || dz != 4)
                && (dx.abs() < 3 || dz.abs() < 3)
            {
                canopy_leaf(world, tx + dx, top_y, tz + dz, leaf);
            }
        }
    }

    for dx in -1..=2 {
        for dz in -1..=2 {
            if (dx < 0 || dx > 1 || dz < 0 || dz > 1) && rand.next_int_bound(3) <= 0 {
                let len = rand.next_int_bound(3) + 2;
                for k in 0..len {
                    canopy_log(world, x + dx, top_y - k - 1, z + dz, wood);
                }
                for ddx in -1..=1 {
                    for ddz in -1..=1 {
                        canopy_leaf(world, tx + dx + ddx, top_y, tz + dz + ddz, leaf);
                    }
                }
                for ddx in -2i32..=2 {
                    for ddz in -2i32..=2 {
                        if ddx.abs() != 2 || ddz.abs() != 2 {
                            canopy_leaf(world, tx + dx + ddx, top_y - 1, tz + dz + ddz, leaf);
                        }
                    }
                }
            }
        }
    }
    true
}

fn canopy_valid(world: &DecorateWorld, x: i32, y: i32, z: i32, height: i32) -> bool {
    for i in 0..=(height + 1) {
        let mut r = 1;
        if i == 0 {
            r = 0;
        }
        if i >= height - 1 {
            r = 2;
        }
        for dx in -r..=r {
            for dz in -r..=r {
                if !leaf_check(world.get_id(x + dx, y + i, z + dz)) {
                    return false;
                }
            }
        }
    }
    true
}

fn canopy_log(world: &mut DecorateWorld, x: i32, y: i32, z: i32, state: u16) {
    if leaf_check(world.get_id(x, y, z)) {
        world.set(x, y, z, state);
    }
}

fn canopy_leaf(world: &mut DecorateWorld, x: i32, y: i32, z: i32, state: u16) {
    if world.get_id(x, y, z) == blocks::AIR {
        world.set(x, y, z, state);
    }
}

// ---- WorldGenSwamp ----

fn gen_swamp(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, mut y: i32, z: i32) -> bool {
    let height = rand.next_int_bound(4) + 5;
    while blocks::material_is_water(world.get_id(x, y - 1, z)) {
        y -= 1;
    }
    let mut ok = true;
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }
    for by in y..=(y + 1 + height) {
        let mut r = 1;
        if by == y {
            r = 0;
        }
        if by >= y + 1 + height - 2 {
            r = 3;
        }
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if !(0..256).contains(&by) {
                    ok = false;
                } else {
                    let id = world.get_id(bx, by, bz);
                    if id != blocks::AIR && !blocks::material_is_leaves(id) {
                        if id != blocks::WATER && id != blocks::FLOWING_WATER {
                            ok = false;
                        } else if by > y {
                            ok = false;
                        }
                    }
                }
            }
        }
    }
    if !ok {
        return false;
    }
    let below = world.get_id(x, y - 1, z);
    if !(below == blocks::GRASS || below == blocks::DIRT) || y >= 256 - height - 1 {
        return false;
    }
    set_dirt(world, x, y - 1, z);

    let wood = log_state(OAK, AXIS_Y);
    let leaf = leaves_state(OAK);
    for by in (y - 3 + height)..=(y + height) {
        let d = by - (y + height);
        let r = 2 - d / 2;
        for bx in (x - r)..=(x + r) {
            let dx = bx - x;
            for bz in (z - r)..=(z + r) {
                let dz = bz - z;
                if dx.abs() != r || dz.abs() != r || (rand.next_int_bound(2) != 0 && d != 0) {
                    if !blocks::is_full_block(world.get_id(bx, by, bz)) {
                        world.set(bx, by, bz, leaf);
                    }
                }
            }
        }
    }
    for i in 0..height {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR
            || blocks::material_is_leaves(id)
            || id == blocks::FLOWING_WATER
            || id == blocks::WATER
        {
            world.set(x, y + i, z, wood);
        }
    }
    for by in (y - 3 + height)..=(y + height) {
        let d = by - (y + height);
        let r = 2 - d / 2;
        for bx in (x - r)..=(x + r) {
            for bz in (z - r)..=(z + r) {
                if blocks::material_is_leaves(world.get_id(bx, by, bz)) {
                    if rand.next_int_bound(4) == 0 && world.get_id(bx - 1, by, bz) == blocks::AIR {
                        swamp_vine(world, bx - 1, by, bz, VINE_EAST);
                    }
                    if rand.next_int_bound(4) == 0 && world.get_id(bx + 1, by, bz) == blocks::AIR {
                        swamp_vine(world, bx + 1, by, bz, VINE_WEST);
                    }
                    if rand.next_int_bound(4) == 0 && world.get_id(bx, by, bz - 1) == blocks::AIR {
                        swamp_vine(world, bx, by, bz - 1, VINE_SOUTH);
                    }
                    if rand.next_int_bound(4) == 0 && world.get_id(bx, by, bz + 1) == blocks::AIR {
                        swamp_vine(world, bx, by, bz + 1, VINE_NORTH);
                    }
                }
            }
        }
    }
    true
}

fn swamp_vine(world: &mut DecorateWorld, x: i32, y: i32, z: i32, bits: u8) {
    let state = vine_state(bits);
    world.set(x, y, z, state);
    let mut remaining = 4;
    let mut yy = y - 1;
    while world.is_air(x, yy, z) && remaining > 0 {
        world.set(x, yy, z, state);
        remaining -= 1;
        yy -= 1;
    }
}

// ---- WorldGenShrub ----

fn gen_shrub(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    mut y: i32,
    z: i32,
    wood: u8,
    leaves: u8,
) -> bool {
    while (world.is_air(x, y, z) || blocks::material_is_leaves(world.get_id(x, y, z))) && y > 0 {
        y -= 1;
    }
    let below = world.get_id(x, y, z);
    if below == blocks::DIRT || below == blocks::GRASS {
        y += 1;
        world.set(x, y, z, log_state(wood, AXIS_Y));
        let leaf = leaves_state(leaves);
        for by in y..=(y + 2) {
            let r = 2 - (by - y);
            for bx in (x - r)..=(x + r) {
                let dx = bx - x;
                for bz in (z - r)..=(z + r) {
                    let dz = bz - z;
                    if dx.abs() != r || dz.abs() != r || rand.next_int_bound(2) != 0 {
                        if !blocks::is_full_block(world.get_id(bx, by, bz)) {
                            world.set(bx, by, bz, leaf);
                        }
                    }
                }
            }
        }
    }
    true
}

// ---- WorldGenHugeTrees (shared by the mega trees) ----

fn huge_height(rand: &mut JavaRandom, base: i32, extra: i32) -> i32 {
    let mut h = rand.next_int_bound(3) + base;
    if extra > 1 {
        h += rand.next_int_bound(extra);
    }
    h
}

fn huge_valid(world: &DecorateWorld, x: i32, y: i32, z: i32, height: i32) -> bool {
    if !(y >= 1 && y + height + 1 <= 256) {
        return false;
    }
    for i in 0..=(1 + height) {
        let rr = if i == 0 { 1 } else { 2 };
        for dx in -rr..=rr {
            for dz in -rr..=rr {
                if !(0..256).contains(&(y + i)) || !leaf_check(world.get_id(x + dx, y + i, z + dz))
                {
                    return false;
                }
            }
        }
    }
    true
}

fn huge_valid_2x2(world: &mut DecorateWorld, x: i32, y: i32, z: i32) -> bool {
    let below = world.get_id(x, y - 1, z);
    if (below == blocks::GRASS || below == blocks::DIRT) && y >= 2 {
        set_dirt(world, x, y - 1, z);
        set_dirt(world, x + 1, y - 1, z);
        set_dirt(world, x, y - 1, z + 1);
        set_dirt(world, x + 1, y - 1, z + 1);
        true
    } else {
        false
    }
}

/// `WorldGenHugeTrees.func_175925_a`: a 2x2-ish leaf blob (a `+1` margin ring).
fn huge_leaf(world: &mut DecorateWorld, x: i32, y: i32, z: i32, r: i32, leaf: u16) {
    let rr = r * r;
    for dx in -r..=r + 1 {
        for dz in -r..=r + 1 {
            let ax = dx - 1;
            let az = dz - 1;
            if dx * dx + dz * dz <= rr
                || ax * ax + az * az <= rr
                || dx * dx + az * az <= rr
                || ax * ax + dz * dz <= rr
            {
                let id = world.get_id(x + dx, y, z + dz);
                if id == blocks::AIR || blocks::material_is_leaves(id) {
                    world.set(x + dx, y, z + dz, leaf);
                }
            }
        }
    }
}

/// `WorldGenHugeTrees.func_175928_b`: a round leaf disc.
fn huge_leaf_disc(world: &mut DecorateWorld, x: i32, y: i32, z: i32, r: i32, leaf: u16) {
    let rr = r * r;
    for dx in -r..=r {
        for dz in -r..=r {
            if dx * dx + dz * dz <= rr {
                let id = world.get_id(x + dx, y, z + dz);
                if id == blocks::AIR || blocks::material_is_leaves(id) {
                    world.set(x + dx, y, z + dz, leaf);
                }
            }
        }
    }
}

fn gen_mega_jungle(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    let height = huge_height(rand, 10, 20);
    if !huge_valid(world, x, y, z, height) || !huge_valid_2x2(world, x, y, z) {
        return false;
    }
    let wood = log_state(JUNGLE, AXIS_Y);
    let leaf = leaves_state(JUNGLE);
    // Top cap.
    for up in -2..=0 {
        huge_leaf(world, x, y + height + up, z, 3 - up, leaf);
    }

    // Branches.
    let mut by = y + height - 2 - rand.next_int_bound(4);
    while by > y + height / 2 {
        let angle = rand.next_float() * std::f32::consts::PI * 2.0;
        let bx = x + (0.5 + cos(angle) as f64 * 4.0) as i32;
        let bz = z + (0.5 + sin(angle) as f64 * 4.0) as i32;
        let mut bx2 = bx;
        let mut bz2 = bz;
        for i in 0..5 {
            bx2 = x + (1.5 + cos(angle) * i as f32) as i32;
            bz2 = z + (1.5 + sin(angle) * i as f32) as i32;
            world.set(bx2, by - 3 + i / 2, bz2, wood);
        }
        let r = 1 + rand.next_int_bound(2);
        for yy in (by - r)..=by {
            huge_leaf_disc(world, bx2, yy, bz2, 1 - (yy - by), leaf);
        }
        by -= 2 + rand.next_int_bound(4);
    }

    for i in 0..height {
        let id = world.get_id(x, y + i, z);
        if leaf_check(id) {
            world.set(x, y + i, z, wood);
            if i > 0 {
                mega_vine(world, rand, x - 1, y + i, z, VINE_EAST);
                mega_vine(world, rand, x, y + i, z - 1, VINE_SOUTH);
            }
        }
        if i < height - 1 {
            let id = world.get_id(x + 1, y + i, z);
            if leaf_check(id) {
                world.set(x + 1, y + i, z, wood);
                if i > 0 {
                    mega_vine(world, rand, x + 2, y + i, z, VINE_WEST);
                    mega_vine(world, rand, x + 1, y + i, z - 1, VINE_SOUTH);
                }
            }
            let id = world.get_id(x + 1, y + i, z + 1);
            if leaf_check(id) {
                world.set(x + 1, y + i, z + 1, wood);
                if i > 0 {
                    mega_vine(world, rand, x + 2, y + i, z + 1, VINE_WEST);
                    mega_vine(world, rand, x + 1, y + i, z + 2, VINE_NORTH);
                }
            }
            let id = world.get_id(x, y + i, z + 1);
            if leaf_check(id) {
                world.set(x, y + i, z + 1, wood);
                if i > 0 {
                    mega_vine(world, rand, x - 1, y + i, z + 1, VINE_EAST);
                    mega_vine(world, rand, x, y + i, z + 2, VINE_NORTH);
                }
            }
        }
    }
    true
}

fn mega_vine(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32, bits: u8) {
    if rand.next_int_bound(3) > 0 && world.is_air(x, y, z) {
        world.set(x, y, z, vine_state(bits));
    }
}

fn mega_pine_disc(
    world: &mut DecorateWorld,
    x: i32,
    z: i32,
    y: i32,
    r0: i32,
    rand: &mut JavaRandom,
    base: i32,
    use_base_height: bool,
) {
    let r = rand.next_int_bound(5) + if use_base_height { base } else { 3 };
    let mut prev = 0;
    for yy in (y - r)..=y {
        let d = y - yy;
        let radius = r0 + floor_float(d as f32 / r as f32 * 3.5);
        let extra = if d > 0 && radius == prev && (yy & 1) == 0 {
            1
        } else {
            0
        };
        huge_leaf(world, x, yy, z, radius + extra, leaves_state(SPRUCE));
        prev = radius;
    }
}

fn gen_mega_pine(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    use_base_height: bool,
) -> bool {
    let height = huge_height(rand, 13, 15);
    if !huge_valid(world, x, y, z, height) || !huge_valid_2x2(world, x, y, z) {
        return false;
    }
    let wood = log_state(SPRUCE, AXIS_Y);
    mega_pine_disc(world, x, z, y + height, 0, rand, 13, use_base_height);

    for i in 0..height {
        let id = world.get_id(x, y + i, z);
        if id == blocks::AIR || blocks::material_is_leaves(id) {
            world.set(x, y + i, z, wood);
        }
        if i < height - 1 {
            let id = world.get_id(x + 1, y + i, z);
            if id == blocks::AIR || blocks::material_is_leaves(id) {
                world.set(x + 1, y + i, z, wood);
            }
            let id = world.get_id(x + 1, y + i, z + 1);
            if id == blocks::AIR || blocks::material_is_leaves(id) {
                world.set(x + 1, y + i, z + 1, wood);
            }
            let id = world.get_id(x, y + i, z + 1);
            if id == blocks::AIR || blocks::material_is_leaves(id) {
                world.set(x, y + i, z + 1, wood);
            }
        }
    }
    true
}

fn mega_pine_post(world: &mut DecorateWorld, rand: &mut JavaRandom, x: i32, y: i32, z: i32) {
    let podzol = ((blocks::DIRT as u16) << 4) | (blocks::PODZOL_META);
    mega_pine_blob(world, x - 1, y, z - 1, podzol);
    mega_pine_blob(world, x + 2, y, z - 1, podzol);
    mega_pine_blob(world, x - 1, y, z + 2, podzol);
    mega_pine_blob(world, x + 2, y, z + 2, podzol);
    for _ in 0..5 {
        let v = rand.next_int_bound(64);
        let dx = v % 8;
        let dz = v / 8;
        if dx == 0 || dx == 7 || dz == 0 || dz == 7 {
            mega_pine_blob(world, x - 3 + dx, y, z - 3 + dz, podzol);
        }
    }
}

/// `WorldGenMegaPineTree.func_175933_b`: a 5x5 podzol blob (corners omitted).
fn mega_pine_blob(world: &mut DecorateWorld, x: i32, y: i32, z: i32, podzol: u16) {
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() != 2 || dz.abs() != 2 {
                mega_podzol(world, x + dx, y, z + dz, podzol);
            }
        }
    }
}

fn mega_podzol(world: &mut DecorateWorld, x: i32, y: i32, z: i32, podzol: u16) {
    for dy in (-3..=2).rev() {
        let id = world.get_id(x, y + dy, z);
        if id == blocks::GRASS || id == blocks::DIRT {
            world.set(x, y + dy, z, podzol);
            break;
        }
        if id != blocks::AIR && dy < 0 {
            break;
        }
    }
}

// ---- WorldGenBigTree (stateful heightLimit) ----

const BIG_TREE_HEIGHT_LIMIT_LIMIT: i32 = 12;
const BIG_TREE_LEAF_DISTANCE_LIMIT: i32 = 5; // set by `func_175904_e`
const BIG_TREE_HEIGHT_ATTENUATION: f64 = 0.618;
const BIG_TREE_BRANCH_SLOPE: f64 = 0.381;

struct Foliage {
    x: i32,
    y: i32,
    z: i32,
    base_y: i32,
}

fn big_tree_greatest_distance(dx: i32, dy: i32, dz: i32) -> i32 {
    let ax = abs_int(dx);
    let ay = abs_int(dy);
    let az = abs_int(dz);
    if az > ax && az > ay {
        az
    } else if ay > ax {
        ay
    } else {
        ax
    }
}

fn big_tree_axis(from_x: i32, from_z: i32, to_x: i32, to_z: i32) -> u8 {
    let dx = (to_x - from_x).abs();
    let dz = (to_z - from_z).abs();
    let m = dx.max(dz);
    if m > 0 {
        if dx == m {
            AXIS_X
        } else if dz == m {
            AXIS_Z
        } else {
            AXIS_Y
        }
    } else {
        AXIS_Y
    }
}

/// `WorldGenBigTree.checkBlockLine`: -1 if clear, else the first blocked step.
fn big_tree_check_line(world: &DecorateWorld, from: (i32, i32, i32), to: (i32, i32, i32)) -> i32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dz = to.2 - from.2;
    let steps = big_tree_greatest_distance(dx, dy, dz);
    if steps == 0 {
        return -1;
    }
    let fx = dx as f32 / steps as f32;
    let fy = dy as f32 / steps as f32;
    let fz = dz as f32 / steps as f32;
    for i in 0..=steps {
        let px = floor_double(from.0 as f64 + (0.5 + i as f32 * fx) as f64);
        let py = floor_double(from.1 as f64 + (0.5 + i as f32 * fy) as f64);
        let pz = floor_double(from.2 as f64 + (0.5 + i as f32 * fz) as f64);
        if !leaf_check(world.get_id(px, py, pz)) {
            return i;
        }
    }
    -1
}

fn big_tree_log_line(
    world: &mut DecorateWorld,
    from: (i32, i32, i32),
    to: (i32, i32, i32),
    variant: u8,
) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dz = to.2 - from.2;
    let steps = big_tree_greatest_distance(dx, dy, dz);
    if steps == 0 {
        return;
    }
    let fx = dx as f32 / steps as f32;
    let fy = dy as f32 / steps as f32;
    let fz = dz as f32 / steps as f32;
    for i in 0..=steps {
        let px = floor_double(from.0 as f64 + (0.5 + i as f32 * fx) as f64);
        let py = floor_double(from.1 as f64 + (0.5 + i as f32 * fy) as f64);
        let pz = floor_double(from.2 as f64 + (0.5 + i as f32 * fz) as f64);
        let axis = big_tree_axis(from.0, from.2, px, pz);
        world.set(px, py, pz, log_state(variant, axis));
    }
}

fn big_tree_layer_size(height_limit: i32, y: i32) -> f32 {
    if (y as f32) < height_limit as f32 * 0.3 {
        return -1.0;
    }
    let half = height_limit as f32 / 2.0;
    let d = half - y as f32;
    let mut r = sqrt_float(half * half - d * d);
    if d == 0.0 {
        r = half;
    } else if d.abs() >= half {
        return 0.0;
    }
    r * 0.5
}

fn big_tree_leaf_size(limit: i32, y: i32) -> f32 {
    if !(0..limit).contains(&y) {
        -1.0
    } else if y != 0 && y != limit - 1 {
        3.0
    } else {
        2.0
    }
}

fn big_tree_leaf_disc(world: &mut DecorateWorld, x: i32, y: i32, z: i32, size: f32, leaf: u16) {
    let r = (size as f64 + 0.618) as i32;
    for dx in -r..=r {
        for dz in -r..=r {
            if (dx.abs() as f64 + 0.5).powi(2) + (dz.abs() as f64 + 0.5).powi(2)
                <= (size * size) as f64
            {
                let id = world.get_id(x + dx, y, z + dz);
                if id == blocks::AIR || blocks::material_is_leaves(id) {
                    world.set(x + dx, y, z + dz, leaf);
                }
            }
        }
    }
}

fn gen_big_tree(
    world: &mut DecorateWorld,
    rand: &mut JavaRandom,
    x: i32,
    y: i32,
    z: i32,
    height_limit: &mut i32,
) -> bool {
    // `WorldGenBigTree.generate` uses its own `Random(rand.nextLong())`.
    let mut trand = JavaRandom::new(rand.next_long());
    if *height_limit == 0 {
        *height_limit = 5 + trand.next_int_bound(BIG_TREE_HEIGHT_LIMIT_LIMIT);
    }
    let base = (x, y, z);
    let below = world.get_id(x, y - 1, z);
    if below != blocks::DIRT && below != blocks::GRASS && below != blocks::FARMLAND {
        return false;
    }
    let blocked = big_tree_check_line(world, base, (x, y + *height_limit - 1, z));
    if blocked != -1 {
        if blocked < 6 {
            return false;
        }
        *height_limit = blocked;
    }

    let height_limit = *height_limit;
    let height = (height_limit as f64 * BIG_TREE_HEIGHT_ATTENUATION) as i32;
    let height = if height >= height_limit {
        height_limit - 1
    } else {
        height
    };

    let leaf = leaves_state(OAK);
    let wood = OAK;

    // Foliage nodes.
    let mut foliage: Vec<Foliage> = Vec::new();
    let count = ((1.382 + (height_limit as f64 / 13.0).powi(2)) as i32).max(1);
    let top = y + height;
    let mut layer = height_limit - BIG_TREE_LEAF_DISTANCE_LIMIT;
    foliage.push(Foliage {
        x,
        y: y + layer,
        z,
        base_y: top,
    });
    while layer >= 0 {
        let size = big_tree_layer_size(height_limit, layer);
        if size >= 0.0 {
            for _ in 0..count {
                let radius = (size as f64) * (trand.next_float() as f64 + 0.328);
                let angle = trand.next_float() as f64 * 2.0 * std::f64::consts::PI;
                let px = floor_double(x as f64 + radius * angle.sin() + 0.5);
                let pz = floor_double(z as f64 + radius * angle.cos() + 0.5);
                let py = y + layer - 1;
                let top_py = py + BIG_TREE_LEAF_DISTANCE_LIMIT;
                if big_tree_check_line(world, (px, py, pz), (px, top_py, pz)) == -1 {
                    let bx = x - px;
                    let bz = z - pz;
                    let by =
                        py as f64 - ((bx * bx + bz * bz) as f64).sqrt() * BIG_TREE_BRANCH_SLOPE;
                    let branch_y = if by > top as f64 { top } else { by as i32 };
                    if big_tree_check_line(world, (x, branch_y, z), (px, py, pz)) == -1 {
                        foliage.push(Foliage {
                            x: px,
                            y: py,
                            z: pz,
                            base_y: branch_y,
                        });
                    }
                }
            }
        }
        layer -= 1;
    }

    for node in &foliage {
        for i in 0..BIG_TREE_LEAF_DISTANCE_LIMIT {
            big_tree_leaf_disc(
                world,
                node.x,
                node.y + i,
                node.z,
                big_tree_leaf_size(BIG_TREE_LEAF_DISTANCE_LIMIT, i),
                leaf,
            );
        }
    }

    // Trunk.
    big_tree_log_line(world, base, (x, y + height, z), wood);

    // Leaf-node bases (branches from the trunk to each foliage node).
    for node in &foliage {
        let directly_above = node.x == x && node.z == z && node.y == node.base_y;
        let needs_base = (node.base_y - y) as f32 >= height_limit as f32 * 0.2;
        if !directly_above && needs_base {
            big_tree_log_line(world, (x, node.base_y, z), (node.x, node.y, node.z), wood);
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;

    fn tree_world() -> DecorateWorld {
        let mut world = DecorateWorld::new(63);
        let mut blocks = vec![0u16; 65536];
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..63 {
                    blocks[(((x & 15) as usize) << 12) | (((z & 15) as usize) << 8) | y] =
                        (blocks::DIRT as u16) << 4;
                }
                blocks[(((x & 15) as usize) << 12) | (((z & 15) as usize) << 8) | 63] =
                    (blocks::GRASS as u16) << 4;
            }
        }
        world.insert_chunk(0, 0, blocks);
        world
    }

    /// `BlockLog`/`BlockOldLeaf` metadata: variant + axis, and `CHECK_DECAY = false` for the leaves.
    #[test]
    fn log_and_leaf_states_encode_variant_and_axis() {
        assert_eq!(log_state(OAK, AXIS_Y), (blocks::LOG as u16) << 4);
        assert_eq!(log_state(SPRUCE, AXIS_Y), ((blocks::LOG as u16) << 4) | 1);
        assert_eq!(
            log_state(JUNGLE, AXIS_X),
            ((blocks::LOG as u16) << 4) | 3 | 4
        );
        assert_eq!(log_state(ACACIA, AXIS_Y), (blocks::LOG2 as u16) << 4);
        assert_eq!(
            log_state(DARK_OAK, AXIS_Y),
            ((blocks::LOG2 as u16) << 4) | 1
        );

        assert_eq!(leaves_state(OAK), (blocks::LEAVES as u16) << 4);
        assert_eq!(leaves_state(SPRUCE), ((blocks::LEAVES as u16) << 4) | 1);
        assert_eq!(leaves_state(ACACIA), (blocks::LEAVES2 as u16) << 4);
        assert_eq!(leaves_state(DARK_OAK), ((blocks::LEAVES2 as u16) << 4) | 1);
    }

    /// A birch tree turns the grass below into dirt and places a birch trunk + leaves.
    #[test]
    fn forest_tree_places_trunk_leaves_and_dirt() {
        let mut world = tree_world();
        let mut rand = JavaRandom::new(1);
        assert!(gen_forest(&mut world, &mut rand, 8, 64, 8, false));

        assert_eq!(world.get(8, 63, 8), (blocks::DIRT as u16) << 4);
        let mut logs = 0;
        let mut leaves = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..128 {
                    let v = world.get(x, y, z);
                    if v >> 4 == blocks::LOG as u16 && (v & 3) == 2 {
                        logs += 1;
                    }
                    if v >> 4 == blocks::LEAVES as u16 && (v & 3) == 2 {
                        leaves += 1;
                    }
                }
            }
        }
        assert!(logs > 0, "expected a birch trunk");
        assert!(leaves > 0, "expected birch leaves");
    }
}
