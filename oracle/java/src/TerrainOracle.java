// Vanilla 1.8.9 base-terrain *shape* (ChunkProviderGenerate.setBlocksInChunk), printed canonically.
// Runs against the remapped server jar (server-mcp.jar); the Rust probe
// (crates/mc-worldgen/examples/terrain_probe.rs) prints the same format.
//
// One line per (x, z) column: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.

import java.io.File;
import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.init.Bootstrap;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.profiler.Profiler;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.chunk.ChunkPrimer;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.chunk.storage.IChunkLoader;
import net.minecraft.world.gen.ChunkProviderGenerate;
import net.minecraft.world.gen.ChunkProviderSettings;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.WorldInfo;

public class TerrainOracle {
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

    public static void main(String[] args) {
        Bootstrap.register();
        long[] seeds = {0L, 12345L, -7L};
        int[][] chunks = {{0, 0}, {1, -1}};
        StringBuilder sb = new StringBuilder();

        for (long seed : seeds) {
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
            ChunkProviderGenerate cp = new ChunkProviderGenerate(world, seed, false, opts);

            for (int[] c : chunks) {
                ChunkPrimer primer = new ChunkPrimer();
                cp.setBlocksInChunk(c[0], c[1], primer);
                sb.append("seed ").append(seed).append(" chunk ").append(c[0]).append(' ').append(c[1]).append('\n');
                for (int x = 0; x < 16; x++) {
                    for (int z = 0; z < 16; z++) {
                        sb.append(x).append(' ').append(z);
                        for (int y = 0; y < 256; y++) {
                            IBlockState s = primer.getBlockState(x, y, z);
                            sb.append(' ').append(Block.getIdFromBlock(s.getBlock()) * 16 + s.getBlock().getMetaFromState(s));
                        }
                        sb.append('\n');
                    }
                }
            }
        }

        System.out.print(sb);
    }
}
