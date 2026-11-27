// Full `populate` oracle: runs the real `ChunkProviderGenerate.populate` (default settings, all
// features) for the four chunks whose `+8` window covers the target chunk, then prints the target
// chunk's blocks + metadata, biome array and height map - the same format as `WorldgenOracle`.
//
// Usage: PopulateOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per target chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines "x z v0 ... v255"  (vi = blockId * 16 + metadata)
//   biome <256 ids>     - Chunk.getBiomeArray(), index x + z*16
//   height <256 values> - Chunk.getHeightValue(x, z), index z*16 + x
//
// The pre-provided 3x3 is generated with `isChunkLoaded = false` (so structure registration uses the
// biome-provider / Voronoi path, as the port does); during `populate` the chunks are "loaded" (so
// `getBiomeGenForCoords` reads the stored generation biome array, as the port's `biome_at` does).
// The world is remote, so `Chunk.setBlockState` skips `Block.breakBlock`/`onBlockAdded` - the
// placement-only scope the other harnesses use (block updates are not ported). The light engine is
// disabled (`checkLight`/`checkLightFor`), matching the port's column-only skylight model; the
// horizontal flood-fill is `light_parity`'s.

import java.util.HashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.entity.Entity;
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
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.MapStorage;
import net.minecraft.world.storage.WorldInfo;

public class PopulateOracle {
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
        public String getWorldDirectoryName() { return "populate"; }
    }

    static class TestWorld extends World {
        boolean generating = false;

        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
            this.mapStorage = new MapStorage(h);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
        // During generation: force the biome-provider path (structure registration). During
        // populate: the pre-provided chunks are loaded (stored biome array + getHeight).
        @Override protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) {
            return !generating && chunkProvider != null && chunkProvider.chunkExists(x, z);
        }
        void setProvider(IChunkProvider p) { this.chunkProvider = p; }
        @Override public boolean checkLight(BlockPos pos) { return false; }
        @Override public boolean checkLightFor(net.minecraft.world.EnumSkyBlock lightType, BlockPos pos) { return false; }
        // Entities are irrelevant to the block artefact; skip the bookkeeping (the RNG draws in
        // `performWorldGenSpawning` happen regardless and nothing after them uses the populate rand).
        @Override public boolean spawnEntityInWorld(Entity entity) { return true; }
    }

    static class StubProvider implements IChunkProvider {
        final ChunkProviderGenerate gen;
        final Map<Long, Chunk> chunks = new HashMap<>();

        StubProvider(ChunkProviderGenerate gen) { this.gen = gen; }

        public boolean chunkExists(int x, int z) { return chunks.containsKey(ChunkCoordIntPair.chunkXZ2Int(x, z)); }
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
        public String makeString() { return "populate"; }
        public List<BiomeGenBase.SpawnListEntry> getPossibleCreatures(EnumCreatureType t, BlockPos p) { return null; }
        public BlockPos getStrongholdGen(World w, String s, BlockPos p) { return null; }
        public int getLoadedChunkCount() { return chunks.size(); }
        public void recreateStructures(Chunk c, int x, int z) {}
        public void saveExtraData() {}
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);

        ChunkProviderSettings.Factory factory = new ChunkProviderSettings.Factory();
        String off = System.getenv("POPULATE_OFF");
        if (off == null) off = "";
        if (off.contains("structures")) {
            factory.useVillages = false;
            factory.useStrongholds = false;
            factory.useTemples = false;
            factory.useMonuments = false;
            factory.useMineShafts = false;
        }
        if (off.contains("lakes")) {
            factory.useWaterLakes = false;
            factory.useLavaLakes = false;
        }
        if (off.contains("water")) factory.useWaterLakes = false;
        if (off.contains("lava")) factory.useLavaLakes = false;
        if (off.contains("dungeons")) factory.useDungeons = false;
        String opts = factory.toString();

        StringBuilder sb = new StringBuilder();
        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);

            WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                    .setWorldName(opts);
            WorldInfo wi = new WorldInfo(ws, "populate");
            WorldProvider provider = new WorldProviderSurface();
            TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
            provider.registerWorld(world);
            ChunkProviderGenerate gen = new ChunkProviderGenerate(world, seed, true, opts);
            StubProvider stub = new StubProvider(gen);
            world.setProvider(stub);

            world.generating = true;
            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    stub.provideChunk(cx + dx, cz + dz);
                }
            }
            world.generating = false;

            for (int a : new int[] { cx - 1, cx }) {
                for (int b : new int[] { cz - 1, cz }) {
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