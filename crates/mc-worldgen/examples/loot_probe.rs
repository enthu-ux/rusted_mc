//! Prints the loot/enchantment RNG dump matching `oracle/java/src/LootOracle.java`:
//!
//! Usage:
//!   loot_probe table
//!   loot_probe enchant <seed> <level> [<seed> <level> ...]
//!   loot_probe chest   <seed> <count> <slots> [<seed> <count> <slots> ...]
//!
//! `table` dumps the enchantment registry (id/weight/max level/type), each level's
//! min/max enchantability, and every ordered pair's `canApplyTogether`.
//! `enchant` rolls a random enchantment list onto a book and prints it plus the next 32
//! `nextInt(1000)` values. `chest` builds the mineshaft chest list and draws contents, printing the
//! list, every slot write and the next 32 `nextInt(1000)` values.

use mc_core::random::JavaRandom;
use mc_worldgen::loot::chest::{
    enchanted_book_content, generate_chest_contents, MINESHAFT_CHEST_CONTENT,
};
use mc_worldgen::loot::enchant::{add_random_enchantment, ENCHANTMENTS};

const STREAM: usize = 32;

fn stream(rand: &mut JavaRandom, out: &mut String) {
    out.push_str("stream");
    for _ in 0..STREAM {
        out.push_str(&format!(" {}", rand.next_int_bound(1000)));
    }
    out.push('\n');
}

fn table() -> String {
    let mut out = String::new();
    for e in ENCHANTMENTS.iter() {
        out.push_str(&format!(
            "ench {} {} {} {}\n",
            e.id,
            e.weight,
            e.max_level,
            e.ty.name()
        ));
        for level in 1..=e.max_level {
            out.push_str(&format!(
                "lvl {} {} {} {}\n",
                e.id,
                level,
                e.min_enchantability(level),
                e.max_enchantability(level)
            ));
        }
    }
    for (i, a) in ENCHANTMENTS.iter().enumerate() {
        for b in ENCHANTMENTS.iter().skip(i + 1) {
            out.push_str(&format!(
                "excl {} {} {} {}\n",
                a.id,
                b.id,
                a.can_apply_together(b) as u8,
                b.can_apply_together(a) as u8
            ));
        }
    }
    out
}

fn enchant(args: &[String]) -> String {
    let mut out = String::new();
    for pair in args.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let seed: i64 = pair[0].parse().expect("seed");
        let level: i32 = pair[1].parse().expect("level");
        let mut rand = JavaRandom::new(seed);
        let list = add_random_enchantment(&mut rand, level);

        out.push_str(&format!("case {seed} {level}\n"));
        out.push_str("enchants");
        if let Some(list) = &list {
            for d in list {
                out.push_str(&format!(" {}:{}", d.enchantment.id, d.level));
            }
        }
        out.push('\n');
        stream(&mut rand, &mut out);
    }
    out
}

fn chest(args: &[String]) -> String {
    let mut out = String::new();
    for triple in args.chunks(3) {
        if triple.len() < 3 {
            break;
        }
        let seed: i64 = triple[0].parse().expect("seed");
        let count: i32 = triple[1].parse().expect("count");
        let slots: i32 = triple[2].parse().expect("slots");

        let mut rand = JavaRandom::new(seed);
        let mut list = MINESHAFT_CHEST_CONTENT.to_vec();
        list.push(enchanted_book_content(&mut rand));

        out.push_str(&format!("case {seed} {count} {slots}\n"));
        for (i, c) in list.iter().enumerate() {
            out.push_str(&format!(
                "content {i} {} {} {} {} {}\n",
                c.item.id, c.item.meta, c.min_size, c.max_size, c.weight
            ));
        }

        let mut writes = String::new();
        generate_chest_contents(&mut rand, &list, slots, count, |slot, c, size| {
            writes.push_str(&format!(
                "write {} {} {} {}\n",
                slot, c.item.id, c.item.meta, size
            ));
        });
        out.push_str(&writes);
        stream(&mut rand, &mut out);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = match args[1].as_str() {
        "table" => table(),
        "enchant" => enchant(&args[2..]),
        "chest" => chest(&args[2..]),
        other => panic!("unknown mode: {other}"),
    };
    print!("{out}");
}
