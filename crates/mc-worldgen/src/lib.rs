//! `mc-worldgen` — Deterministic 1.8.9 world generation: RNG -> noise -> biome layers -> terrain -> structures -> decoration.
//!
//! See `docs/ARCHITECTURE.md` and `plan.md` for the two gates (parity + performance).

pub mod cave;
pub mod chunk;
pub mod layer;
pub mod mapgen;
pub mod noise;
pub mod structure;
pub mod terrain;
