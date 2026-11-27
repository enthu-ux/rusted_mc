//! Prints canonical `mc-core::jmath` (MathHelper) results for fixed inputs.
//!
//! The Java oracle (`oracle/java/src/MathOracle.java`) prints the identical format;
//! `tools/test_math_parity.py` compares them line-by-line. Floats/doubles as raw bits.

use mc_core::jmath;

fn hf(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn hd(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn main() {
    let mut out = String::new();

    for i in 0..200 {
        let v = (i as f32 - 100.0) * 0.37;
        out.push_str(&format!("sin {} {}\n", hf(v), hf(jmath::sin(v))));
    }
    for i in 0..200 {
        let v = (i as f32 - 100.0) * 0.37;
        out.push_str(&format!("cos {} {}\n", hf(v), hf(jmath::cos(v))));
    }
    for v in [0.0f32, 1.0, -1.0, 1000.0, -1000.0, 123456.78, -0.0, 3.14159] {
        out.push_str(&format!("sinX {} {}\n", hf(v), hf(jmath::sin(v))));
        out.push_str(&format!("cosX {} {}\n", hf(v), hf(jmath::cos(v))));
    }

    let dvals = [
        0.0f64,
        0.5,
        -0.5,
        1.9,
        -1.9,
        2.0,
        -2.0,
        100.999,
        -100.001,
        1.0e9 + 0.5,
    ];
    for &v in &dvals {
        out.push_str(&format!("fd {} {}\n", hd(v), jmath::floor_double(v)));
    }
    for &v in &dvals {
        out.push_str(&format!("fdl {} {}\n", hd(v), jmath::floor_double_long(v)));
    }
    let fvals = [0.0f32, 0.5, -0.5, 1.9, -1.9, 2.0, -2.0, 100.999, -100.001];
    for &v in &fvals {
        out.push_str(&format!("ff {} {}\n", hf(v), jmath::floor_float(v)));
    }

    for v in [0.0f32, 1.0, 2.0, 4.0, 9.0, 0.25, 123.456, 1000000.0] {
        out.push_str(&format!("sq {} {}\n", hf(v), hf(jmath::sqrt_float(v))));
    }

    for &v in &fvals {
        out.push_str(&format!("cf {} {}\n", hf(v), jmath::ceiling_float_int(v)));
    }
    for &v in &dvals {
        out.push_str(&format!("cd {} {}\n", hd(v), jmath::ceiling_double_int(v)));
    }

    for v in [0.0f32, -0.0, 1.5, -1.5] {
        out.push_str(&format!("abs {} {}\n", hf(v), hf(jmath::abs(v))));
    }
    for v in [0i32, -1, 5, -5, i32::MIN + 1] {
        out.push_str(&format!("absi {} {}\n", v, jmath::abs_int(v)));
    }

    out.push_str(&format!("ci {}\n", jmath::clamp_int(5, 1, 3)));
    out.push_str(&format!("ci2 {}\n", jmath::clamp_int(-5, 1, 3)));
    out.push_str(&format!("ci3 {}\n", jmath::clamp_int(2, 1, 3)));
    out.push_str(&format!("cfl {}\n", hf(jmath::clamp_float(5.0, 1.0, 3.0))));
    out.push_str(&format!(
        "cfl2 {}\n",
        hf(jmath::clamp_float(-5.0, 1.0, 3.0))
    ));
    out.push_str(&format!("cdl {}\n", hd(jmath::clamp_double(5.0, 1.0, 3.0))));

    out.push_str(&format!(
        "dc {}\n",
        hd(jmath::denormalize_clamp(2.0, 5.0, -1.0))
    ));
    out.push_str(&format!(
        "dc2 {}\n",
        hd(jmath::denormalize_clamp(2.0, 5.0, 2.0))
    ));
    out.push_str(&format!(
        "dc3 {}\n",
        hd(jmath::denormalize_clamp(2.0, 5.0, 0.25))
    ));

    out.push_str(&format!("am {}\n", hd(jmath::abs_max(-3.0, -7.0))));
    out.push_str(&format!("am2 {}\n", hd(jmath::abs_max(4.0, -2.0))));

    out.push_str(&format!("table {:016x}\n", jmath::table_hash()));

    print!("{out}");
}
