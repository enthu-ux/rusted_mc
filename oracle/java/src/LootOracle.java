// Loot + enchantment RNG oracle (1.8.9): runs the real game code for the subsystem
// `StructureMineshaftPieces` needs before it can write blocks, and prints a canonical, comparable
// dump. The interesting thing is not the items but the *draw sequence*: a chest roll consumes
// enchantment RNG (`EnchantmentHelper.buildEnchantmentList` -> `mapEnchantmentData`, a HashMap whose
// iteration order feeds `WeightedRandom`), which shifts the rest of the chunk's block placement.
//
// Usage:
//   LootOracle table
//   LootOracle enchant <seed> <level> [<seed> <level> ...]
//   LootOracle chest   <seed> <count> <slots> [<seed> <count> <slots> ...]
//
// `table` dumps `Enchantment.enchantmentsBookList` (id/weight/max level/type), each level's
// min/max enchantability, and every ordered pair's `canApplyTogether` (asymmetric).
//
// `enchant` runs `EnchantmentHelper.addRandomEnchantment(rand, new ItemStack(Items.book), level)`
// and prints the resulting stored enchantments (id:level, in NBT order) plus the next 32
// `nextInt(1000)` values.
//
// `chest` builds the mineshaft list exactly as `addComponentParts` does (`CHEST_CONTENT_WEIGHT_LIST`
// + `Items.enchanted_book.getRandom(rand)` via `func_177629_a`) and runs the real
// `WeightedRandomChestContent.generateChestContents`; prints the list, every slot write
// (slot/itemId/meta/size) and the next 32 `nextInt(1000)` values.
//
// The private `StructureMineshaftPieces.CHEST_CONTENT_WEIGHT_LIST` and the
// `WeightedRandomChestContent` fields are read by reflection so the oracle uses the real data.

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

import net.minecraft.enchantment.Enchantment;
import net.minecraft.enchantment.EnchantmentHelper;
import net.minecraft.init.Bootstrap;
import net.minecraft.init.Items;
import net.minecraft.inventory.InventoryBasic;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.nbt.NBTTagList;
import net.minecraft.util.WeightedRandom;
import net.minecraft.util.WeightedRandomChestContent;

public class LootOracle {
    private static final int STREAM = 32;

    private static Field fWeight;
    private static Field fItem;
    private static Field fMin;
    private static Field fMax;

    static {
        try {
            fWeight = WeightedRandom.Item.class.getDeclaredField("itemWeight");
            fWeight.setAccessible(true);
            fItem = WeightedRandomChestContent.class.getDeclaredField("theItemId");
            fItem.setAccessible(true);
            fMin = WeightedRandomChestContent.class.getDeclaredField("minStackSize");
            fMin.setAccessible(true);
            fMax = WeightedRandomChestContent.class.getDeclaredField("maxStackSize");
            fMax.setAccessible(true);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    /** Records the slot writes `generateChestContents` makes. */
    static class RecInventory extends InventoryBasic {
        final List<String> writes = new ArrayList<>();

        RecInventory(int slots) {
            super("chest", false, slots);
        }

        @Override
        public void setInventorySlotContents(int slot, ItemStack stack) {
            writes.add(slot + " " + Item.getIdFromItem(stack.getItem()) + " "
                    + stack.getMetadata() + " " + stack.stackSize);
            super.setInventorySlotContents(slot, stack);
        }
    }

    @SuppressWarnings("unchecked")
    private static List<WeightedRandomChestContent> mineshaftList() throws ReflectiveOperationException {
        Class<?> pieces = Class.forName("net.minecraft.world.gen.structure.StructureMineshaftPieces");
        Field field = pieces.getDeclaredField("CHEST_CONTENT_WEIGHT_LIST");
        field.setAccessible(true);
        return (List<WeightedRandomChestContent>) field.get(null);
    }

    private static String stream(Random rand) {
        StringBuilder sb = new StringBuilder("stream");
        for (int i = 0; i < STREAM; i++) {
            sb.append(' ').append(rand.nextInt(1000));
        }
        return sb.toString();
    }

    private static String table() {
        StringBuilder sb = new StringBuilder();
        for (Enchantment e : Enchantment.enchantmentsBookList) {
            sb.append("ench ").append(e.effectId).append(' ').append(e.getWeight())
                    .append(' ').append(e.getMaxLevel()).append(' ').append(e.type.name()).append('\n');
            for (int l = 1; l <= e.getMaxLevel(); l++) {
                sb.append("lvl ").append(e.effectId).append(' ').append(l)
                        .append(' ').append(e.getMinEnchantability(l))
                        .append(' ').append(e.getMaxEnchantability(l)).append('\n');
            }
        }
        for (Enchantment a : Enchantment.enchantmentsBookList) {
            for (Enchantment b : Enchantment.enchantmentsBookList) {
                if (a.effectId < b.effectId) {
                    sb.append("excl ").append(a.effectId).append(' ').append(b.effectId)
                            .append(' ').append(a.canApplyTogether(b) ? 1 : 0)
                            .append(' ').append(b.canApplyTogether(a) ? 1 : 0).append('\n');
                }
            }
        }
        return sb.toString();
    }

    private static String enchant(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i + 1 < args.length; i += 2) {
            long seed = Long.parseLong(args[i]);
            int level = Integer.parseInt(args[i + 1]);
            Random rand = new Random(seed);
            ItemStack stack = new ItemStack(Items.book, 1, 0);
            EnchantmentHelper.addRandomEnchantment(rand, stack, level);
            NBTTagList ench = Items.enchanted_book.getEnchantments(stack);

            sb.append("case ").append(seed).append(' ').append(level).append('\n');
            sb.append("enchants");
            for (int j = 0; j < ench.tagCount(); j++) {
                NBTTagCompound c = ench.getCompoundTagAt(j);
                sb.append(' ').append(c.getShort("id")).append(':').append(c.getShort("lvl"));
            }
            sb.append('\n').append(stream(rand)).append('\n');
        }
        return sb.toString();
    }

    private static String chest(String[] args) throws ReflectiveOperationException {
        List<WeightedRandomChestContent> base = mineshaftList();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i + 2 < args.length; i += 3) {
            long seed = Long.parseLong(args[i]);
            int count = Integer.parseInt(args[i + 1]);
            int slots = Integer.parseInt(args[i + 2]);

            Random rand = new Random(seed);
            List<WeightedRandomChestContent> list = new ArrayList<>(base);
            list.add(Items.enchanted_book.getRandom(rand));

            sb.append("case ").append(seed).append(' ').append(count).append(' ').append(slots).append('\n');
            for (int j = 0; j < list.size(); j++) {
                WeightedRandomChestContent c = list.get(j);
                ItemStack stack = (ItemStack) fItem.get(c);
                sb.append("content ").append(j)
                        .append(' ').append(Item.getIdFromItem(stack.getItem()))
                        .append(' ').append(stack.getMetadata())
                        .append(' ').append(fMin.getInt(c))
                        .append(' ').append(fMax.getInt(c))
                        .append(' ').append(fWeight.getInt(c)).append('\n');
            }

            RecInventory inv = new RecInventory(slots);
            WeightedRandomChestContent.generateChestContents(rand, list, inv, count);
            for (String w : inv.writes) {
                sb.append("write ").append(w).append('\n');
            }
            sb.append(stream(rand)).append('\n');
        }
        return sb.toString();
    }

    public static void main(String[] args) throws ReflectiveOperationException {
        Bootstrap.register();
        String mode = args[0];
        String[] rest = new String[args.length - 1];
        System.arraycopy(args, 1, rest, 0, rest.length);

        String out;
        if (mode.equals("table")) {
            out = table();
        } else if (mode.equals("enchant")) {
            out = enchant(rest);
        } else if (mode.equals("chest")) {
            out = chest(rest);
        } else {
            throw new IllegalArgumentException("unknown mode: " + mode);
        }
        System.out.print(out);
    }
}
