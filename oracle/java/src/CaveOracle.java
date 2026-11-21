// Vanilla 1.8.9 `ChunkProviderGenerate.provideChunk` with caves and/or ravines toggled
// (structures always off), printed canonically.
//
// Usage: CaveOracle <seed> <mode> <cx> <cz> [<cx> <cz> ...]
//   mode 0 = caves only, 1 = ravines only, 2 = both, 3 = neither (base terrain)
// Format per chunk:
//   seed <seed> mode <mode> chunk <cx> <cz>
//   one line per (x, z) column: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata.
// The Rust probe (crates/mc-worldgen/examples/cave_probe.rs) prints the same format;
// tools/test_caves_parity.py compares them line-by-line.
//
// One seed per JVM: BiomeGenMesa caches its per-world noise on first use (see TerrainOracle).

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

public class CaveOracle {
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
        public String getWorldDirectoryName() { return "caves"; }
    }

    static class TestWorld extends World {
        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }

        // The chunk being generated is not yet in ChunkProviderServer.id2ChunkMap (ChunkProviderServer
        // .loadChunk registers it *after* generateChunk returns), so the real game's
        // World.getBiomeGenForCoords -> isBlockLoaded is false for cave/ravine biome lookups, and
        // the biome provider's cache path is used. Reproduce that with a world whose chunk provider
        // never reports a chunk as loaded.
        @Override
        protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) { return false; }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);
        int mode = Integer.parseInt(args[1]);

        ChunkProviderSettings.Factory f = new ChunkProviderSettings.Factory();
        f.useCaves = mode == 0 || mode == 2;
        f.useRavines = mode == 1 || mode == 2;
        String opts = f.toString();
        WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                .setWorldName(opts);
        WorldInfo wi = new WorldInfo(ws, "caves");
        WorldProvider provider = new WorldProviderSurface();
        TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
        provider.registerWorld(world);
        // mapFeaturesEnabled = false keeps mineshafts/villages/etc. off.
        ChunkProviderGenerate cp = new ChunkProviderGenerate(world, seed, false, opts);

        StringBuilder sb = new StringBuilder();
        for (int i = 2; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);
            Chunk chunk = cp.provideChunk(cx, cz);
            sb.append("seed ").append(seed).append(" mode ").append(mode)
                    .append(" chunk ").append(cx).append(' ').append(cz).append('\n');
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
