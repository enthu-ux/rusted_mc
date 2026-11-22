// Dumps `Material` flags per registered block id, used by the decoration world shim.
//
// `World.getTopSolidOrLiquidBlock` / `Chunk.getPrecipitationHeight` scan columns by
// `material.blocksMovement() || material.isLiquid()`; `BlockCactus.canBlockStay` checks a horizontal
// neighbour's `material.isSolid()`; `BlockLilyPad.canBlockStay` needs the water level; the plant
// placement rules use the soil block id. Printing the flags avoids guessing `Material` defaults.
//
// Usage: MaterialTableOracle            (all ids 0..255)

import net.minecraft.block.Block;
import net.minecraft.block.material.Material;
import net.minecraft.init.Blocks;
import net.minecraft.init.Bootstrap;

public class MaterialTableOracle {
    public static void main(String[] args) {
        Bootstrap.register();
        StringBuilder sb = new StringBuilder();
        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            // The registry is defaulted to air, so unregistered ids resolve to Blocks.air; skip them.
            if (b == null || (b == Blocks.air && id != 0)) {
                continue;
            }
            Material m = b.getMaterial();
            sb.append(id).append(' ')
                    .append(m.isSolid() ? "solid" : "notsolid")
                    .append(' ')
                    .append(m.blocksMovement() ? "move" : "pass")
                    .append(' ')
                    .append(m.isLiquid() ? "liquid" : "dry")
                    .append(' ')
                    .append(m.isReplaceable() ? "replace" : "keep")
                    .append(' ')
                    .append(m.getMaterialMapColor().colorValue)
                    .append(' ')
                    .append(b.getUnlocalizedName())
                    .append('\n');
        }
        System.out.print(sb);
    }
}
