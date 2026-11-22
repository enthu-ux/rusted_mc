// Dumps each biome's `BiomeDecorator` counts (and its class), read straight from the live game, so
// the Rust decoration table can be transcribed without guessing.
//
// Usage: BiomeDecorTableOracle

import java.lang.reflect.Field;

import net.minecraft.init.Bootstrap;
import net.minecraft.world.biome.BiomeDecorator;
import net.minecraft.world.biome.BiomeGenBase;

public class BiomeDecorTableOracle {
    static int f(BiomeDecorator d, String name) throws ReflectiveOperationException {
        Field field = BiomeDecorator.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.getInt(d);
    }

    public static void main(String[] args) throws Exception {
        Bootstrap.register();
        String[] fields = {
            "treesPerChunk", "flowersPerChunk", "grassPerChunk", "deadBushPerChunk", "mushroomsPerChunk",
            "reedsPerChunk", "cactiPerChunk", "waterlilyPerChunk", "bigMushroomsPerChunk",
            "sandPerChunk", "sandPerChunk2", "clayPerChunk"
        };
        StringBuilder sb = new StringBuilder();
        BiomeGenBase[] arr = BiomeGenBase.getBiomeGenArray();
        for (int id = 0; id < arr.length; id++) {
            BiomeGenBase b = arr[id];
            if (b == null) continue;
            BiomeDecorator d = b.theBiomeDecorator;
            sb.append(id).append(' ').append(b.getBiomeClass().getSimpleName()).append(' ');
            for (String name : fields) {
                sb.append(f(d, name)).append(' ');
            }
            sb.append(b.getEnableSnow() ? 1 : 0).append('\n');
        }
        System.out.print(sb);
    }
}
