// Vanilla 1.8.9 noise samples, printed in a canonical format.
// Runs against the remapped server jar (server-mcp.jar); the Rust probe
// (crates/mc-worldgen/examples/noise_probe.rs) prints the same format.

import java.util.Random;
import net.minecraft.world.gen.NoiseGeneratorImproved;
import net.minecraft.world.gen.NoiseGeneratorOctaves;
import net.minecraft.world.gen.NoiseGeneratorPerlin;
import net.minecraft.world.gen.NoiseGeneratorSimplex;

public class NoiseOracle {
    static String hex(double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    public static void main(String[] args) {
        long[] seeds = {0L, 12345L, -1L};
        StringBuilder sb = new StringBuilder();

        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorImproved g = new NoiseGeneratorImproved(r);

            double[] buf = new double[5 * 5 * 5];
            g.populateNoiseArray(buf, 0.0, 0.0, 0.0, 5, 5, 5, 1.0, 1.0, 1.0, 1.0);
            sb.append("improved3d seed ").append(s).append('\n');
            for (double v : buf) sb.append(hex(v)).append('\n');

            double[] buf2 = new double[4 * 1 * 4];
            g.populateNoiseArray(buf2, 1.5, 2.5, 3.5, 4, 1, 4, 0.5, 0.5, 0.5, 2.0);
            sb.append("improved2d seed ").append(s).append('\n');
            for (double v : buf2) sb.append(hex(v)).append('\n');
        }

        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorSimplex g = new NoiseGeneratorSimplex(r);
            sb.append("simplex seed ").append(s).append('\n');
            for (int xi = 0; xi < 8; xi++) {
                for (int yi = 0; yi < 8; yi++) {
                    double x = xi * 0.7 - 2.0;
                    double y = yi * 0.7 - 2.0;
                    sb.append(hex(g.func_151605_a(x, y))).append('\n');
                }
            }
        }

        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorPerlin g = new NoiseGeneratorPerlin(r, 4);
            sb.append("perlin4 seed ").append(s).append('\n');
            for (int xi = 0; xi < 8; xi++) {
                for (int yi = 0; yi < 8; yi++) {
                    double x = xi * 0.7 - 2.0;
                    double y = yi * 0.7 - 2.0;
                    sb.append(hex(g.func_151601_a(x, y))).append('\n');
                }
            }
        }

        for (long s : seeds) {
            Random r = new Random(s);
            NoiseGeneratorOctaves g = new NoiseGeneratorOctaves(r, 16);
            double[] buf = new double[5 * 3 * 5];
            g.generateNoiseOctaves(buf, 0, 0, 0, 5, 3, 5, 1.0, 1.0, 1.0);
            sb.append("octaves16 seed ").append(s).append('\n');
            for (double v : buf) sb.append(hex(v)).append('\n');
        }

        System.out.print(sb);
    }
}
