//! `StructureOceanMonumentPieces` block placement.
//!
//! `MonumentBuilding` is the only serialized piece, but its `addComponentParts` walks a **room
//! graph** it builds in its constructor (`func_175836_a` + the fit helpers). The graph is rebuilt
//! here deterministically from `(worldSeed, startChunkX, startChunkZ)` by replaying
//! `StartMonument.func_175789_b` (re-seed, two `nextLong`, the orientation draw) and then the
//! building constructor's `Random` stream. It is placed through the same `+8` populate passes as the
//! other structures (see `structure/placement.rs`).
//!
//! Ported classes: `StructureOceanMonumentPieces.{MonumentBuilding, Piece, RoomDefinition}` and the
//! room kinds `EntryRoom`, `MonumentCoreRoom`, `SimpleRoom`, `SimpleTopRoom`, `DoubleX/XY/Y/YZ/ZRoom`,
//! `WingRoom`, `Penthouse`, plus the seven `MonumentRoomFitHelper`s.
//!
//! Block states come from `StructureOceanMonumentPieces.Piece`'s static fields (rough/bricks/dark
//! prismarine, sea lantern, water) plus `gold_block` and wet `sponge`.

use mc_core::random::JavaRandom;

use super::placement::{ChunkWorld, Component};
use super::{BoundingBox, Facing};
use crate::chunk::material_is_air;

const SEA_LEVEL: i32 = 63;

// `EnumFacing.getIndex()` order.
const DOWN: usize = 0;
const UP: usize = 1;
const NORTH: usize = 2;
const SOUTH: usize = 3;
const WEST: usize = 4;
const EAST: usize = 5;

const DIR_OFFSETS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

/// `EnumFacing.getOpposite().getIndex()`.
fn opposite_index(i: usize) -> usize {
    [1, 0, 3, 2, 5, 4][i]
}

// `StructureOceanMonumentPieces.Piece`'s static block states.
const ROUGH: u16 = 168 << 4;
const BRICKS: u16 = (168 << 4) | 1;
const DARK: u16 = (168 << 4) | 2;
const SEA_LANTERN: u16 = 169 << 4;
const WATER: u16 = 9 << 4;
const GOLD_BLOCK: u16 = 41 << 4;
const SPONGE: u16 = (19 << 4) | 1;
const AIR: u16 = 0;

/// `StructureOceanMonumentPieces.func_175820_a(x, y, z)`: the room-definition grid id.
fn grid_id(x: i32, y: i32, z: i32) -> i32 {
    y * 25 + z * 5 + x
}

#[derive(Clone, Copy)]
struct RoomDef {
    id: i32,
    conn: [Option<usize>; 6],
    open: [bool; 6],
    chosen: bool,
    is_end: bool,
    visited: i32,
}

impl RoomDef {
    fn new(id: i32) -> RoomDef {
        RoomDef {
            id,
            conn: [None; 6],
            open: [false; 6],
            chosen: false,
            is_end: false,
            visited: 0,
        }
    }

    fn refresh_open(&mut self) {
        for i in 0..6 {
            self.open[i] = self.conn[i].is_some();
        }
    }
}

fn connect(rooms: &mut [RoomDef], a: usize, dir: usize, b: usize) {
    rooms[a].conn[dir] = Some(b);
    rooms[b].conn[opposite_index(dir)] = Some(a);
}

/// `RoomDefinition.func_175959_a`: true when `idx` reaches the entry room through open edges.
fn connected(rooms: &mut [RoomDef], idx: usize, gen: i32) -> bool {
    if rooms[idx].is_end {
        return true;
    }
    rooms[idx].visited = gen;
    for d in 0..6 {
        if let Some(nb) = rooms[idx].conn[d] {
            if rooms[idx].open[d] && rooms[nb].visited != gen && connected(rooms, nb, gen) {
                return true;
            }
        }
    }
    false
}

struct RoomGraph {
    rooms: Vec<RoomDef>,
    list: Vec<usize>,
    entry: usize,
    core: usize,
}

/// `MonumentBuilding.func_175836_a` + `Collections.shuffle` + the edge-removal loop.
fn build_room_graph(rand: &mut JavaRandom) -> RoomGraph {
    let mut rooms: Vec<RoomDef> = Vec::new();
    let mut grid: Vec<Option<usize>> = vec![None; 75];

    let add = |x: i32, y: i32, z: i32, rooms: &mut Vec<RoomDef>, grid: &mut Vec<Option<usize>>| {
        let id = grid_id(x, y, z) as usize;
        let idx = rooms.len();
        rooms.push(RoomDef::new(id as i32));
        grid[id] = Some(idx);
    };
    for x in 0..5 {
        for z in 0..4 {
            add(x, 0, z, &mut rooms, &mut grid);
        }
    }
    for x in 0..5 {
        for z in 0..4 {
            add(x, 1, z, &mut rooms, &mut grid);
        }
    }
    for x in 1..4 {
        for z in 0..2 {
            add(x, 2, z, &mut rooms, &mut grid);
        }
    }

    for x in 0..5 {
        for z in 0..5 {
            for y in 0..3 {
                let id = grid_id(x, y, z) as usize;
                let Some(a) = grid[id] else { continue };
                for dir in 0..6 {
                    let (dx, dy, dz) = DIR_OFFSETS[dir];
                    let nx = x + dx;
                    let ny = y + dy;
                    let nz = z + dz;
                    if !(0..5).contains(&nx) || !(0..5).contains(&nz) || !(0..3).contains(&ny) {
                        continue;
                    }
                    let Some(b) = grid[grid_id(nx, ny, nz) as usize] else {
                        continue;
                    };
                    if nz != z {
                        connect(&mut rooms, a, opposite_index(dir), b);
                    } else {
                        connect(&mut rooms, a, dir, b);
                    }
                }
            }
        }
    }

    let entry = grid[grid_id(2, 0, 0) as usize].unwrap();
    let extra1 = rooms.len();
    rooms.push(RoomDef::new(1003));
    let extra2 = rooms.len();
    rooms.push(RoomDef::new(1001));
    let extra3 = rooms.len();
    rooms.push(RoomDef::new(1002));
    let h = grid[grid_id(2, 2, 0) as usize].unwrap();
    connect(&mut rooms, h, UP, extra1);
    let i = grid[grid_id(0, 1, 0) as usize].unwrap();
    connect(&mut rooms, i, SOUTH, extra2);
    let j = grid[grid_id(4, 1, 0) as usize].unwrap();
    connect(&mut rooms, j, SOUTH, extra3);
    rooms[extra1].chosen = true;
    rooms[extra2].chosen = true;
    rooms[extra3].chosen = true;
    rooms[entry].chosen = true;
    rooms[entry].is_end = true;

    let core = grid[grid_id(rand.next_int_bound(4), 0, 2) as usize].unwrap();
    rooms[core].chosen = true;
    let e = rooms[core].conn[EAST].unwrap();
    let n = rooms[core].conn[NORTH].unwrap();
    let u = rooms[core].conn[UP].unwrap();
    rooms[e].chosen = true;
    rooms[n].chosen = true;
    let en = rooms[e].conn[NORTH].unwrap();
    rooms[en].chosen = true;
    rooms[u].chosen = true;
    let eu = rooms[e].conn[UP].unwrap();
    rooms[eu].chosen = true;
    let nu = rooms[n].conn[UP].unwrap();
    rooms[nu].chosen = true;
    let enu = rooms[en].conn[UP].unwrap();
    rooms[enu].chosen = true;

    let mut list: Vec<usize> = Vec::new();
    for slot in &grid {
        if let Some(idx) = *slot {
            rooms[idx].refresh_open();
            list.push(idx);
        }
    }
    rooms[extra1].refresh_open();

    // `Collections.shuffle(list, rand)`.
    let len = list.len();
    for i in (1..len).rev() {
        let j = rand.next_int_bound((i + 1) as i32) as usize;
        list.swap(i, j);
    }

    let mut gen = 1;
    for k in 0..list.len() {
        let room = list[k];
        let mut count = 0;
        let mut attempts = 0;
        while count < 2 && attempts < 5 {
            attempts += 1;
            let d = rand.next_int_bound(6) as usize;
            if !rooms[room].open[d] {
                continue;
            }
            let nb = rooms[room].conn[d].unwrap();
            let opp = opposite_index(d);
            rooms[room].open[d] = false;
            rooms[nb].open[opp] = false;
            let g1 = gen;
            gen += 1;
            let g2 = gen;
            gen += 1;
            if connected(&mut rooms, room, g1) && connected(&mut rooms, nb, g2) {
                count += 1;
            } else {
                rooms[room].open[d] = true;
                rooms[nb].open[opp] = true;
            }
        }
    }

    list.push(extra1);
    list.push(extra2);
    list.push(extra3);

    RoomGraph {
        rooms,
        list,
        entry,
        core,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RoomKind {
    Entry,
    Core,
    DoubleX,
    DoubleXY,
    DoubleY,
    DoubleYZ,
    DoubleZ,
    Simple,
    SimpleTop,
    Wing,
    Penthouse,
}

struct RoomPiece {
    kind: RoomKind,
    comp: Component,
    room: usize,
    variant: i32,
    wing: i32,
}

/// `StructureOceanMonumentPieces.Piece(int, EnumFacing, RoomDefinition, int, int, int)`'s bbox.
fn room_piece_box(facing: Facing, id: i32, size_x: i32, size_y: i32, size_z: i32) -> BoundingBox {
    let gx = id % 5;
    let gz = id / 5 % 5;
    let gy = id / 25;
    let mut b = if facing == Facing::North || facing == Facing::South {
        BoundingBox::new(0, 0, 0, size_x * 8 - 1, size_y * 4 - 1, size_z * 8 - 1)
    } else {
        BoundingBox::new(0, 0, 0, size_z * 8 - 1, size_y * 4 - 1, size_x * 8 - 1)
    };
    match facing {
        Facing::North => b.offset(gx * 8, gy * 4, -(gz + size_z) * 8 + 1),
        Facing::South => b.offset(gx * 8, gy * 4, gz * 8),
        Facing::West => b.offset(-(gz + size_z) * 8 + 1, gy * 4, gx * 8),
        Facing::East => b.offset(gz * 8, gy * 4, gx * 8),
    }
    b
}

fn piece_box(facing: Facing, id: i32, size: (i32, i32, i32)) -> BoundingBox {
    room_piece_box(facing, id, size.0, size.1, size.2)
}

fn box_from_corners(x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> BoundingBox {
    BoundingBox::new(
        x1.min(x2),
        y1.min(y2),
        z1.min(z2),
        x1.max(x2),
        y1.max(y2),
        z1.max(z2),
    )
}

fn push_room(
    pieces: &mut Vec<RoomPiece>,
    facing: Facing,
    rooms: &[RoomDef],
    idx: usize,
    kind: RoomKind,
    size: (i32, i32, i32),
    variant: i32,
) {
    pieces.push(RoomPiece {
        kind,
        comp: Component {
            bbox: piece_box(facing, rooms[idx].id, size),
            facing: Some(facing),
        },
        room: idx,
        variant,
        wing: 0,
    });
}

fn chosen_all(rooms: &mut [RoomDef], ids: &[usize]) {
    for &id in ids {
        rooms[id].chosen = true;
    }
}

/// The constructor's fit loop + the wing/penthouse creation.
fn build_pieces(
    rand: &mut JavaRandom,
    rooms: &mut [RoomDef],
    entry: usize,
    core: usize,
    list: &[usize],
    facing: Facing,
    building: &Component,
) -> Vec<RoomPiece> {
    let mut pieces = Vec::new();
    push_room(
        &mut pieces,
        facing,
        rooms,
        entry,
        RoomKind::Entry,
        (1, 1, 1),
        0,
    );
    push_room(
        &mut pieces,
        facing,
        rooms,
        core,
        RoomKind::Core,
        (2, 2, 2),
        0,
    );

    for &idx in list {
        if rooms[idx].chosen || rooms[idx].id >= 75 {
            continue;
        }

        // XYDoubleRoomFitHelper
        {
            let r = rooms[idx];
            if r.open[EAST]
                && !rooms[r.conn[EAST].unwrap()].chosen
                && r.open[UP]
                && !rooms[r.conn[UP].unwrap()].chosen
            {
                let e = r.conn[EAST].unwrap();
                let u = r.conn[UP].unwrap();
                if rooms[e].open[UP] && !rooms[rooms[e].conn[UP].unwrap()].chosen {
                    let eu = rooms[e].conn[UP].unwrap();
                    chosen_all(rooms, &[idx, e, u, eu]);
                    push_room(
                        &mut pieces,
                        facing,
                        rooms,
                        idx,
                        RoomKind::DoubleXY,
                        (2, 2, 1),
                        0,
                    );
                    continue;
                }
            }
        }
        // YZDoubleRoomFitHelper
        {
            let r = rooms[idx];
            if r.open[NORTH]
                && !rooms[r.conn[NORTH].unwrap()].chosen
                && r.open[UP]
                && !rooms[r.conn[UP].unwrap()].chosen
            {
                let n = r.conn[NORTH].unwrap();
                let u = r.conn[UP].unwrap();
                if rooms[n].open[UP] && !rooms[rooms[n].conn[UP].unwrap()].chosen {
                    let nu = rooms[n].conn[UP].unwrap();
                    chosen_all(rooms, &[idx, n, u, nu]);
                    push_room(
                        &mut pieces,
                        facing,
                        rooms,
                        idx,
                        RoomKind::DoubleYZ,
                        (1, 2, 2),
                        0,
                    );
                    continue;
                }
            }
        }
        // ZDoubleRoomFitHelper
        {
            let r = rooms[idx];
            if r.open[NORTH] && !rooms[r.conn[NORTH].unwrap()].chosen {
                let mut base = idx;
                if !rooms[idx].open[NORTH] || rooms[rooms[idx].conn[NORTH].unwrap()].chosen {
                    base = rooms[idx].conn[SOUTH].unwrap();
                }
                let nb = rooms[base].conn[NORTH].unwrap();
                chosen_all(rooms, &[base, nb]);
                push_room(
                    &mut pieces,
                    facing,
                    rooms,
                    base,
                    RoomKind::DoubleZ,
                    (1, 1, 2),
                    0,
                );
                continue;
            }
        }
        // XDoubleRoomFitHelper
        {
            let r = rooms[idx];
            if r.open[EAST] && !rooms[r.conn[EAST].unwrap()].chosen {
                let e = r.conn[EAST].unwrap();
                chosen_all(rooms, &[idx, e]);
                push_room(
                    &mut pieces,
                    facing,
                    rooms,
                    idx,
                    RoomKind::DoubleX,
                    (2, 1, 1),
                    0,
                );
                continue;
            }
        }
        // YDoubleRoomFitHelper
        {
            let r = rooms[idx];
            if r.open[UP] && !rooms[r.conn[UP].unwrap()].chosen {
                let u = r.conn[UP].unwrap();
                chosen_all(rooms, &[idx, u]);
                push_room(
                    &mut pieces,
                    facing,
                    rooms,
                    idx,
                    RoomKind::DoubleY,
                    (1, 2, 1),
                    0,
                );
                continue;
            }
        }
        // FitSimpleRoomTopHelper
        {
            let r = rooms[idx];
            if !r.open[WEST] && !r.open[EAST] && !r.open[NORTH] && !r.open[SOUTH] && !r.open[UP] {
                rooms[idx].chosen = true;
                push_room(
                    &mut pieces,
                    facing,
                    rooms,
                    idx,
                    RoomKind::SimpleTop,
                    (1, 1, 1),
                    0,
                );
                continue;
            }
        }
        // FitSimpleRoomHelper
        {
            rooms[idx].chosen = true;
            let variant = rand.next_int_bound(3);
            push_room(
                &mut pieces,
                facing,
                rooms,
                idx,
                RoomKind::Simple,
                (1, 1, 1),
                variant,
            );
        }
    }

    // `MonumentBuilding` offsets every room piece (entry/core/fitted) by
    // `(getXWithOffset(9, 22), boundingBox.minY, getZWithOffset(9, 22))` before adding the wings.
    let ox = building.get_x_with_offset(9, 22);
    let oy = building.bbox.min_y;
    let oz = building.get_z_with_offset(9, 22);
    for p in &mut pieces {
        p.comp.bbox.offset(ox, oy, oz);
    }

    let bx = |a, b| building.get_x_with_offset(a, b);
    let bz = |a, b| building.get_z_with_offset(a, b);
    let by = |a| building.get_y_with_offset(a);
    let wing1 = box_from_corners(bx(1, 1), by(1), bz(1, 1), bx(23, 21), by(8), bz(23, 21));
    let wing2 = box_from_corners(bx(34, 1), by(1), bz(34, 1), bx(56, 21), by(8), bz(56, 21));
    let pent = box_from_corners(
        bx(22, 22),
        by(13),
        bz(22, 22),
        bx(35, 35),
        by(17),
        bz(35, 35),
    );
    let seed = rand.next_int();
    pieces.push(RoomPiece {
        kind: RoomKind::Wing,
        comp: Component {
            bbox: wing1,
            facing: Some(facing),
        },
        room: 0,
        variant: 0,
        wing: seed & 1,
    });
    pieces.push(RoomPiece {
        kind: RoomKind::Wing,
        comp: Component {
            bbox: wing2,
            facing: Some(facing),
        },
        room: 0,
        variant: 0,
        wing: (seed + 1) & 1,
    });
    pieces.push(RoomPiece {
        kind: RoomKind::Penthouse,
        comp: Component {
            bbox: pent,
            facing: Some(facing),
        },
        room: 0,
        variant: 0,
        wing: 0,
    });
    pieces
}

/// `Piece.func_181655_a`: fill air above sea level, water below.
fn fill_water(
    c: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    x1: i32,
    y1: i32,
    z1: i32,
    x2: i32,
    y2: i32,
    z2: i32,
    always: bool,
) {
    for y in y1..=y2 {
        for x in x1..=x2 {
            for z in z1..=z2 {
                if always {
                    let existing = c.get_block_state_from_pos(world, x, y, z, chunk);
                    if material_is_air(existing) {
                        continue;
                    }
                }
                if c.get_y_with_offset(y) >= SEA_LEVEL {
                    c.set_block_state(world, AIR, x, y, z, chunk);
                } else {
                    c.set_block_state(world, WATER, x, y, z, chunk);
                }
            }
        }
    }
}

/// `Piece.func_175819_a`: replace water with `state`.
#[allow(clippy::too_many_arguments)]
fn replace_water(
    c: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    x1: i32,
    y1: i32,
    z1: i32,
    x2: i32,
    y2: i32,
    z2: i32,
    state: u16,
) {
    for y in y1..=y2 {
        for x in x1..=x2 {
            for z in z1..=z2 {
                if c.get_block_state_from_pos(world, x, y, z, chunk) == WATER {
                    c.set_block_state(world, state, x, y, z, chunk);
                }
            }
        }
    }
}

/// `Piece.func_175818_a`: does the piece's X/Z span meet the pass box?
fn intersects(c: &Component, chunk: &BoundingBox, x1: i32, z1: i32, x2: i32, z2: i32) -> bool {
    let ax = c.get_x_with_offset(x1, z1);
    let az = c.get_z_with_offset(x1, z1);
    let bx = c.get_x_with_offset(x2, z2);
    let bz = c.get_z_with_offset(x2, z2);
    chunk.intersects_with_xz(ax.min(bx), az.min(bz), ax.max(bx), az.max(bz))
}

/// `Piece.func_175821_a`: the room floor/roof pattern.
fn base_floor(
    c: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    x: i32,
    z: i32,
    open_down: bool,
) {
    if open_down {
        c.fill_with_blocks(world, chunk, x, 0, z, x + 2, 0, z + 7, ROUGH, ROUGH, false);
        c.fill_with_blocks(
            world,
            chunk,
            x + 5,
            0,
            z,
            x + 7,
            0,
            z + 7,
            ROUGH,
            ROUGH,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 3,
            0,
            z,
            x + 4,
            0,
            z + 2,
            ROUGH,
            ROUGH,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 3,
            0,
            z + 5,
            x + 4,
            0,
            z + 7,
            ROUGH,
            ROUGH,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 3,
            0,
            z + 2,
            x + 4,
            0,
            z + 2,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 3,
            0,
            z + 5,
            x + 4,
            0,
            z + 5,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 2,
            0,
            z + 3,
            x + 2,
            0,
            z + 4,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            x + 5,
            0,
            z + 3,
            x + 5,
            0,
            z + 4,
            BRICKS,
            BRICKS,
            false,
        );
    } else {
        c.fill_with_blocks(world, chunk, x, 0, z, x + 7, 0, z + 7, ROUGH, ROUGH, false);
    }
}

/// `Piece.func_175840_a`.
fn monument_wing(
    c: &Component,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    flip: bool,
    base: i32,
) {
    if !intersects(c, chunk, base, 0, base + 23, 20) {
        return;
    }
    c.fill_with_blocks(
        world,
        chunk,
        base,
        0,
        0,
        base + 24,
        0,
        20,
        ROUGH,
        ROUGH,
        false,
    );
    fill_water(c, world, chunk, base, 1, 0, base + 24, 10, 20, false);
    for i in 0..4 {
        c.fill_with_blocks(
            world,
            chunk,
            base + i,
            i + 1,
            i,
            base + i,
            i + 1,
            20,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            base + i + 7,
            i + 5,
            i + 7,
            base + i + 7,
            i + 5,
            20,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            base + 17 - i,
            i + 5,
            i + 7,
            base + 17 - i,
            i + 5,
            20,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            base + 24 - i,
            i + 1,
            i,
            base + 24 - i,
            i + 1,
            20,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            base + i + 1,
            i + 1,
            i,
            base + 23 - i,
            i + 1,
            i,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            base + i + 8,
            i + 5,
            i + 7,
            base + 16 - i,
            i + 5,
            i + 7,
            BRICKS,
            BRICKS,
            false,
        );
    }
    c.fill_with_blocks(
        world,
        chunk,
        base + 4,
        4,
        4,
        base + 6,
        4,
        20,
        ROUGH,
        ROUGH,
        false,
    );
    c.fill_with_blocks(
        world,
        chunk,
        base + 7,
        4,
        4,
        base + 17,
        4,
        6,
        ROUGH,
        ROUGH,
        false,
    );
    c.fill_with_blocks(
        world,
        chunk,
        base + 18,
        4,
        4,
        base + 20,
        4,
        20,
        ROUGH,
        ROUGH,
        false,
    );
    c.fill_with_blocks(
        world,
        chunk,
        base + 11,
        8,
        11,
        base + 13,
        8,
        20,
        ROUGH,
        ROUGH,
        false,
    );
    c.set_block_state(world, BRICKS, base + 12, 9, 12, chunk);
    c.set_block_state(world, BRICKS, base + 12, 9, 15, chunk);
    c.set_block_state(world, BRICKS, base + 12, 9, 18, chunk);
    let left = if flip { base + 19 } else { base + 5 };
    let right = if flip { base + 5 } else { base + 19 };
    let mut y = 20;
    while y >= 5 {
        c.set_block_state(world, BRICKS, left, 5, y, chunk);
        y -= 3;
    }
    let mut y = 19;
    while y >= 7 {
        c.set_block_state(world, BRICKS, right, 5, y, chunk);
        y -= 3;
    }
    for i in 0..4 {
        let x = if flip {
            base + (24 - (17 - i * 3))
        } else {
            base + 17 - i * 3
        };
        c.set_block_state(world, BRICKS, x, 5, 5, chunk);
    }
    c.set_block_state(world, BRICKS, right, 5, 5, chunk);
    c.fill_with_blocks(
        world,
        chunk,
        base + 11,
        1,
        12,
        base + 13,
        7,
        12,
        ROUGH,
        ROUGH,
        false,
    );
    c.fill_with_blocks(
        world,
        chunk,
        base + 12,
        1,
        11,
        base + 12,
        7,
        13,
        ROUGH,
        ROUGH,
        false,
    );
}

/// `Piece.func_175839_b`.
fn monument_entrance(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if !intersects(c, chunk, 22, 5, 35, 17) {
        return;
    }
    fill_water(c, world, chunk, 25, 0, 0, 32, 8, 20, false);
    for i in 0..4 {
        let z = 5 + i * 4;
        c.fill_with_blocks(world, chunk, 24, 2, z, 24, 4, z, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 22, 4, z, 23, 4, z, BRICKS, BRICKS, false);
        c.set_block_state(world, BRICKS, 25, 5, z, chunk);
        c.set_block_state(world, BRICKS, 26, 6, z, chunk);
        c.set_block_state(world, SEA_LANTERN, 26, 5, z, chunk);
        c.fill_with_blocks(world, chunk, 33, 2, z, 33, 4, z, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 34, 4, z, 35, 4, z, BRICKS, BRICKS, false);
        c.set_block_state(world, BRICKS, 32, 5, z, chunk);
        c.set_block_state(world, BRICKS, 31, 6, z, chunk);
        c.set_block_state(world, SEA_LANTERN, 31, 5, z, chunk);
        c.fill_with_blocks(world, chunk, 27, 6, z, 30, 6, z, ROUGH, ROUGH, false);
    }
}

/// `Piece.func_175837_c`.
fn monument_front(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if !intersects(c, chunk, 15, 21, 42, 21) {
        return;
    }
    c.fill_with_blocks(world, chunk, 15, 0, 21, 42, 0, 21, ROUGH, ROUGH, false);
    fill_water(c, world, chunk, 26, 1, 21, 31, 3, 21, false);
    c.fill_with_blocks(world, chunk, 21, 12, 21, 36, 12, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 17, 11, 21, 40, 11, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 16, 10, 21, 41, 10, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 15, 7, 21, 42, 9, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 16, 6, 21, 41, 6, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 17, 5, 21, 40, 5, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 21, 4, 21, 36, 4, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 22, 3, 21, 26, 3, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 31, 3, 21, 35, 3, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 23, 2, 21, 25, 2, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 32, 2, 21, 34, 2, 21, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 28, 4, 20, 29, 4, 21, BRICKS, BRICKS, false);
    c.set_block_state(world, BRICKS, 27, 3, 21, chunk);
    c.set_block_state(world, BRICKS, 30, 3, 21, chunk);
    c.set_block_state(world, BRICKS, 26, 2, 21, chunk);
    c.set_block_state(world, BRICKS, 31, 2, 21, chunk);
    c.set_block_state(world, BRICKS, 25, 1, 21, chunk);
    c.set_block_state(world, BRICKS, 32, 1, 21, chunk);
    for i in 0..7 {
        c.set_block_state(world, DARK, 28 - i, 6 + i, 21, chunk);
        c.set_block_state(world, DARK, 29 + i, 6 + i, 21, chunk);
    }
    for i in 0..4 {
        c.set_block_state(world, DARK, 28 - i, 9 + i, 21, chunk);
        c.set_block_state(world, DARK, 29 + i, 9 + i, 21, chunk);
    }
    c.set_block_state(world, DARK, 28, 12, 21, chunk);
    c.set_block_state(world, DARK, 29, 12, 21, chunk);
    for i in 0..3 {
        c.set_block_state(world, DARK, 22 - i * 2, 8, 21, chunk);
        c.set_block_state(world, DARK, 22 - i * 2, 9, 21, chunk);
        c.set_block_state(world, DARK, 35 + i * 2, 8, 21, chunk);
        c.set_block_state(world, DARK, 35 + i * 2, 9, 21, chunk);
    }
    fill_water(c, world, chunk, 15, 13, 21, 42, 15, 21, false);
    fill_water(c, world, chunk, 15, 1, 21, 15, 6, 21, false);
    fill_water(c, world, chunk, 16, 1, 21, 16, 5, 21, false);
    fill_water(c, world, chunk, 17, 1, 21, 20, 4, 21, false);
    fill_water(c, world, chunk, 21, 1, 21, 21, 3, 21, false);
    fill_water(c, world, chunk, 22, 1, 21, 22, 2, 21, false);
    fill_water(c, world, chunk, 23, 1, 21, 24, 1, 21, false);
    fill_water(c, world, chunk, 42, 1, 21, 42, 6, 21, false);
    fill_water(c, world, chunk, 41, 1, 21, 41, 5, 21, false);
    fill_water(c, world, chunk, 37, 1, 21, 40, 4, 21, false);
    fill_water(c, world, chunk, 36, 1, 21, 36, 3, 21, false);
    fill_water(c, world, chunk, 33, 1, 21, 34, 1, 21, false);
    fill_water(c, world, chunk, 35, 1, 21, 35, 2, 21, false);
}

/// `Piece.func_175841_d`.
fn monument_top(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if !intersects(c, chunk, 21, 21, 36, 36) {
        return;
    }
    c.fill_with_blocks(world, chunk, 21, 0, 22, 36, 0, 36, ROUGH, ROUGH, false);
    fill_water(c, world, chunk, 21, 1, 22, 36, 23, 36, false);
    for i in 0..4 {
        c.fill_with_blocks(
            world,
            chunk,
            21 + i,
            13 + i,
            21 + i,
            36 - i,
            13 + i,
            21 + i,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            21 + i,
            13 + i,
            36 - i,
            36 - i,
            13 + i,
            36 - i,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            21 + i,
            13 + i,
            22 + i,
            21 + i,
            13 + i,
            35 - i,
            BRICKS,
            BRICKS,
            false,
        );
        c.fill_with_blocks(
            world,
            chunk,
            36 - i,
            13 + i,
            22 + i,
            36 - i,
            13 + i,
            35 - i,
            BRICKS,
            BRICKS,
            false,
        );
    }
    c.fill_with_blocks(world, chunk, 25, 16, 25, 32, 16, 32, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 25, 17, 25, 25, 19, 25, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 32, 17, 25, 32, 19, 25, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 25, 17, 32, 25, 19, 32, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 32, 17, 32, 32, 19, 32, BRICKS, BRICKS, false);
    c.set_block_state(world, BRICKS, 26, 20, 26, chunk);
    c.set_block_state(world, BRICKS, 27, 21, 27, chunk);
    c.set_block_state(world, SEA_LANTERN, 27, 20, 27, chunk);
    c.set_block_state(world, BRICKS, 26, 20, 31, chunk);
    c.set_block_state(world, BRICKS, 27, 21, 30, chunk);
    c.set_block_state(world, SEA_LANTERN, 27, 20, 30, chunk);
    c.set_block_state(world, BRICKS, 31, 20, 31, chunk);
    c.set_block_state(world, BRICKS, 30, 21, 30, chunk);
    c.set_block_state(world, SEA_LANTERN, 30, 20, 30, chunk);
    c.set_block_state(world, BRICKS, 31, 20, 26, chunk);
    c.set_block_state(world, BRICKS, 30, 21, 27, chunk);
    c.set_block_state(world, SEA_LANTERN, 30, 20, 27, chunk);
    c.fill_with_blocks(world, chunk, 28, 21, 27, 29, 21, 27, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 27, 21, 28, 27, 21, 29, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 28, 21, 30, 29, 21, 30, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 30, 21, 28, 30, 21, 29, ROUGH, ROUGH, false);
}

/// `Piece.func_175835_e`.
fn monument_side_west(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if intersects(c, chunk, 0, 21, 6, 58) {
        c.fill_with_blocks(world, chunk, 0, 0, 21, 6, 0, 57, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 0, 1, 21, 6, 7, 57, false);
        c.fill_with_blocks(world, chunk, 4, 4, 21, 6, 4, 53, ROUGH, ROUGH, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                i,
                i + 1,
                21,
                i,
                i + 1,
                57 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 23;
        while z < 53 {
            c.set_block_state(world, BRICKS, 5, 5, z, chunk);
            z += 3;
        }
        c.set_block_state(world, BRICKS, 5, 5, 52, chunk);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                i,
                i + 1,
                21,
                i,
                i + 1,
                57 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        c.fill_with_blocks(world, chunk, 4, 1, 52, 6, 3, 52, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 5, 1, 51, 5, 3, 53, ROUGH, ROUGH, false);
    }
    if intersects(c, chunk, 51, 21, 58, 58) {
        c.fill_with_blocks(world, chunk, 51, 0, 21, 57, 0, 57, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 51, 1, 21, 57, 7, 57, false);
        c.fill_with_blocks(world, chunk, 51, 4, 21, 53, 4, 53, ROUGH, ROUGH, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                57 - i,
                i + 1,
                21,
                57 - i,
                i + 1,
                57 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 23;
        while z < 53 {
            c.set_block_state(world, BRICKS, 52, 5, z, chunk);
            z += 3;
        }
        c.set_block_state(world, BRICKS, 52, 5, 52, chunk);
        c.fill_with_blocks(world, chunk, 51, 1, 52, 53, 3, 52, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 52, 1, 51, 52, 3, 53, ROUGH, ROUGH, false);
    }
    if intersects(c, chunk, 0, 51, 57, 57) {
        c.fill_with_blocks(world, chunk, 7, 0, 51, 50, 0, 57, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 7, 1, 51, 50, 10, 57, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                i + 1,
                i + 1,
                57 - i,
                56 - i,
                i + 1,
                57 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
    }
}

/// `Piece.func_175842_f`.
fn monument_sides(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if intersects(c, chunk, 7, 21, 13, 50) {
        c.fill_with_blocks(world, chunk, 7, 0, 21, 13, 0, 50, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 7, 1, 21, 13, 10, 50, false);
        c.fill_with_blocks(world, chunk, 11, 8, 21, 13, 8, 53, ROUGH, ROUGH, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                i + 7,
                i + 5,
                21,
                i + 7,
                i + 5,
                54,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 21;
        while z <= 45 {
            c.set_block_state(world, BRICKS, 12, 9, z, chunk);
            z += 3;
        }
    }
    if intersects(c, chunk, 44, 21, 50, 54) {
        c.fill_with_blocks(world, chunk, 44, 0, 21, 50, 0, 50, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 44, 1, 21, 50, 10, 50, false);
        c.fill_with_blocks(world, chunk, 44, 8, 21, 46, 8, 53, ROUGH, ROUGH, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                50 - i,
                i + 5,
                21,
                50 - i,
                i + 5,
                54,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 21;
        while z <= 45 {
            c.set_block_state(world, BRICKS, 45, 9, z, chunk);
            z += 3;
        }
    }
    if intersects(c, chunk, 8, 44, 49, 54) {
        c.fill_with_blocks(world, chunk, 14, 0, 44, 43, 0, 50, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 14, 1, 44, 43, 10, 50, false);
        let mut x = 12;
        while x <= 45 {
            c.set_block_state(world, BRICKS, x, 9, 45, chunk);
            c.set_block_state(world, BRICKS, x, 9, 52, chunk);
            if [12, 18, 24, 33, 39, 45].contains(&x) {
                c.set_block_state(world, BRICKS, x, 9, 47, chunk);
                c.set_block_state(world, BRICKS, x, 9, 50, chunk);
                c.set_block_state(world, BRICKS, x, 10, 45, chunk);
                c.set_block_state(world, BRICKS, x, 10, 46, chunk);
                c.set_block_state(world, BRICKS, x, 10, 51, chunk);
                c.set_block_state(world, BRICKS, x, 10, 52, chunk);
                c.set_block_state(world, BRICKS, x, 11, 47, chunk);
                c.set_block_state(world, BRICKS, x, 11, 50, chunk);
                c.set_block_state(world, BRICKS, x, 12, 48, chunk);
                c.set_block_state(world, BRICKS, x, 12, 49, chunk);
            }
            x += 3;
        }
        for i in 0..3 {
            c.fill_with_blocks(
                world,
                chunk,
                8 + i,
                5 + i,
                54,
                49 - i,
                5 + i,
                54,
                ROUGH,
                ROUGH,
                false,
            );
        }
        c.fill_with_blocks(world, chunk, 11, 8, 54, 46, 8, 54, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 14, 8, 44, 43, 8, 53, ROUGH, ROUGH, false);
    }
}

/// `Piece.func_175838_g`.
fn monument_back(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if intersects(c, chunk, 14, 21, 20, 43) {
        c.fill_with_blocks(world, chunk, 14, 0, 21, 20, 0, 43, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 14, 1, 22, 20, 14, 43, false);
        c.fill_with_blocks(world, chunk, 18, 12, 22, 20, 12, 39, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 18, 12, 21, 20, 12, 21, BRICKS, BRICKS, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                i + 14,
                i + 9,
                21,
                i + 14,
                i + 9,
                43 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 23;
        while z <= 39 {
            c.set_block_state(world, BRICKS, 19, 13, z, chunk);
            z += 3;
        }
    }
    if intersects(c, chunk, 37, 21, 43, 43) {
        c.fill_with_blocks(world, chunk, 37, 0, 21, 43, 0, 43, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 37, 1, 22, 43, 14, 43, false);
        c.fill_with_blocks(world, chunk, 37, 12, 22, 39, 12, 39, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 37, 12, 21, 39, 12, 21, BRICKS, BRICKS, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                43 - i,
                i + 9,
                21,
                43 - i,
                i + 9,
                43 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut z = 23;
        while z <= 39 {
            c.set_block_state(world, BRICKS, 38, 13, z, chunk);
            z += 3;
        }
    }
    if intersects(c, chunk, 15, 37, 42, 43) {
        c.fill_with_blocks(world, chunk, 21, 0, 37, 36, 0, 43, ROUGH, ROUGH, false);
        fill_water(c, world, chunk, 21, 1, 37, 36, 14, 43, false);
        c.fill_with_blocks(world, chunk, 21, 12, 37, 36, 12, 39, ROUGH, ROUGH, false);
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                15 + i,
                i + 9,
                43 - i,
                42 - i,
                i + 9,
                43 - i,
                BRICKS,
                BRICKS,
                false,
            );
        }
        let mut x = 21;
        while x <= 36 {
            c.set_block_state(world, BRICKS, x, 13, 38, chunk);
            x += 3;
        }
    }
}

// ----- room bodies -------------------------------------------------------

fn room_parts(
    piece: &RoomPiece,
    rooms: &[RoomDef],
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    rand: &mut JavaRandom,
) {
    let c = &piece.comp;
    match piece.kind {
        RoomKind::Entry => entry_room(c, rooms, piece.room, world, chunk),
        RoomKind::Core => core_room(c, world, chunk),
        RoomKind::DoubleX => double_x(c, rooms, piece.room, world, chunk),
        RoomKind::DoubleXY => double_xy(c, rooms, piece.room, world, chunk),
        RoomKind::DoubleY => double_y(c, rooms, piece.room, world, chunk),
        RoomKind::DoubleYZ => double_yz(c, rooms, piece.room, world, chunk),
        RoomKind::DoubleZ => double_z(c, rooms, piece.room, world, chunk),
        RoomKind::Simple => simple_room(c, rooms, piece.room, piece.variant, world, chunk, rand),
        RoomKind::SimpleTop => simple_top(c, rooms, piece.room, world, chunk, rand),
        RoomKind::Wing => wing_room(c, piece.wing, world, chunk),
        RoomKind::Penthouse => penthouse(c, world, chunk),
    }
}

fn entry_room(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    c.fill_with_blocks(world, chunk, 0, 3, 0, 2, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 3, 0, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 1, 2, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 2, 0, 7, 2, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 0, 0, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 0, 7, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 7, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 0, 2, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 0, 6, 3, 0, BRICKS, BRICKS, false);
    if rooms[r].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 7, 4, 2, 7, false);
    }
    if rooms[r].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 1, 2, 4, false);
    }
    if rooms[r].open[EAST] {
        fill_water(c, world, chunk, 6, 1, 3, 7, 2, 4, false);
    }
}

fn core_room(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    replace_water(c, world, chunk, 1, 8, 0, 14, 8, 14, ROUGH);
    let mut b = BRICKS;
    c.fill_with_blocks(world, chunk, 0, 7, 0, 0, 7, 15, b, b, false);
    c.fill_with_blocks(world, chunk, 15, 7, 0, 15, 7, 15, b, b, false);
    c.fill_with_blocks(world, chunk, 1, 7, 0, 15, 7, 0, b, b, false);
    c.fill_with_blocks(world, chunk, 1, 7, 15, 14, 7, 15, b, b, false);
    for y in 1..=6 {
        b = if y == 2 || y == 6 { ROUGH } else { BRICKS };
        for x in [0, 15] {
            c.fill_with_blocks(world, chunk, x, y, 0, x, y, 1, b, b, false);
            c.fill_with_blocks(world, chunk, x, y, 6, x, y, 9, b, b, false);
            c.fill_with_blocks(world, chunk, x, y, 14, x, y, 15, b, b, false);
        }
        c.fill_with_blocks(world, chunk, 1, y, 0, 1, y, 0, b, b, false);
        c.fill_with_blocks(world, chunk, 6, y, 0, 9, y, 0, b, b, false);
        c.fill_with_blocks(world, chunk, 14, y, 0, 14, y, 0, b, b, false);
        c.fill_with_blocks(world, chunk, 1, y, 15, 14, y, 15, b, b, false);
    }
    c.fill_with_blocks(world, chunk, 6, 3, 6, 9, 6, 9, DARK, DARK, false);
    c.fill_with_blocks(
        world, chunk, 7, 4, 7, 8, 5, 8, GOLD_BLOCK, GOLD_BLOCK, false,
    );
    for y in [3, 6] {
        for z in [6, 9] {
            c.set_block_state(world, SEA_LANTERN, z, y, 6, chunk);
            c.set_block_state(world, SEA_LANTERN, z, y, 9, chunk);
        }
    }
    c.fill_with_blocks(world, chunk, 5, 1, 6, 5, 2, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 9, 5, 2, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 1, 6, 10, 2, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 1, 9, 10, 2, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 5, 6, 2, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 1, 5, 9, 2, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 10, 6, 2, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 1, 10, 9, 2, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 2, 5, 5, 6, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 2, 10, 5, 6, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 2, 5, 10, 6, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 2, 10, 10, 6, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 7, 1, 5, 7, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 7, 1, 10, 7, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 7, 9, 5, 7, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 7, 9, 10, 7, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 7, 5, 6, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 7, 10, 6, 7, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 7, 5, 14, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 7, 10, 14, 7, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 1, 2, 2, 1, 3, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 1, 2, 3, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 13, 1, 2, 13, 1, 3, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 12, 1, 2, 12, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 1, 12, 2, 1, 13, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 1, 13, 3, 1, 13, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 13, 1, 12, 13, 1, 13, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 12, 1, 13, 12, 1, 13, BRICKS, BRICKS, false);
}

fn double_x(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    let e = rooms[r].conn[EAST].unwrap();
    let b = r;
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 8, 0, rooms[e].open[DOWN]);
        base_floor(c, world, chunk, 0, 0, rooms[b].open[DOWN]);
    }
    if rooms[b].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 4, 1, 7, 4, 6, ROUGH);
    }
    if rooms[e].conn[UP].is_none() {
        replace_water(c, world, chunk, 8, 4, 1, 14, 4, 6, ROUGH);
    }
    c.fill_with_blocks(world, chunk, 0, 3, 0, 0, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 15, 3, 0, 15, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 0, 15, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 7, 14, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 0, 2, 7, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 15, 2, 0, 15, 2, 7, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 0, 15, 2, 0, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 7, 14, 2, 7, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 0, 1, 0, 0, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 15, 1, 0, 15, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 0, 15, 1, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 7, 14, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 0, 10, 1, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 2, 0, 9, 2, 3, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 5, 3, 0, 10, 3, 4, BRICKS, BRICKS, false);
    c.set_block_state(world, SEA_LANTERN, 6, 2, 3, chunk);
    c.set_block_state(world, SEA_LANTERN, 9, 2, 3, chunk);
    if rooms[b].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
    if rooms[b].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 7, 4, 2, 7, false);
    }
    if rooms[b].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 0, 2, 4, false);
    }
    if rooms[e].open[SOUTH] {
        fill_water(c, world, chunk, 11, 1, 0, 12, 2, 0, false);
    }
    if rooms[e].open[NORTH] {
        fill_water(c, world, chunk, 11, 1, 7, 12, 2, 7, false);
    }
    if rooms[e].open[EAST] {
        fill_water(c, world, chunk, 15, 1, 3, 15, 2, 4, false);
    }
}

fn double_xy(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    let e = rooms[r].conn[EAST].unwrap();
    let b = r;
    let bu = rooms[b].conn[UP].unwrap();
    let eu = rooms[e].conn[UP].unwrap();
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 8, 0, rooms[e].open[DOWN]);
        base_floor(c, world, chunk, 0, 0, rooms[b].open[DOWN]);
    }
    if rooms[bu].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 8, 1, 7, 8, 6, ROUGH);
    }
    if rooms[eu].conn[UP].is_none() {
        replace_water(c, world, chunk, 8, 8, 1, 14, 8, 6, ROUGH);
    }
    for y in 1..=7 {
        let s = if y == 2 || y == 6 { ROUGH } else { BRICKS };
        c.fill_with_blocks(world, chunk, 0, y, 0, 0, y, 7, s, s, false);
        c.fill_with_blocks(world, chunk, 15, y, 0, 15, y, 7, s, s, false);
        c.fill_with_blocks(world, chunk, 1, y, 0, 15, y, 0, s, s, false);
        c.fill_with_blocks(world, chunk, 1, y, 7, 14, y, 7, s, s, false);
    }
    c.fill_with_blocks(world, chunk, 2, 1, 3, 2, 7, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 1, 2, 4, 7, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 1, 5, 4, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 13, 1, 3, 13, 7, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 11, 1, 2, 12, 7, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 11, 1, 5, 12, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 3, 5, 3, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 1, 3, 10, 3, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 7, 2, 10, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 5, 2, 5, 7, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 5, 2, 10, 7, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 5, 5, 5, 7, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 10, 5, 5, 10, 7, 5, BRICKS, BRICKS, false);
    c.set_block_state(world, BRICKS, 6, 6, 2, chunk);
    c.set_block_state(world, BRICKS, 9, 6, 2, chunk);
    c.set_block_state(world, BRICKS, 6, 6, 5, chunk);
    c.set_block_state(world, BRICKS, 9, 6, 5, chunk);
    c.fill_with_blocks(world, chunk, 5, 4, 3, 6, 4, 4, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 4, 3, 10, 4, 4, BRICKS, BRICKS, false);
    c.set_block_state(world, SEA_LANTERN, 5, 4, 2, chunk);
    c.set_block_state(world, SEA_LANTERN, 5, 4, 5, chunk);
    c.set_block_state(world, SEA_LANTERN, 10, 4, 2, chunk);
    c.set_block_state(world, SEA_LANTERN, 10, 4, 5, chunk);
    if rooms[b].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
    if rooms[b].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 7, 4, 2, 7, false);
    }
    if rooms[b].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 0, 2, 4, false);
    }
    if rooms[e].open[SOUTH] {
        fill_water(c, world, chunk, 11, 1, 0, 12, 2, 0, false);
    }
    if rooms[e].open[NORTH] {
        fill_water(c, world, chunk, 11, 1, 7, 12, 2, 7, false);
    }
    if rooms[e].open[EAST] {
        fill_water(c, world, chunk, 15, 1, 3, 15, 2, 4, false);
    }
    if rooms[bu].open[SOUTH] {
        fill_water(c, world, chunk, 3, 5, 0, 4, 6, 0, false);
    }
    if rooms[bu].open[NORTH] {
        fill_water(c, world, chunk, 3, 5, 7, 4, 6, 7, false);
    }
    if rooms[bu].open[WEST] {
        fill_water(c, world, chunk, 0, 5, 3, 0, 6, 4, false);
    }
    if rooms[eu].open[SOUTH] {
        fill_water(c, world, chunk, 11, 5, 0, 12, 6, 0, false);
    }
    if rooms[eu].open[NORTH] {
        fill_water(c, world, chunk, 11, 5, 7, 12, 6, 7, false);
    }
    if rooms[eu].open[EAST] {
        fill_water(c, world, chunk, 15, 5, 3, 15, 6, 4, false);
    }
}

fn double_y(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 0, 0, rooms[r].open[DOWN]);
    }
    let u = rooms[r].conn[UP].unwrap();
    if rooms[u].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 8, 1, 6, 8, 6, ROUGH);
    }
    c.fill_with_blocks(world, chunk, 0, 4, 0, 0, 4, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 4, 0, 7, 4, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 4, 0, 6, 4, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 4, 7, 6, 4, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 4, 1, 2, 4, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 4, 2, 1, 4, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 4, 1, 5, 4, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 4, 2, 6, 4, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 4, 5, 2, 4, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 4, 5, 1, 4, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 4, 5, 5, 4, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 4, 5, 6, 4, 5, BRICKS, BRICKS, false);
    let mut base = r;
    for y in [1, 5] {
        if rooms[base].open[SOUTH] {
            c.fill_with_blocks(world, chunk, 2, y, 0, 2, y + 2, 0, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 5, y, 0, 5, y + 2, 0, BRICKS, BRICKS, false);
            c.fill_with_blocks(
                world,
                chunk,
                3,
                y + 2,
                0,
                4,
                y + 2,
                0,
                BRICKS,
                BRICKS,
                false,
            );
        } else {
            c.fill_with_blocks(world, chunk, 0, y, 0, 7, y + 2, 0, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 0, y + 1, 0, 7, y + 1, 0, ROUGH, ROUGH, false);
        }
        if rooms[base].open[NORTH] {
            c.fill_with_blocks(world, chunk, 2, y, 7, 2, y + 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 5, y, 7, 5, y + 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(
                world,
                chunk,
                3,
                y + 2,
                7,
                4,
                y + 2,
                7,
                BRICKS,
                BRICKS,
                false,
            );
        } else {
            c.fill_with_blocks(world, chunk, 0, y, 7, 7, y + 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 0, y + 1, 7, 7, y + 1, 7, ROUGH, ROUGH, false);
        }
        if rooms[base].open[WEST] {
            c.fill_with_blocks(world, chunk, 0, y, 2, 0, y + 2, 2, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 0, y, 5, 0, y + 2, 5, BRICKS, BRICKS, false);
            c.fill_with_blocks(
                world,
                chunk,
                0,
                y + 2,
                3,
                0,
                y + 2,
                4,
                BRICKS,
                BRICKS,
                false,
            );
        } else {
            c.fill_with_blocks(world, chunk, 0, y, 0, 0, y + 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 0, y + 1, 0, 0, y + 1, 7, ROUGH, ROUGH, false);
        }
        if rooms[base].open[EAST] {
            c.fill_with_blocks(world, chunk, 7, y, 2, 7, y + 2, 2, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 7, y, 5, 7, y + 2, 5, BRICKS, BRICKS, false);
            c.fill_with_blocks(
                world,
                chunk,
                7,
                y + 2,
                3,
                7,
                y + 2,
                4,
                BRICKS,
                BRICKS,
                false,
            );
        } else {
            c.fill_with_blocks(world, chunk, 7, y, 0, 7, y + 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, 7, y + 1, 0, 7, y + 1, 7, ROUGH, ROUGH, false);
        }
        base = u;
    }
}

fn double_yz(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    let n = rooms[r].conn[NORTH].unwrap();
    let b = r;
    let nu = rooms[n].conn[UP].unwrap();
    let bu = rooms[b].conn[UP].unwrap();
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 0, 8, rooms[n].open[DOWN]);
        base_floor(c, world, chunk, 0, 0, rooms[b].open[DOWN]);
    }
    if rooms[bu].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 8, 1, 6, 8, 7, ROUGH);
    }
    if rooms[nu].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 8, 8, 6, 8, 14, ROUGH);
    }
    for y in 1..=7 {
        let s = if y == 2 || y == 6 { ROUGH } else { BRICKS };
        c.fill_with_blocks(world, chunk, 0, y, 0, 0, y, 15, s, s, false);
        c.fill_with_blocks(world, chunk, 7, y, 0, 7, y, 15, s, s, false);
        c.fill_with_blocks(world, chunk, 1, y, 0, 6, y, 0, s, s, false);
        c.fill_with_blocks(world, chunk, 1, y, 15, 6, y, 15, s, s, false);
    }
    for y in 1..=7 {
        let s = if y == 2 || y == 6 { SEA_LANTERN } else { DARK };
        c.fill_with_blocks(world, chunk, 3, y, 7, 4, y + 1, 7, s, s, false);
    }
    if rooms[b].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
    if rooms[b].open[EAST] {
        fill_water(c, world, chunk, 7, 1, 3, 7, 2, 4, false);
    }
    if rooms[b].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 0, 2, 4, false);
    }
    if rooms[n].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 15, 4, 2, 15, false);
    }
    if rooms[n].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 11, 0, 2, 12, false);
    }
    if rooms[n].open[EAST] {
        fill_water(c, world, chunk, 7, 1, 11, 7, 2, 12, false);
    }
    if rooms[bu].open[SOUTH] {
        fill_water(c, world, chunk, 3, 5, 0, 4, 6, 0, false);
    }
    if rooms[bu].open[EAST] {
        fill_water(c, world, chunk, 7, 5, 3, 7, 6, 4, false);
        c.fill_with_blocks(world, chunk, 5, 4, 2, 6, 4, 5, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 6, 1, 2, 6, 3, 2, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 6, 1, 5, 6, 3, 5, BRICKS, BRICKS, false);
    }
    if rooms[bu].open[WEST] {
        fill_water(c, world, chunk, 0, 5, 3, 0, 6, 4, false);
        c.fill_with_blocks(world, chunk, 1, 4, 2, 2, 4, 5, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 1, 2, 1, 3, 2, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 1, 5, 1, 3, 5, BRICKS, BRICKS, false);
    }
    if rooms[nu].open[NORTH] {
        fill_water(c, world, chunk, 3, 5, 15, 4, 6, 15, false);
    }
    if rooms[nu].open[WEST] {
        fill_water(c, world, chunk, 0, 5, 11, 0, 6, 12, false);
        c.fill_with_blocks(world, chunk, 1, 4, 10, 2, 4, 13, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 1, 10, 1, 3, 10, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 1, 13, 1, 3, 13, BRICKS, BRICKS, false);
    }
    if rooms[nu].open[EAST] {
        fill_water(c, world, chunk, 7, 5, 11, 7, 6, 12, false);
        c.fill_with_blocks(world, chunk, 5, 4, 10, 6, 4, 13, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 6, 1, 10, 6, 3, 10, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 6, 1, 13, 6, 3, 13, BRICKS, BRICKS, false);
    }
}

fn double_z(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    let n = rooms[r].conn[NORTH].unwrap();
    let b = r;
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 0, 8, rooms[n].open[DOWN]);
        base_floor(c, world, chunk, 0, 0, rooms[b].open[DOWN]);
    }
    if rooms[b].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 4, 1, 6, 4, 7, ROUGH);
    }
    if rooms[n].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 4, 8, 6, 4, 14, ROUGH);
    }
    c.fill_with_blocks(world, chunk, 0, 3, 0, 0, 3, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 3, 0, 7, 3, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 0, 7, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 15, 6, 3, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 0, 2, 15, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 7, 2, 0, 7, 2, 15, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 0, 7, 2, 0, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 15, 6, 2, 15, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 0, 1, 0, 0, 1, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 0, 7, 1, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 0, 7, 1, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 15, 6, 1, 15, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 1, 1, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 1, 6, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 1, 1, 3, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 3, 1, 6, 3, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 13, 1, 1, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 13, 6, 1, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 13, 1, 3, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 3, 13, 6, 3, 14, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 1, 6, 2, 3, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 6, 5, 3, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 1, 9, 2, 3, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 9, 5, 3, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 2, 6, 4, 2, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 3, 2, 9, 4, 2, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 2, 7, 2, 2, 8, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 2, 7, 5, 2, 8, BRICKS, BRICKS, false);
    c.set_block_state(world, SEA_LANTERN, 2, 2, 5, chunk);
    c.set_block_state(world, SEA_LANTERN, 5, 2, 5, chunk);
    c.set_block_state(world, SEA_LANTERN, 2, 2, 10, chunk);
    c.set_block_state(world, SEA_LANTERN, 5, 2, 10, chunk);
    c.set_block_state(world, BRICKS, 2, 3, 5, chunk);
    c.set_block_state(world, BRICKS, 5, 3, 5, chunk);
    c.set_block_state(world, BRICKS, 2, 3, 10, chunk);
    c.set_block_state(world, BRICKS, 5, 3, 10, chunk);
    if rooms[b].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
    if rooms[b].open[EAST] {
        fill_water(c, world, chunk, 7, 1, 3, 7, 2, 4, false);
    }
    if rooms[b].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 0, 2, 4, false);
    }
    if rooms[n].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 15, 4, 2, 15, false);
    }
    if rooms[n].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 11, 0, 2, 12, false);
    }
    if rooms[n].open[EAST] {
        fill_water(c, world, chunk, 7, 1, 11, 7, 2, 12, false);
    }
}

fn simple_room(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    variant: i32,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    rand: &mut JavaRandom,
) {
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 0, 0, rooms[r].open[DOWN]);
    }
    if rooms[r].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 4, 1, 6, 4, 6, ROUGH);
    }
    let special = variant != 0
        && rand.next_boolean()
        && !rooms[r].open[DOWN]
        && !rooms[r].open[UP]
        && open_count(rooms, r) > 1;
    if variant == 0 {
        simple_variant0(c, rooms, r, world, chunk);
    } else if variant == 1 {
        simple_variant1(c, rooms, r, world, chunk);
    } else if variant == 2 {
        simple_variant2(c, rooms, r, world, chunk);
    }
    if special {
        c.fill_with_blocks(world, chunk, 3, 1, 3, 4, 1, 4, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 3, 2, 3, 4, 2, 4, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 3, 3, 3, 4, 3, 4, BRICKS, BRICKS, false);
    }
}

fn open_count(rooms: &[RoomDef], r: usize) -> i32 {
    rooms[r].open.iter().filter(|&&o| o).count() as i32
}

fn simple_variant0(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    c.fill_with_blocks(world, chunk, 0, 1, 0, 2, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 3, 0, 2, 3, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 0, 2, 2, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 0, 2, 2, 0, ROUGH, ROUGH, false);
    c.set_block_state(world, SEA_LANTERN, 1, 2, 1, chunk);
    c.fill_with_blocks(world, chunk, 5, 1, 0, 7, 1, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 3, 0, 7, 3, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 2, 0, 7, 2, 2, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 5, 2, 0, 6, 2, 0, ROUGH, ROUGH, false);
    c.set_block_state(world, SEA_LANTERN, 6, 2, 1, chunk);
    c.fill_with_blocks(world, chunk, 0, 1, 5, 2, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 3, 5, 2, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 5, 0, 2, 7, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 1, 2, 7, 2, 2, 7, ROUGH, ROUGH, false);
    c.set_block_state(world, SEA_LANTERN, 1, 2, 6, chunk);
    c.fill_with_blocks(world, chunk, 5, 1, 5, 7, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 3, 5, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 2, 5, 7, 2, 7, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 5, 2, 7, 6, 2, 7, ROUGH, ROUGH, false);
    c.set_block_state(world, SEA_LANTERN, 6, 2, 6, chunk);
    if rooms[r].open[SOUTH] {
        c.fill_with_blocks(world, chunk, 3, 3, 0, 4, 3, 0, BRICKS, BRICKS, false);
    } else {
        c.fill_with_blocks(world, chunk, 3, 3, 0, 4, 3, 1, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 3, 2, 0, 4, 2, 0, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 3, 1, 0, 4, 1, 1, BRICKS, BRICKS, false);
    }
    if rooms[r].open[NORTH] {
        c.fill_with_blocks(world, chunk, 3, 3, 7, 4, 3, 7, BRICKS, BRICKS, false);
    } else {
        c.fill_with_blocks(world, chunk, 3, 3, 6, 4, 3, 7, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 3, 2, 7, 4, 2, 7, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 3, 1, 6, 4, 1, 7, BRICKS, BRICKS, false);
    }
    if rooms[r].open[WEST] {
        c.fill_with_blocks(world, chunk, 0, 3, 3, 0, 3, 4, BRICKS, BRICKS, false);
    } else {
        c.fill_with_blocks(world, chunk, 0, 3, 3, 1, 3, 4, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 0, 2, 3, 0, 2, 4, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 0, 1, 3, 1, 1, 4, BRICKS, BRICKS, false);
    }
    if rooms[r].open[EAST] {
        c.fill_with_blocks(world, chunk, 7, 3, 3, 7, 3, 4, BRICKS, BRICKS, false);
    } else {
        c.fill_with_blocks(world, chunk, 6, 3, 3, 7, 3, 4, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 7, 2, 3, 7, 2, 4, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 6, 1, 3, 7, 1, 4, BRICKS, BRICKS, false);
    }
}

fn simple_variant1(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    c.fill_with_blocks(world, chunk, 2, 1, 2, 2, 3, 2, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 2, 1, 5, 2, 3, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 5, 5, 3, 5, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 5, 1, 2, 5, 3, 2, BRICKS, BRICKS, false);
    c.set_block_state(world, SEA_LANTERN, 2, 2, 2, chunk);
    c.set_block_state(world, SEA_LANTERN, 2, 2, 5, chunk);
    c.set_block_state(world, SEA_LANTERN, 5, 2, 5, chunk);
    c.set_block_state(world, SEA_LANTERN, 5, 2, 2, chunk);
    c.fill_with_blocks(world, chunk, 0, 1, 0, 1, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 1, 0, 3, 1, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 7, 1, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 6, 0, 3, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 7, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 6, 7, 3, 6, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, 1, 0, 7, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 1, 7, 3, 1, BRICKS, BRICKS, false);
    c.set_block_state(world, ROUGH, 1, 2, 0, chunk);
    c.set_block_state(world, ROUGH, 0, 2, 1, chunk);
    c.set_block_state(world, ROUGH, 1, 2, 7, chunk);
    c.set_block_state(world, ROUGH, 0, 2, 6, chunk);
    c.set_block_state(world, ROUGH, 6, 2, 7, chunk);
    c.set_block_state(world, ROUGH, 7, 2, 6, chunk);
    c.set_block_state(world, ROUGH, 6, 2, 0, chunk);
    c.set_block_state(world, ROUGH, 7, 2, 1, chunk);
    if !rooms[r].open[SOUTH] {
        c.fill_with_blocks(world, chunk, 1, 3, 0, 6, 3, 0, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 2, 0, 6, 2, 0, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 1, 1, 0, 6, 1, 0, BRICKS, BRICKS, false);
    }
    if !rooms[r].open[NORTH] {
        c.fill_with_blocks(world, chunk, 1, 3, 7, 6, 3, 7, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 1, 2, 7, 6, 2, 7, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 1, 1, 7, 6, 1, 7, BRICKS, BRICKS, false);
    }
    if !rooms[r].open[WEST] {
        c.fill_with_blocks(world, chunk, 0, 3, 1, 0, 3, 6, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 0, 2, 1, 0, 2, 6, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 0, 1, 1, 0, 1, 6, BRICKS, BRICKS, false);
    }
    if !rooms[r].open[EAST] {
        c.fill_with_blocks(world, chunk, 7, 3, 1, 7, 3, 6, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 7, 2, 1, 7, 2, 6, ROUGH, ROUGH, false);
        c.fill_with_blocks(world, chunk, 7, 1, 1, 7, 1, 6, BRICKS, BRICKS, false);
    }
}

fn simple_variant2(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
) {
    c.fill_with_blocks(world, chunk, 0, 1, 0, 0, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 0, 7, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 0, 6, 1, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 7, 6, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 0, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 7, 2, 0, 7, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 1, 2, 0, 6, 2, 0, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 1, 2, 7, 6, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 0, 3, 0, 0, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 3, 0, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 0, 6, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 7, 6, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 3, 0, 2, 4, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 7, 1, 3, 7, 2, 4, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 3, 1, 0, 4, 2, 0, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 3, 1, 7, 4, 2, 7, DARK, DARK, false);
    if rooms[r].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
    if rooms[r].open[NORTH] {
        fill_water(c, world, chunk, 3, 1, 7, 4, 2, 7, false);
    }
    if rooms[r].open[WEST] {
        fill_water(c, world, chunk, 0, 1, 3, 0, 2, 4, false);
    }
    if rooms[r].open[EAST] {
        fill_water(c, world, chunk, 7, 1, 3, 7, 2, 4, false);
    }
}

fn simple_top(
    c: &Component,
    rooms: &[RoomDef],
    r: usize,
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    rand: &mut JavaRandom,
) {
    if rooms[r].id / 25 > 0 {
        base_floor(c, world, chunk, 0, 0, rooms[r].open[DOWN]);
    }
    if rooms[r].conn[UP].is_none() {
        replace_water(c, world, chunk, 1, 4, 1, 6, 4, 6, ROUGH);
    }
    for x in 1..=6 {
        for z in 1..=6 {
            if rand.next_int_bound(3) != 0 {
                let y = 2 + if rand.next_int_bound(4) == 0 { 0 } else { 1 };
                c.fill_with_blocks(world, chunk, x, y, z, x, 3, z, SPONGE, SPONGE, false);
            }
        }
    }
    c.fill_with_blocks(world, chunk, 0, 1, 0, 0, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 1, 0, 7, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 0, 6, 1, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 1, 7, 6, 1, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 2, 0, 0, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 7, 2, 0, 7, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 1, 2, 0, 6, 2, 0, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 1, 2, 7, 6, 2, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 0, 3, 0, 0, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 7, 3, 0, 7, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 0, 6, 3, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 3, 7, 6, 3, 7, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, 1, 3, 0, 2, 4, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 7, 1, 3, 7, 2, 4, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 3, 1, 0, 4, 2, 0, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 3, 1, 7, 4, 2, 7, DARK, DARK, false);
    if rooms[r].open[SOUTH] {
        fill_water(c, world, chunk, 3, 1, 0, 4, 2, 0, false);
    }
}

fn wing_room(c: &Component, wing: i32, world: &mut ChunkWorld, chunk: &BoundingBox) {
    if wing == 0 {
        for i in 0..4 {
            c.fill_with_blocks(
                world,
                chunk,
                10 - i,
                3 - i,
                20 - i,
                12 + i,
                3 - i,
                20,
                BRICKS,
                BRICKS,
                false,
            );
        }
        c.fill_with_blocks(world, chunk, 7, 0, 6, 15, 0, 16, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 6, 0, 6, 6, 3, 20, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 16, 0, 6, 16, 3, 20, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 7, 1, 7, 7, 1, 20, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 15, 1, 7, 15, 1, 20, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 7, 1, 6, 9, 3, 6, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 13, 1, 6, 15, 3, 6, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 8, 1, 7, 9, 1, 7, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 13, 1, 7, 14, 1, 7, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 9, 0, 5, 13, 0, 5, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 10, 0, 7, 12, 0, 7, DARK, DARK, false);
        c.fill_with_blocks(world, chunk, 8, 0, 10, 8, 0, 12, DARK, DARK, false);
        c.fill_with_blocks(world, chunk, 14, 0, 10, 14, 0, 12, DARK, DARK, false);
        let mut z = 18;
        while z >= 7 {
            c.set_block_state(world, SEA_LANTERN, 6, 3, z, chunk);
            c.set_block_state(world, SEA_LANTERN, 16, 3, z, chunk);
            z -= 3;
        }
        c.set_block_state(world, SEA_LANTERN, 10, 0, 10, chunk);
        c.set_block_state(world, SEA_LANTERN, 12, 0, 10, chunk);
        c.set_block_state(world, SEA_LANTERN, 10, 0, 12, chunk);
        c.set_block_state(world, SEA_LANTERN, 12, 0, 12, chunk);
        c.set_block_state(world, SEA_LANTERN, 8, 3, 6, chunk);
        c.set_block_state(world, SEA_LANTERN, 14, 3, 6, chunk);
        c.set_block_state(world, BRICKS, 4, 2, 4, chunk);
        c.set_block_state(world, SEA_LANTERN, 4, 1, 4, chunk);
        c.set_block_state(world, BRICKS, 4, 0, 4, chunk);
        c.set_block_state(world, BRICKS, 18, 2, 4, chunk);
        c.set_block_state(world, SEA_LANTERN, 18, 1, 4, chunk);
        c.set_block_state(world, BRICKS, 18, 0, 4, chunk);
        c.set_block_state(world, BRICKS, 4, 2, 18, chunk);
        c.set_block_state(world, SEA_LANTERN, 4, 1, 18, chunk);
        c.set_block_state(world, BRICKS, 4, 0, 18, chunk);
        c.set_block_state(world, BRICKS, 18, 2, 18, chunk);
        c.set_block_state(world, SEA_LANTERN, 18, 1, 18, chunk);
        c.set_block_state(world, BRICKS, 18, 0, 18, chunk);
        c.set_block_state(world, BRICKS, 9, 7, 20, chunk);
        c.set_block_state(world, BRICKS, 13, 7, 20, chunk);
        c.fill_with_blocks(world, chunk, 6, 0, 21, 7, 4, 21, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 15, 0, 21, 16, 4, 21, BRICKS, BRICKS, false);
    } else if wing == 1 {
        c.fill_with_blocks(world, chunk, 9, 3, 18, 13, 3, 20, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 9, 0, 18, 9, 2, 18, BRICKS, BRICKS, false);
        c.fill_with_blocks(world, chunk, 13, 0, 18, 13, 2, 18, BRICKS, BRICKS, false);
        let mut x = 9;
        let z = 20;
        let y = 5;
        for _ in 0..2 {
            c.set_block_state(world, BRICKS, x, y + 1, z, chunk);
            c.set_block_state(world, SEA_LANTERN, x, y, z, chunk);
            c.set_block_state(world, BRICKS, x, y - 1, z, chunk);
            x = 13;
        }
        c.fill_with_blocks(world, chunk, 7, 3, 7, 15, 3, 14, BRICKS, BRICKS, false);
        let mut x = 10;
        for _ in 0..2 {
            c.fill_with_blocks(world, chunk, x, 0, 10, x, 6, 10, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, x, 0, 12, x, 6, 12, BRICKS, BRICKS, false);
            c.set_block_state(world, SEA_LANTERN, x, 0, 10, chunk);
            c.set_block_state(world, SEA_LANTERN, x, 0, 12, chunk);
            c.set_block_state(world, SEA_LANTERN, x, 4, 10, chunk);
            c.set_block_state(world, SEA_LANTERN, x, 4, 12, chunk);
            x = 12;
        }
        let mut x = 8;
        for _ in 0..2 {
            c.fill_with_blocks(world, chunk, x, 0, 7, x, 2, 7, BRICKS, BRICKS, false);
            c.fill_with_blocks(world, chunk, x, 0, 14, x, 2, 14, BRICKS, BRICKS, false);
            x = 14;
        }
        c.fill_with_blocks(world, chunk, 8, 3, 8, 8, 3, 13, DARK, DARK, false);
        c.fill_with_blocks(world, chunk, 14, 3, 8, 14, 3, 13, DARK, DARK, false);
    }
}

fn penthouse(c: &Component, world: &mut ChunkWorld, chunk: &BoundingBox) {
    c.fill_with_blocks(world, chunk, 2, -1, 2, 11, -1, 11, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 0, -1, 0, 1, -1, 11, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 12, -1, 0, 13, -1, 11, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 2, -1, 0, 11, -1, 1, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 2, -1, 12, 11, -1, 13, ROUGH, ROUGH, false);
    c.fill_with_blocks(world, chunk, 0, 0, 0, 0, 0, 13, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 13, 0, 0, 13, 0, 13, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 0, 0, 12, 0, 0, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 1, 0, 13, 12, 0, 13, BRICKS, BRICKS, false);
    let mut z = 2;
    while z <= 11 {
        c.set_block_state(world, SEA_LANTERN, 0, 0, z, chunk);
        c.set_block_state(world, SEA_LANTERN, 13, 0, z, chunk);
        c.set_block_state(world, SEA_LANTERN, z, 0, 0, chunk);
        z += 3;
    }
    c.fill_with_blocks(world, chunk, 2, 0, 3, 4, 0, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 9, 0, 3, 11, 0, 9, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 4, 0, 9, 9, 0, 11, BRICKS, BRICKS, false);
    c.set_block_state(world, BRICKS, 5, 0, 8, chunk);
    c.set_block_state(world, BRICKS, 8, 0, 8, chunk);
    c.set_block_state(world, BRICKS, 10, 0, 10, chunk);
    c.set_block_state(world, BRICKS, 3, 0, 10, chunk);
    c.fill_with_blocks(world, chunk, 3, 0, 3, 3, 0, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 10, 0, 3, 10, 0, 7, DARK, DARK, false);
    c.fill_with_blocks(world, chunk, 6, 0, 10, 7, 0, 10, DARK, DARK, false);
    let mut x = 3;
    for _ in 0..2 {
        let mut z = 2;
        while z <= 8 {
            c.fill_with_blocks(world, chunk, x, 0, z, x, 2, z, BRICKS, BRICKS, false);
            z += 3;
        }
        x = 10;
    }
    c.fill_with_blocks(world, chunk, 5, 0, 10, 5, 2, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 8, 0, 10, 8, 2, 10, BRICKS, BRICKS, false);
    c.fill_with_blocks(world, chunk, 6, -1, 7, 7, -1, 8, DARK, DARK, false);
    fill_water(c, world, chunk, 6, -1, 3, 7, -1, 4, false);
}

/// `MonumentBuilding.addComponentParts`: the shell + helpers, the base pillars, the water walls,
/// then every room whose box meets the pass.
pub(crate) fn monument_building_parts(
    world: &mut ChunkWorld,
    chunk: &BoundingBox,
    populate_rand: &mut JavaRandom,
    world_seed: i64,
    start_chunk_x: i32,
    start_chunk_z: i32,
    building_bbox: BoundingBox,
    facing: Facing,
) {
    let c = Component {
        bbox: building_bbox,
        facing: Some(facing),
    };
    let mut graph_rand = JavaRandom::new(world_seed);
    let a = graph_rand.next_long();
    let b = graph_rand.next_long();
    graph_rand.set_seed(
        (start_chunk_x as i64).wrapping_mul(a)
            ^ (start_chunk_z as i64).wrapping_mul(b)
            ^ world_seed,
    );
    let _ = graph_rand.next_int_bound(4);
    let mut graph = build_room_graph(&mut graph_rand);
    let pieces = build_pieces(
        &mut graph_rand,
        &mut graph.rooms,
        graph.entry,
        graph.core,
        &graph.list,
        facing,
        &c,
    );
    let _ = populate_rand;

    let depth = (SEA_LEVEL.max(64) - building_bbox.min_y).max(0);
    fill_water(&c, world, chunk, 0, 0, 0, 58, depth, 58, false);
    monument_wing(&c, world, chunk, false, 0);
    monument_wing(&c, world, chunk, true, 33);
    monument_entrance(&c, world, chunk);
    monument_front(&c, world, chunk);
    monument_top(&c, world, chunk);
    monument_side_west(&c, world, chunk);
    monument_sides(&c, world, chunk);
    monument_back(&c, world, chunk);

    for x in 0..7 {
        let mut z = 0;
        while z < 7 {
            if z == 0 && x == 3 {
                z = 6;
            }
            let px = x * 9;
            let pz = z * 9;
            for dx in 0..4 {
                for dz in 0..4 {
                    c.set_block_state(world, BRICKS, px + dx, 0, pz + dz, chunk);
                    c.replace_air_and_liquid_downwards(world, chunk, BRICKS, px + dx, -1, pz + dz);
                }
            }
            if x != 0 && x != 6 {
                z += 6;
            } else {
                z += 1;
            }
        }
    }

    for i in 0..5 {
        let a = -1 - i;
        let b = i * 2;
        let d = 58 + i;
        fill_water(&c, world, chunk, a, b, a, a, 23, d, false);
        fill_water(&c, world, chunk, d, b, a, d, 23, d, false);
        fill_water(&c, world, chunk, -i, b, a, 57 + i, 23, a, false);
        fill_water(&c, world, chunk, -i, b, d, 57 + i, 23, d, false);
    }

    for piece in &pieces {
        if piece.comp.bbox.intersects_with(chunk) {
            room_parts(piece, &graph.rooms, world, chunk, populate_rand);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// `Piece(int, facing, room, sizeX, sizeY, sizeZ)`'s bbox for the entry grid id (2).
    #[test]
    fn room_piece_box_matches_vanilla_offsets() {
        use Facing::*;
        // NORTH: bbox (0,0,0)-(7,3,7), offset (gx*8, 0, -(gz+sizeZ)*8+1) = (16, 0, -7).
        assert_eq!(
            room_piece_box(North, 2, 1, 1, 1),
            BoundingBox::new(16, 0, -7, 23, 3, 0)
        );
        // SOUTH: offset (16, 0, 0).
        assert_eq!(
            room_piece_box(South, 2, 1, 1, 1),
            BoundingBox::new(16, 0, 0, 23, 3, 7)
        );
        // EAST (default): offset (gz*8, 0, gx*8) = (0, 0, 16).
        assert_eq!(
            room_piece_box(East, 2, 1, 1, 1),
            BoundingBox::new(0, 0, 16, 7, 3, 23)
        );
        // WEST: offset (-(gz+sizeZ)*8+1, 0, gx*8) = (-7, 0, 16).
        assert_eq!(
            room_piece_box(West, 2, 1, 1, 1),
            BoundingBox::new(-7, 0, 16, 0, 3, 23)
        );
        // A 2x1 footprint on North/South keeps x=2 blocks worth (sizeX=2).
        assert_eq!(
            room_piece_box(North, 2, 2, 1, 1),
            BoundingBox::new(16, 0, -7, 31, 3, 0)
        );
    }

    /// The room graph has the 46 grid rooms + 3 end rooms, and the entry is chosen and an end.
    #[test]
    fn room_graph_entry_and_core_are_chosen() {
        let mut rand = JavaRandom::new(0);
        let a = rand.next_long();
        let b = rand.next_long();
        rand.set_seed(77i64.wrapping_mul(a) ^ (-50i64).wrapping_mul(b));
        let _ = rand.next_int_bound(4);
        let g = build_room_graph(&mut rand);
        assert_eq!(g.list.len(), 46 + 3);
        assert!(g.rooms[g.entry].chosen);
        assert!(g.rooms[g.entry].is_end);
        assert!(g.rooms[g.core].chosen);
        assert!(g.rooms[g.core].id >= 10 && g.rooms[g.core].id <= 13);
        // Exactly the entry is an end room.
        assert_eq!(g.rooms.iter().filter(|r| r.is_end).count(), 1);
    }

    /// Rebuilding the graph from the same seed is deterministic.
    #[test]
    fn room_graph_is_deterministic() {
        let build = || {
            let mut rand = JavaRandom::new(0);
            let a = rand.next_long();
            let b = rand.next_long();
            rand.set_seed(77i64.wrapping_mul(a) ^ (-50i64).wrapping_mul(b));
            let _ = rand.next_int_bound(4);
            build_room_graph(&mut rand)
        };
        let a = build();
        let b = build();
        assert_eq!(a.list, b.list);
        assert_eq!((a.entry, a.core), (b.entry, b.core));
        let chosen: Vec<bool> = a.rooms.iter().map(|r| r.chosen).collect();
        let chosen_b: Vec<bool> = b.rooms.iter().map(|r| r.chosen).collect();
        assert_eq!(chosen, chosen_b);
    }

    /// `MonumentCoreRoom` writes the gold block at relative (7, 4, 7).
    #[test]
    fn core_room_writes_gold_block() {
        let mut world = ChunkWorld::new();
        world.insert_chunk(0, 0, vec![ROUGH; 65536]);
        let c = Component {
            bbox: BoundingBox::new(0, 0, 0, 15, 7, 15),
            facing: Some(Facing::North),
        };
        let chunk = BoundingBox::new(0, 1, 0, 15, 512, 15);
        core_room(&c, &mut world, &chunk);
        // North: (x, z) -> (minX + x, maxZ - z), so (7, 4, 7) -> (7, 4, 8).
        assert_eq!(
            world.chunk_blocks(0, 0)[(7 << 12) | (8 << 8) | 4],
            GOLD_BLOCK
        );
    }
}
