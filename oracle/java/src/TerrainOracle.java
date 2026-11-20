// Vanilla 1.8.9 base terrain (ChunkProviderGenerate.provideChunk, caves/ravines/structures off),
// printed canonically. Runs against the remapped server jar (server-mcp.jar); the Rust probe
// (crates/mc-worldgen/examples/terrain_probe.rs) prints the same format.
//
// One line per (x, z) column: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.
// One seed per process (Mesa caches per-world noise on first use, so a fresh JVM per seed is
// required for reproducibility).

import java.io.File;
import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.init.Bootstrap;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.profiler.Profiler;
import net.minecraft.util.BlockPos;
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
        long seed = Long.parseLong(args[0]);
        int[][] chunks = {
            {-48, -48}, {-22, -22}, {-48, -43}, {-6, 47}, {-42, 15}, {-22, 46}, {-33, 39}, {-47, 47},
            {-169, 30}, {88, -81}, {126, -14}, {-184, -135}, {-40, -126}, {-111, 6},
            {-38, -192}, {-124, -128}, {27, -110}, {-48, 185}, {36, -163}, {100, -52}, {-4096, -2048}
        };
        StringBuilder sb = new StringBuilder();

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
            Chunk chunk = cp.provideChunk(c[0], c[1]);
            sb.append("seed ").append(seed).append(" chunk ").append(c[0]).append(' ').append(c[1]).append('\n');
            for (int x = 0; x < 16; x++) {
                for (int z = 0; z < 16; z++) {
                    sb.append(x).append(' ').append(z);
                    BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
                    for (int y = 0; y < 256; y++) {
                        IBlockState s = chunk.getBlockState(pos.set(x, y, z));
                        sb.append(' ').append(Block.getIdFromBlock(s.getBlock()) * 16 + s.getBlock().getMetaFromState(s));
                    }
                    sb.append('\n');
                }
            }
        }

        System.out.print(sb);
    }
}
