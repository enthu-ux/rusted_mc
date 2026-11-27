//! Loot + enchantment RNG (1.8.9), the subsystem `StructureMineshaftPieces` needs before it can write
//! its blocks.
//!
//! A mineshaft corridor rolls `rand.nextInt(100) == 0` twice per section (~5-10 rolls per shaft); on a
//! hit it appends `Items.enchanted_book.getRandom(rand)` and calls
//! `WeightedRandomChestContent.generateChestContents`. That path draws **enchantment** RNG
//! (`EnchantmentHelper.buildEnchantmentList` -> `mapEnchantmentData`, a `java.util.HashMap` whose
//! iteration order feeds `WeightedRandom`), so the *draw sequence* shifts the rest of the chunk's block
//! placement. Ports, in order:
//!
//! * `WeightedRandom` (`getTotalWeight`, `getRandomItem`), [`weighted_index`];
//! * `WeightedRandomChestContent.generateChestContents` + the 13-entry mineshaft weight list and
//!   `func_177629_a` ([`chest`]);
//! * the `Enchantment` registry table + subclasses' `getMinEnchantability`/`getMaxEnchantability`/
//!   `canApplyTogether`/`getMaxLevel` ([`Enchantment`], [`ENCHANTMENTS`]);
//! * `EnchantmentHelper.mapEnchantmentData`/`buildEnchantmentList`/`addRandomEnchantment` ([`enchant`]),
//!   using [`mc_core::jhash::JavaHashMap`] for the map's iteration order.
//!
//! Only the **book** path (`ItemBook.getItemEnchantability() == 1`, all enchantment types applicable)
//! is ported: that is what a chest book needs. Non-book applicability needs the item registry and
//! lands with the item port.
//!
//! Verified by `tools/test_loot_parity.py` (`LootOracle.java` + `examples/loot_probe.rs`), which runs
//! the real game code and compares the *items written and the 32 draws afterwards*.

use mc_core::jhash::{int_hash_code, JavaHashMap};
use mc_core::random::JavaRandom;

/// `WeightedRandom.getTotalWeight`.
pub fn total_weight(weights: &[i32]) -> i32 {
    weights.iter().sum()
}

/// `WeightedRandom.getRandomItem(Collection, int)`: draws `nextInt(totalWeight)` and walks the weights.
/// Returns the chosen index, or `None` for an empty/zero-weight list (vanilla would NPE).
pub fn weighted_index(rand: &mut JavaRandom, weights: &[i32]) -> Option<usize> {
    let total = total_weight(weights);
    if total <= 0 {
        return None;
    }
    weighted_index_from(weights, rand.next_int_bound(total))
}

/// `WeightedRandom.getRandomItem(Collection, int)`'s inner loop: subtract each weight, first negative wins.
pub fn weighted_index_from(weights: &[i32], mut value: i32) -> Option<usize> {
    for (i, &w) in weights.iter().enumerate() {
        value -= w;
        if value < 0 {
            return Some(i);
        }
    }
    None
}

/// Loot + chest contents (`WeightedRandomChestContent`).
pub mod chest {
    use super::{weighted_index, JavaRandom};

    /// An `ItemStack` reduced to what loot drawing observes: the item id, damage/metadata, and the
    /// item's max stack size (`Item.getItemStackLimit()`, which decides the `generateChestContents`
    /// branch and therefore how many draws a stack costs).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ItemStackSpec {
        pub id: i32,
        pub meta: i32,
        pub max_stack_size: i32,
    }

    impl ItemStackSpec {
        pub const fn new(id: i32, meta: i32, max_stack_size: i32) -> Self {
            ItemStackSpec {
                id,
                meta,
                max_stack_size,
            }
        }
    }

    /// A `WeightedRandomChestContent`: a weighted `(item, min..=max)` stack entry.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ChestContent {
        pub item: ItemStackSpec,
        pub min_size: i32,
        pub max_size: i32,
        pub weight: i32,
    }

    const fn content(
        item: ItemStackSpec,
        min_size: i32,
        max_size: i32,
        weight: i32,
    ) -> ChestContent {
        ChestContent {
            item,
            min_size,
            max_size,
            weight,
        }
    }

    /// `Items.enchanted_book` (`setMaxStackSize(1)`).
    pub const ENCHANTED_BOOK: ItemStackSpec = ItemStackSpec::new(403, 0, 1);

    /// `StructureMineshaftPieces.CHEST_CONTENT_WEIGHT_LIST` (13 entries, total weight 79). Item ids and
    /// max stack sizes follow `Item.registerItems()` (`Items.DYE` uses `EnumDyeColor.BLUE.getDyeDamage()` = 4).
    pub const MINESHAFT_CHEST_CONTENT: [ChestContent; 13] = [
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 10), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 1, 3, 5),  // gold_ingot
        content(ItemStackSpec::new(331, 0, 64), 4, 9, 5),  // redstone
        content(ItemStackSpec::new(351, 4, 64), 4, 9, 5),  // dye (blue)
        content(ItemStackSpec::new(264, 0, 64), 1, 2, 3),  // diamond
        content(ItemStackSpec::new(263, 0, 64), 3, 8, 10), // coal
        content(ItemStackSpec::new(297, 0, 64), 1, 3, 15), // bread
        content(ItemStackSpec::new(257, 0, 1), 1, 1, 1),   // iron_pickaxe
        content(ItemStackSpec::new(66, 0, 64), 4, 8, 1),   // rail (block item)
        content(ItemStackSpec::new(362, 0, 64), 2, 4, 10), // melon_seeds
        content(ItemStackSpec::new(361, 0, 64), 2, 4, 10), // pumpkin_seeds
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 3),   // saddle
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 1),   // iron_horse_armor
    ];

    /// `WorldGenDungeons.CHESTCONTENT` (15 entries, total weight 125). The dungeon chest; the book
    /// is appended per chest and `generateChestContents` fills a 27-slot `TileEntityChest` with 8
    /// stacks. Item ids/max stack sizes from `Item.registerItems()` / `items.json`.
    pub const DUNGEON_CHEST_CONTENT: [ChestContent; 15] = [
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 10), // saddle
        content(ItemStackSpec::new(265, 0, 64), 1, 4, 10), // iron_ingot
        content(ItemStackSpec::new(297, 0, 64), 1, 1, 10), // bread
        content(ItemStackSpec::new(296, 0, 64), 1, 4, 10), // wheat
        content(ItemStackSpec::new(289, 0, 64), 1, 4, 10), // gunpowder
        content(ItemStackSpec::new(287, 0, 64), 1, 4, 10), // string
        content(ItemStackSpec::new(325, 0, 1), 1, 1, 10), // bucket
        content(ItemStackSpec::new(322, 0, 64), 1, 1, 1), // golden_apple
        content(ItemStackSpec::new(331, 0, 64), 1, 4, 10), // redstone
        content(ItemStackSpec::new(2256, 0, 1), 1, 1, 4), // record_13
        content(ItemStackSpec::new(2257, 0, 1), 1, 1, 4), // record_cat
        content(ItemStackSpec::new(421, 0, 64), 1, 1, 10), // name_tag
        content(ItemStackSpec::new(418, 0, 1), 1, 1, 2),  // golden_horse_armor
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 5),  // iron_horse_armor
        content(ItemStackSpec::new(419, 0, 1), 1, 1, 1),  // diamond_horse_armor
    ];

    /// `ComponentScatteredFeaturePieces.DesertPyramid.itemsToGenerateInTemple` (10 entries, total
    /// weight 71). Item ids/max stack sizes from `Item.registerItems()`.
    pub const TEMPLE_CHEST_CONTENT: [ChestContent; 10] = [
        content(ItemStackSpec::new(264, 0, 64), 1, 3, 3), // diamond
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 10), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 2, 7, 15), // gold_ingot
        content(ItemStackSpec::new(388, 0, 64), 1, 3, 2), // emerald
        content(ItemStackSpec::new(352, 0, 64), 4, 6, 20), // bone
        content(ItemStackSpec::new(367, 0, 64), 3, 7, 16), // rotten_flesh
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 3),  // saddle
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 1),  // iron_horse_armor
        content(ItemStackSpec::new(418, 0, 1), 1, 1, 1),  // golden_horse_armor
        content(ItemStackSpec::new(419, 0, 1), 1, 1, 1),  // diamond_horse_armor
    ];

    /// `ComponentScatteredFeaturePieces.JunglePyramid.field_175815_j`: the tripwire dispensers'
    /// payload (`Items.arrow`, 2..=7, weight 30). `TileEntityDispenser` has 9 slots.
    pub const JUNGLE_DISPENSER_CONTENT: [ChestContent; 1] = [
        content(ItemStackSpec::new(262, 0, 64), 2, 7, 30), // arrow
    ];

    /// `StructureNetherBridgePieces.Piece.field_111019_a` (12 entries, total weight 75). The
    /// fortress corridor chest list; `generateChestContents` fills a 27-slot `TileEntityChest`.
    /// Item ids/max stack sizes from `Item.registerItems()` and the obsidian block item.
    pub const NETHER_BRIDGE_CHEST_CONTENT: [ChestContent; 12] = [
        content(ItemStackSpec::new(264, 0, 64), 1, 3, 5), // diamond
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 5), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 1, 3, 15), // gold_ingot
        content(ItemStackSpec::new(283, 0, 1), 1, 1, 5),  // golden_sword
        content(ItemStackSpec::new(315, 0, 1), 1, 1, 5),  // golden_chestplate
        content(ItemStackSpec::new(259, 0, 1), 1, 1, 5),  // flint_and_steel
        content(ItemStackSpec::new(372, 0, 64), 3, 7, 5), // nether_wart
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 10), // saddle
        content(ItemStackSpec::new(418, 0, 1), 1, 1, 8),  // golden_horse_armor
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 5),  // iron_horse_armor
        content(ItemStackSpec::new(419, 0, 1), 1, 1, 3),  // diamond_horse_armor
        content(ItemStackSpec::new(49, 0, 64), 2, 4, 2),  // obsidian (block item)
    ];

    /// `StructureVillagePieces.House2.villageBlacksmithChestContents` (17 entries, total weight
    /// 100). The blacksmith chest; `generateChestContents` fills a 27-slot `TileEntityChest`.
    pub const VILLAGE_BLACKSMITH_CHEST_CONTENT: [ChestContent; 17] = [
        content(ItemStackSpec::new(264, 0, 64), 1, 3, 3), // diamond
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 10), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 1, 3, 5), // gold_ingot
        content(ItemStackSpec::new(297, 0, 64), 1, 3, 15), // bread
        content(ItemStackSpec::new(260, 0, 64), 1, 3, 15), // apple
        content(ItemStackSpec::new(257, 0, 1), 1, 1, 5),  // iron_pickaxe
        content(ItemStackSpec::new(267, 0, 1), 1, 1, 5),  // iron_sword
        content(ItemStackSpec::new(307, 0, 1), 1, 1, 5),  // iron_chestplate
        content(ItemStackSpec::new(306, 0, 1), 1, 1, 5),  // iron_helmet
        content(ItemStackSpec::new(308, 0, 1), 1, 1, 5),  // iron_leggings
        content(ItemStackSpec::new(309, 0, 1), 1, 1, 5),  // iron_boots
        content(ItemStackSpec::new(49, 0, 64), 3, 7, 5),  // obsidian (block item)
        content(ItemStackSpec::new(6, 0, 64), 3, 7, 5),   // sapling (block item)
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 3),  // saddle
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 1),  // iron_horse_armor
        content(ItemStackSpec::new(418, 0, 1), 1, 1, 1),  // golden_horse_armor
        content(ItemStackSpec::new(419, 0, 1), 1, 1, 1),  // diamond_horse_armor
    ];

    /// `StructureStrongholdPieces.ChestCorridor.strongholdChestContents` (18 entries, total
    /// weight 100).
    pub const STRONGHOLD_CHEST_CONTENT: [ChestContent; 18] = [
        content(ItemStackSpec::new(368, 0, 16), 1, 1, 10), // ender_pearl
        content(ItemStackSpec::new(264, 0, 64), 1, 3, 3),  // diamond
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 10), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 1, 3, 5),  // gold_ingot
        content(ItemStackSpec::new(331, 0, 64), 4, 9, 5),  // redstone
        content(ItemStackSpec::new(297, 0, 64), 1, 3, 15), // bread
        content(ItemStackSpec::new(260, 0, 64), 1, 3, 15), // apple
        content(ItemStackSpec::new(257, 0, 1), 1, 1, 5),   // iron_pickaxe
        content(ItemStackSpec::new(267, 0, 1), 1, 1, 5),   // iron_sword
        content(ItemStackSpec::new(307, 0, 1), 1, 1, 5),   // iron_chestplate
        content(ItemStackSpec::new(306, 0, 1), 1, 1, 5),   // iron_helmet
        content(ItemStackSpec::new(308, 0, 1), 1, 1, 5),   // iron_leggings
        content(ItemStackSpec::new(309, 0, 1), 1, 1, 5),   // iron_boots
        content(ItemStackSpec::new(322, 0, 64), 1, 1, 1),  // golden_apple
        content(ItemStackSpec::new(329, 0, 1), 1, 1, 1),   // saddle
        content(ItemStackSpec::new(417, 0, 1), 1, 1, 1),   // iron_horse_armor
        content(ItemStackSpec::new(418, 0, 1), 1, 1, 1),   // golden_horse_armor
        content(ItemStackSpec::new(419, 0, 1), 1, 1, 1),   // diamond_horse_armor
    ];

    /// `StructureStrongholdPieces.Library.strongholdLibraryChestContents` (4 entries, total
    /// weight 42).
    pub const STRONGHOLD_LIBRARY_CHEST_CONTENT: [ChestContent; 4] = [
        content(ItemStackSpec::new(340, 0, 64), 1, 3, 20), // book
        content(ItemStackSpec::new(339, 0, 64), 2, 7, 20), // paper
        content(ItemStackSpec::new(395, 0, 1), 1, 1, 1),   // map
        content(ItemStackSpec::new(345, 0, 1), 1, 1, 1),   // compass
    ];

    /// `StructureStrongholdPieces.RoomCrossing.strongholdRoomCrossingChestContents` (7 entries,
    /// total weight 61).
    pub const STRONGHOLD_ROOM_CROSSING_CHEST_CONTENT: [ChestContent; 7] = [
        content(ItemStackSpec::new(265, 0, 64), 1, 5, 10), // iron_ingot
        content(ItemStackSpec::new(266, 0, 64), 1, 3, 5),  // gold_ingot
        content(ItemStackSpec::new(331, 0, 64), 4, 9, 5),  // redstone
        content(ItemStackSpec::new(263, 0, 64), 3, 8, 10), // coal
        content(ItemStackSpec::new(297, 0, 64), 1, 3, 15), // bread
        content(ItemStackSpec::new(260, 0, 64), 1, 3, 15), // apple
        content(ItemStackSpec::new(257, 0, 1), 1, 1, 1),   // iron_pickaxe
    ];

    /// `WeightedRandomChestContent.func_177629_a`: a new list of `base` followed by `extra`.
    pub fn with_extra(base: &[ChestContent], extra: ChestContent) -> Vec<ChestContent> {
        let mut list = base.to_vec();
        list.push(extra);
        list
    }

    /// Draws one stack from `list` (`WeightedRandom.getRandomItem`): returns the entry and the drawn
    /// stack size. Consumes the same draws as `generateChestContents` before the slot write.
    fn draw_content<'a>(
        rand: &mut JavaRandom,
        list: &'a [ChestContent],
    ) -> Option<(&'a ChestContent, i32)> {
        let weights: Vec<i32> = list.iter().map(|c| c.weight).collect();
        let content = list.get(weighted_index(rand, &weights)?)?;
        let size = content.min_size + rand.next_int_bound(content.max_size - content.min_size + 1);
        Some((content, size))
    }

    /// `WeightedRandomChestContent.generateChestContents`: `count` stacks into `slots`-many slots.
    ///
    /// `place(slot, content, size)` is called once per `setInventorySlotContents` in order. When the
    /// drawn size exceeds the item's max stack size, vanilla writes `size` single-item stacks (one slot
    /// draw each) instead of one stack - the branch that shifts the draw count.
    pub fn generate_chest_contents(
        rand: &mut JavaRandom,
        list: &[ChestContent],
        slots: i32,
        count: i32,
        mut place: impl FnMut(i32, &ChestContent, i32),
    ) {
        for _ in 0..count {
            let (content, size) = match draw_content(rand, list) {
                Some(v) => v,
                None => return,
            };
            if content.item.max_stack_size >= size {
                place(rand.next_int_bound(slots), content, size);
            } else {
                for _ in 0..size {
                    place(rand.next_int_bound(slots), content, 1);
                }
            }
        }
    }

    /// `Items.enchanted_book.getRandom(rand)` -> `getRandom(rand, 1, 1, 1)`: builds a random enchanted
    /// book (level 30, enchantability 1) and wraps it as a single-item chest entry. The enchantment
    /// draws happen here, before the chest rolls.
    pub fn enchanted_book_content(rand: &mut JavaRandom) -> ChestContent {
        enchanted_book_content_with(rand, 1, 1, 1)
    }

    /// `ItemEnchantedBook.getRandom(rand, min, max, weight)`: the same enchantment draw, but the
    /// resulting chest entry can appear `min..=max` times with `weight` (the library chests).
    pub fn enchanted_book_content_with(
        rand: &mut JavaRandom,
        min_size: i32,
        max_size: i32,
        weight: i32,
    ) -> ChestContent {
        let _ = super::enchant::add_random_enchantment(rand, 30);
        content(ENCHANTED_BOOK, min_size, max_size, weight)
    }
}

pub use chest::{ChestContent, ItemStackSpec};

/// The enchantment registry + `EnchantmentHelper` RNG (`mapEnchantmentData`/`buildEnchantmentList`).
pub mod enchant {
    use super::*;

    /// `EnumEnchantmentType`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EnchantType {
        All,
        Armor,
        ArmorFeet,
        ArmorLegs,
        ArmorTorso,
        ArmorHead,
        Weapon,
        Digger,
        FishingRod,
        Breakable,
        Bow,
    }

    impl EnchantType {
        /// The enum constant name, as dumped by the oracle.
        pub fn name(self) -> &'static str {
            match self {
                EnchantType::All => "ALL",
                EnchantType::Armor => "ARMOR",
                EnchantType::ArmorFeet => "ARMOR_FEET",
                EnchantType::ArmorLegs => "ARMOR_LEGS",
                EnchantType::ArmorTorso => "ARMOR_TORSO",
                EnchantType::ArmorHead => "ARMOR_HEAD",
                EnchantType::Weapon => "WEAPON",
                EnchantType::Digger => "DIGGER",
                EnchantType::FishingRod => "FISHING_ROD",
                EnchantType::Breakable => "BREAKABLE",
                EnchantType::Bow => "BOW",
            }
        }
    }

    /// Which concrete `Enchantment` subclass (and constructor argument) a table entry is, i.e. which
    /// `getMinEnchantability`/`getMaxEnchantability`/`canApplyTogether`/`getMaxLevel` body applies.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EnchantKind {
        /// `EnchantmentProtection` with its `protectionType` (0..=4).
        Protection(u8),
        /// `EnchantmentDamage` with its `damageType` (0..=2).
        Damage(u8),
        Oxygen,
        WaterWorker,
        Thorns,
        WaterWalker,
        Knockback,
        FireAspect,
        /// `EnchantmentLootBonus` (looting / fortune / luck of the sea).
        LootBonus,
        Digging,
        Untouching,
        Durability,
        ArrowDamage,
        ArrowKnockback,
        ArrowFire,
        ArrowInfinite,
        FishingSpeed,
    }

    /// `EnchantmentProtection`'s `baseEnchantability`/`levelEnchantability`/`thresholdEnchantability`.
    const PROTECTION_BASE: [i32; 5] = [1, 10, 5, 5, 3];
    const PROTECTION_LEVEL: [i32; 5] = [11, 8, 6, 8, 6];
    const PROTECTION_THRESHOLD: [i32; 5] = [20, 12, 10, 12, 15];
    /// `EnchantmentDamage`'s tables.
    const DAMAGE_BASE: [i32; 3] = [1, 5, 5];
    const DAMAGE_LEVEL: [i32; 3] = [11, 8, 8];
    const DAMAGE_THRESHOLD: [i32; 3] = [20, 20, 20];

    /// `Enchantment.silkTouch.effectId` / `Enchantment.fortune.effectId`.
    const SILK_TOUCH: i32 = 33;
    const FORTUNE: i32 = 35;

    /// One `Enchantment` (id, weight, type, max level, subclass).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Enchantment {
        pub id: i32,
        pub weight: i32,
        pub ty: EnchantType,
        pub max_level: i32,
        pub kind: EnchantKind,
    }

    impl Enchantment {
        /// `Enchantment.getMinEnchantability` (the **base** class body, `1 + level * 10`) - the value
        /// subclasses call via `super.getMinEnchantability`.
        fn base_min_enchantability(level: i32) -> i32 {
            1 + level * 10
        }

        pub fn min_enchantability(&self, level: i32) -> i32 {
            match self.kind {
                EnchantKind::Protection(p) => {
                    PROTECTION_BASE[p as usize] + (level - 1) * PROTECTION_LEVEL[p as usize]
                }
                EnchantKind::Damage(d) => {
                    DAMAGE_BASE[d as usize] + (level - 1) * DAMAGE_LEVEL[d as usize]
                }
                EnchantKind::Oxygen => 10 * level,
                EnchantKind::WaterWorker => 1,
                EnchantKind::Thorns => 10 + 20 * (level - 1),
                EnchantKind::WaterWalker => level * 10,
                EnchantKind::Knockback => 5 + 20 * (level - 1),
                EnchantKind::FireAspect => 10 + 20 * (level - 1),
                EnchantKind::LootBonus => 15 + (level - 1) * 9,
                EnchantKind::Digging => 1 + 10 * (level - 1),
                EnchantKind::Untouching => 15,
                EnchantKind::Durability => 5 + (level - 1) * 8,
                EnchantKind::ArrowDamage => 1 + (level - 1) * 10,
                EnchantKind::ArrowKnockback => 12 + (level - 1) * 20,
                EnchantKind::ArrowFire => 20,
                EnchantKind::ArrowInfinite => 20,
                EnchantKind::FishingSpeed => 15 + (level - 1) * 9,
            }
        }

        pub fn max_enchantability(&self, level: i32) -> i32 {
            match self.kind {
                EnchantKind::Protection(p) => {
                    self.min_enchantability(level) + PROTECTION_THRESHOLD[p as usize]
                }
                EnchantKind::Damage(d) => {
                    self.min_enchantability(level) + DAMAGE_THRESHOLD[d as usize]
                }
                EnchantKind::Oxygen => self.min_enchantability(level) + 30,
                EnchantKind::WaterWorker => self.min_enchantability(level) + 40,
                EnchantKind::WaterWalker => self.min_enchantability(level) + 15,
                EnchantKind::ArrowDamage => self.min_enchantability(level) + 15,
                EnchantKind::ArrowKnockback => self.min_enchantability(level) + 25,
                EnchantKind::ArrowFire => 50,
                EnchantKind::ArrowInfinite => 50,
                // Thorns/Knockback/FireAspect/LootBonus/Digging/Untouching/Durability/FishingSpeed add
                // 50 to the **base** min enchantability, not their own override.
                EnchantKind::Thorns
                | EnchantKind::Knockback
                | EnchantKind::FireAspect
                | EnchantKind::LootBonus
                | EnchantKind::Digging
                | EnchantKind::Untouching
                | EnchantKind::Durability
                | EnchantKind::FishingSpeed => Self::base_min_enchantability(level) + 50,
            }
        }

        /// `Enchantment.canApplyTogether` (the subclass overrides; default is `this != other`).
        pub fn can_apply_together(&self, other: &Enchantment) -> bool {
            match self.kind {
                EnchantKind::Protection(p) => match other.kind {
                    EnchantKind::Protection(o) => {
                        if o == p {
                            false
                        } else {
                            p == 2 || o == 2
                        }
                    }
                    _ => self.id != other.id,
                },
                EnchantKind::Damage(_) => !matches!(other.kind, EnchantKind::Damage(_)),
                EnchantKind::LootBonus => self.id != other.id && other.id != SILK_TOUCH,
                EnchantKind::Untouching => self.id != other.id && other.id != FORTUNE,
                _ => self.id != other.id,
            }
        }
    }

    /// `Enchantment.enchantmentsBookList`: the registry in ascending id order (ids 0..8, 16..21, 32..35,
    /// 48..51, 61..62). Weights/levels/types are from `Enchantment`'s static initialiser.
    pub const ENCHANTMENTS: [Enchantment; 25] = [
        Enchantment {
            id: 0,
            weight: 10,
            ty: EnchantType::Armor,
            max_level: 4,
            kind: EnchantKind::Protection(0),
        },
        Enchantment {
            id: 1,
            weight: 5,
            ty: EnchantType::Armor,
            max_level: 4,
            kind: EnchantKind::Protection(1),
        },
        Enchantment {
            id: 2,
            weight: 5,
            ty: EnchantType::ArmorFeet,
            max_level: 4,
            kind: EnchantKind::Protection(2),
        },
        Enchantment {
            id: 3,
            weight: 2,
            ty: EnchantType::Armor,
            max_level: 4,
            kind: EnchantKind::Protection(3),
        },
        Enchantment {
            id: 4,
            weight: 5,
            ty: EnchantType::Armor,
            max_level: 4,
            kind: EnchantKind::Protection(4),
        },
        Enchantment {
            id: 5,
            weight: 2,
            ty: EnchantType::ArmorHead,
            max_level: 3,
            kind: EnchantKind::Oxygen,
        },
        Enchantment {
            id: 6,
            weight: 2,
            ty: EnchantType::ArmorHead,
            max_level: 1,
            kind: EnchantKind::WaterWorker,
        },
        Enchantment {
            id: 7,
            weight: 1,
            ty: EnchantType::ArmorTorso,
            max_level: 3,
            kind: EnchantKind::Thorns,
        },
        Enchantment {
            id: 8,
            weight: 2,
            ty: EnchantType::ArmorFeet,
            max_level: 3,
            kind: EnchantKind::WaterWalker,
        },
        Enchantment {
            id: 16,
            weight: 10,
            ty: EnchantType::Weapon,
            max_level: 5,
            kind: EnchantKind::Damage(0),
        },
        Enchantment {
            id: 17,
            weight: 5,
            ty: EnchantType::Weapon,
            max_level: 5,
            kind: EnchantKind::Damage(1),
        },
        Enchantment {
            id: 18,
            weight: 5,
            ty: EnchantType::Weapon,
            max_level: 5,
            kind: EnchantKind::Damage(2),
        },
        Enchantment {
            id: 19,
            weight: 5,
            ty: EnchantType::Weapon,
            max_level: 2,
            kind: EnchantKind::Knockback,
        },
        Enchantment {
            id: 20,
            weight: 2,
            ty: EnchantType::Weapon,
            max_level: 2,
            kind: EnchantKind::FireAspect,
        },
        Enchantment {
            id: 21,
            weight: 2,
            ty: EnchantType::Weapon,
            max_level: 3,
            kind: EnchantKind::LootBonus,
        },
        Enchantment {
            id: 32,
            weight: 10,
            ty: EnchantType::Digger,
            max_level: 5,
            kind: EnchantKind::Digging,
        },
        Enchantment {
            id: 33,
            weight: 1,
            ty: EnchantType::Digger,
            max_level: 1,
            kind: EnchantKind::Untouching,
        },
        Enchantment {
            id: 34,
            weight: 5,
            ty: EnchantType::Breakable,
            max_level: 3,
            kind: EnchantKind::Durability,
        },
        Enchantment {
            id: 35,
            weight: 2,
            ty: EnchantType::Digger,
            max_level: 3,
            kind: EnchantKind::LootBonus,
        },
        Enchantment {
            id: 48,
            weight: 10,
            ty: EnchantType::Bow,
            max_level: 5,
            kind: EnchantKind::ArrowDamage,
        },
        Enchantment {
            id: 49,
            weight: 2,
            ty: EnchantType::Bow,
            max_level: 2,
            kind: EnchantKind::ArrowKnockback,
        },
        Enchantment {
            id: 50,
            weight: 2,
            ty: EnchantType::Bow,
            max_level: 1,
            kind: EnchantKind::ArrowFire,
        },
        Enchantment {
            id: 51,
            weight: 1,
            ty: EnchantType::Bow,
            max_level: 1,
            kind: EnchantKind::ArrowInfinite,
        },
        Enchantment {
            id: 61,
            weight: 2,
            ty: EnchantType::FishingRod,
            max_level: 3,
            kind: EnchantKind::LootBonus,
        },
        Enchantment {
            id: 62,
            weight: 2,
            ty: EnchantType::FishingRod,
            max_level: 3,
            kind: EnchantKind::FishingSpeed,
        },
    ];

    /// `Enchantment.getEnchantmentById`.
    pub fn enchantment_by_id(id: i32) -> Option<&'static Enchantment> {
        ENCHANTMENTS.iter().find(|e| e.id == id)
    }

    /// A `WeightedRandom.Item` carrying an `Enchantment` and the rolled level.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct EnchantmentData {
        pub enchantment: &'static Enchantment,
        pub level: i32,
    }

    /// `EnchantmentHelper.mapEnchantmentData` for a **book** (all types applicable): every enchantment
    /// whose `[min, max]` enchantability range contains `level` at some level 1..=max, keyed by id.
    /// Later levels overwrite earlier ones (`put`), so the value is the highest matching level.
    pub fn map_enchantment_data(level: i32) -> JavaHashMap<i32, EnchantmentData> {
        let mut map: JavaHashMap<i32, EnchantmentData> = JavaHashMap::new();
        for e in ENCHANTMENTS.iter() {
            for lvl in 1..=e.max_level {
                if level >= e.min_enchantability(lvl) && level <= e.max_enchantability(lvl) {
                    map.insert(
                        e.id,
                        EnchantmentData {
                            enchantment: e,
                            level: lvl,
                        },
                        int_hash_code(e.id),
                    );
                }
            }
        }
        map
    }

    /// `WeightedRandom.getRandomItem(rand, map.values())`: draws over the map's **iteration order**
    /// (which is why `JavaHashMap` matters).
    fn pick_from_map(
        rand: &mut JavaRandom,
        map: &JavaHashMap<i32, EnchantmentData>,
    ) -> Option<EnchantmentData> {
        let total: i32 = map.iter().map(|(_, d)| d.enchantment.weight).sum();
        if total <= 0 {
            return None;
        }
        let mut value = rand.next_int_bound(total);
        for (_, d) in map.iter() {
            value -= d.enchantment.weight;
            if value < 0 {
                return Some(*d);
            }
        }
        None
    }

    /// `EnchantmentHelper.buildEnchantmentList` (book path). Returns `None` when the item has no
    /// enchantability or the map is empty - vanilla's `null`.
    pub fn build_enchantment_list(
        rand: &mut JavaRandom,
        enchantability: i32,
        extra_level: i32,
    ) -> Option<Vec<EnchantmentData>> {
        if enchantability <= 0 {
            return None;
        }
        let e = enchantability / 2;
        let e = 1 + rand.next_int_bound((e >> 1) + 1) + rand.next_int_bound((e >> 1) + 1);
        let level = e + extra_level;
        let f = (rand.next_float() + rand.next_float() - 1.0) * 0.15;
        let mut power = (level as f32 * (1.0 + f) + 0.5) as i32;
        if power < 1 {
            power = 1;
        }

        let mut map = map_enchantment_data(power);
        if map.is_empty() {
            return None;
        }

        let mut list = Vec::new();
        list.push(pick_from_map(rand, &map)?);

        // `for (; rand.nextInt(50) <= power; power >>= 1)`: drop enchantments that conflict with what
        // is already on the item, then roll another from the survivors.
        let mut k = power;
        while rand.next_int_bound(50) <= k {
            map.retain(|_, d| {
                list.iter()
                    .all(|x| x.enchantment.can_apply_together(d.enchantment))
            });
            if !map.is_empty() {
                if let Some(d) = pick_from_map(rand, &map) {
                    list.push(d);
                }
            }
            k >>= 1;
        }

        Some(list)
    }

    /// `EnchantmentHelper.addRandomEnchantment(rand, stack, level)` for an `Items.book` stack: rolls
    /// the enchantment list (book enchantability is 1). NBT assembly does not draw.
    pub fn add_random_enchantment(
        rand: &mut JavaRandom,
        level: i32,
    ) -> Option<Vec<EnchantmentData>> {
        build_enchantment_list(rand, 1, level)
    }
}

#[cfg(test)]
mod tests {
    use super::chest::{
        enchanted_book_content, generate_chest_contents, with_extra, ChestContent,
        MINESHAFT_CHEST_CONTENT,
    };
    use super::enchant::{
        add_random_enchantment, enchantment_by_id, map_enchantment_data, EnchantmentData,
        ENCHANTMENTS,
    };
    use super::{total_weight, weighted_index_from};
    use mc_core::jhash::JavaHashMap;
    use mc_core::random::JavaRandom;

    /// `WeightedRandom`'s first-negative walk (oracle values from the mineshaft weight list).
    #[test]
    fn weighted_walk_selects_first_negative() {
        let weights = [10, 5, 5, 5];
        assert_eq!(weighted_index_from(&weights, 0), Some(0));
        assert_eq!(weighted_index_from(&weights, 9), Some(0));
        assert_eq!(weighted_index_from(&weights, 10), Some(1));
        assert_eq!(weighted_index_from(&weights, 24), Some(3));
        assert_eq!(weighted_index_from(&weights, 25), None);
        assert_eq!(total_weight(&[10, 5, 5, 5]), 25);
    }

    /// The 13-entry mineshaft list totals 79; `func_177629_a` appends the book entry (weight 1) -> 80.
    #[test]
    fn mineshaft_list_weights_and_append() {
        let weights: Vec<i32> = MINESHAFT_CHEST_CONTENT.iter().map(|c| c.weight).collect();
        assert_eq!(total_weight(&weights), 79);
        let mut list = with_extra(
            &MINESHAFT_CHEST_CONTENT,
            ChestContent {
                item: super::chest::ENCHANTED_BOOK,
                min_size: 1,
                max_size: 1,
                weight: 1,
            },
        );
        assert_eq!(list.len(), 14);
        assert_eq!(list[13].weight, 1);
        assert_eq!(list[13].item, super::chest::ENCHANTED_BOOK);
        // The append does not mutate the base list.
        list.truncate(13);
        assert_eq!(list.len(), 13);
    }

    /// `generateChestContents` with a count of 0 consumes no draws.
    #[test]
    fn empty_count_draws_nothing() {
        let before = JavaRandom::new(1).next_int();
        let mut rand = JavaRandom::new(1);
        generate_chest_contents(&mut rand, &MINESHAFT_CHEST_CONTENT, 27, 0, |_, _, _| {});
        assert_eq!(rand.next_int(), before);
    }

    /// A stack larger than the item's limit is written as that many single-item stacks.
    #[test]
    fn oversized_stack_writes_one_slot_per_item() {
        let list = [ChestContent {
            item: super::ItemStackSpec::new(329, 0, 1), // saddle: limit 1
            min_size: 4,
            max_size: 4,
            weight: 1,
        }];
        let mut rand = JavaRandom::new(7);
        let mut writes = Vec::new();
        generate_chest_contents(&mut rand, &list, 27, 3, |slot, _, size| {
            writes.push((slot, size));
        });
        assert_eq!(writes.len(), 12); // 3 draws x 4 single-item writes
        assert!(writes
            .iter()
            .all(|(slot, size)| (0..27).contains(slot) && *size == 1));
    }

    /// A stack within the item's limit is written as a single stack.
    #[test]
    fn small_stack_writes_one_slot() {
        let list = [ChestContent {
            item: super::ItemStackSpec::new(265, 0, 64), // iron_ingot: limit 64
            min_size: 5,
            max_size: 5,
            weight: 1,
        }];
        let mut rand = JavaRandom::new(7);
        let mut writes = 0;
        generate_chest_contents(&mut rand, &list, 27, 3, |slot, _, size| {
            assert!((0..27).contains(&slot));
            assert_eq!(size, 5);
            writes += 1;
        });
        assert_eq!(writes, 3);
    }

    /// An enchanted book entry is a single 1-item entry and the enchantment draws happen first.
    #[test]
    fn enchanted_book_entry_is_single_item() {
        let mut with_draws = JavaRandom::new(12345);
        let content = enchanted_book_content(&mut with_draws);
        assert_eq!(content.weight, 1);
        assert_eq!((content.min_size, content.max_size), (1, 1));
        assert_eq!(content.item, super::chest::ENCHANTED_BOOK);

        // The book roll consumed draws: the stream after it differs from a fresh random.
        let mut fresh = JavaRandom::new(12345);
        assert_ne!(with_draws.next_int(), fresh.next_int());
    }

    /// The registry has the 25 vanilla enchantments in ascending id order.
    #[test]
    fn enchantment_registry_ids_and_order() {
        let ids: Vec<i32> = ENCHANTMENTS.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 19, 20, 21, 32, 33, 34, 35, 48, 49, 50, 51,
                61, 62
            ]
        );
        assert_eq!(enchantment_by_id(0).unwrap().weight, 10);
        assert!(enchantment_by_id(9).is_none());
    }

    /// A couple of enchantability endpoints straight from the subclass bodies.
    #[test]
    fn enchantability_endpoints() {
        let protection = enchantment_by_id(0).unwrap(); // base 1, level 11, threshold 20
        assert_eq!(protection.min_enchantability(1), 1);
        assert_eq!(protection.max_enchantability(1), 21);
        assert_eq!(protection.min_enchantability(3), 23);
        let oxygen = enchantment_by_id(5).unwrap();
        assert_eq!(oxygen.min_enchantability(2), 20);
        assert_eq!(oxygen.max_enchantability(2), 50);
        let silk = enchantment_by_id(33).unwrap(); // min 15, max = base(1) + 50 = 61
        assert_eq!(silk.min_enchantability(1), 15);
        assert_eq!(silk.max_enchantability(1), 61);
        let thorns = enchantment_by_id(7).unwrap(); // min 10+20*(l-1), max = base + 50
        assert_eq!(thorns.min_enchantability(2), 30);
        assert_eq!(thorns.max_enchantability(2), 71);
    }

    /// Exclusivity is vanilla's own two overrides: loot bonus excludes silk touch, but silk touch only
    /// excludes fortune - so silk↔looting is asymmetric while silk↔fortune is mutual. Same protection
    /// types clash; fire/feather coexist.
    #[test]
    fn can_apply_together_is_asymmetric() {
        let fortune = enchantment_by_id(35).unwrap();
        let silk = enchantment_by_id(33).unwrap();
        let looting = enchantment_by_id(21).unwrap();
        // fortune (loot bonus) excludes silk; silk excludes fortune too (its own override).
        assert!(!fortune.can_apply_together(silk));
        assert!(!silk.can_apply_together(fortune));
        // silk touches only excludes fortune, so it allows looting; looting still excludes silk.
        assert!(silk.can_apply_together(looting));
        assert!(!looting.can_apply_together(silk));
        let fire = enchantment_by_id(1).unwrap();
        let feather = enchantment_by_id(2).unwrap();
        assert!(fire.can_apply_together(feather));
        assert!(!fire.can_apply_together(enchantment_by_id(1).unwrap()));
        assert!(!enchantment_by_id(16)
            .unwrap()
            .can_apply_together(enchantment_by_id(17).unwrap()));
    }

    /// At power 30 every enchantment's range contains it; at power 1 only the low-tier ones qualify.
    #[test]
    fn map_enchantability_filtering() {
        let sorted_ids = |map: &JavaHashMap<i32, EnchantmentData>| {
            let mut ids: Vec<i32> = map.iter().map(|(k, _)| *k).collect();
            ids.sort_unstable();
            ids
        };
        assert_eq!(
            sorted_ids(&map_enchantment_data(30)),
            vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 19, 20, 21, 32, 33, 34, 35, 48, 49, 50, 51,
                61, 62
            ]
        );
        assert_eq!(sorted_ids(&map_enchantment_data(1)), vec![0, 6, 16, 32, 48]);
        // The value is the highest matching level (protection level 3 for power 30).
        assert_eq!(
            map_enchantment_data(30)
                .iter()
                .find(|(k, _)| **k == 0)
                .unwrap()
                .1
                .level,
            3
        );
    }

    /// A book roll produces a self-consistent list: no duplicates, and each entry is order-compatible
    /// with everything added after it (`existing.can_apply_together(new)`), which is what the map
    /// removal enforces.
    #[test]
    fn golden_book_roll_seed0() {
        let mut rand = JavaRandom::new(0);
        let list = add_random_enchantment(&mut rand, 30).expect("book rolls");
        assert!(!list.is_empty());
        let mut ids: Vec<i32> = list.iter().map(|d| d.enchantment.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "no enchantment may appear twice");
        for a in 0..list.len() {
            for b in (a + 1)..list.len() {
                assert!(
                    list[a].enchantment.can_apply_together(list[b].enchantment),
                    "{} at index {a} rejects {} at index {b}",
                    list[a].enchantment.id,
                    list[b].enchantment.id
                );
            }
        }
    }

    /// Enchantability <= 0 (a non-enchantable item) draws nothing.
    #[test]
    fn non_enchantable_item_draws_nothing() {
        let mut rand = JavaRandom::new(0);
        assert!(super::enchant::build_enchantment_list(&mut rand, 0, 30).is_none());
    }
}
