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
        PieceKind::TempleDesertPyramid {
            size,
            h_pos,
            chests,
        } => vec![
            ("Depth", size[2].to_string()),
            ("HPos", h_pos.to_string()),
            ("Height", size[1].to_string()),
            ("Width", size[0].to_string()),
            ("hasPlacedChest0", bit(chests[0]).to_string()),
            ("hasPlacedChest1", bit(chests[1]).to_string()),
            ("hasPlacedChest2", bit(chests[2]).to_string()),
            ("hasPlacedChest3", bit(chests[3]).to_string()),
        ],
        PieceKind::TempleJunglePyramid {
            size,
            h_pos,
            main_chest,
            hidden_chest,
            trap1,
            trap2,
        } => vec![
            ("Depth", size[2].to_string()),
            ("HPos", h_pos.to_string()),
            ("Height", size[1].to_string()),
            ("Width", size[0].to_string()),
            ("placedHiddenChest", bit(*hidden_chest).to_string()),
            ("placedMainChest", bit(*main_chest).to_string()),
            ("placedTrap1", bit(*trap1).to_string()),
            ("placedTrap2", bit(*trap2).to_string()),
        ],
        PieceKind::TempleSwampHut { size, h_pos, witch } => vec![
            ("Depth", size[2].to_string()),
            ("HPos", h_pos.to_string()),
            ("Height", size[1].to_string()),
            ("Width", size[0].to_string()),
            ("Witch", bit(*witch).to_string()),
        ],
        PieceKind::MonumentBuilding => Vec::new(),
        // Nether fortress pieces serialize `Chest` (corridors), `Seed` (End) or `Mob` (Throne).
        PieceKind::Nether {
            kind,
            chest,
            seed,
            has_spawner,
        } => match kind {
            mc_worldgen::structure::NetherKind::Corridor
            | mc_worldgen::structure::NetherKind::Corridor2 => {
                vec![("Chest", bit(*chest).to_string())]
            }
            mc_worldgen::structure::NetherKind::End => vec![("Seed", seed.to_string())],
            mc_worldgen::structure::NetherKind::Throne => {
                vec![("Mob", bit(*has_spawner).to_string())]
            }
            _ => Vec::new(),
        },
        PieceKind::Village(v) => {
            let mut pairs: Vec<(&str, String)> = vec![
                ("HPos", v.h_pos.to_string()),
                ("VCount", v.v_count.to_string()),
                ("Desert", bit(v.desert).to_string()),
            ];
            match v.kind {
                mc_worldgen::structure::VillageKind::Field1 => {
                    pairs.push(("CA", v.crops[0].to_string()));
                    pairs.push(("CB", v.crops[1].to_string()));
                    pairs.push(("CC", v.crops[2].to_string()));
                    pairs.push(("CD", v.crops[3].to_string()));
                }
                mc_worldgen::structure::VillageKind::Field2 => {
                    pairs.push(("CA", v.crops[0].to_string()));
                    pairs.push(("CB", v.crops[1].to_string()));
                }
                mc_worldgen::structure::VillageKind::House2 => {
                    pairs.push(("Chest", bit(v.chest).to_string()));
                }
                mc_worldgen::structure::VillageKind::House4Garden => {
                    pairs.push(("Terrace", bit(v.terrace).to_string()));
                }
                mc_worldgen::structure::VillageKind::WoodHut => {
                    pairs.push(("T", v.table.to_string()));
                    pairs.push(("C", bit(v.tall).to_string()));
                }
                mc_worldgen::structure::VillageKind::Path => {
                    pairs.push(("Length", v.length.to_string()));
                }
                _ => {}
            }
            pairs
        }
        PieceKind::Stronghold(s) => {
            let mut pairs: Vec<(&str, String)> =
                vec![("EntryDoor", format!("\"{}\"", s.door.name()))];
            match s.kind {
                mc_worldgen::structure::StrongholdKind::Corridor => {
                    pairs.push(("Steps", s.steps.to_string()));
                }
                mc_worldgen::structure::StrongholdKind::ChestCorridor => {
                    pairs.push(("Chest", bit(s.chest).to_string()));
                }
                mc_worldgen::structure::StrongholdKind::Crossing => {
                    pairs.push(("leftLow", bit(s.left_low).to_string()));
                    pairs.push(("leftHigh", bit(s.left_high).to_string()));
                    pairs.push(("rightLow", bit(s.right_low).to_string()));
                    pairs.push(("rightHigh", bit(s.right_high).to_string()));
                }
                mc_worldgen::structure::StrongholdKind::Library => {
                    pairs.push(("Tall", bit(s.tall).to_string()));
                }
                mc_worldgen::structure::StrongholdKind::PortalRoom => {
                    pairs.push(("Mob", bit(s.has_spawner).to_string()));
                }
                mc_worldgen::structure::StrongholdKind::RoomCrossing => {
                    pairs.push(("Type", s.room_type.to_string()));
                }
                mc_worldgen::structure::StrongholdKind::Stairs
                | mc_worldgen::structure::StrongholdKind::Stairs2 => {
                    pairs.push(("Source", bit(s.source).to_string()));
                }
                mc_worldgen::structure::StrongholdKind::Straight => {
                    pairs.push(("Left", bit(s.expands_x).to_string()));
                    pairs.push(("Right", bit(s.expands_z).to_string()));
                }
                _ => {}
            }
            pairs
        }
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
    let mut nether = mc_worldgen::structure::netherbridge::NetherBridge::new();
    let mut out = String::new();
    for &(cx, cz) in &chunks {
        let chunk = gen.generate_chunk(cx, cz, GenOptions::vanilla());
        // Fortresses run in the nether, so they are a standalone generator.
        nether.generate(seed, cx, cz);
        out.push_str(&format!("chunk {cx} {cz}\n"));
        for start in &chunk.structures {
            dump(&mut out, start);
        }
        for start in &nether.starts_intersecting(cx, cz) {
            dump(&mut out, start);
        }
    }

    print!("{out}");
}
