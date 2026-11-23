// Temple block placement (the `populate` half) oracle: runs the real `ChunkProviderGenerate` with
// only temples enabled, overrides `populate` to run *only* the scattered-feature `generateStructure`,
// populates the two chunks per axis whose `generateStructure` box (offset +8) covers the target
// chunk, and prints the target chunk's blocks.
//
// Usage: TemplePlacementOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per target chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines "x z v0 ... v255"  (vi = blockId * 16 + metadata)
//
// The `+8` box means a piece can be written by up to four populate calls; a temple's `func_74935_a`
// offsets its box on the first pass (state carries across passes), so the orders are checked
// (env `TEMPLE_REVERSE` runs the four passes in reverse). Pre-providing the 3x3 keeps
// `world.setBlockState` from generating a chunk mid-iteration (which would mutate `structureMap`).
//
// Note: `Chunk.setBlockState` calls `Block.onBlockAdded`; the temples place tripwire hooks,
// pressure plates and the like, so fixtures that trigger block-update side effects can differ from
// the pure placement port (see `handoff.md`).

import java.lang.reflect.Field;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;

import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.entity.EnumCreatureType;
import net.minecraft.init.Bootstrap;
import net.minecraft.profiler.Profiler;
import net.minecraft.util.BlockPos;
import net.minecraft.util.IProgressUpdate;
import net.minecraft.world.ChunkCoordIntPair;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.biome.BiomeGenBase;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.chunk.storage.IChunkLoader;
import net.minecraft.world.gen.ChunkProviderGenerate;
import net.minecraft.world.gen.ChunkProviderSettings;
import net.minecraft.world.gen.structure.MapGenScatteredFeature;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.MapStorage;
import net.minecraft.world.storage.WorldInfo;

public class TemplePlacementOracle {
    static class SaveHandler implements ISaveHandler {
        public WorldInfo loadWorldInfo() { return null; }
        public void checkSessionLock() {}
        public IChunkLoader getChunkLoader(WorldProvider p) { return null; }
        public void saveWorldInfoWithPlayer(WorldInfo w, net.minecraft.nbt.NBTTagCompound n) {}
        public void saveWorldInfo(WorldInfo w) {}
        public IPlayerFileData getPlayerNBTManager() { return null; }
        public void flush() {}
        public java.io.File getWorldDirectory() { return new java.io.File("."); }
        public java.io.File getMapFileFromName(String s) { return null; }
        public String getWorldDirectoryName() { return "temple"; }
    }

    static class TestWorld extends World {
        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
            this.mapStorage = new MapStorage(h);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
        @Override protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) { return false; }
        void setProvider(IChunkProvider p) { this.chunkProvider = p; }
        // Blocks are all that matter here; skip the (slow) light engine.
        @Override public boolean checkLight(BlockPos pos) { return false; }
    }

    /** Serves the generated chunks back to `World.getChunkFromBlockCoords` (generating on demand). */
    static class StubProvider implements IChunkProvider {
        final ChunkProviderGenerate gen;
        final Map<Long, Chunk> chunks = new HashMap<>();

        StubProvider(ChunkProviderGenerate gen) { this.gen = gen; }

        public boolean chunkExists(int x, int z) {
            return chunks.containsKey(ChunkCoordIntPair.chunkXZ2Int(x, z));
        }
        public Chunk provideChunk(int x, int z) {
            long k = ChunkCoordIntPair.chunkXZ2Int(x, z);
            Chunk c = chunks.get(k);
            if (c == null) { c = gen.provideChunk(x, z); chunks.put(k, c); }
            return c;
        }
        public Chunk provideChunk(BlockPos pos) { return provideChunk(pos.getX() >> 4, pos.getZ() >> 4); }
        public void populate(IChunkProvider p, int x, int z) {}
        public boolean populateChunk(IChunkProvider p, Chunk c, int x, int z) { return false; }
        public boolean saveChunks(boolean b, IProgressUpdate p) { return true; }
        public boolean unloadQueuedChunks() { return false; }
        public boolean canSave() { return false; }
        public String makeString() { return "temple"; }
        public List<BiomeGenBase.SpawnListEntry> getPossibleCreatures(EnumCreatureType t, BlockPos p) { return null; }
        public BlockPos getStrongholdGen(World w, String s, BlockPos p) { return null; }
        public int getLoadedChunkCount() { return chunks.size(); }
        public void recreateStructures(Chunk c, int x, int z) {}
        public void saveExtraData() {}
    }

    /** `ChunkProviderGenerate` with `populate` reduced to the scattered-feature `generateStructure`. */
    static class TempleProvider extends ChunkProviderGenerate {
        private final World world;

        TempleProvider(World w, long seed, String opts) { super(w, seed, true, opts); this.world = w; }

        @Override
        public void populate(IChunkProvider provider, int x, int z) {
            Random rand = new Random();
            rand.setSeed(this.world.getSeed());
            long a = rand.nextLong() / 2L * 2L + 1L;
            long b = rand.nextLong() / 2L * 2L + 1L;
            rand.setSeed((long) x * a + (long) z * b ^ this.world.getSeed());
            try {
                Field f = ChunkProviderGenerate.class.getDeclaredField("scatteredFeatureGenerator");
                f.setAccessible(true);
                ((MapGenScatteredFeature) f.get(this)).generateStructure(
                        this.world, rand, new ChunkCoordIntPair(x, z));
            } catch (ReflectiveOperationException e) {
                throw new RuntimeException(e);
            }
        }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);

        ChunkProviderSettings.Factory factory = new ChunkProviderSettings.Factory();
        factory.useVillages = false;
        factory.useStrongholds = false;
        factory.useTemples = true;
        factory.useMonuments = false;
        factory.useMineShafts = false;
        factory.useWaterLakes = false;
        factory.useLavaLakes = false;
        factory.useDungeons = false;
        String opts = factory.toString();

        StringBuilder sb = new StringBuilder();
        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);

            WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                    .setWorldName(opts);
            WorldInfo wi = new WorldInfo(ws, "temple");
            WorldProvider provider = new WorldProviderSurface();
            TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
            provider.registerWorld(world);
            TempleProvider gen = new TempleProvider(world, seed, opts);
            StubProvider stub = new StubProvider(gen);
            world.setProvider(stub);

            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    stub.provideChunk(cx + dx, cz + dz);
                }
            }
            boolean reverse = System.getenv("TEMPLE_REVERSE") != null;
            int[] as = reverse ? new int[] { cx, cx - 1 } : new int[] { cx - 1, cx };
            int[] bs = reverse ? new int[] { cz, cz - 1 } : new int[] { cz - 1, cz };
            for (int a : as) {
                for (int b : bs) {
                    gen.populate(stub, a, b);
                }
            }

            Chunk chunk = stub.provideChunk(cx, cz);
            sb.append("seed ").append(seed).append(" chunk ").append(cx).append(' ').append(cz).append('\n');
            BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
            for (int x = 0; x < 16; x++) {
                for (int z = 0; z < 16; z++) {
                    sb.append(x).append(' ').append(z);
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