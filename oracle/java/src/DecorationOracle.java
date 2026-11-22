// Decoration oracle: runs the real `ChunkProviderGenerate` for terrain, then a *guarded* mirror of
// `BiomeDecorator.genDecorations` (only the layers named by the mode), and prints the target chunk's
// blocks. Same shape as `MineshaftPlacementOracle`.
//
// Usage: DecorationOracle <seed> <mode> <cx> <cz> [<cx> <cz> ...]
//   mode = ores | surface | ore_surface
// Per target chunk:
//   seed <seed> chunk <cx> <cz>
//   256 lines "x z v0 ... v255"  (vi = blockId * 16 + metadata)
//
// Decoration is `populate`'s shifted 16x16 window (`field_180294_c + 8..+23`, features spread a few
// blocks further), so the target chunk is the union of the four covering passes
// ((cx-1..cx) x (cz-1..cz)); their regions overlap, so a fixed pass order is used (vanilla's is
// chunk-load order). The surrounding 3x3 is pre-generated before any pass writes.

import java.lang.reflect.Field;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;

import net.minecraft.block.Block;
import net.minecraft.block.BlockFlower;
import net.minecraft.block.BlockStone;
import net.minecraft.block.material.Material;
import net.minecraft.block.state.IBlockState;
import net.minecraft.entity.EnumCreatureType;
import net.minecraft.init.Bootstrap;
import net.minecraft.init.Blocks;
import net.minecraft.profiler.Profiler;
import net.minecraft.util.BlockPos;
import net.minecraft.util.IProgressUpdate;
import net.minecraft.world.ChunkCoordIntPair;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.biome.BiomeDecorator;
import net.minecraft.world.biome.BiomeGenBase;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.gen.ChunkProviderGenerate;
import net.minecraft.world.gen.ChunkProviderSettings;
import net.minecraft.world.gen.GeneratorBushFeature;
import net.minecraft.world.gen.feature.WorldGenBigMushroom;
import net.minecraft.world.gen.feature.WorldGenClay;
import net.minecraft.world.gen.feature.WorldGenDeadBush;
import net.minecraft.world.gen.feature.WorldGenFlowers;
import net.minecraft.world.gen.feature.WorldGenMinable;
import net.minecraft.world.gen.feature.WorldGenLiquids;
import net.minecraft.world.gen.feature.WorldGenPumpkin;
import net.minecraft.world.gen.feature.WorldGenSand;
import net.minecraft.world.gen.feature.WorldGenerator;

public class DecorationOracle {
    static class SaveHandler implements net.minecraft.world.storage.ISaveHandler {
        public net.minecraft.world.storage.WorldInfo loadWorldInfo() { return null; }
        public void checkSessionLock() {}
        public net.minecraft.world.chunk.storage.IChunkLoader getChunkLoader(WorldProvider p) { return null; }
        public void saveWorldInfoWithPlayer(net.minecraft.world.storage.WorldInfo w, net.minecraft.nbt.NBTTagCompound n) {}
        public void saveWorldInfo(net.minecraft.world.storage.WorldInfo w) {}
        public net.minecraft.world.storage.IPlayerFileData getPlayerNBTManager() { return null; }
        public void flush() {}
        public java.io.File getWorldDirectory() { return new java.io.File("."); }
        public java.io.File getMapFileFromName(String s) { return null; }
        public String getWorldDirectoryName() { return "decoration"; }
    }

    static class TestWorld extends World {
        TestWorld(net.minecraft.world.storage.ISaveHandler h, net.minecraft.world.storage.WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
            this.mapStorage = new net.minecraft.world.storage.MapStorage(h);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
        // During `populate` the pre-provided chunks are loaded (needed by `World.getHeight`); during
        // generation the chunk is not yet in the stub, so the biome-provider path is taken.
        @Override protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) {
            return this.chunkProvider != null && this.chunkProvider.chunkExists(x, z);
        }
        void setProvider(IChunkProvider p) { this.chunkProvider = p; }
        @Override public boolean checkLight(BlockPos pos) { return false; }
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
        public String makeString() { return "decoration"; }
        public List<BiomeGenBase.SpawnListEntry> getPossibleCreatures(EnumCreatureType t, BlockPos p) { return null; }
        public BlockPos getStrongholdGen(World w, String s, BlockPos p) { return null; }
        public int getLoadedChunkCount() { return chunks.size(); }
        public void recreateStructures(Chunk c, int x, int z) {}
        public void saveExtraData() {}
    }

    static ChunkProviderSettings settings;

    static class DecorProvider extends ChunkProviderGenerate {
        private final World world;
        private final String mode;

        DecorProvider(World w, long seed, String opts, String mode) { super(w, seed, true, opts); this.world = w; this.mode = mode; }

        @Override
        public void populate(IChunkProvider provider, int x, int z) {
            Random rand = new Random();
            rand.setSeed(this.world.getSeed());
            long a = rand.nextLong() / 2L * 2L + 1L;
            long b = rand.nextLong() / 2L * 2L + 1L;
            rand.setSeed((long) x * a + (long) z * b ^ this.world.getSeed());

            int bx = x * 16;
            int bz = z * 16;
            BlockPos field = new BlockPos(bx, 0, bz);
            if (mode.contains("ore")) {
                generateOres(this.world, rand, field);
            }
            if (mode.contains("surface")) {
                surface(this.world, rand, field);
            }
            if (mode.contains("tree") || mode.contains("plant")) {
                BiomeGenBase biome = this.world.getBiomeGenForCoords(field.add(16, 0, 16));
                if (mode.contains("tree")) {
                    trees(this.world, rand, field, biome);
                }
                if (mode.contains("plant")) {
                    plants(this.world, rand, field, biome);
                }
            }
            if (mode.contains("spring")) {
                springs(this.world, rand, field);
            }
        }
    }

    // ---- BiomeDecorator.genDecorations' springs section ----

    static void springs(World world, Random rand, BlockPos field) {
        for (int i = 0; i < 50; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int y = rand.nextInt(248) + 8;
            if (y > 0) {
                int yy = rand.nextInt(y);
                new WorldGenLiquids(Blocks.flowing_water).generate(world, rand, field.add(x, yy, z));
            }
        }
        for (int i = 0; i < 20; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int y = rand.nextInt(rand.nextInt(rand.nextInt(240) + 8) + 8);
            new WorldGenLiquids(Blocks.flowing_lava).generate(world, rand, field.add(x, y, z));
        }
    }

    // ---- BiomeDecorator.genDecorations' tree loop ----

    static void trees(World world, Random rand, BlockPos field, BiomeGenBase biome) {
        int count = f(biome.theBiomeDecorator, "treesPerChunk");
        if (rand.nextInt(10) == 0) count++;
        for (int i = 0; i < count; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            net.minecraft.world.gen.feature.WorldGenAbstractTree tree = biome.genBigTreeChance(rand);
            tree.func_175904_e();
            BlockPos pos = world.getHeight(field.add(x, 0, z));
            if (tree.generate(world, rand, pos)) {
                tree.func_180711_a(world, rand, pos);
            }
        }
    }

    // ---- BiomeDecorator.genDecorations' vegetation section ----
    //
    // The counts and generator instances are read from the real `BiomeDecorator`, so the per-biome
    // table and the shared `WorldGenBigMushroom` colour state are exactly the game's.

    static int f(BiomeDecorator d, String name) {
        try {
            java.lang.reflect.Field field = BiomeDecorator.class.getDeclaredField(name);
            field.setAccessible(true);
            return field.getInt(d);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    static WorldGenerator g(BiomeDecorator d, String name) {
        try {
            java.lang.reflect.Field field = BiomeDecorator.class.getDeclaredField(name);
            field.setAccessible(true);
            return (WorldGenerator) field.get(d);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    static void plants(World world, Random rand, BlockPos field, BiomeGenBase biome) {
        BiomeDecorator d = biome.theBiomeDecorator;
        WorldGenerator bigGen = g(d, "bigMushroomGen");
        WorldGenerator brownGen = g(d, "mushroomBrownGen");
        WorldGenerator redGen = g(d, "mushroomRedGen");
        WorldGenerator reedGen = g(d, "reedGen");
        WorldGenerator cactusGen = g(d, "cactusGen");
        WorldGenerator waterlilyGen = g(d, "waterlilyGen");
        WorldGenFlowers yellowFlowerGen = (WorldGenFlowers) g(d, "yellowFlowerGen");

        for (int i = 0; i < f(d, "bigMushroomsPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            bigGen.generate(world, rand, world.getHeight(field.add(x, 0, z)));
        }
        for (int i = 0; i < f(d, "flowersPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() + 32;
            if (h > 0) {
                int y = rand.nextInt(h);
                BlockPos pos = field.add(x, y, z);
                BlockFlower.EnumFlowerType type = biome.pickRandomFlower(rand, pos);
                BlockFlower block = type.getBlockType().getBlock();
                if (block.getMaterial() != Material.air) {
                    yellowFlowerGen.setGeneratedBlock(block, type);
                    yellowFlowerGen.generate(world, rand, pos);
                }
            }
        }
        for (int i = 0; i < f(d, "grassPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                biome.getRandomWorldGenForGrass(rand).generate(world, rand, field.add(x, y, z));
            }
        }
        for (int i = 0; i < f(d, "deadBushPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                new WorldGenDeadBush().generate(world, rand, field.add(x, y, z));
            }
        }
        for (int i = 0; i < f(d, "waterlilyPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                BlockPos pos = field.add(x, y, z);
                while (pos.getY() > 0) {
                    BlockPos down = pos.down();
                    if (!world.isAirBlock(down)) break;
                    pos = down;
                }
                waterlilyGen.generate(world, rand, pos);
            }
        }
        for (int i = 0; i < f(d, "mushroomsPerChunk"); i++) {
            if (rand.nextInt(4) == 0) {
                int x = rand.nextInt(16) + 8;
                int z = rand.nextInt(16) + 8;
                brownGen.generate(world, rand, world.getHeight(field.add(x, 0, z)));
            }
            if (rand.nextInt(8) == 0) {
                int x = rand.nextInt(16) + 8;
                int z = rand.nextInt(16) + 8;
                int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
                if (h > 0) {
                    int y = rand.nextInt(h);
                    redGen.generate(world, rand, field.add(x, y, z));
                }
            }
        }
        if (rand.nextInt(4) == 0) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                brownGen.generate(world, rand, field.add(x, y, z));
            }
        }
        if (rand.nextInt(8) == 0) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                redGen.generate(world, rand, field.add(x, y, z));
            }
        }
        for (int i = 0; i < f(d, "reedsPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                reedGen.generate(world, rand, field.add(x, y, z));
            }
        }
        for (int i = 0; i < 10; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                reedGen.generate(world, rand, field.add(x, y, z));
            }
        }
        if (rand.nextInt(32) == 0) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                new WorldGenPumpkin().generate(world, rand, field.add(x, y, z));
            }
        }
        for (int i = 0; i < f(d, "cactiPerChunk"); i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            int h = world.getHeight(field.add(x, 0, z)).getY() * 2;
            if (h > 0) {
                int y = rand.nextInt(h);
                cactusGen.generate(world, rand, field.add(x, y, z));
            }
        }
    }

    // ---- BiomeDecorator.generateOres ----

    static void generateOres(World world, Random rand, BlockPos field) {
        genStandardOre1(world, rand, field, settings.dirtCount, new WorldGenMinable(Blocks.dirt.getDefaultState(), settings.dirtSize), settings.dirtMinHeight, settings.dirtMaxHeight);
        genStandardOre1(world, rand, field, settings.gravelCount, new WorldGenMinable(Blocks.gravel.getDefaultState(), settings.gravelSize), settings.gravelMinHeight, settings.gravelMaxHeight);
        genStandardOre1(world, rand, field, settings.dioriteCount, new WorldGenMinable(Blocks.stone.getDefaultState().withProperty(BlockStone.VARIANT, BlockStone.EnumType.DIORITE), settings.dioriteSize), settings.dioriteMinHeight, settings.dioriteMaxHeight);
        genStandardOre1(world, rand, field, settings.graniteCount, new WorldGenMinable(Blocks.stone.getDefaultState().withProperty(BlockStone.VARIANT, BlockStone.EnumType.GRANITE), settings.graniteSize), settings.graniteMinHeight, settings.graniteMaxHeight);
        genStandardOre1(world, rand, field, settings.andesiteCount, new WorldGenMinable(Blocks.stone.getDefaultState().withProperty(BlockStone.VARIANT, BlockStone.EnumType.ANDESITE), settings.andesiteSize), settings.andesiteMinHeight, settings.andesiteMaxHeight);
        genStandardOre1(world, rand, field, settings.coalCount, new WorldGenMinable(Blocks.coal_ore.getDefaultState(), settings.coalSize), settings.coalMinHeight, settings.coalMaxHeight);
        genStandardOre1(world, rand, field, settings.ironCount, new WorldGenMinable(Blocks.iron_ore.getDefaultState(), settings.ironSize), settings.ironMinHeight, settings.ironMaxHeight);
        genStandardOre1(world, rand, field, settings.goldCount, new WorldGenMinable(Blocks.gold_ore.getDefaultState(), settings.goldSize), settings.goldMinHeight, settings.goldMaxHeight);
        genStandardOre1(world, rand, field, settings.redstoneCount, new WorldGenMinable(Blocks.redstone_ore.getDefaultState(), settings.redstoneSize), settings.redstoneMinHeight, settings.redstoneMaxHeight);
        genStandardOre1(world, rand, field, settings.diamondCount, new WorldGenMinable(Blocks.diamond_ore.getDefaultState(), settings.diamondSize), settings.diamondMinHeight, settings.diamondMaxHeight);
        genStandardOre2(world, rand, field, settings.lapisCount, new WorldGenMinable(Blocks.lapis_ore.getDefaultState(), settings.lapisSize), settings.lapisCenterHeight, settings.lapisSpread);
    }

    static void genStandardOre1(World world, Random rand, BlockPos field, int count, WorldGenMinable gen, int minH, int maxH) {
        if (maxH < minH) { int t = minH; minH = maxH; maxH = t; }
        else if (maxH == minH) { if (minH < 255) maxH++; else minH--; }
        for (int i = 0; i < count; i++) {
            BlockPos pos = field.add(rand.nextInt(16), rand.nextInt(maxH - minH) + minH, rand.nextInt(16));
            gen.generate(world, rand, pos);
        }
    }

    static void genStandardOre2(World world, Random rand, BlockPos field, int count, WorldGenMinable gen, int center, int spread) {
        for (int i = 0; i < count; i++) {
            BlockPos pos = field.add(rand.nextInt(16), rand.nextInt(spread) + rand.nextInt(spread) + center - spread, rand.nextInt(16));
            gen.generate(world, rand, pos);
        }
    }

    // ---- BiomeDecorator.genDecorations' sand/clay/gravel loops ----

    static WorldGenSand sandGen;
    static WorldGenSand gravelAsSandGen;
    static WorldGenClay clayGen;

    static void surface(World world, Random rand, BlockPos field) {
        for (int i = 0; i < 3; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            sandGen.generate(world, rand, world.getTopSolidOrLiquidBlock(field.add(x, 0, z)));
        }
        for (int i = 0; i < 1; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            clayGen.generate(world, rand, world.getTopSolidOrLiquidBlock(field.add(x, 0, z)));
        }
        for (int i = 0; i < 1; i++) {
            int x = rand.nextInt(16) + 8;
            int z = rand.nextInt(16) + 8;
            gravelAsSandGen.generate(world, rand, world.getTopSolidOrLiquidBlock(field.add(x, 0, z)));
        }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        sandGen = new WorldGenSand(Blocks.sand, 7);
        gravelAsSandGen = new WorldGenSand(Blocks.gravel, 6);
        clayGen = new WorldGenClay(4);
        long seed = Long.parseLong(args[0]);
        String mode = args[1];
        settings = new ChunkProviderSettings.Factory().func_177864_b();

        StringBuilder sb = new StringBuilder();
        for (int i = 2; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);

            WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT)
                    .setWorldName("{}");
            net.minecraft.world.storage.WorldInfo wi = new net.minecraft.world.storage.WorldInfo(ws, "decoration");
            WorldProvider provider = new WorldProviderSurface();
            TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
            provider.registerWorld(world);
            DecorProvider gen = new DecorProvider(world, seed, "{}", mode);
            StubProvider stub = new StubProvider(gen);
            world.setProvider(stub);

            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    stub.provideChunk(cx + dx, cz + dz);
                }
            }
            for (int a = cx - 1; a <= cx; a++) {
                for (int b = cz - 1; b <= cz; b++) {
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
