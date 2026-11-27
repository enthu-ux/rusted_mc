//! `net.minecraft.util.MathHelper` — Java-compatible math primitives.
//!
//! Ported from the 1.8.9 decompile. The `sin`/`cos` table is built exactly as vanilla's static
//! initializer (`SIN_TABLE[i] = (float)Math.sin(i * PI * 2.0 / 65536.0)`), and the lookup keeps
//! vanilla's `(int)(v * 10430.378F) & 65535` index arithmetic.
//!
//! `atan2` (the fdlibm table version) is not needed by worldgen yet and is deferred until
//! entity physics.

use std::f64::consts::PI;
use std::sync::OnceLock;

/// `MathHelper.sin`/`cos` lookup table.
static SIN_TABLE: OnceLock<[f32; 65536]> = OnceLock::new();

fn sin_table() -> &'static [f32; 65536] {
    SIN_TABLE.get_or_init(|| {
        let mut t = [0.0f32; 65536];
        for (i, v) in t.iter_mut().enumerate() {
            *v = ((i as f64) * PI * 2.0 / 65536.0).sin() as f32;
        }
        // The one entry where the platform libm differs from Java's fdlibm: at exactly `PI`,
        // `(float) Math.sin(Math.PI)` is 1.2246469e-16 (bits 0x250d3132), while this target's
        // `f64::sin` mis-reduces and returns 1.2246064e-16 (bits 0x250d3000). A lookup near `PI`
        // (e.g. the `WorldGenMinable` ellipsoid at `t -> 1`) then flips by one block. Pin it.
        t[32768] = f32::from_bits(0x250d_3132);
        t
    })
}

/// FNV-1a over the `SIN_TABLE` bits, for the `math_parity` full-table check (the Java oracle reads
/// the game's real `MathHelper.SIN_TABLE` by reflection).
pub fn table_hash() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in sin_table() {
        let bits = v.to_bits();
        for b in 0..4 {
            h ^= ((bits >> (8 * b)) & 0xff) as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

/// `MathHelper.sin`.
pub fn sin(v: f32) -> f32 {
    sin_table()[(((v * 10430.378f32) as i32) & 65535) as usize]
}

/// `MathHelper.cos`.
pub fn cos(v: f32) -> f32 {
    sin_table()[(((v * 10430.378f32 + 16384.0f32) as i32) & 65535) as usize]
}

/// `MathHelper.sqrt_float`.
pub fn sqrt_float(v: f32) -> f32 {
    (v as f64).sqrt() as f32
}

/// `MathHelper.sqrt_double`.
pub fn sqrt_double(v: f64) -> f32 {
    v.sqrt() as f32
}

/// `MathHelper.floor_float`.
pub fn floor_float(v: f32) -> i32 {
    let i = v as i32;
    if v < i as f32 {
        i - 1
    } else {
        i
    }
}

/// `MathHelper.floor_double`.
pub fn floor_double(v: f64) -> i32 {
    let i = v as i32;
    if v < i as f64 {
        i - 1
    } else {
        i
    }
}

/// `MathHelper.floor_double_long`.
pub fn floor_double_long(v: f64) -> i64 {
    let i = v as i64;
    if v < i as f64 {
        i - 1
    } else {
        i
    }
}

/// `MathHelper.abs`.
pub fn abs(v: f32) -> f32 {
    if v >= 0.0 {
        v
    } else {
        -v
    }
}

/// `MathHelper.abs_int`.
pub fn abs_int(v: i32) -> i32 {
    if v >= 0 {
        v
    } else {
        -v
    }
}

/// `MathHelper.ceiling_float_int`.
pub fn ceiling_float_int(v: f32) -> i32 {
    let i = v as i32;
    if v > i as f32 {
        i + 1
    } else {
        i
    }
}

/// `MathHelper.ceiling_double_int`.
pub fn ceiling_double_int(v: f64) -> i32 {
    let i = v as i32;
    if v > i as f64 {
        i + 1
    } else {
        i
    }
}

/// `MathHelper.clamp_int`.
pub fn clamp_int(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `MathHelper.clamp_float`.
pub fn clamp_float(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `MathHelper.clamp_double`.
pub fn clamp_double(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `MathHelper.denormalizeClamp`.
pub fn denormalize_clamp(lo: f64, hi: f64, alpha: f64) -> f64 {
    if alpha < 0.0 {
        lo
    } else if alpha > 1.0 {
        hi
    } else {
        lo + (hi - lo) * alpha
    }
}

/// `MathHelper.abs_max`.
pub fn abs_max(a: f64, b: f64) -> f64 {
    let a = if a < 0.0 { -a } else { a };
    let b = if b < 0.0 { -b } else { b };
    if a > b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sin`/`cos` values (raw bits), taken from the 1.8.9 oracle.
    #[test]
    fn sin_cos_match_oracle() {
        assert_eq!(sin(0.0).to_bits(), 0x0000_0000);
        assert_eq!(cos(0.0).to_bits(), 0x3f80_0000);
        assert_eq!(sin(1.0).to_bits(), 0x3f57_695c);
        assert_eq!(cos(1.0).to_bits(), 0x3f0a_5341);
        assert_eq!(sin(-1.0).to_bits(), 0xbf57_695c);
        // `MathHelper.sin(PI)` (index 32768): fdlibm returns the positive residual.
        assert_eq!(sin_table()[32768].to_bits(), 0x250d_3132);
    }

    /// The full 65536-entry table matches the game's `MathHelper.SIN_TABLE` (FNV-1a of the bits;
    /// the expected hash comes from `MathOracle`'s reflection into the real table).
    #[test]
    fn sin_table_matches_the_game() {
        assert_eq!(table_hash(), 0x115b_15b6_ad9d_2006);
    }

    #[test]
    fn floor_and_clamp() {
        assert_eq!(floor_double(-0.5), -1);
        assert_eq!(floor_double(1.9), 1);
        assert_eq!(floor_double_long(100.999), 100);
        assert_eq!(floor_float(-1.9), -2);
        assert_eq!(clamp_int(5, 1, 3), 3);
        assert_eq!(clamp_int(-5, 1, 3), 1);
        assert_eq!(denormalize_clamp(2.0, 5.0, 0.25), 2.75);
    }
}
