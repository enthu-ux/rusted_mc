// Vanilla 1.8.9 `ChunkProviderGenerate.provideChunk` canonical chunk, printed for parity:
// blocks + metadata, the chunk biome array, and the chunk height map.
//
// Usage: WorldgenOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines: "x z v0 v1 ... v255" where vi = blockId * 16 + metadata
//   biome <256 ids>     - Chunk.getBiomeArray(), unsigned, index x + z*16
//   height <256 values> - Chunk.getHeightValue(x, z), index z*16 + x
//
// Settings are `ChunkProviderSettings.Factory` defaults (caves + ravines on); structures stay off
// (`mapFeaturesEnabled = false`), so this is the pre-decoration chunk. One seed per JVM: BiomeGenMesa
// caches its per-world noise on first use (see TerrainOracle).

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

public class WorldgenOracle {
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
        public String getWorldDirectoryName() { return "worldgen"; }
    }

    static class TestWorld extends World {
        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
        // As in CaveOracle: while a chunk is being generated it is not in the server's loaded-chunk
        // map, so biome lookups go through the biome provider.
        @Override
        protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) { return false; }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);

        ChunkProviderSettings.Factory f = new ChunkProviderSettings.Factory();
        String opts = f.toString();
        WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                .setWorldName(opts);
        WorldInfo wi = new WorldInfo(ws, "worldgen");
        WorldProvider provider = new WorldProviderSurface();
        TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
        provider.registerWorld(world);
        ChunkProviderGenerate cp = new ChunkProviderGenerate(world, seed, false, opts);

        StringBuilder sb = new StringBuilder();
        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);
            Chunk chunk = cp.provideChunk(cx, cz);
            sb.append("seed ").append(seed).append(" chunk ").append(cx).append(' ').append(cz).append('\n');
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

            byte[] biomeArray = chunk.getBiomeArray();
            sb.append("biome");
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    sb.append(' ').append(biomeArray[x + z * 16] & 255);
                }
            }
            sb.append('\n');

            sb.append("height");
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    sb.append(' ').append(chunk.getHeightValue(x, z));
                }
            }
            sb.append('\n');
        }

        System.out.print(sb);
    }
}
