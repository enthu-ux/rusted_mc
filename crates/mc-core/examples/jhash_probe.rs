//! Prints `java.util.HashMap` iteration order for a key sequence, matching
//! `oracle/java/src/HashOrderOracle.java`:
//!
//! Usage: jhash_probe <int|long> <key> [<key> ...]
//!
//! `int` keys hash as `Integer.hashCode()`, `long` keys as `Long.hashCode()`; the keys are printed
//! space-separated in `keySet()` order.

use mc_core::jhash::{int_hash_code, long_hash_code, JavaHashMap};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let longs = args[1] == "long";

    let mut map: JavaHashMap<i64, usize> = JavaHashMap::new();
    for (i, raw) in args.iter().skip(2).enumerate() {
        let key: i64 = raw.parse().expect("key");
        let hash = if longs {
            long_hash_code(key)
        } else {
            int_hash_code(key as i32)
        };
        map.insert(key, i, hash);
    }

    let mut out = String::new();
    for (key, _) in map.iter() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&key.to_string());
    }
    println!("{out}");
}
