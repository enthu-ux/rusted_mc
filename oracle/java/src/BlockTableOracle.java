// Dumps `Block.getLightOpacity()`, whether the material is air, whether it is liquid, and
// `Block.isFullBlock()`, per registered block id. The chunk height map scans columns with
// `Chunk.getBlockLightOpacity` (opacity != 0 = "solid"), and structure placement needs the liquid
// material (`isLiquidInStructureBoundingBox`) and `isFullBlock` (the mineshaft rail loop).
// Printed once and transcribed into `mc-worldgen`.
//
// Usage: BlockTableOracle            (all ids 0..255)

import net.minecraft.block.Block;
import net.minecraft.block.material.Material;
import net.minecraft.init.Blocks;
import net.minecraft.init.Bootstrap;

public class BlockTableOracle {
    public static void main(String[] args) {
        Bootstrap.register();
        StringBuilder sb = new StringBuilder();
        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            // The registry is defaulted to air, so unregistered ids resolve to Blocks.air; skip them.
            if (b == null || (b == Blocks.air && id != 0)) {
                continue;
            }
            sb.append(id).append(' ')
                    .append(b.getLightOpacity()).append(' ')
                    .append(b.getMaterial() == Material.air ? "air" : "solid")
                    .append(' ')
                    .append(b.getMaterial().isLiquid() ? "liquid" : "dry")
                    .append(' ')
                    .append(b.isFullBlock() ? "full" : "notfull")
                    .append(' ')
                    .append(b.getUnlocalizedName())
                    .append('\n');
        }
        System.out.print(sb);
    }
}
