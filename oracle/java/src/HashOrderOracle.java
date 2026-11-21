// `java.util.HashMap` iteration order for a key sequence, which vanilla's worldgen depends on
// (`MapGenStructure.structureMap` with Long chunk keys, `EnchantmentHelper.mapEnchantmentData` with
// Integer enchantment ids).
//
// Usage: HashOrderOracle <int|long> <key> [<key> ...]
// Prints the keys in `keySet()` order, space-separated.

import java.util.HashMap;

public class HashOrderOracle {
    public static void main(String[] args) {
        boolean longs = args[0].equals("long");
        HashMap<Object, Integer> map = new HashMap<>();
        for (int i = 1; i < args.length; i++) {
            Object key = longs ? Long.valueOf(args[i]) : Integer.valueOf(args[i]);
            map.put(key, i);
        }

        StringBuilder sb = new StringBuilder();
        for (Object key : map.keySet()) {
            if (sb.length() > 0) {
                sb.append(' ');
            }
            sb.append(key);
        }
        System.out.println(sb);
    }
}