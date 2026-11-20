// Vanilla behaviour for `java.util.Random`, printed in a canonical format.
// The Rust probe (crates/mc-core/examples/random_probe.rs) prints the same format.
// Compiled and run by tools/test_random_parity.py with the JDK 8 javac/java.

import java.util.Random;

public class RandomOracle {
    public static void main(String[] args) {
        long[] seeds = {
            0L, 1L, 12345L, -1L,
            2147483647L, -2147483648L,
            Long.MAX_VALUE, Long.MIN_VALUE
        };
        int[] bounds = {1, 2, 3, 6, 7, 16, 255, 256, 1000, 123456789};

        StringBuilder sb = new StringBuilder();
        for (long s : seeds) {
            sb.append("seed ").append(s).append('\n');
            Random r = new Random(s);
            for (int i = 0; i < 64; i++) {
                sb.append("nextInt ").append(r.nextInt()).append('\n');
            }
            for (int b : bounds) {
                for (int i = 0; i < 16; i++) {
                    sb.append("nextInt(").append(b).append(") ")
                      .append(r.nextInt(b)).append('\n');
                }
            }
            for (int i = 0; i < 64; i++) {
                sb.append("nextLong ").append(r.nextLong()).append('\n');
            }
            for (int i = 0; i < 64; i++) {
                sb.append("nextBoolean ").append(r.nextBoolean() ? 1 : 0).append('\n');
            }
            for (int i = 0; i < 64; i++) {
                sb.append("nextFloat ")
                  .append(String.format("%08x", Float.floatToRawIntBits(r.nextFloat())))
                  .append('\n');
            }
            for (int i = 0; i < 64; i++) {
                sb.append("nextDouble ")
                  .append(String.format("%016x", Double.doubleToRawLongBits(r.nextDouble())))
                  .append('\n');
            }
        }
        System.out.print(sb);
    }
}
