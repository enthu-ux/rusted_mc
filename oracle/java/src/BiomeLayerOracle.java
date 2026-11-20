// Vanilla 1.8.9 biome-layer output (biome ids), printed in a canonical format.
// Runs against the remapped server jar (server-mcp.jar); the Rust probe
// (crates/mc-worldgen/examples/biome_layer_probe.rs) prints the same format.
//
// Layer 0 = genBiomes (var5[0]); layer 1 = biomeIndexLayer (var5[1], Voronoi).

import net.minecraft.init.Bootstrap;
import net.minecraft.world.WorldType;
import net.minecraft.world.gen.layer.GenLayer;
import net.minecraft.world.gen.layer.IntCache;

public class BiomeLayerOracle {
    public static void main(String[] args) {
        Bootstrap.register();

        long[] seeds = {0L, 12345L, -7L};
        int[][] regions = {{0, 0, 16, 16}, {-37, 210, 16, 16}, {512, 512, 16, 16}};
        StringBuilder sb = new StringBuilder();

        for (long s : seeds) {
            GenLayer[] layers = GenLayer.initializeAllBiomeGenerators(s, WorldType.DEFAULT, "");
            for (int li = 0; li < 2; li++) {
                for (int[] r : regions) {
                    IntCache.resetIntCache();
                    int[] ids = layers[li].getInts(r[0], r[1], r[2], r[3]);
                    sb.append("seed ").append(s).append(" layer ").append(li)
                      .append(' ').append(r[0]).append(' ').append(r[1]).append(' ')
                      .append(r[2]).append(' ').append(r[3]).append('\n');
                    for (int v : ids) sb.append(v).append('\n');
                }
            }
        }

        System.out.print(sb);
    }
}
