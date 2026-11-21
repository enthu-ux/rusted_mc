//! Prints the structure starts a chunk's generation registers (the `provideChunk` half), in the
//! same format as `oracle/java/src/StructuresOracle.java`:
//!
//! Usage: structures_probe <seed> <cx> <cz> [<cx> <cz> ...]
//! Per target chunk:
//!   chunk <cx> <cz>
//!   start <id> ChunkX <x> ChunkZ <z> BB <6 ints> pieces <n>
//!   piece <id> BB <6 ints> GD <gd> O <o> [<key>=<value> ...]
//!
//! Only starts whose box meets the chunk are printed, sorted by start chunk. Blocks are written
//! later, in `populate` (not ported yet), so nothing here touches the chunk primer.

use mc_worldgen::structure::{Facing, Piece, PieceKind, StructureStart};
use mc_worldgen::terrain::{GenOptions, TerrainGen};

fn bb(p: &mc_worldgen::structure::BoundingBox) -> String {
    format!(
        "{} {} {} {} {} {}",
        p.min_x, p.min_y, p.min_z, p.max_x, p.max_y, p.max_z
    )
}

fn bit(v: bool) -> i32 {
    if v {
        1
    } else {
        0
    }
}

fn orientation(facing: Option<Facing>) -> i32 {
    facing.map_or(-1, Facing::horizontal_index)
}

/// The piece's extra NBT keys (`writeStructureToNBT`), ASCII-sorted like the oracle's `TreeSet`.
fn extras(piece: &Piece) -> String {
    let mut pairs: Vec<(&str, String)> = match &piece.kind {
        PieceKind::MineshaftCorridor {
            has_rails,
            has_spiders,
            spawner_placed,
            section_count,
        } => vec![
            ("Num", section_count.to_string()),
            ("hr", bit(*has_rails).to_string()),
            ("hps", bit(*spawner_placed).to_string()),
            ("sc", bit(*has_spiders).to_string()),
        ],
        // Cross serializes its own direction as `D`; its `coordBaseMode` stays null (`O` -1).
        PieceKind::MineshaftCross {
            direction,
            multiple_floors,
        } => vec![
            ("D", direction.horizontal_index().to_string()),
            ("tf", bit(*multiple_floors).to_string()),
        ],
        // `Room` serializes its linked boxes as "Entrances": six ints per box, ';'-separated.
        PieceKind::MineshaftRoom { linked_rooms } => {
            let joined = linked_rooms
                .iter()
                .map(|b| {
                    format!(
                        "{} {} {} {} {} {}",
                        b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            vec![("Entrances", joined)]
        }
        PieceKind::MineshaftStairs => Vec::new(),
    };

    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .into_iter()
        .map(|(key, value)| format!(" {key}={value}"))
        .collect()
}

fn dump(out: &mut String, start: &StructureStart) {
    out.push_str(&format!(
        "start {} ChunkX {} ChunkZ {} BB {} pieces {}\n",
        start.id,
        start.chunk_x,
        start.chunk_z,
        bb(&start.bbox),
        start.pieces.len()
    ));
    for piece in &start.pieces {
        out.push_str(&format!(
            "piece {} BB {} GD {} O {}{}\n",
            piece.kind.id(),
            bb(&piece.bbox),
            piece.component_type,
            orientation(piece.coord_base_mode),
            extras(piece)
        ));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args[1].parse().expect("seed");
    let mut chunks: Vec<(i32, i32)> = Vec::new();
    let mut i = 2;
    while i + 1 < args.len() {
        chunks.push((
            args[i].parse().expect("cx"),
            args[i + 1].parse().expect("cz"),
        ));
        i += 2;
    }

    let mut gen = TerrainGen::new(seed);
    let mut out = String::new();
    for &(cx, cz) in &chunks {
        let chunk = gen.generate_chunk(cx, cz, GenOptions::vanilla());
        out.push_str(&format!("chunk {cx} {cz}\n"));
        for start in &chunk.structures {
            dump(&mut out, start);
        }
    }

    print!("{out}");
}
