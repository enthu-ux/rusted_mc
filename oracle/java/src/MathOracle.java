// Vanilla 1.8.9 `net.minecraft.util.MathHelper` results, printed canonically (floats/doubles as
// raw bits). Runs against the remapped server jar; the Rust probe
// (crates/mc-core/examples/math_probe.rs) prints the same format.

import net.minecraft.util.MathHelper;

public class MathOracle {
    static String hf(float v) {
        return String.format("%08x", Float.floatToRawIntBits(v));
    }

    static String hd(double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();

        for (int i = 0; i < 200; i++) {
            float v = (i - 100) * 0.37F;
            sb.append("sin ").append(hf(v)).append(' ').append(hf(MathHelper.sin(v))).append('\n');
        }
        for (int i = 0; i < 200; i++) {
            float v = (i - 100) * 0.37F;
            sb.append("cos ").append(hf(v)).append(' ').append(hf(MathHelper.cos(v))).append('\n');
        }
        float[] ex = {0F, 1F, -1F, 1000F, -1000F, 123456.78F, -0.0F, 3.14159F};
        for (float v : ex) {
            sb.append("sinX ").append(hf(v)).append(' ').append(hf(MathHelper.sin(v))).append('\n');
            sb.append("cosX ").append(hf(v)).append(' ').append(hf(MathHelper.cos(v))).append('\n');
        }

        double[] dvals = {0.0, 0.5, -0.5, 1.9, -1.9, 2.0, -2.0, 100.999, -100.001, 1.0E9 + 0.5};
        for (double v : dvals) {
            sb.append("fd ").append(hd(v)).append(' ').append(MathHelper.floor_double(v)).append('\n');
        }
        for (double v : dvals) {
            sb.append("fdl ").append(hd(v)).append(' ').append(MathHelper.floor_double_long(v)).append('\n');
        }
        float[] fvals = {0F, 0.5F, -0.5F, 1.9F, -1.9F, 2F, -2F, 100.999F, -100.001F};
        for (float v : fvals) {
            sb.append("ff ").append(hf(v)).append(' ').append(MathHelper.floor_float(v)).append('\n');
        }

        float[] sq = {0F, 1F, 2F, 4F, 9F, 0.25F, 123.456F, 1000000F};
        for (float v : sq) {
            sb.append("sq ").append(hf(v)).append(' ').append(hf(MathHelper.sqrt_float(v))).append('\n');
        }

        for (float v : fvals) {
            sb.append("cf ").append(hf(v)).append(' ').append(MathHelper.ceiling_float_int(v)).append('\n');
        }
        for (double v : dvals) {
            sb.append("cd ").append(hd(v)).append(' ').append(MathHelper.ceiling_double_int(v)).append('\n');
        }

        float[] absv = {0F, -0.0F, 1.5F, -1.5F};
        for (float v : absv) {
            sb.append("abs ").append(hf(v)).append(' ').append(hf(MathHelper.abs(v))).append('\n');
        }
        int[] absi = {0, -1, 5, -5, Integer.MIN_VALUE + 1};
        for (int v : absi) {
            sb.append("absi ").append(v).append(' ').append(MathHelper.abs_int(v)).append('\n');
        }

        sb.append("ci ").append(MathHelper.clamp_int(5, 1, 3)).append('\n');
        sb.append("ci2 ").append(MathHelper.clamp_int(-5, 1, 3)).append('\n');
        sb.append("ci3 ").append(MathHelper.clamp_int(2, 1, 3)).append('\n');
        sb.append("cfl ").append(hf(MathHelper.clamp_float(5F, 1F, 3F))).append('\n');
        sb.append("cfl2 ").append(hf(MathHelper.clamp_float(-5F, 1F, 3F))).append('\n');
        sb.append("cdl ").append(hd(MathHelper.clamp_double(5.0, 1.0, 3.0))).append('\n');

        sb.append("dc ").append(hd(MathHelper.denormalizeClamp(2.0, 5.0, -1.0))).append('\n');
        sb.append("dc2 ").append(hd(MathHelper.denormalizeClamp(2.0, 5.0, 2.0))).append('\n');
        sb.append("dc3 ").append(hd(MathHelper.denormalizeClamp(2.0, 5.0, 0.25))).append('\n');

        sb.append("am ").append(hd(MathHelper.abs_max(-3.0, -7.0))).append('\n');
        sb.append("am2 ").append(hd(MathHelper.abs_max(4.0, -2.0))).append('\n');

        System.out.print(sb);
    }
}
