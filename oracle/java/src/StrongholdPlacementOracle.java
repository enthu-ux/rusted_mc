// Stronghold block placement (the `populate` half) oracle: runs the real `ChunkProviderGenerate`
// with default settings, overrides `populate` to run *only* the stronghold `generateStructure`,
// populates the two chunks per axis whose `generateStructure` box (offset +8) covers the target
// chunk, and prints the target chunk's blocks.
//
// Usage: StrongholdPlacementOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per target chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines "x z v0 ... v255"  (vi = blockId * 16 + metadata)
//
// The `+8` box means a piece can be written by up to four populate calls; the stronghold pieces are
// large and underground, and every body except `PortalRoom` starts with
// `isLiquidInStructureBoundingBox` (a `true` drops the piece). State carries across the passes, so
// the port uses a fixed order; `STRONGHOLD_REVERSE` checks a fixture's order independence.
//
// Scope: `TestWorld` is a **remote** world, so `Chunk.setBlockState` skips `onBlockAdded` (torch
// support, door neighbours) and neighbour notifications are no-ops. Pre-providing the 3x3 keeps
// `world.setBlockState` from generating a chunk mid-iteration (which would mutate `structureMap`).

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
import net.minecraft.world.gen.structure.MapGenStronghold;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.MapStorage;
import net.minecraft.world.storage.WorldInfo;

public class StrongholdPlacementOracle {
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
        public String getWorldDirectoryName() { return "stronghold"; }
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
        @Override public boolean checkLight(BlockPos pos) { return false; }
        @Override public void forceBlockUpdateTick(Block b, BlockPos p, Random r) {}
    }

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
        public String makeString() { return "stronghold"; }
        public List<BiomeGenBase.SpawnListEntry> getPossibleCreatures(EnumCreatureType t, BlockPos p) { return null; }
        public BlockPos getStrongholdGen(World w, String s, BlockPos p) { return null; }
        public int getLoadedChunkCount() { return chunks.size(); }
        public void recreateStructures(Chunk c, int x, int z) {}
        public void saveExtraData() {}
    }

    static class StrongholdProvider extends ChunkProviderGenerate {
        private final World world;

        StrongholdProvider(World w, long seed, String opts) { super(w, seed, true, opts); this.world = w; }

        @Override
        public void populate(IChunkProvider provider, int x, int z) {
            Random rand = new Random();
            rand.setSeed(this.world.getSeed());
            long a = rand.nextLong() / 2L * 2L + 1L;
            long b = rand.nextLong() / 2L * 2L + 1L;
            rand.setSeed((long) x * a + (long) z * b ^ this.world.getSeed());
            try {
                Field f = ChunkProviderGenerate.class.getDeclaredField("strongholdGenerator");
                f.setAccessible(true);
                ((MapGenStronghold) f.get(this)).generateStructure(
                        this.world, rand, new ChunkCoordIntPair(x, z));
            } catch (ReflectiveOperationException e) {
                throw new RuntimeException(e);
            }
        }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);
        boolean reverse = System.getenv("STRONGHOLD_REVERSE") != null;
        String opts = new ChunkProviderSettings.Factory().toString();

        StringBuilder sb = new StringBuilder();
        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);

            WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                    .setWorldName(opts);
            WorldInfo wi = new WorldInfo(ws, "stronghold");
            WorldProvider provider = new WorldProviderSurface();
            TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), true);
            provider.registerWorld(world);
            StrongholdProvider gen = new StrongholdProvider(world, seed, opts);
            StubProvider stub = new StubProvider(gen);
            world.setProvider(stub);

            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    stub.provideChunk(cx + dx, cz + dz);
                }
            }
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