// Vanilla 1.8.9 *batch* noise samples, printed in a canonical format.
// Companion to NoiseOracle.java: covers the array paths
// (Perlin.func_151599_a / func_151600_a and Simplex.func_151606_a).
// Runs against the remapped server jar (server-mcp.jar); the Rust probe
// (crates/mc-worldgen/examples/noise_batch_probe.rs) prints the same format.

import java.util.Random;
import net.minecraft.world.gen.NoiseGeneratorPerlin;
import net.minecraft.world.gen.NoiseGeneratorSimplex;

public class NoiseBatchOracle {
    static String hex(double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    static void dump(StringBuilder sb, String tag, double[] a) {
        sb.append(tag).append('\n');
        for (double v : a) sb.append(hex(v)).append('\n');
    }

    public static void main(String[] args) {
        long[] seeds = {0L, 12345L, -1L};
        StringBuilder sb = new StringBuilder();

        // NoiseGeneratorSimplex.func_151606_a (2D batch; accumulates into a zeroed array).
        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorSimplex g = new NoiseGeneratorSimplex(r);

            double[] a = new double[5 * 4];
            g.func_151606_a(a, 1.5, -2.25, 5, 4, 1.0, 1.0, 1.0);
            dump(sb, "simplex_batch seed " + s, a);

            double[] b = new double[8 * 3];
            g.func_151606_a(b, 0.0, 0.0, 8, 3, 0.5, 0.75, 2.0);
            dump(sb, "simplex_batch_scaled seed " + s, b);
        }

        // NoiseGeneratorPerlin.func_151600_a (explicit freq/amp decay).
        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorPerlin g = new NoiseGeneratorPerlin(r, 4);

            double[] a = new double[5 * 5];
            g.func_151600_a(a, 0.0, 0.0, 5, 5, 1.0, 1.0, 0.5, 0.5);
            dump(sb, "perlin_batch seed " + s, a);

            double[] b = new double[4 * 6];
            g.func_151600_a(b, -3.0, 2.0, 4, 6, 0.25, 0.5, 0.6, 0.75);
            dump(sb, "perlin_batch_scaled seed " + s, b);
        }

        // NoiseGeneratorPerlin.func_151599_a (amp decay fixed at 0.5).
        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorPerlin g = new NoiseGeneratorPerlin(r, 8);

            double[] a = new double[5 * 5];
            g.func_151599_a(a, 0.0, 0.0, 5, 5, 1.0, 1.0, 0.5);
            dump(sb, "perlin_default seed " + s, a);
        }

        System.out.print(sb);
    }
}
