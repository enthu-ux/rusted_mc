// In-process base-terrain chunk-generation benchmark against the real 1.8.9 server code.
// Prints one JSON line; paired with the Rust `terrain_bench` by tools/perf_baseline.py.

import java.io.File;
import java.util.Arrays;
import java.util.Locale;
import net.minecraft.init.Bootstrap;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.profiler.Profiler;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.chunk.storage.IChunkLoader;
import net.minecraft.world.gen.ChunkProviderGenerate;
import net.minecraft.world.gen.ChunkProviderSettings;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.WorldInfo;

public class TerrainBench {
    static class SaveHandler implements ISaveHandler {
        public WorldInfo loadWorldInfo() { return null; }
        public void checkSessionLock() {}
        public IChunkLoader getChunkLoader(WorldProvider p) { return null; }
        public void saveWorldInfoWithPlayer(WorldInfo w, NBTTagCompound n) {}
        public void saveWorldInfo(WorldInfo w) {}
        public IPlayerFileData getPlayerNBTManager() { return null; }
        public void flush() {}
        public File getWorldDirectory() { return new File("."); }
        public File getMapFileFromName(String s) { return null; }
        public String getWorldDirectoryName() { return "terrain"; }
    }

    static class TestWorld extends World {
        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
    }

    static ChunkProviderGenerate provider(long seed) {
        ChunkProviderSettings.Factory f = new ChunkProviderSettings.Factory();
        f.useCaves = false;
        f.useRavines = false;
        String opts = f.toString();
        WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                .setWorldName(opts);
        WorldInfo wi = new WorldInfo(ws, "terrain");
        WorldProvider provider = new WorldProviderSurface();
        TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
        provider.registerWorld(world);
        return new ChunkProviderGenerate(world, seed, false, opts);
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long[] seeds = {0L, 12345L, -7L};
        int[][] chunks = {{0, 0}, {1, -1}, {-48, -48}, {-37, 210}, {512, 512}};
        int iters = 5;

        ChunkProviderGenerate[] providers = new ChunkProviderGenerate[seeds.length];
        for (int i = 0; i < seeds.length; i++) {
            providers[i] = provider(seeds[i]);
            for (int[] c : chunks) {
                providers[i].provideChunk(c[0], c[1]); // warm up
            }
        }

        long[] samples = new long[seeds.length * chunks.length * iters];
        int n = 0;
        for (int it = 0; it < iters; it++) {
            for (int i = 0; i < seeds.length; i++) {
                for (int[] c : chunks) {
                    long start = System.nanoTime();
                    Chunk chunk = providers[i].provideChunk(c[0], c[1]);
                    samples[n++] = System.nanoTime() - start;
                    if (chunk == null) throw new IllegalStateException("null chunk");
                }
            }
        }

        Arrays.sort(samples);
        double min = samples[0] / 1.0e6;
        double median = samples[samples.length / 2] / 1.0e6;
        int p95i = (int) Math.ceil(0.95 * samples.length) - 1;
        if (p95i < 0) p95i = 0;
        if (p95i >= samples.length) p95i = samples.length - 1;
        double p95 = samples[p95i] / 1.0e6;
        double max = samples[samples.length - 1] / 1.0e6;
        double mean = 0.0;
        for (long s : samples) mean += s / 1.0e6;
        mean /= samples.length;

        System.out.println(String.format(
                Locale.US,
                "{\"bench\":\"terrain_java\",\"samples\":%d,\"per_chunk_ms\":{\"min\":%.4f,\"median\":%.4f,\"p95\":%.4f,\"max\":%.4f,\"mean\":%.4f}}",
                samples.length, min, median, p95, max, mean));
    }
}