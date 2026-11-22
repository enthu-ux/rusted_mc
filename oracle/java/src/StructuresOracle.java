// Vanilla 1.8.9 structure *registration* (the `provideChunk` half): every structure start a chunk's
// generation registers, dumped through the game's own serializer.
//
// Usage: StructuresOracle <seed> <cx> <cz> [<cx> <cz> ...]
// Per target chunk:
//   chunk <cx> <cz>
//   start <id> ChunkX <x> ChunkZ <z> BB <6 ints> pieces <n>
//   piece <id> BB <6 ints> GD <gd> O <o> [<key>=<value> ...]   (extra keys ASCII-sorted)
//
// The starts dumped for a chunk are those whose bounding box meets its 16x16 area - exactly the set
// `MapGenStructure.generateStructure` would place while populating it - sorted by start chunk, since
// vanilla holds them in a HashMap. Block placement itself happens in `populate` (not ported yet).
//
// Registration uses `MapGenBase`'s per-neighbour RNG streams, so this only needs the headless world
// for `StructureMineshaftStart`'s `world.getSeaLevel()`.

import java.io.File;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.TreeSet;

import net.minecraft.init.Bootstrap;
import net.minecraft.nbt.NBTBase;
import net.minecraft.nbt.NBTTagByte;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.nbt.NBTTagInt;
import net.minecraft.nbt.NBTTagList;
import net.minecraft.profiler.Profiler;
import net.minecraft.world.ChunkCoordIntPair;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.WorldSettings;
import net.minecraft.world.WorldType;
import net.minecraft.world.chunk.ChunkPrimer;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.chunk.storage.IChunkLoader;
import net.minecraft.world.gen.structure.MapGenMineshaft;
import net.minecraft.world.gen.structure.MapGenNetherBridge;
import net.minecraft.world.gen.structure.MapGenScatteredFeature;
import net.minecraft.world.gen.structure.MapGenStronghold;
import net.minecraft.world.gen.structure.MapGenVillage;
import net.minecraft.world.gen.structure.StructureOceanMonument;
import net.minecraft.world.gen.structure.StructureStart;
import net.minecraft.world.storage.IPlayerFileData;
import net.minecraft.world.storage.ISaveHandler;
import net.minecraft.world.storage.MapStorage;
import net.minecraft.world.storage.WorldInfo;

public class StructuresOracle {
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
        public String getWorldDirectoryName() { return "structures"; }
    }

    static class TestWorld extends World {
        TestWorld(ISaveHandler h, WorldInfo i, WorldProvider p, Profiler pr, boolean remote) {
            super(h, i, p, pr, remote);
            // `World` leaves `mapStorage` null; `WorldServer` normally creates it. Structure
            // registration goes through `loadItemData`/`setItemData`, so the headless world needs one.
            this.mapStorage = new MapStorage(h);
        }
        protected IChunkProvider createChunkProvider() { return null; }
        protected int getRenderDistanceChunks() { return 0; }
        @Override
        protected boolean isChunkLoaded(int x, int z, boolean allowEmpty) { return false; }
    }

    /** `MapGenMineshaft` with its `structureMap` exposed (`protected` in `MapGenStructure`). */
    static class Probe extends MapGenMineshaft {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    /** `MapGenScatteredFeature`'s `structureMap` exposed. */
    static class ScatteredProbe extends MapGenScatteredFeature {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    /** `StructureOceanMonument`'s `structureMap` exposed. */
    static class MonumentProbe extends StructureOceanMonument {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    /** `MapGenNetherBridge`'s `structureMap` exposed. */
    static class NetherProbe extends MapGenNetherBridge {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    /** `MapGenVillage`'s `structureMap` exposed. */
    static class VillageProbe extends MapGenVillage {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    /** `MapGenStronghold`'s `structureMap` exposed. */
    static class StrongholdProbe extends MapGenStronghold {
        java.util.Map<Long, StructureStart> map() { return this.structureMap; }
    }

    private static String vec(NBTTagCompound c, String key) {
        int[] v = c.getIntArray(key);
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < v.length; i++) {
            if (i > 0) {
                b.append(' ');
            }
            b.append(v[i]);
        }
        return b.toString();
    }

    private static String value(NBTBase v) {
        if (v instanceof NBTTagByte) {
            return String.valueOf(((NBTTagByte) v).getByte() & 255);
        }
        if (v instanceof NBTTagInt) {
            return String.valueOf(((NBTTagInt) v).getInt());
        }
        if (v instanceof NBTTagList) {
            // `Room` writes its linked boxes as "Entrances": a list of int arrays. Render each box as
            // six space-separated ints, boxes separated by ';' (empty list -> empty string).
            NBTTagList list = (NBTTagList) v;
            StringBuilder b = new StringBuilder();
            for (int i = 0; i < list.tagCount(); i++) {
                if (i > 0) {
                    b.append(';');
                }
                int[] ints = list.getIntArrayAt(i);
                for (int j = 0; j < ints.length; j++) {
                    if (j > 0) {
                        b.append(' ');
                    }
                    b.append(ints[j]);
                }
            }
            return b.toString();
        }
        return v.toString();
    }

    private static void dump(StringBuilder sb, StructureStart start) {
        NBTTagCompound t = start.writeStructureComponentsToNBT(start.getChunkPosX(), start.getChunkPosZ());
        NBTTagList children = t.getTagList("Children", 10);
        sb.append("start ").append(t.getString("id"))
                .append(" ChunkX ").append(t.getInteger("ChunkX"))
                .append(" ChunkZ ").append(t.getInteger("ChunkZ"))
                .append(" BB ").append(vec(t, "BB"))
                .append(" pieces ").append(children.tagCount())
                .append('\n');

        for (int i = 0; i < children.tagCount(); i++) {
            NBTTagCompound c = children.getCompoundTagAt(i);
            sb.append("piece ").append(c.getString("id"))
                    .append(" BB ").append(vec(c, "BB"))
                    .append(" GD ").append(c.getInteger("GD"))
                    .append(" O ").append(c.getInteger("O"));
            for (String key : new TreeSet<>(c.getKeySet())) {
                if (key.equals("id") || key.equals("BB") || key.equals("GD") || key.equals("O")) {
                    continue;
                }
                sb.append(' ').append(key).append('=').append(value(c.getTag(key)));
            }
            sb.append('\n');
        }
    }

    public static void main(String[] args) {
        Bootstrap.register();
        long seed = Long.parseLong(args[0]);

        WorldSettings ws = new WorldSettings(seed, WorldSettings.GameType.CREATIVE, false, false, WorldType.DEFAULT);
        WorldInfo wi = new WorldInfo(ws, "structures");
        WorldProvider provider = new WorldProviderSurface();
        TestWorld world = new TestWorld(new SaveHandler(), wi, provider, new Profiler(), false);
        provider.registerWorld(world);

        Probe mineshafts = new Probe();
        ScatteredProbe scattered = new ScatteredProbe();
        MonumentProbe monuments = new MonumentProbe();
        VillageProbe villages = new VillageProbe();
        StrongholdProbe strongholds = new StrongholdProbe();
        NetherProbe nether = new NetherProbe();
        StringBuilder sb = new StringBuilder();
        for (int i = 1; i + 1 < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);
            mineshafts.generate(null, world, cx, cz, new ChunkPrimer());
            scattered.generate(null, world, cx, cz, new ChunkPrimer());
            monuments.generate(null, world, cx, cz, new ChunkPrimer());
            villages.generate(null, world, cx, cz, new ChunkPrimer());
            strongholds.generate(null, world, cx, cz, new ChunkPrimer());
            nether.generate(null, world, cx, cz, new ChunkPrimer());

            sb.append("chunk ").append(cx).append(' ').append(cz).append('\n');
            dumpMeeting(sb, mineshafts.map(), cx, cz);
            dumpMeeting(sb, scattered.map(), cx, cz);
            dumpMeeting(sb, monuments.map(), cx, cz);
            dumpMeeting(sb, villages.map(), cx, cz);
            dumpMeeting(sb, strongholds.map(), cx, cz);
            dumpMeeting(sb, nether.map(), cx, cz);
        }

        System.out.print(sb);
    }

    /** Dumps the starts in `map` (one structure generator) whose box meets chunk `(cx, cz)`, sorted
     *  by start chunk. The port mirrors this per-generator grouping (mineshafts, then temples). */
    private static void dumpMeeting(
            StringBuilder sb, java.util.Map<Long, StructureStart> map, int cx, int cz) {
        List<StructureStart> starts = new ArrayList<>();
        for (StructureStart s : map.values()) {
            if (s.isSizeableStructure() && s.func_175788_a(new net.minecraft.world.ChunkCoordIntPair(cx, cz))) {
                int x0 = (cx << 4) + 8;
                int z0 = (cz << 4) + 8;
                if (s.getBoundingBox().intersectsWith(x0, z0, x0 + 15, z0 + 15)) {
                    starts.add(s);
                }
            }
        }
        starts.sort(Comparator.comparingInt(StructureStart::getChunkPosX)
                .thenComparingInt(StructureStart::getChunkPosZ));
        for (StructureStart s : starts) {
            dump(sb, s);
        }
    }
}
