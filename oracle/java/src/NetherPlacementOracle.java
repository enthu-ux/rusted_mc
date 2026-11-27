// Nether-fortress block placement (the `populate` half) oracle: builds a headless nether world whose
// chunk provider serves a *synthetic* base chunk (bedrock at y=0, netherrack y=1..39, air above),
// registers the fortresses with the real `MapGenNetherBridge`, then runs the four covering
// `generateStructure` passes and prints the target chunk's blocks.
//
// Usage: NetherPlacementOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per target chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines "x z v0 ... v255"  (vi = blockId * 16 + metadata)
//
// The placement RNG is `ChunkProviderHell`'s shared `hellRNG`: `provideChunk` seeds it with
// `cx * 341873128712 + cz * 132897987541`, and `func_180516_b` advances it by a fixed pattern
// (3 nextDouble per column, then two conditionally short-circuited nextInt(5) per y). `hellRng`
// replays that, so each pass uses the state it would have right after that chunk was generated.
//
// Scope notes:
//   - The nether terrain is not ported, so both sides use the same synthetic base. Fortress
//     placement only reads it through `replaceAirAndLiquidDownwards`; nether pieces never call
//     `isLiquidInStructureBoundingBox`, so no piece is dropped.
//   - `Entrance` calls `World.forceBlockUpdateTick(flowing_lava)`, which would flow the lava
//     (block-update semantics, out of scope). `TestWorld` neutralises it, matching the port.
//   - `TEMPLE`-style chunk-load history: vanilla only re-seeds `hellRNG` while generating a chunk, so
//     a chunk already in memory keeps a stale state. The corpus uses the canonical
//     generate-then-populate sequence; `NETHER_REVERSE` checks pass-order independence.

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;

import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.entity.EnumCreatureType;
import net.minecraft.init.Blocks;
import net.minecraft.init.Bootstrap;
import net.minecraft.profiler.Profiler;
import net.minecraft.util.BlockPos;
import net.minecraft.util.IProgressUpdate;
import net.minecraft.world.ChunkCoordIntPair;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderHell;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.biome.BiomeGenBase;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.chunk.ChunkPrimer;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.gen.structure.MapGenNetherBridge;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.MapStorage;
import net.minecraft.world.storage.WorldInfo;

public class NetherPlacementOracle {
    static class SaveHandler implements ISaveHandler {
        public WorldInfo loadWorldInfo() { return null; }
        public void checkSessionLock() {}
        public net.minecraft.world.chunk.storage.IChunkLoader getChunkLoader(WorldProvider p) { return null; }
        public void saveWorldInfoWithPlayer(WorldInfo w, net.minecraft.nbt.NBTTagCompound n) {}
        public void saveWorldInfo(WorldInfo w) {}
        public IPlayerFileData getPlayerNBTManager() { return null; }
        public void flush() {}
        public java.io.File getWorldDirectory() { return new java.io.File("."); }
        public java.io.File getMapFileFromName(String s) { return null; }
        public String getWorldDirectoryName() { return "nether"; }
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
        // `Entrance` forces a lava tick; block updates are outside the placement port.
        @Override public void forceBlockUpdateTick(Block b, BlockPos p, Random r) {}
    }

    /** Serves synthetic nether chunks (so the base is identical to the Rust probe). */
    static class StubProvider implements IChunkProvider {
        final World world;
        final Map<Long, Chunk> chunks = new HashMap<>();

        StubProvider(World w) { this.world = w; }

        static Chunk synthetic(World w, int x, int z) {
            ChunkPrimer primer = new ChunkPrimer();
            for (int bx = 0; bx < 16; bx++) {
                for (int bz = 0; bz < 16; bz++) {
                    primer.setBlockState(bx, 0, bz, Blocks.bedrock.getDefaultState());
                    for (int y = 1; y <= 39; y++) {
                        primer.setBlockState(bx, y, bz, Blocks.netherrack.getDefaultState());
                    }
                }
            }
            return new Chunk(w, primer, x, z);
        }

        public boolean chunkExists(int x, int z) {
            return chunks.containsKey(ChunkCoordIntPair.chunkXZ2Int(x, z));
        }
        public Chunk provideChunk(int x, int z) {
            long k = ChunkCoordIntPair.chunkXZ2Int(x, z);
            Chunk c = chunks.get(k);
            if (c == null) { c = synthetic(world, x, z); chunks.put(k, c); }
            return c;
        }
        public Chunk provideChunk(BlockPos pos) { return provideChunk(pos.getX() >> 4, pos.getZ() >> 4); }
        public void populate(IChunkProvider p, int x, int z) {}
        public boolean populateChunk(IChunkProvider p, Chunk c, int x, int z) { return false; }
        public boolean saveChunks(boolean b, IProgressUpdate p) { return true; }
        public boolean unloadQueuedChunks() { return false; }
        public boolean canSave() { return false; }
        public String makeString() { return "nether"; }
        public List<BiomeGenBase.SpawnListEntry> getPossibleCreatures(EnumCreatureType t, BlockPos p) { return null; }
        public BlockPos getStrongholdGen(World w, String s, BlockPos p) { return null; }
        public int getLoadedChunkCount() { return chunks.size(); }
        public void recreateStructures(Chunk c, int x, int z) {}
        public void saveExtraData() {}
    }

    /** `ChunkProviderHell.provideChunk`'s `hellRNG` state at `populate(cx, cz)`, without terrain. */
    static Random hellRng(int cx, int cz) {
        Random rand = new Random();
        rand.setSeed((long) cx * 341873128712L + (long) cz * 132897987541L);
        for (int z = 0; z < 16; z++) {
            for (int x = 0; x < 16; x++) {
                rand.nextDouble(); // slowsand
                rand.nextDouble(); // gravel
                rand.nextDouble(); // netherrack exclusivity
                for (int y = 127; y >= 0; y--) {
                    int d1 = rand.nextInt(5);
                    if (y < 127 - d1) {
                        rand.nextInt(5);
                    }
                }
            }
        }
        return rand;
    }

    public static void main(String[] args) {
        Bootstrap.register();
        // `rng <cx> <cz> ...`: print the 8 draws after the hellRNG replay (the placement stream the
        // block dump cannot see - only chest contents consume it).
        if (args[0].equals("rng")) {
            StringBuilder rb = new StringBuilder();
            for (int i = 1; i + 1 < args.length; i += 2) {
                int cx = Integer.parseInt(args[i]);
                int cz = Integer.parseInt(args[i + 1]);
                Random r = hellRng(cx, cz);
                rb.append("rng ").append(cx).append(' ').append(cz);
                for (int k = 0; k < 8; k++) {
                    rb.append(' ').append(r.nextInt());
                }
                rb.append('\n');
            }
            System.out.print(rb);
            return;
        }

        long seed = Long.parseLong(args[0]);
        boolean reverse = System.getenv("NETHER_REVERSE") != null;
        StringBuilder sb = new StringBuilder();

        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);

            WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                    .setWorldName("nether");
            WorldInfo wi = new WorldInfo(ws, "nether");
            WorldProvider provider = new WorldProviderHell();
            TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
            provider.registerWorld(world);
            StubProvider stub = new StubProvider(world);
            world.setProvider(stub);

            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    stub.provideChunk(cx + dx, cz + dz);
                }
            }

            MapGenNetherBridge bridge = new MapGenNetherBridge();
            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    bridge.generate(stub, world, cx + dx, cz + dz, new ChunkPrimer());
                }
            }

            int[] as = reverse ? new int[] { cx, cx - 1 } : new int[] { cx - 1, cx };
            int[] bs = reverse ? new int[] { cz, cz - 1 } : new int[] { cz - 1, cz };
            for (int a : as) {
                for (int b : bs) {
                    bridge.generateStructure(world, hellRng(a, b), new ChunkCoordIntPair(a, b));
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