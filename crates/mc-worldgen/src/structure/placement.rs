//! `StructureComponent`'s placement helpers and the `StructureMineshaftPieces.addComponentParts`
//! bodies - the `populate` half of a mineshaft (the `provideChunk` half is `mineshaft.rs`).
//!
//! Ported from the decompiled vanilla:
//!
//! * `StructureComponent` - `isLiquidInStructureBoundingBox`, `getX/Y/ZWithOffset`,
//!   `setBlockState`, `getBlockStateFromPos`, `fillWithBlocks`, `func_175805_a`,
//!   `randomlyPlaceBlock`, `randomlyRareFillWithBlocks`, `getMetadataWithOffset` (rail path);
//! * `StructureMineshaftPieces.{Corridor,Cross,Room,Stairs}.addComponentParts` + the corridor's
//!   `generateChestContents` override;
//! * `MapGenStructure.generateStructure`'s bounding box and `StructureStart.generateStructure`.
//!
//! **The bounding box is offset by 8**: vanilla passes
//! `new StructureBoundingBox((cx << 4) + 8, (cz << 4) + 8, +15, +15)` (bytecode-verified), so a
//! `populate(cx, cz)` writes into `[cx*16+8, cx*16+23] x [cz*16+8, cz*16+23]`, spanning two chunks per
//! axis. Everything a piece reads or writes is clipped to that box, and the boxes of the four chunks
//! covering a target chunk are disjoint, so the target chunk is the union of four passes.
//!
//! Two pieces of state do carry across those passes (`spawnerPlaced`, and dropping a piece whose box
//! holds a liquid), so the passes run in a fixed order ([`crate::terrain::TerrainGen::populate_mineshaft_chunk`]);
//! vanilla's real order is chunk-load order. When several starts meet one box, vanilla's `HashMap`
//! order is history-dependent (see `handoff.md`), so placement corpora use single-start-per-box chunks.
//!
//! RNG order matters: the corridor chest rolls consume the enchantment + chest draws ported in
//! `loot.rs`, before the rest of the section's placement.

use std::collections::BTreeMap;

use mc_core::random::JavaRandom;

use super::{BoundingBox, Facing, Piece, PieceKind};
use crate::chunk::{is_full_block, material_is_air, material_is_liquid};
use crate::decorate::blocks::{material_blocks_movement, material_is_leaves};
use crate::loot::chest::MINESHAFT_CHEST_CONTENT;
use crate::loot::chest::{enchanted_book_content, generate_chest_contents, with_extra};

/// `WorldProvider.getAverageGroundLevel()` = `world.getSeaLevel() + 1` (63 + 1 for the overworld).
const AVERAGE_GROUND_LEVEL: i32 = 64;

/// `ChunkProviderGenerate.populate`'s per-chunk seed: `rand.setSeed(worldSeed)`, two
/// `nextLong()/2*2+1` draws, then `chunkX*a + chunkZ*b ^ worldSeed`.
pub fn populate_rand(world_seed: i64, chunk_x: i32, chunk_z: i32) -> JavaRandom {
    let mut rand = JavaRandom::new(world_seed);
    let a = rand.next_long() / 2 * 2 + 1;
    let b = rand.next_long() / 2 * 2 + 1;
    rand.set_seed(
        (chunk_x as i64)
            .wrapping_mul(a)
            .wrapping_add((chunk_z as i64).wrapping_mul(b))
            ^ world_seed,
    );
    rand
}

/// `MapGenStructure.generateStructure`'s box for one populate chunk: the `+8`-offset 16x16 column
/// with the 4-arg `StructureBoundingBox` y range (1..512).
pub fn structure_box(chunk_x: i32, chunk_z: i32) -> BoundingBox {
    let x = (chunk_x << 4) + 8;
    let z = (chunk_z << 4) + 8;
    BoundingBox::new(x, 1, z, x + 15, 512, z + 15)
}

/// A read/write view over the chunk buffers a `populate` pass can reach (`World.getBlockState` /
/// `World.setBlockState`). Blocks are `id << 4 | meta`, indexed `x << 12 | z << 8 | y`.
#[derive(Default)]
pub struct ChunkWorld {
    chunks: BTreeMap<(i32, i32), Vec<u16>>,
}

#[inline]
fn chunk_index(x: i32, y: i32, z: i32) -> usize {
    (((x & 15) as usize) << 12) | (((z & 15) as usize) << 8) | y as usize
}

impl ChunkWorld {
    pub fn new() -> ChunkWorld {
        ChunkWorld::default()
    }

    /// Supplies a chunk's terrain (from `TerrainGen::generate_chunk`).
    pub fn insert_chunk(&mut self, chunk_x: i32, chunk_z: i32, blocks: Vec<u16>) {
        self.chunks.insert((chunk_x, chunk_z), blocks);
    }

    /// The chunk's final block array (`id << 4 | meta`).
    pub fn chunk_blocks(&self, chunk_x: i32, chunk_z: i32) -> &[u16] {
        &self.chunks[&(chunk_x, chunk_z)]
    }

    /// `World.getBlockState` at absolute coordinates. Air outside the build height or a missing
    /// chunk (vanilla generates on demand; the driver supplies every reachable chunk).
    fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if !(0..256).contains(&y) {
            return 0;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(blocks) => blocks[chunk_index(x, y, z)],
            None => 0,
        }
    }

    /// `World.setBlockState` at absolute coordinates (drops writes to a missing chunk, which the
    /// driver prevents).
    fn set(&mut self, x: i32, y: i32, z: i32, value: u16) {
        if !(0..256).contains(&y) {
            return;
        }
        if let Some(blocks) = self.chunks.get_mut(&(x >> 4, z >> 4)) {
            blocks[chunk_index(x, y, z)] = value;
        }
    }

    /// `World.getTopSolidOrLiquidBlock`'s y: scan down from the top of the build height for the
    /// first block that blocks movement and is not leaves (liquids do not block movement).
    pub fn get_top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
        let mut y = 255;
        while y >= 0 {
            let id = (self.get(x, y - 1, z) >> 4) as i32;
            if material_blocks_movement(id) && !material_is_leaves(id) {
                break;
            }
            y -= 1;
        }
        y
    }
}

/// `MapGenStructure.generateStructure`: run every piece of `start` that meets `chunk`, in order.
///
/// Vanilla removes a piece from the start when `addComponentParts` returns false (liquid in the box),
/// and a corridor's `spawnerPlaced` persists; both carry across the four passes that cover a chunk, so
/// the passes run in a fixed order (the driver's chunk order) and this advances `start` in place.
pub fn generate_structure(
    start: &mut super::StructureStart,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
) {
    let mut i = 0;
    while i < start.pieces.len() {
        let piece = &mut start.pieces[i];
        if piece.bbox.intersects_with(chunk) && !add_component_parts(piece, world, rand, chunk) {
            start.pieces.remove(i);
            continue;
        }
        i += 1;
    }
}

// Block states (`id << 4 | meta`).
const AIR: u16 = 0;
const DIRT: u16 = 3 << 4;
const PLANKS: u16 = 5 << 4;
const WEB: u16 = 30 << 4;
const MOB_SPAWNER: u16 = 52 << 4;
/// `Blocks.torch.getStateFromMeta(EnumFacing.UP.getIndex())` -> `FACING=EAST`, meta 1.
const TORCH: u16 = (50 << 4) | 1;

/// `BlockRailBase.getStateFromMeta(meta)` for `Blocks.rail` (meta 0 = north/south, 1 = east/west).
fn rail(meta: i32) -> u16 {
    (66 << 4) | (meta as u16)
}

/// `StructureComponent.getMetadataWithOffset(Blocks.rail, meta)`.
fn rail_meta_with_offset(facing: Option<Facing>, meta: i32) -> i32 {
    if matches!(facing, Some(Facing::West) | Some(Facing::East)) {
        if meta == 1 {
            0
        } else {
            1
        }
    } else {
        meta
    }
}

/// The piece's `coordBaseMode` + `boundingBox`, i.e. everything `StructureComponent`'s helpers use.
struct Component {
    bbox: BoundingBox,
    facing: Option<Facing>,
}

impl Component {
    fn new(piece: &Piece) -> Component {
        Component {
            bbox: piece.bbox,
            facing: piece.coord_base_mode,
        }
    }

    /// `isLiquidInStructureBoundingBox`: the box expanded by 1, clipped to `chunk`, scanned for any
    /// liquid-material block (a shell, not the interior).
    fn is_liquid_in_structure_bounding_box(&self, world: &ChunkWorld, chunk: &BoundingBox) -> bool {
        let x1 = (self.bbox.min_x - 1).max(chunk.min_x);
        let y1 = (self.bbox.min_y - 1).max(chunk.min_y);
        let z1 = (self.bbox.min_z - 1).max(chunk.min_z);
        let x2 = (self.bbox.max_x + 1).min(chunk.max_x);
        let y2 = (self.bbox.max_y + 1).min(chunk.max_y);
        let z2 = (self.bbox.max_z + 1).min(chunk.max_z);

        let liquid = |x, y, z| material_is_liquid((world.get(x, y, z) >> 4) as i32);
        for x in x1..=x2 {
            for z in z1..=z2 {
                if liquid(x, y1, z) || liquid(x, y2, z) {
                    return true;
                }
            }
        }
        for x in x1..=x2 {
            for y in y1..=y2 {
                if liquid(x, y, z1) || liquid(x, y, z2) {
                    return true;
                }
            }
        }
        for z in z1..=z2 {
            for y in y1..=y2 {
                if liquid(x1, y, z) || liquid(x2, y, z) {
                    return true;
                }
            }
        }
        false
    }

    fn get_x_with_offset(&self, x: i32, z: i32) -> i32 {
        match self.facing {
            None => x,
            Some(Facing::North) | Some(Facing::South) => self.bbox.min_x + x,
            Some(Facing::West) => self.bbox.max_x - z,
            Some(Facing::East) => self.bbox.min_x + z,
        }
    }

    fn get_y_with_offset(&self, y: i32) -> i32 {
        match self.facing {
            None => y,
            _ => y + self.bbox.min_y,
        }
    }

    fn get_z_with_offset(&self, x: i32, z: i32) -> i32 {
        match self.facing {
            None => z,
            Some(Facing::North) => self.bbox.max_z - z,
            Some(Facing::South) => self.bbox.min_z + z,
            Some(Facing::West) | Some(Facing::East) => self.bbox.min_z + x,
        }
    }

    /// `setBlockState`: write only if the offset position is inside the chunk box.
    fn set_block_state(
        &self,
        world: &mut ChunkWorld,
        value: u16,
        x: i32,
        y: i32,
        z: i32,
        chunk: &BoundingBox,
    ) {
        let px = self.get_x_with_offset(x, z);
        let py = self.get_y_with_offset(y);
        let pz = self.get_z_with_offset(x, z);
        if chunk.is_vec_inside(px, py, pz) {
            world.set(px, py, pz, value);
        }
    }

    /// `getBlockStateFromPos`: air when the offset position is outside the chunk box.
    fn get_block_state_from_pos(
        &self,
        world: &ChunkWorld,
        x: i32,
        y: i32,
        z: i32,
        chunk: &BoundingBox,
    ) -> u16 {
        let px = self.get_x_with_offset(x, z);
        let py = self.get_y_with_offset(y);
        let pz = self.get_z_with_offset(x, z);
        if !chunk.is_vec_inside(px, py, pz) {
            return AIR;
        }
        world.get(px, py, pz)
    }

    /// `fillWithBlocks`. `always` is vanilla's `var11`: when true, only positions whose current
    /// material is not air are written.
    #[allow(clippy::too_many_arguments)]
    fn fill_with_blocks(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        outer: u16,
        inner: u16,
        always: bool,
    ) {
        for y in y1..=y2 {
            for x in x1..=x2 {
                for z in z1..=z2 {
                    if always {
                        let existing = self.get_block_state_from_pos(world, x, y, z, chunk);
                        if material_is_air(existing) {
                            continue;
                        }
                    }
                    let value = if y == y1 || y == y2 || x == x1 || x == x2 || z == z1 || z == z2 {
                        outer
                    } else {
                        inner
                    };
                    self.set_block_state(world, value, x, y, z, chunk);
                }
            }
        }
    }

    /// `func_175805_a`: like `fillWithBlocks`, but each position is written with probability
    /// `prob` (`rand.nextFloat() <= prob`).
    #[allow(clippy::too_many_arguments)]
    fn fill_with_random_blocks(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        rand: &mut JavaRandom,
        prob: f32,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        outer: u16,
        inner: u16,
        always: bool,
    ) {
        for y in y1..=y2 {
            for x in x1..=x2 {
                for z in z1..=z2 {
                    if rand.next_float() > prob {
                        continue;
                    }
                    if always {
                        let existing = self.get_block_state_from_pos(world, x, y, z, chunk);
                        if material_is_air(existing) {
                            continue;
                        }
                    }
                    let value = if y == y1 || y == y2 || x == x1 || x == x2 || z == z1 || z == z2 {
                        outer
                    } else {
                        inner
                    };
                    self.set_block_state(world, value, x, y, z, chunk);
                }
            }
        }
    }

    /// `randomlyPlaceBlock`.
    #[allow(clippy::too_many_arguments)]
    fn randomly_place_block(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        rand: &mut JavaRandom,
        prob: f32,
        x: i32,
        y: i32,
        z: i32,
        value: u16,
    ) {
        if rand.next_float() < prob {
            self.set_block_state(world, value, x, y, z, chunk);
        }
    }

    /// `randomlyRareFillWithBlocks`: fills the ellipsoid `<= 1.05` inside the box.
    #[allow(clippy::too_many_arguments)]
    fn randomly_rare_fill_with_blocks(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        value: u16,
        always: bool,
    ) {
        let dx = (x2 - x1 + 1) as f32;
        let dy = (y2 - y1 + 1) as f32;
        let dz = (z2 - z1 + 1) as f32;
        let cx = x1 as f32 + dx / 2.0;
        let cz = z1 as f32 + dz / 2.0;

        for y in y1..=y2 {
            let ny = (y - y1) as f32 / dy;
            for x in x1..=x2 {
                let nx = (x as f32 - cx) / (dx * 0.5);
                for z in z1..=z2 {
                    let nz = (z as f32 - cz) / (dz * 0.5);
                    if always {
                        let existing = self.get_block_state_from_pos(world, x, y, z, chunk);
                        if material_is_air(existing) {
                            continue;
                        }
                    }
                    if nx * nx + ny * ny + nz * nz <= 1.05 {
                        self.set_block_state(world, value, x, y, z, chunk);
                    }
                }
            }
        }
    }

    /// `replaceAirAndLiquidDownwards`: fill air/liquid downwards from `(x, y, z)` while inside the
    /// chunk box, stopping at the first non-air/non-liquid block or `y <= 1`.
    fn replace_air_and_liquid_downwards(
        &self,
        world: &mut ChunkWorld,
        chunk: &BoundingBox,
        value: u16,
        x: i32,
        y: i32,
        z: i32,
    ) {
        let px = self.get_x_with_offset(x, z);
        let mut py = self.get_y_with_offset(y);
        let pz = self.get_z_with_offset(x, z);
        if !chunk.is_vec_inside(px, py, pz) {
            return;
        }
        while py > 1 {
            let id = (world.get(px, py, pz) >> 4) as i32;
            if !(id == 0 || material_is_liquid(id)) {
                break;
            }
            world.set(px, py, pz, value);
            py -= 1;
        }
    }
}

/// `getMetadataWithOffset` for the stair blocks (`stone/oak/nether_brick/stone_brick/sandstone`).
fn stairs_meta_with_offset(facing: Option<Facing>, meta: i32) -> i32 {
    match facing {
        Some(Facing::South) => match meta {
            2 => 3,
            3 => 2,
            m => m,
        },
        Some(Facing::West) => match meta {
            0 => 2,
            1 => 3,
            2 => 0,
            3 => 1,
            m => m,
        },
        Some(Facing::East) => match meta {
            0 => 2,
            1 => 3,
            2 => 1,
            3 => 0,
            m => m,
        },
        _ => meta,
    }
}

/// `ComponentScatteredFeaturePieces.Feature.func_74935_a`: if the piece's `HPos` is unset, average
/// `max(getTopSolidOrLiquidBlock, averageGroundLevel)` over the piece/box intersection and offset the
/// piece so its `minY` lands at that average. Returns false when the intersection is empty.
fn temple_adjust_height(piece: &mut Piece, world: &ChunkWorld, chunk: &BoundingBox) -> bool {
    let h_pos = match &piece.kind {
        PieceKind::TempleJunglePyramid { h_pos, .. } | PieceKind::TempleSwampHut { h_pos, .. } => {
            *h_pos
        }
        _ => return true,
    };
    if h_pos >= 0 {
        return true;
    }

    let bb = piece.bbox;
    let mut sum = 0i64;
    let mut count = 0i64;
    for z in bb.min_z..=bb.max_z {
        for x in bb.min_x..=bb.max_x {
            if chunk.is_vec_inside(x, 64, z) {
                sum += world
                    .get_top_solid_or_liquid(x, z)
                    .max(AVERAGE_GROUND_LEVEL) as i64;
                count += 1;
            }
        }
    }
    if count == 0 {
        return false;
    }
    let h_pos = (sum / count) as i32;
    piece.bbox.offset(0, h_pos - bb.min_y, 0);
    match &mut piece.kind {
        PieceKind::TempleJunglePyramid { h_pos: h, .. }
        | PieceKind::TempleSwampHut { h_pos: h, .. } => *h = h_pos,
        _ => {}
    }
    true
}

/// `StructureComponent.addComponentParts`, dispatched on the piece kind. Returns false when the box
/// contains a liquid (vanilla drops the piece; the port only reports it).
pub fn add_component_parts(
    piece: &mut Piece,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
) -> bool {
    // `func_74935_a` offsets the piece (and seeds `HPos`) before any block is written; the jungle
    // pyramid and swamp hut use it, the desert pyramid does not.
    if matches!(
        &piece.kind,
        PieceKind::TempleJunglePyramid { .. } | PieceKind::TempleSwampHut { .. }
    ) && !temple_adjust_height(piece, world, chunk)
    {
        return false;
    }
    let comp = Component::new(piece);
    match &mut piece.kind {
        PieceKind::MineshaftCorridor {
            has_rails,
            has_spiders,
            spawner_placed,
            section_count,
        } => corridor(
            &comp,
            world,
            rand,
            chunk,
            *has_rails,
            *has_spiders,
            spawner_placed,
            *section_count,
        ),
        PieceKind::MineshaftCross {
            multiple_floors, ..
        } => cross(&comp, world, chunk, *multiple_floors),
        PieceKind::MineshaftRoom { linked_rooms } => room(&comp, world, chunk, linked_rooms),
        PieceKind::MineshaftStairs => stairs(&comp, world, chunk),
        // Scattered-feature block placement is not ported yet (layout only). Nothing reaches this
        // arm today: `populate` only walks the registered mineshafts. Kept `true` (piece retained)
        // so an accidental walk never silently drops a piece.
        PieceKind::TempleDesertPyramid { .. } => true,
        PieceKind::TempleJunglePyramid { .. } => true,
        PieceKind::TempleSwampHut { witch, .. } => swamp_hut(&comp, world, chunk, witch),
        // Monument block placement is not ported yet (registration/layout only). Nothing reaches
        // this arm today: `populate` only walks the registered mineshafts.
        PieceKind::MonumentBuilding => true,
        // Nether-fortress block placement is not ported yet (registration/layout only).
        PieceKind::Nether { .. } => true,
        // Village block placement is not ported yet (registration/layout only).
        PieceKind::Village(_) => true,
        // Stronghold block placement is not ported yet (registration/layout only).
        PieceKind::Stronghold(_) => true,
    }
}

/// `Corridor.addComponentParts`.
#[allow(clippy::too_many_arguments)]
fn corridor(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    has_rails: bool,
    has_spiders: bool,
    spawner_placed: &mut bool,
    section_count: i32,
) -> bool {
    if comp.is_liquid_in_structure_bounding_box(world, chunk) {
        return false;
    }

    let width = 2;
    let length = section_count * 5 - 1;
    comp.fill_with_blocks(world, chunk, 0, 0, 0, width, 1, length, AIR, AIR, false);
    comp.fill_with_random_blocks(
        world, chunk, rand, 0.8, 0, 2, 0, width, 2, length, AIR, AIR, false,
    );
    if has_spiders {
        comp.fill_with_random_blocks(
            world, chunk, rand, 0.6, 0, 0, 0, width, 1, length, WEB, AIR, false,
        );
    }

    for section in 0..section_count {
        let z = 2 + section * 5;
        let fence = 85 << 4; // Blocks.oak_fence
        comp.fill_with_blocks(world, chunk, 0, 0, z, 0, 1, z, fence, AIR, false);
        comp.fill_with_blocks(world, chunk, width, 0, z, width, 1, z, fence, AIR, false);
        if rand.next_int_bound(4) == 0 {
            comp.fill_with_blocks(world, chunk, 0, 2, z, 0, 2, z, PLANKS, AIR, false);
            comp.fill_with_blocks(world, chunk, width, 2, z, width, 2, z, PLANKS, AIR, false);
        } else {
            comp.fill_with_blocks(world, chunk, 0, 2, z, width, 2, z, PLANKS, AIR, false);
        }

        for (prob, x, y, dz) in [
            (0.1, 0, 2, -1),
            (0.1, 2, 2, -1),
            (0.1, 0, 2, 1),
            (0.1, 2, 2, 1),
            (0.05, 0, 2, -2),
            (0.05, 2, 2, -2),
            (0.05, 0, 2, 2),
            (0.05, 2, 2, 2),
        ] {
            comp.randomly_place_block(world, chunk, rand, prob, x, y, z + dz, WEB);
        }
        comp.randomly_place_block(world, chunk, rand, 0.05, 1, 2, z - 1, TORCH);
        comp.randomly_place_block(world, chunk, rand, 0.05, 1, 2, z + 1, TORCH);

        if rand.next_int_bound(100) == 0 {
            let book = enchanted_book_content(rand);
            let count = 3 + rand.next_int_bound(4);
            let list = with_extra(&MINESHAFT_CHEST_CONTENT, book);
            corridor_chest(comp, world, rand, chunk, 2, 0, z - 1, &list, count);
        }
        if rand.next_int_bound(100) == 0 {
            let book = enchanted_book_content(rand);
            let count = 3 + rand.next_int_bound(4);
            let list = with_extra(&MINESHAFT_CHEST_CONTENT, book);
            corridor_chest(comp, world, rand, chunk, 0, 0, z + 1, &list, count);
        }

        if has_spiders && !*spawner_placed {
            let y = comp.get_y_with_offset(0);
            let local_z = z - 1 + rand.next_int_bound(3);
            let x = comp.get_x_with_offset(1, local_z);
            let wz = comp.get_z_with_offset(1, local_z);
            if chunk.is_vec_inside(x, y, wz) {
                *spawner_placed = true;
                world.set(x, y, wz, MOB_SPAWNER);
            }
        }
    }

    for x in 0..=width {
        for z in 0..=length {
            if material_is_air(comp.get_block_state_from_pos(world, x, -1, z, chunk)) {
                comp.set_block_state(world, PLANKS, x, -1, z, chunk);
            }
        }
    }

    if has_rails {
        for z in 0..=length {
            let existing = comp.get_block_state_from_pos(world, 1, -1, z, chunk);
            if !material_is_air(existing) && is_full_block((existing >> 4) as i32) {
                comp.randomly_place_block(
                    world,
                    chunk,
                    rand,
                    0.7,
                    1,
                    0,
                    z,
                    rail(rail_meta_with_offset(comp.facing, 0)),
                );
            }
        }
    }

    true
}

/// `Corridor.generateChestContents`'s override: place a rail and fill a 27-slot minecart.
#[allow(clippy::too_many_arguments)]
fn corridor_chest(
    comp: &Component,
    world: &mut ChunkWorld,
    rand: &mut JavaRandom,
    chunk: &BoundingBox,
    x: i32,
    y: i32,
    z: i32,
    list: &[crate::loot::ChestContent],
    count: i32,
) -> bool {
    let px = comp.get_x_with_offset(x, z);
    let py = comp.get_y_with_offset(y);
    let pz = comp.get_z_with_offset(x, z);
    if chunk.is_vec_inside(px, py, pz) && material_is_air(world.get(px, py, pz)) {
        let meta = if rand.next_boolean() { 1 } else { 0 };
        world.set(px, py, pz, rail(rail_meta_with_offset(comp.facing, meta)));
        // `EntityMinecartChest.getSizeInventory() == 27`; the drawn slots are discarded (no blocks).
        generate_chest_contents(rand, list, 27, count, |_slot, _content, _size| {});
        true
    } else {
        false
    }
}

/// `Cross.addComponentParts`.
fn cross(
    comp: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    multiple_floors: bool,
) -> bool {
    if comp.is_liquid_in_structure_bounding_box(world, chunk) {
        return false;
    }

    let b = comp.bbox;
    if multiple_floors {
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x + 1,
            b.min_y,
            b.min_z,
            b.max_x - 1,
            b.min_y + 2,
            b.max_z,
            AIR,
            AIR,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x,
            b.min_y,
            b.min_z + 1,
            b.max_x,
            b.min_y + 2,
            b.max_z - 1,
            AIR,
            AIR,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x + 1,
            b.max_y - 2,
            b.min_z,
            b.max_x - 1,
            b.max_y,
            b.max_z,
            AIR,
            AIR,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x,
            b.max_y - 2,
            b.min_z + 1,
            b.max_x,
            b.max_y,
            b.max_z - 1,
            AIR,
            AIR,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x + 1,
            b.min_y + 3,
            b.min_z + 1,
            b.max_x - 1,
            b.min_y + 3,
            b.max_z - 1,
            AIR,
            AIR,
            false,
        );
    } else {
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x + 1,
            b.min_y,
            b.min_z,
            b.max_x - 1,
            b.max_y,
            b.max_z,
            AIR,
            AIR,
            false,
        );
        comp.fill_with_blocks(
            world,
            chunk,
            b.min_x,
            b.min_y,
            b.min_z + 1,
            b.max_x,
            b.max_y,
            b.max_z - 1,
            AIR,
            AIR,
            false,
        );
    }

    comp.fill_with_blocks(
        world,
        chunk,
        b.min_x + 1,
        b.min_y,
        b.min_z + 1,
        b.min_x + 1,
        b.max_y,
        b.min_z + 1,
        PLANKS,
        AIR,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        b.min_x + 1,
        b.min_y,
        b.max_z - 1,
        b.min_x + 1,
        b.max_y,
        b.max_z - 1,
        PLANKS,
        AIR,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        b.max_x - 1,
        b.min_y,
        b.min_z + 1,
        b.max_x - 1,
        b.max_y,
        b.min_z + 1,
        PLANKS,
        AIR,
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        b.max_x - 1,
        b.min_y,
        b.max_z - 1,
        b.max_x - 1,
        b.max_y,
        b.max_z - 1,
        PLANKS,
        AIR,
        false,
    );

    for x in b.min_x..=b.max_x {
        for z in b.min_z..=b.max_z {
            if material_is_air(comp.get_block_state_from_pos(world, x, b.min_y - 1, z, chunk)) {
                comp.set_block_state(world, PLANKS, x, b.min_y - 1, z, chunk);
            }
        }
    }

    true
}

/// `Room.addComponentParts`.
fn room(
    comp: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    linked_rooms: &[BoundingBox],
) -> bool {
    if comp.is_liquid_in_structure_bounding_box(world, chunk) {
        return false;
    }

    let b = comp.bbox;
    comp.fill_with_blocks(
        world, chunk, b.min_x, b.min_y, b.min_z, b.max_x, b.min_y, b.max_z, DIRT, AIR, true,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        b.min_x,
        b.min_y + 1,
        b.min_z,
        b.max_x,
        (b.min_y + 3).min(b.max_y),
        b.max_z,
        AIR,
        AIR,
        false,
    );
    for linked in linked_rooms {
        comp.fill_with_blocks(
            world,
            chunk,
            linked.min_x,
            linked.max_y - 2,
            linked.min_z,
            linked.max_x,
            linked.max_y,
            linked.max_z,
            AIR,
            AIR,
            false,
        );
    }
    comp.randomly_rare_fill_with_blocks(
        world,
        chunk,
        b.min_x,
        b.min_y + 4,
        b.min_z,
        b.max_x,
        b.max_y,
        b.max_z,
        AIR,
        false,
    );
    true
}

/// `Stairs.addComponentParts`.
fn stairs(comp: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) -> bool {
    if comp.is_liquid_in_structure_bounding_box(world, chunk) {
        return false;
    }

    comp.fill_with_blocks(world, chunk, 0, 5, 0, 2, 7, 1, AIR, AIR, false);
    comp.fill_with_blocks(world, chunk, 0, 0, 7, 2, 2, 8, AIR, AIR, false);
    for i in 0..5 {
        let y1 = 5 - i - if i < 4 { 1 } else { 0 };
        comp.fill_with_blocks(world, chunk, 0, y1, 2 + i, 2, 7 - i, 2 + i, AIR, AIR, false);
    }
    true
}

/// `ComponentScatteredFeaturePieces.SwampHut.addComponentParts`.
fn swamp_hut(
    comp: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    has_witch: &mut bool,
) -> bool {
    const PLANKS: u16 = (5 << 4) | 1; // planks, SPRUCE
    const LOG: u16 = 17 << 4;
    const FENCE: u16 = 85 << 4;
    const FLOWER_POT: u16 = 140 << 4; // CONTENTS lives on the tile entity; `getMetaFromState` is 0
    const CRAFTING_TABLE: u16 = 58 << 4;
    const CAULDRON: u16 = 118 << 4;
    let stairs = |meta: i32| ((134u16) << 4) | meta as u16; // spruce_stairs

    comp.fill_with_blocks(world, chunk, 1, 1, 1, 5, 1, 7, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 1, 4, 2, 5, 4, 7, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 2, 1, 0, 4, 1, 0, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 2, 2, 2, 3, 3, 2, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 1, 2, 3, 1, 3, 6, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 5, 2, 3, 5, 3, 6, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 2, 2, 7, 4, 3, 7, PLANKS, PLANKS, false);
    comp.fill_with_blocks(world, chunk, 1, 0, 2, 1, 3, 2, LOG, LOG, false);
    comp.fill_with_blocks(world, chunk, 5, 0, 2, 5, 3, 2, LOG, LOG, false);
    comp.fill_with_blocks(world, chunk, 1, 0, 7, 1, 3, 7, LOG, LOG, false);
    comp.fill_with_blocks(world, chunk, 5, 0, 7, 5, 3, 7, LOG, LOG, false);
    comp.set_block_state(world, FENCE, 2, 3, 2, chunk);
    comp.set_block_state(world, FENCE, 3, 3, 7, chunk);
    comp.set_block_state(world, AIR, 1, 3, 4, chunk);
    comp.set_block_state(world, AIR, 5, 3, 4, chunk);
    comp.set_block_state(world, AIR, 5, 3, 5, chunk);
    comp.set_block_state(world, FLOWER_POT, 1, 3, 5, chunk);
    comp.set_block_state(world, CRAFTING_TABLE, 3, 2, 6, chunk);
    comp.set_block_state(world, CAULDRON, 4, 2, 6, chunk);
    comp.set_block_state(world, FENCE, 1, 2, 1, chunk);
    comp.set_block_state(world, FENCE, 5, 2, 1, chunk);

    let v4 = stairs_meta_with_offset(comp.facing, 3);
    let v5 = stairs_meta_with_offset(comp.facing, 1);
    let v6 = stairs_meta_with_offset(comp.facing, 0);
    let v7 = stairs_meta_with_offset(comp.facing, 2);
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        4,
        1,
        6,
        4,
        1,
        stairs(v4),
        stairs(v4),
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        4,
        2,
        0,
        4,
        7,
        stairs(v6),
        stairs(v6),
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        6,
        4,
        2,
        6,
        4,
        7,
        stairs(v5),
        stairs(v5),
        false,
    );
    comp.fill_with_blocks(
        world,
        chunk,
        0,
        4,
        8,
        6,
        4,
        8,
        stairs(v7),
        stairs(v7),
        false,
    );

    for z in [2, 7] {
        for x in [1, 5] {
            comp.replace_air_and_liquid_downwards(world, chunk, LOG, x, -1, z);
        }
    }

    if !*has_witch {
        let x = comp.get_x_with_offset(2, 5);
        let y = comp.get_y_with_offset(2);
        let z = comp.get_z_with_offset(2, 5);
        if chunk.is_vec_inside(x, y, z) {
            *has_witch = true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::{Piece, PieceKind};

    const STONE: u16 = 1 << 4;

    fn stone_world() -> ChunkWorld {
        let mut world = ChunkWorld::new();
        world.insert_chunk(0, 0, vec![STONE; 65536]);
        world
    }

    fn chunk_box() -> BoundingBox {
        // The 4-arg `StructureBoundingBox` y range is 1..512.
        BoundingBox::new(0, 1, 0, 15, 512, 15)
    }

    fn at(world: &ChunkWorld, x: i32, y: i32, z: i32) -> u16 {
        world.get(x, y, z)
    }

    /// `MapGenStructure.generateStructure`'s box is the chunk shifted by `+8`, y range 1..512.
    #[test]
    fn structure_box_is_offset_by_eight() {
        assert_eq!(structure_box(0, 0), BoundingBox::new(8, 1, 8, 23, 512, 23));
        assert_eq!(
            structure_box(-1, 2),
            BoundingBox::new(-8, 1, 40, 7, 512, 55)
        );
    }

    /// A room on solid stone: dirt floor at `minY`, air `minY+1..minY+3`, and the ellipsoid air fill
    /// at `minY+4` (centre in, corner out).
    #[test]
    fn room_carves_floor_air_and_ellipsoid() {
        let mut world = stone_world();
        // y >= 1: the pass box is a 4-arg `StructureBoundingBox`, whose y range is 1..512.
        let bbox = BoundingBox::new(0, 2, 0, 6, 8, 6);
        let mut piece = Piece {
            kind: PieceKind::MineshaftRoom {
                linked_rooms: Vec::new(),
            },
            coord_base_mode: None,
            component_type: 0,
            bbox,
        };
        let mut rand = JavaRandom::new(0);
        assert!(add_component_parts(
            &mut piece,
            &mut world,
            &mut rand,
            &chunk_box()
        ));

        assert_eq!(at(&world, 2, 2, 2), DIRT); // floor at minY
        assert_eq!(at(&world, 2, 4, 2), AIR); // carved minY+1..minY+3
        assert_eq!(at(&world, 2, 7, 2), AIR); // ellipsoid centre
        assert_eq!(at(&world, 0, 6, 0), STONE); // ellipsoid corner (nx=nz=-1 -> 2 > 1.05)
        assert_eq!(at(&world, 3, 9, 3), STONE); // above the room
    }

    /// `fillWithBlocks` writes `outer` on the box faces and `inner` inside.
    #[test]
    fn fill_with_blocks_shell_and_interior() {
        let mut world = stone_world();
        let comp = Component {
            bbox: BoundingBox::new(0, 0, 0, 2, 2, 2),
            facing: None,
        };
        comp.fill_with_blocks(
            &mut world,
            &chunk_box(),
            1,
            1,
            1,
            3,
            3,
            3,
            DIRT,
            PLANKS,
            false,
        );
        assert_eq!(at(&world, 1, 1, 1), DIRT); // corner -> outer
        assert_eq!(at(&world, 2, 1, 3), DIRT); // face -> outer
        assert_eq!(at(&world, 2, 2, 2), PLANKS); // interior -> inner
    }

    /// With `always`, a position whose current material is air is skipped.
    #[test]
    fn fill_with_blocks_always_skips_air() {
        let mut world = stone_world();
        let comp = Component {
            bbox: BoundingBox::new(0, 0, 0, 3, 0, 2),
            facing: None,
        };
        comp.fill_with_blocks(&mut world, &chunk_box(), 2, 2, 2, 2, 2, 2, DIRT, AIR, true);
        assert_eq!(at(&world, 2, 2, 2), DIRT); // stone -> dirt
        world.set(3, 2, 2, AIR);
        comp.fill_with_blocks(&mut world, &chunk_box(), 3, 2, 2, 3, 2, 2, DIRT, AIR, true);
        assert_eq!(at(&world, 3, 2, 2), AIR); // already air, skipped
    }

    /// A liquid on the box shell makes `isLiquidInStructureBoundingBox` report true.
    #[test]
    fn liquid_shell_detected() {
        let mut world = stone_world();
        let comp = Component {
            bbox: BoundingBox::new(4, 4, 4, 6, 6, 6),
            facing: None,
        };
        assert!(!comp.is_liquid_in_structure_bounding_box(&world, &chunk_box()));
        world.set(3, 4, 5, 8 << 4); // flowing water on the shell
        assert!(comp.is_liquid_in_structure_bounding_box(&world, &chunk_box()));
    }

    /// `getMetadataWithOffset(Blocks.rail, meta)` inverts on a west/east corridor, passes through
    /// on north/south (the oracle's `O`/meta mapping).
    #[test]
    fn rail_meta_offset_inverts_on_east_west() {
        assert_eq!(rail_meta_with_offset(Some(Facing::East), 0), 1);
        assert_eq!(rail_meta_with_offset(Some(Facing::East), 1), 0);
        assert_eq!(rail_meta_with_offset(Some(Facing::West), 0), 1);
        assert_eq!(rail_meta_with_offset(Some(Facing::North), 0), 0);
        assert_eq!(rail_meta_with_offset(Some(Facing::South), 1), 1);
        assert_eq!(rail_meta_with_offset(None, 0), 0);
    }

    /// A north-facing corridor's floor/rail pass writes a rail at relative `(1, 0, z)` where the
    /// block below is solid, with meta `0` (north/south).
    #[test]
    fn corridor_writes_rails_along_the_floor() {
        let mut world = stone_world();
        // Corridor at absolute (4, 4, 4), north-facing, one section (length 4), with rails.
        let bbox = BoundingBox::new(4, 4, 4, 6, 6, 4);
        let mut piece = Piece {
            kind: PieceKind::MineshaftCorridor {
                has_rails: true,
                has_spiders: false,
                spawner_placed: false,
                section_count: 1,
            },
            coord_base_mode: Some(Facing::North),
            component_type: 1,
            bbox,
        };
        let mut rand = JavaRandom::new(1);
        assert!(add_component_parts(
            &mut piece,
            &mut world,
            &mut rand,
            &chunk_box()
        ));
        // At least one rail on the corridor floor line (relative x=1 -> world x=5, y=minY=4).
        let rails = (0..16).any(|z| at(&world, 5, 4, z) >> 4 == 66);
        assert!(rails, "expected at least one rail on the corridor floor");
    }

    /// A chunk filled with stone up to `top` (air above), in chunk (0,0).
    fn low_world(top: i32) -> ChunkWorld {
        let mut world = ChunkWorld::new();
        let mut blocks = vec![0u16; 65536];
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..=top {
                    blocks[chunk_index(x, y, z)] = STONE;
                }
            }
        }
        world.insert_chunk(0, 0, blocks);
        world
    }

    /// `getMetadataWithOffset` for the stair blocks (south swaps 2/3, west/east rotate).
    #[test]
    fn stairs_meta_offset_matches_vanilla() {
        use Facing::*;
        assert_eq!(stairs_meta_with_offset(Some(South), 2), 3);
        assert_eq!(stairs_meta_with_offset(Some(South), 3), 2);
        assert_eq!(stairs_meta_with_offset(Some(South), 0), 0);
        assert_eq!(stairs_meta_with_offset(Some(West), 0), 2);
        assert_eq!(stairs_meta_with_offset(Some(West), 1), 3);
        assert_eq!(stairs_meta_with_offset(Some(West), 2), 0);
        assert_eq!(stairs_meta_with_offset(Some(East), 0), 2);
        assert_eq!(stairs_meta_with_offset(Some(East), 3), 0);
        assert_eq!(stairs_meta_with_offset(Some(North), 2), 2);
        assert_eq!(stairs_meta_with_offset(None, 1), 1);
    }

    /// `replaceAirAndLiquidDownwards` fills air downwards until the first solid block.
    #[test]
    fn replace_air_and_liquid_downwards_fills_air_to_ground() {
        let mut world = low_world(50);
        for y in 51..=55 {
            world.set(2, y, 2, 0);
        }
        let comp = Component {
            bbox: BoundingBox::new(0, 64, 0, 8, 70, 6),
            facing: None,
        };
        comp.replace_air_and_liquid_downwards(&mut world, &chunk_box(), 17 << 4, 2, 55, 2);
        assert_eq!(at(&world, 2, 55, 2), 17 << 4);
        assert_eq!(at(&world, 2, 51, 2), 17 << 4);
        assert_eq!(at(&world, 2, 50, 2), STONE);
    }

    /// `func_74935_a` averages `max(topSolid, averageGroundLevel = 64)` over the piece/box
    /// intersection and offsets the piece to that height.
    #[test]
    fn temple_height_averages_top_and_ground_level() {
        let world = low_world(70);
        let mut piece = Piece {
            kind: PieceKind::TempleSwampHut {
                size: [7, 7, 9],
                h_pos: -1,
                witch: false,
            },
            coord_base_mode: Some(Facing::East),
            component_type: 0,
            bbox: BoundingBox::new(0, 64, 0, 8, 70, 6),
        };
        assert!(temple_adjust_height(&mut piece, &world, &chunk_box()));
        // top = 71 for every column, so HPos = 71 and the box shifts down by 7 from 64.
        assert_eq!(piece.bbox.min_y, 71);
        assert!(matches!(
            piece.kind,
            PieceKind::TempleSwampHut { h_pos: 71, .. }
        ));

        // An empty intersection drops the piece.
        let world = low_world(50);
        let mut piece = Piece {
            kind: PieceKind::TempleSwampHut {
                size: [7, 7, 9],
                h_pos: -1,
                witch: false,
            },
            coord_base_mode: Some(Facing::East),
            component_type: 0,
            bbox: BoundingBox::new(100, 64, 100, 108, 70, 106),
        };
        let outside = BoundingBox::new(0, 1, 0, 15, 512, 15);
        assert!(!temple_adjust_height(&mut piece, &world, &outside));
    }

    /// The swamp hut (east-facing) writes spruce planks/stairs at the vanilla offsets.
    #[test]
    fn swamp_hut_places_planks_and_roof_stairs() {
        let mut world = stone_world();
        let mut piece = Piece {
            kind: PieceKind::TempleSwampHut {
                size: [7, 7, 9],
                h_pos: 64, // skip the height adjustment
                witch: false,
            },
            coord_base_mode: Some(Facing::East),
            component_type: 0,
            bbox: BoundingBox::new(0, 64, 0, 8, 70, 6),
        };
        let chunk = BoundingBox::new(0, 1, 0, 15, 512, 15);
        let mut rand = JavaRandom::new(0);
        assert!(add_component_parts(
            &mut piece, &mut world, &mut rand, &chunk
        ));

        // Relative (2, 1, 0) -> world (minX + 0, minY + 1, minZ + 2) = (0, 65, 2).
        assert_eq!(at(&world, 0, 65, 2), (5 << 4) | 1);
        // Relative (0, 4, 1) -> world (minX + 1, minY + 4, minZ + 0) = (1, 68, 0): roof stairs.
        assert_eq!(at(&world, 1, 68, 0) >> 4, 134);
        // Relative (2, 3, 2) -> world (minX + 2, minY + 3, minZ + 2) = (2, 67, 2): a fence post.
        assert_eq!(at(&world, 2, 67, 2) >> 4, 85);
    }
}
