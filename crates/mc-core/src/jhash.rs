//! `java.util.HashMap` iteration order (OpenJDK 8), for the places where vanilla's *output* depends
//! on it.
//!
//! `HashMap` iteration is deterministic for a given sequence of insertions, but it is
//! implementation-defined: bucket index ascending (`hash & (capacity - 1)`), insertion order inside
//! each bucket, with a doubling resize at `size > 0.75 * capacity`. Two worldgen paths depend on it:
//!
//! * `MapGenStructure.structureMap` (`Long` chunk keys) - whose pieces draw first from the shared
//!   `populate` random when several starts meet one chunk;
//! * `EnchantmentHelper.mapEnchantmentData` (`Integer` enchantment ids) - which enchantment a chest
//!   book gets, and therefore how many random draws the loot roll consumes.
//!
//! `plan.md` §7 flagged this ("Java `HashMap` iteration order - Horizon had to reimplement it").
//! Only `put`/iteration are modelled; that is all vanilla's worldgen needs.

/// One bucket entry: Java's spread hash, the key and the value.
#[derive(Clone, Debug)]
struct Entry<K, V> {
    hash: i32,
    key: K,
    value: V,
}

/// A map reproducing `java.util.HashMap`'s `entrySet()` iteration order.
///
/// `K`/`V` are plain Rust values; the Java `hashCode()` is supplied by the caller (see
/// [`int_hash_code`] / [`long_hash_code`]) because the hash of a Java boxed value depends on its
/// type, not on the Rust type used to store it.
#[derive(Clone, Debug)]
pub struct JavaHashMap<K, V> {
    table: Vec<Vec<Entry<K, V>>>,
    size: usize,
    threshold: usize,
}

impl<K: PartialEq, V> Default for JavaHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq, V> JavaHashMap<K, V> {
    /// `new HashMap<>()`: the table is allocated on the first `put`.
    pub fn new() -> JavaHashMap<K, V> {
        JavaHashMap {
            table: Vec::new(),
            size: 0,
            threshold: 0,
        }
    }

    /// `HashMap.put(key, value)`: replaces an equal key in place, otherwise appends to its bucket.
    pub fn insert(&mut self, key: K, value: V, java_hash_code: i32) {
        let hash = spread(java_hash_code);
        if self.table.is_empty() {
            self.resize();
        }

        let index = (hash as u32 as usize) & (self.table.len() - 1);
        let mut replaced = false;
        {
            let bin = &mut self.table[index];
            if let Some(existing) = bin.iter_mut().find(|e| e.hash == hash && e.key == key) {
                existing.value = value;
                replaced = true;
            } else {
                bin.push(Entry { hash, key, value });
            }
        }
        if replaced {
            return;
        }

        // `putVal` runs `treeifyBin` *before* the size bump; with a short table Java resizes
        // instead of building a tree, which is why a run of colliding keys resizes earlier than
        // `size > threshold` would suggest.
        if self.table[index].len() >= 9 {
            self.treeify_bin();
        }

        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `entrySet()` order: bucket index ascending, insertion order within a bucket.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.table.iter().flatten().map(|e| (&e.key, &e.value))
    }

    /// Drops every entry for which `keep` is false, preserving the order of the rest.
    ///
    /// Models removing entries through a `keySet()`/`entrySet()` iterator (`it.remove()`), as
    /// `EnchantmentHelper.buildEnchantmentList` does: removal unlinks a node from its bucket without
    /// resizing or rehashing, so the remaining entries keep their bucket/position order.
    pub fn retain<F: FnMut(&K, &V) -> bool>(&mut self, mut keep: F) {
        let mut size = 0;
        for bin in &mut self.table {
            bin.retain(|entry| {
                if keep(&entry.key, &entry.value) {
                    size += 1;
                    true
                } else {
                    false
                }
            });
        }
        self.size = size;
    }

    /// `treeifyBin`: below `MIN_TREEIFY_CAPACITY` (64) Java doubles the table instead of turning the
    /// bin into a red-black tree; the resize rehashes the entry that just triggered it.
    ///
    /// Actual treeification (a bin of 8+ entries in a table of 64+) is **not** modelled - its
    /// iteration order comes from the tree, not insertion order. The maps vanilla's worldgen reads
    /// (enchantment ids, chunk keys) do not reach that shape.
    fn treeify_bin(&mut self) {
        if self.table.len() < 64 {
            self.resize();
        }
    }

    /// `resize()`: doubles the table (16 on the first call) and rehashes, preserving the relative
    /// order inside each new bucket, exactly as OpenJDK splits `lo`/`hi` lists.
    fn resize(&mut self) {
        let old_capacity = self.table.len();
        let capacity = if old_capacity == 0 {
            16
        } else {
            old_capacity << 1
        };
        let threshold = capacity / 4 * 3;

        let mut table: Vec<Vec<Entry<K, V>>> = (0..capacity).map(|_| Vec::new()).collect();
        for bin in std::mem::take(&mut self.table) {
            for entry in bin {
                let index = (entry.hash as u32 as usize) & (capacity - 1);
                table[index].push(entry);
            }
        }

        self.table = table;
        self.threshold = threshold;
    }
}

/// `HashMap.hash(h)`: `h ^ (h >>> 16)` (unsigned shift).
#[inline]
fn spread(hash: i32) -> i32 {
    hash ^ ((hash as u32) >> 16) as i32
}

/// `Integer.hashCode()`.
#[inline]
pub fn int_hash_code(value: i32) -> i32 {
    value
}

/// `Long.hashCode()`: `(int)(value ^ (value >>> 32))`.
#[inline]
pub fn long_hash_code(value: i64) -> i32 {
    (value ^ ((value as u64) >> 32) as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order_int(keys: &[i32]) -> Vec<i32> {
        let mut map: JavaHashMap<i32, ()> = JavaHashMap::new();
        for &k in keys {
            map.insert(k, (), int_hash_code(k));
        }
        map.iter().map(|(k, _)| *k).collect()
    }

    fn order_long(keys: &[i64]) -> Vec<i64> {
        let mut map: JavaHashMap<i64, ()> = JavaHashMap::new();
        for &k in keys {
            map.insert(k, (), long_hash_code(k));
        }
        map.iter().map(|(k, _)| *k).collect()
    }

    /// Twelve distinct small hashes keep insertion order in the first table (oracle).
    #[test]
    fn small_int_keys_keep_insertion_order() {
        assert_eq!(
            order_int(&(0..12).collect::<Vec<_>>()),
            (0..12).collect::<Vec<_>>()
        );
    }

    /// `Integer.MIN_VALUE`/`MAX_VALUE` and negatives (oracle: -1 -17 -2147483648 2147483647 0 1 -16).
    #[test]
    fn negative_and_extreme_int_keys() {
        let keys = [-1, -16, -17, i32::MIN, i32::MAX, 0, 1];
        assert_eq!(
            order_int(&keys),
            vec![-1, -17, i32::MIN, i32::MAX, 0, 1, -16]
        );
    }

    /// A ninth colliding key makes Java resize early (`treeifyBin` with a short table):
    /// oracle `0 32 64 96 128 16 48 80 112`.
    #[test]
    fn ninth_collision_resizes_the_table() {
        let keys: Vec<i32> = (0..9).map(|i| i * 16).collect();
        assert_eq!(order_int(&keys), vec![0, 32, 64, 96, 128, 16, 48, 80, 112]);
    }

    /// Eight colliding keys still fit one bucket (oracle: insertion order).
    #[test]
    fn eight_collisions_do_not_resize() {
        let keys: Vec<i32> = (0..8).map(|i| i * 16).collect();
        assert_eq!(order_int(&keys), keys);
    }

    /// Twenty keys are rehashed into two buckets in insertion order (0..19 stays in order).
    #[test]
    fn twenty_keys_after_one_resize() {
        assert_eq!(
            order_int(&(0..20).collect::<Vec<_>>()),
            (0..20).collect::<Vec<_>>()
        );
    }

    /// Removing through the iterator keeps the surviving entries' bucket order: of the nine colliding
    /// keys in order `0 32 64 96 128 16 48 80 112`, dropping the odd multiples of 16 leaves
    /// `0 32 64 96 128`.
    #[test]
    fn retain_keeps_bucket_order() {
        let keys: Vec<i32> = (0..9).map(|i| i * 16).collect();
        let mut map: JavaHashMap<i32, ()> = JavaHashMap::new();
        for &k in &keys {
            map.insert(k, (), int_hash_code(k));
        }
        map.retain(|k, _| k % 32 == 0);
        assert_eq!(
            map.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            vec![0, 32, 64, 96, 128]
        );
        assert_eq!(map.len(), 5);
    }

    /// `Long` chunk keys (`ChunkCoordIntPair.chunkXZ2Int`) - oracle order.
    #[test]
    fn chunk_keys_use_long_hash_code() {
        let keys = [
            0i64,
            1,
            4294967297,
            8589934594,
            4294967296,
            -1,
            -4294967296,
            2,
            8589934592,
            12884901889,
            12884901888,
            30064771072,
        ];
        assert_eq!(
            order_long(&keys),
            vec![
                0,
                4294967297,
                8589934594,
                -1,
                -4294967296,
                1,
                4294967296,
                2,
                8589934592,
                12884901889,
                12884901888,
                30064771072
            ]
        );
    }
}
