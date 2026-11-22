//! The canonical chunk artefacts the game builds at the end of
//! `ChunkProviderGenerate.provideChunk`: the block array, the biome array and the height map.
//!
//! `ChunkProviderGenerate.provideChunk` finishes with
//!
//! ```text
//! Chunk chunk = new Chunk(world, primer, chunkX, chunkZ);
//! byte[] biomes = chunk.getBiomeArray();
//! for (int i = 0; i < biomes.length; i++) biomes[i] = (byte) biomesForGeneration[i].biomeID;
//! chunk.generateSkylightMap();
//! ```
//!
//! The biome array is therefore the generation biome ids copied *linearly*: `biomesForGeneration`
//! comes from the biome cache / `biomeIndexLayer.getInts(.., 16, 16)`, whose layout is
//! `x + z * 16`, so index `i` is the column `(x = i & 15, z = i >> 4)`.
//!
//! The height map is computed by `Chunk.generateSkylightMap` (not by the primer constructor):
//! `heightMap[z << 4 | x]` is one past the highest block in the column whose
//! `getLightOpacity() != 0`, scanning down from `getTopFilledSegment() + 16`. `light_opacity` below
//! is dumped from the live game (`oracle/java/src/BlockTableOracle.java`); it is a per-block value
//! (`Block.getLightOpacity()` takes no state), and `getTopFilledSegment` depends on which blocks
//! are material-air (`[0]` only).
//!
//! Skylight/block-light *values* are not part of this artefact (see `light_parity`).

use crate::structure::StructureStart;

/// The result of a full `provideChunk` pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedChunk {
    /// Block states `id << 4 | meta`, indexed `x << 12 | z << 8 | y` (vanilla's `ChunkPrimer`).
    pub blocks: Vec<u16>,
    /// `Chunk.getBiomeArray()` - 256 generation biome ids, index `x + z * 16`.
    pub biomes: Vec<u8>,
    /// `Chunk.heightMap` - 256 values, index `z << 4 | x` (`Chunk.getHeightValue(x, z)`).
    pub height_map: Vec<i32>,
    /// Structure starts registered by `provideChunk` whose box meets this chunk: the set
    /// `MapGenStructure.generateStructure` will write while populating it. Vanilla keeps them in
    /// the generator's `structureMap`, not in the chunk, so this is the port's view of that state.
    pub structures: Vec<StructureStart>,
}

#[inline]
fn block_id(v: u16) -> u16 {
    v >> 4
}

#[inline]
fn idx(x: usize, y: usize, z: usize) -> usize {
    (x << 12) | (z << 8) | y
}

/// `Block.getLightOpacity()` per block id, dumped from the live game
/// (`BlockTableOracle`). Ids not listed use the `Block` default of 255.
pub fn light_opacity(id: i32) -> i32 {
    match id {
        0 => 0,
        6 => 0,
        8 => 3,
        9 => 3,
        10 => 0,
        11 => 0,
        18 => 1,
        20 => 0,
        26 => 0,
        27 => 0,
        28 => 0,
        29 => 0,
        30 => 1,
        31 => 0,
        32 => 0,
        33 => 0,
        34 => 0,
        36 => 0,
        37 => 0,
        38 => 0,
        39 => 0,
        40 => 0,
        50 => 0,
        51 => 0,
        52 => 0,
        54 => 0,
        55 => 0,
        59 => 0,
        63 => 0,
        64 => 0,
        65 => 0,
        66 => 0,
        68 => 0,
        69 => 0,
        70 => 0,
        71 => 0,
        72 => 0,
        75 => 0,
        76 => 0,
        77 => 0,
        78 => 0,
        79 => 3,
        81 => 0,
        83 => 0,
        85 => 0,
        90 => 0,
        92 => 0,
        93 => 0,
        94 => 0,
        95 => 0,
        96 => 0,
        101 => 0,
        102 => 0,
        104 => 0,
        105 => 0,
        106 => 0,
        107 => 0,
        111 => 0,
        113 => 0,
        115 => 0,
        116 => 0,
        117 => 0,
        118 => 0,
        119 => 0,
        120 => 0,
        122 => 0,
        127 => 0,
        130 => 0,
        131 => 0,
        132 => 0,
        138 => 0,
        139 => 0,
        140 => 0,
        141 => 0,
        142 => 0,
        143 => 0,
        144 => 0,
        145 => 0,
        146 => 0,
        147 => 0,
        148 => 0,
        149 => 0,
        150 => 0,
        151 => 0,
        154 => 0,
        157 => 0,
        160 => 0,
        161 => 1,
        165 => 0,
        166 => 0,
        167 => 0,
        171 => 0,
        175 => 0,
        176 => 0,
        177 => 0,
        178 => 0,
        183 => 0,
        184 => 0,
        185 => 0,
        186 => 0,
        187 => 0,
        188 => 0,
        189 => 0,
        190 => 0,
        191 => 0,
        192 => 0,
        193 => 0,
        194 => 0,
        195 => 0,
        196 => 0,
        197 => 0,
        _ => 255,
    }
}

/// `Block.getMaterial() == Material.air` (from the same dump: only id 0).
#[inline]
pub fn material_is_air(v: u16) -> bool {
    block_id(v) == 0
}

/// `Block.getMaterial().isLiquid()` (same dump): water and lava, flowing and still. Used by
/// `StructureComponent.isLiquidInStructureBoundingBox`.
#[inline]
pub fn material_is_liquid(id: i32) -> bool {
    (8..=11).contains(&id)
}

/// `Block.isFullBlock()` (`fullBlock`, set from `isOpaqueCube()`), dumped from the live game. Listed
/// ids are **not** full blocks; every other id is. Used by the mineshaft rail loop.
#[inline]
pub fn is_full_block(id: i32) -> bool {
    !matches!(
        id,
        0 | 6
            | 8
            | 9
            | 10
            | 11
            | 20
            | 26
            | 27
            | 28
            | 29
            | 30
            | 31
            | 32
            | 33
            | 34
            | 36
            | 37
            | 38
            | 39
            | 40
            | 44
            | 50
            | 51
            | 52
            | 53
            | 54
            | 55
            | 59
            | 60
            | 63
            | 64
            | 65
            | 66
            | 67
            | 68
            | 69
            | 70
            | 71
            | 72
            | 75
            | 76
            | 77
            | 78
            | 79
            | 81
            | 83
            | 85
            | 90
            | 92
            | 93
            | 94
            | 95
            | 96
            | 101
            | 102
            | 104
            | 105
            | 106
            | 107
            | 108
            | 109
            | 111
            | 113
            | 114
            | 115
            | 116
            | 117
            | 118
            | 119
            | 120
            | 122
            | 126
            | 127
            | 128
            | 130
            | 131
            | 132
            | 134
            | 135
            | 136
            | 138
            | 139
            | 140
            | 141
            | 142
            | 143
            | 144
            | 145
            | 146
            | 147
            | 148
            | 149
            | 150
            | 151
            | 154
            | 156
            | 157
            | 160
            | 163
            | 164
            | 165
            | 166
            | 167
            | 171
            | 175
            | 176
            | 177
            | 178
            | 180
            | 182
            | 183
            | 184
            | 185
            | 186
            | 187
            | 188
            | 189
            | 190
            | 191
            | 192
            | 193
            | 194
            | 195
            | 196
            | 197
    )
}

/// `Chunk.getBiomeArray()`: the generation biome ids, low byte each (Java's `(byte)` cast).
pub fn biome_array(biome_ids: &[i32]) -> Vec<u8> {
    biome_ids.iter().map(|&b| b as u8).collect()
}

/// `Chunk.generateSkylightMap`'s height map: `height_map[z << 4 | x]` is one past the highest block
/// in the column with `getLightOpacity() != 0`. Columns with no such block stay 0.
pub fn height_map(blocks: &[u16]) -> Vec<i32> {
    let mut heights = vec![0i32; 256];

    // `Chunk.getTopFilledSegment`: the highest 16-block section holding a non-air-material block.
    let mut top = 0usize;
    'sections: for section in (0..16usize).rev() {
        for x in 0..16 {
            for z in 0..16 {
                for y in section * 16..section * 16 + 16 {
                    if !material_is_air(blocks[idx(x, y, z)]) {
                        top = section * 16;
                        break 'sections;
                    }
                }
            }
        }
    }

    for x in 0..16usize {
        for z in 0..16usize {
            for y in (1..=top + 16).rev() {
                if light_opacity(block_id(blocks[idx(x, y - 1, z)]) as i32) != 0 {
                    heights[z << 4 | x] = y as i32;
                    break;
                }
            }
        }
    }

    heights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{GenOptions, TerrainGen};

    /// `Chunk.getBiomeArray()` is the generation biome ids copied linearly (oracle: seed 0,
    /// chunk (0, 0) is forest).
    #[test]
    fn biome_array_matches_generation_ids() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(0, 0, GenOptions::vanilla());
        assert_eq!(chunk.biomes.len(), 256);
        assert_eq!(chunk.biomes[0], 4);
    }

    /// Mutation ids survive the byte copy (oracle: seed 12345, chunk (0, 0) is taiga M, id 133).
    #[test]
    fn biome_array_keeps_mutation_ids() {
        let mut gen = TerrainGen::new(12345);
        let chunk = gen.generate_chunk(0, 0, GenOptions::vanilla());
        assert_eq!(chunk.biomes[0], 133);
    }

    /// `light_opacity` is dumped from the game (`BlockTableOracle`): transparent blocks do not
    /// count for the height map.
    #[test]
    fn light_opacity_table() {
        assert_eq!(light_opacity(0), 0); // air
        assert_eq!(light_opacity(1), 255); // stone (Block's default)
        assert_eq!(light_opacity(9), 3); // water
        assert_eq!(light_opacity(11), 0); // lava
        assert_eq!(light_opacity(18), 1); // leaves
        assert_eq!(light_opacity(78), 0); // snow layer -> height map ignores it
        assert_eq!(light_opacity(79), 3); // ice
    }

    /// The height map finds the topmost light-blocking block; a snow layer above stone is
    /// transparent, so it does not raise the height.
    #[test]
    fn height_map_ignores_transparent_blocks() {
        let mut blocks = vec![0u16; 65536];
        blocks[(3 << 12) | (7 << 8) | 5] = 1 << 4; // stone at (3, 5, 7)
        blocks[(3 << 12) | (7 << 8) | 6] = 78 << 4; // snow layer at (3, 6, 7)
        let heights = height_map(&blocks);
        assert_eq!(heights[(7 << 4) | 3], 6);
        assert_eq!(heights[0], 0); // a column with no light-blocking block
    }

    /// Over open ocean the topmost light-blocking block is water at y=62, so every height is 63
    /// (oracle: seed 0, chunk (512, 512)).
    #[test]
    fn height_map_over_open_ocean() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(512, 512, GenOptions::vanilla());
        assert!(chunk.height_map.iter().all(|&h| h == 63));
    }

    /// Height values for a land chunk, in `z << 4 | x` order (oracle: seed 0, chunk (-3, 0)).
    #[test]
    fn height_map_matches_oracle_columns() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-3, 0, GenOptions::vanilla());
        assert_eq!(chunk.height_map[0], 65); // x=0, z=0
        assert_eq!(chunk.height_map[5 << 4], 65); // x=0, z=5
    }

    /// A mesa chunk's biome and height values (oracle: seed 0, chunk (-48, -48)).
    #[test]
    fn height_map_and_biome_on_mesa() {
        let mut gen = TerrainGen::new(0);
        let chunk = gen.generate_chunk(-48, -48, GenOptions::vanilla());
        assert_eq!(chunk.biomes[0], 37);
        assert_eq!(chunk.height_map[0], 63);
    }
}
