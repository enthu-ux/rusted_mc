//! Bit-exact ports of the 1.8.9 noise generators.
//!
//! Ported from `net.minecraft.world.gen.{NoiseGeneratorImproved, NoiseGeneratorSimplex,
//! NoiseGeneratorOctaves, NoiseGeneratorPerlin}` and `MathHelper.floor_double_long`.
//! Every arithmetic step keeps vanilla's order and type (Java `double`).

use mc_core::random::JavaRandom;

const SQRT_3: f64 = 1.732_050_807_568_877_2; // Math.sqrt(3.0)

// NoiseGeneratorImproved gradient tables (field_152381_e .. field_152385_i).
const E: [f64; 16] = [
    1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0,
];
const F: [f64; 16] = [
    1.0, 1.0, -1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0,
];
const G: [f64; 16] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 0.0, 1.0, 0.0, -1.0,
];
const H: [f64; 16] = [
    1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0,
];
const I: [f64; 16] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 0.0, 1.0, 0.0, -1.0,
];

// NoiseGeneratorSimplex gradient vectors (field_151611_e).
const SIMPLEX_GRAD: [[i32; 3]; 12] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
];

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

/// Java's `(int) value` truncation, matching `func_151607_a`'s test.
#[inline]
fn to_int(v: f64) -> i32 {
    if v > 0.0 {
        v as i32
    } else {
        v as i32 - 1
    }
}

/// `MathHelper.floor_double_long`.
#[inline]
fn floor_double_long(v: f64) -> i64 {
    let i = v as i64;
    if v < i as f64 {
        i - 1
    } else {
        i
    }
}

#[inline]
fn grad3(i: i32, x: f64, y: f64, z: f64) -> f64 {
    let j = (i & 15) as usize;
    E[j] * x + F[j] * y + G[j] * z
}

#[inline]
fn grad2(i: i32, x: f64, y: f64) -> f64 {
    let j = (i & 15) as usize;
    H[j] * x + I[j] * y
}

/// Port of `NoiseGeneratorImproved`.
pub struct Improved {
    perm: [i32; 512],
    x_coord: f64,
    y_coord: f64,
    z_coord: f64,
}

impl Improved {
    pub fn new(rng: &mut JavaRandom) -> Self {
        let x_coord = rng.next_double() * 256.0;
        let y_coord = rng.next_double() * 256.0;
        let z_coord = rng.next_double() * 256.0;

        let mut perm = [0i32; 512];
        for (i, p) in perm.iter_mut().enumerate().take(256) {
            *p = i as i32;
        }
        for i in 0..256usize {
            let j = (rng.next_int_bound((256 - i) as i32) as usize) + i;
            perm.swap(i, j);
            perm[i + 256] = perm[i];
        }
        Improved {
            perm,
            x_coord,
            y_coord,
            z_coord,
        }
    }

    /// `NoiseGeneratorImproved.populateNoiseArray`.
    #[allow(clippy::too_many_arguments)]
    pub fn populate_noise_array(
        &self,
        out: &mut Vec<f64>,
        x: f64,
        y: f64,
        z: f64,
        x_size: i32,
        y_size: i32,
        z_size: i32,
        x_scale: f64,
        y_scale: f64,
        z_scale: f64,
        amplitude: f64,
    ) {
        // Vanilla `populateNoiseArray` ACCUMULATES (`+=`); it never clears the buffer.
        // Clearing is the caller's job (Octaves/Perlin), so octaves can stack.
        let need = (x_size * y_size * z_size) as usize;
        if out.len() < need {
            out.resize(need, 0.0);
        }

        if y_size == 1 {
            let inv = 1.0 / amplitude;
            let mut idx = 0usize;
            for i in 0..x_size {
                let d = x + i as f64 * x_scale + self.x_coord;
                let mut fi = d as i32;
                if d < fi as f64 {
                    fi -= 1;
                }
                let pi = (fi & 0xFF) as usize;
                let fx = d - fi as f64;
                let fxx = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);

                for k in 0..z_size {
                    let d2 = z + k as f64 * z_scale + self.z_coord;
                    let mut fk = d2 as i32;
                    if d2 < fk as f64 {
                        fk -= 1;
                    }
                    let pk = (fk & 0xFF) as i32;
                    let fz = d2 - fk as f64;
                    let fzz = fz * fz * fz * (fz * (fz * 6.0 - 15.0) + 10.0);

                    let a = self.perm[pi];
                    let b = self.perm[a as usize] + pk;
                    let c = self.perm[pi + 1];
                    let dd = self.perm[c as usize] + pk;

                    let l1 = lerp(
                        fxx,
                        grad2(self.perm[b as usize], fx, fz),
                        grad3(self.perm[dd as usize], fx - 1.0, 0.0, fz),
                    );
                    let l2 = lerp(
                        fxx,
                        grad3(self.perm[b as usize + 1], fx, 0.0, fz - 1.0),
                        grad3(self.perm[dd as usize + 1], fx - 1.0, 0.0, fz - 1.0),
                    );
                    out[idx] += lerp(fzz, l1, l2) * inv;
                    idx += 1;
                }
            }
            return;
        }

        let inv = 1.0 / amplitude;
        let mut idx = 0usize;
        let mut last_pj: i32 = -1;
        let (mut l1, mut l2, mut l3, mut l4) = (0.0f64, 0.0, 0.0, 0.0);

        for i in 0..x_size {
            let d = x + i as f64 * x_scale + self.x_coord;
            let mut fi = d as i32;
            if d < fi as f64 {
                fi -= 1;
            }
            let pi = (fi & 0xFF) as usize;
            let fx = d - fi as f64;
            let fxx = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);

            for k in 0..z_size {
                let dz = z + k as f64 * z_scale + self.z_coord;
                let mut fk = dz as i32;
                if dz < fk as f64 {
                    fk -= 1;
                }
                let pk = (fk & 0xFF) as i32;
                let fz = dz - fk as f64;
                let fzz = fz * fz * fz * (fz * (fz * 6.0 - 15.0) + 10.0);

                for j in 0..y_size {
                    let dy = y + j as f64 * y_scale + self.y_coord;
                    let mut fj = dy as i32;
                    if dy < fj as f64 {
                        fj -= 1;
                    }
                    let pj = fj & 0xFF;
                    let fy = dy - fj as f64;
                    let fyy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);

                    if j == 0 || pj != last_pj {
                        last_pj = pj;
                        let a = self.perm[pi] + pj;
                        let b = self.perm[a as usize] + pk;
                        let c = self.perm[a as usize + 1] + pk;
                        let e = self.perm[pi + 1] + pj;
                        let f = self.perm[e as usize] + pk;
                        let g = self.perm[e as usize + 1] + pk;

                        l1 = lerp(
                            fxx,
                            grad3(self.perm[b as usize], fx, fy, fz),
                            grad3(self.perm[f as usize], fx - 1.0, fy, fz),
                        );
                        l2 = lerp(
                            fxx,
                            grad3(self.perm[c as usize], fx, fy - 1.0, fz),
                            grad3(self.perm[g as usize], fx - 1.0, fy - 1.0, fz),
                        );
                        l3 = lerp(
                            fxx,
                            grad3(self.perm[b as usize + 1], fx, fy, fz - 1.0),
                            grad3(self.perm[f as usize + 1], fx - 1.0, fy, fz - 1.0),
                        );
                        l4 = lerp(
                            fxx,
                            grad3(self.perm[c as usize + 1], fx, fy - 1.0, fz - 1.0),
                            grad3(self.perm[g as usize + 1], fx - 1.0, fy - 1.0, fz - 1.0),
                        );
                    }

                    let m1 = lerp(fyy, l1, l2);
                    let m2 = lerp(fyy, l3, l4);
                    out[idx] += lerp(fzz, m1, m2) * inv;
                    idx += 1;
                }
            }
        }
    }
}

/// Port of `NoiseGeneratorSimplex`.
pub struct Simplex {
    perm: [i32; 512],
    b: f64,
    c: f64,
    d: f64,
}

impl Simplex {
    pub fn new(rng: &mut JavaRandom) -> Self {
        let b = rng.next_double() * 256.0;
        let c = rng.next_double() * 256.0;
        let d = rng.next_double() * 256.0;

        let mut perm = [0i32; 512];
        for (i, p) in perm.iter_mut().enumerate().take(256) {
            *p = i as i32;
        }
        for i in 0..256usize {
            let j = (rng.next_int_bound((256 - i) as i32) as usize) + i;
            perm.swap(i, j);
            perm[i + 256] = perm[i];
        }
        Simplex { perm, b, c, d }
    }

    /// `NoiseGeneratorSimplex.func_151605_a`.
    pub fn noise_2d(&self, x: f64, y: f64) -> f64 {
        let g = 0.5 * (SQRT_3 - 1.0);
        let h = (3.0 - SQRT_3) / 6.0;

        let s = (x + y) * g;
        let i = to_int(x + s);
        let j = to_int(y + s);
        let t = (i + j) as f64 * h;
        let x0 = x - (i as f64 - t);
        let y0 = y - (j as f64 - t);

        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        let x1 = x0 - i1 as f64 + h;
        let y1 = y0 - j1 as f64 + h;
        let x2 = x0 - 1.0 + 2.0 * h;
        let y2 = y0 - 1.0 + 2.0 * h;

        let ii = (i & 0xFF) as usize;
        let jj = (j & 0xFF) as usize;
        let gi0 = (self.perm[ii + self.perm[jj] as usize] % 12) as usize;
        let gi1 =
            (self.perm[ii + i1 as usize + self.perm[jj + j1 as usize] as usize] % 12) as usize;
        let gi2 = (self.perm[ii + 1 + self.perm[jj + 1] as usize] % 12) as usize;

        let dot = |gi: usize, xx: f64, yy: f64| {
            SIMPLEX_GRAD[gi][0] as f64 * xx + SIMPLEX_GRAD[gi][1] as f64 * yy
        };

        let t0 = 0.5 - x0 * x0 - y0 * y0;
        let n0 = if t0 < 0.0 {
            0.0
        } else {
            let t0 = t0 * t0;
            t0 * t0 * dot(gi0, x0, y0)
        };
        let t1 = 0.5 - x1 * x1 - y1 * y1;
        let n1 = if t1 < 0.0 {
            0.0
        } else {
            let t1 = t1 * t1;
            t1 * t1 * dot(gi1, x1, y1)
        };
        let t2 = 0.5 - x2 * x2 - y2 * y2;
        let n2 = if t2 < 0.0 {
            0.0
        } else {
            let t2 = t2 * t2;
            t2 * t2 * dot(gi2, x2, y2)
        };

        70.0 * (n0 + n1 + n2)
    }
}

/// Port of `NoiseGeneratorOctaves`.
pub struct Octaves {
    generators: Vec<Improved>,
    octaves: usize,
}

impl Octaves {
    pub fn new(rng: &mut JavaRandom, octaves: i32) -> Self {
        let generators = (0..octaves).map(|_| Improved::new(rng)).collect();
        Octaves {
            generators,
            octaves: octaves as usize,
        }
    }

    /// `NoiseGeneratorOctaves.generateNoiseOctaves` (9-argument form).
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &self,
        out: &mut Vec<f64>,
        x: i32,
        y: i32,
        z: i32,
        x_size: i32,
        y_size: i32,
        z_size: i32,
        x_scale: f64,
        y_scale: f64,
        z_scale: f64,
    ) {
        let need = (x_size * y_size * z_size) as usize;
        if out.len() >= need {
            for v in out.iter_mut() {
                *v = 0.0;
            }
        } else {
            out.clear();
            out.resize(need, 0.0);
        }

        let mut scale = 1.0;
        for gen in self.generators.iter().take(self.octaves) {
            let mut xx = x as f64 * scale * x_scale;
            let yy = y as f64 * scale * y_scale;
            let mut zz = z as f64 * scale * z_scale;

            let flx = floor_double_long(xx);
            let flz = floor_double_long(zz);
            xx -= flx as f64;
            zz -= flz as f64;
            let mx = flx % 16_777_216;
            let mz = flz % 16_777_216;
            xx += mx as f64;
            zz += mz as f64;

            gen.populate_noise_array(
                out,
                xx,
                yy,
                zz,
                x_size,
                y_size,
                z_size,
                x_scale * scale,
                y_scale * scale,
                z_scale * scale,
                scale,
            );
            scale /= 2.0;
        }
    }
}

/// Port of `NoiseGeneratorPerlin`.
pub struct Perlin {
    generators: Vec<Simplex>,
    octaves: usize,
}

impl Perlin {
    pub fn new(rng: &mut JavaRandom, octaves: i32) -> Self {
        let generators = (0..octaves).map(|_| Simplex::new(rng)).collect();
        Perlin {
            generators,
            octaves: octaves as usize,
        }
    }

    /// `NoiseGeneratorPerlin.func_151601_a`.
    pub fn sample_2d(&self, x: f64, y: f64) -> f64 {
        let mut v = 0.0;
        let mut scale = 1.0;
        for gen in self.generators.iter().take(self.octaves) {
            v += gen.noise_2d(x * scale, y * scale) / scale;
            scale /= 2.0;
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::random::JavaRandom;

    /// First three samples, taken from the 1.8.9 oracle.
    #[test]
    fn improved_golden_seed0() {
        let mut r = JavaRandom::new(0);
        let g = Improved::new(&mut r);
        let mut out = Vec::new();
        g.populate_noise_array(&mut out, 0.0, 0.0, 0.0, 5, 5, 5, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(out[0].to_bits(), 0xbfb87d67ea2e10e4);
        assert_eq!(out[1].to_bits(), 0x3f84b2c2f4eb6e2c);
        assert_eq!(out[2].to_bits(), 0x3fb4bf24b876f5d7);
    }

    /// Regression: `populate_noise_array` must accumulate, not overwrite.
    #[test]
    fn populate_accumulates_into_existing_buffer() {
        let mut r = JavaRandom::new(0);
        let g = Improved::new(&mut r);
        let mut out = Vec::new();
        g.populate_noise_array(&mut out, 0.0, 0.0, 0.0, 5, 5, 5, 1.0, 1.0, 1.0, 1.0);
        let first = out.clone();
        g.populate_noise_array(&mut out, 0.0, 0.0, 0.0, 5, 5, 5, 1.0, 1.0, 1.0, 1.0);
        for (a, b) in first.iter().zip(out.iter()) {
            assert_eq!(*b, *a * 2.0, "buffer should have doubled (accumulated)");
        }
    }
}
