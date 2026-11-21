//! `MapGenBase` - the driver shared by the chunk features that work on a 17x17 chunk
//! neighbourhood with a per-neighbour RNG stream (`MapGenCaves`, `MapGenRavine`,
//! `MapGenStructure`).
//!
//! ```text
//! rand.setSeed(worldSeed);
//! long a = rand.nextLong();
//! long b = rand.nextLong();
//! for (nx = targetX - range; nx <= targetX + range; nx++)
//!    for (nz = targetZ - range; nz <= targetZ + range; nz++) {
//!       rand.setSeed(nx * a ^ nz * b ^ worldSeed);
//!       recursiveGenerate(world, nx, nz, targetX, targetZ, primer);
//!    }
//! ```
//!
//! Consequences worth remembering: the stream is reseeded before every neighbour, so a neighbour's
//! draws never depend on which chunk asked for it (skipping a neighbour, as
//! `MapGenStructure` does for already-registered ones, cannot disturb its successors).

use mc_core::random::JavaRandom;

/// `MapGenBase.range`.
pub const RANGE: i32 = 8;

/// `MapGenBase.generate`. `visit` receives the freshly reseeded stream and the neighbour chunk
/// coordinates; the target chunk coordinates are captured by the caller.
pub fn generate<F>(
    world_seed: i64,
    rand: &mut JavaRandom,
    target_x: i32,
    target_z: i32,
    mut visit: F,
) where
    F: FnMut(&mut JavaRandom, i32, i32),
{
    rand.set_seed(world_seed);
    let a = rand.next_long();
    let b = rand.next_long();

    for nx in target_x - RANGE..=target_x + RANGE {
        for nz in target_z - RANGE..=target_z + RANGE {
            rand.set_seed((nx as i64).wrapping_mul(a) ^ (nz as i64).wrapping_mul(b) ^ world_seed);
            visit(rand, nx, nz);
        }
    }
}
