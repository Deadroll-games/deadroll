//! Patch D: map size classes and the `MapLayout` format version 5 (docs/MAP_DESCRIPTION.md 13,
//! 17.6).
//!
//! A version 5 layout is as big as its class and sides need. After the discriminator:
//!
//! ```text
//! header   version, map, width, height, class, boss_table, bump, spawn_count, point_count
//! spawns   CLASS_PACKS[class] * Spawn            (each pack's visible monster)
//! points   (2 + CLASS_CHESTS[class]) * MapPoint
//! packs    CLASS_PACKS[class] * PackLayout
//! threat   CLASS_PACKS[class]
//! declared SECRET_TYPES
//! arenas   CLASS_PACKS[class] * ARENA_CELLS      (ARENA_SIDE_MAX per row)
//! cells    width * height                        (width per row)
//! blob_len u16, blob CLASS_BLOB[class]           (the DM's encrypted secrets)
//! check    LayoutGeom::CHECK_LEN                 (the reachability check's cursor)
//! pass     height * row words * 8                (passable tiles, a bit per cell)
//! reach    height * row words * 8                (cells reached from the entrance)
//! ```
//!
//! `layout_hash` covers `[8 .. blob_len)`: everything public, not the encrypted copy or the check's
//! scratch. The programs and the website read and write it through [`LayoutGeom`]; nothing decodes
//! the whole account (a Dungeon layout is ~32 KB).

use crate::{
    Cell, Facing, MapArenasData, MapLayoutDataV2, MapPoint, MapSecretsData, PackLayout, PointKind,
    Spawn, ARENA_CELLS, ARENA_SIDE_MAX, CLASS_BLOB, CLASS_CHESTS, CLASS_PACKS, CLASS_SECRETS,
    MAP_CLASSES, MAP_SIDE_CAP, PACKS_CAP, POINTS_CAP, SECRET_TYPES,
};

/// `MapLayout` version of this format.
pub const LAYOUT_V5: u8 = 5;

/// A map's cells, row-major, `stride` cells per row (a version 5 layout stores `width` per row).
#[derive(Clone, Copy, Debug)]
pub struct Board<'a> {
    pub width: u8,
    pub height: u8,
    pub stride: usize,
    pub cells: &'a [u8],
}

impl<'a> Board<'a> {
    /// Cells stored `width` per row.
    pub const fn new(width: u8, height: u8, cells: &'a [u8]) -> Self {
        Self {
            width,
            height,
            stride: width as usize,
            cells,
        }
    }

    pub const fn contains(&self, cell: Cell) -> bool {
        cell.x < self.width && cell.y < self.height
    }

    /// The cell byte at `(x, y)`; `0` outside the map or the storage.
    #[inline(always)]
    pub fn byte(&self, x: usize, y: usize) -> u8 {
        if x >= self.width as usize || y >= self.height as usize {
            return 0;
        }
        self.cells.get(y * self.stride + x).copied().unwrap_or(0)
    }

    /// The cell byte of `cell`, `None` outside the map.
    pub fn get(&self, cell: Cell) -> Option<u8> {
        self.contains(cell)
            .then(|| self.byte(cell.x as usize, cell.y as usize))
    }

    /// Row `y` inside the map (`width` cells).
    pub fn row(&self, y: usize) -> &'a [u8] {
        let at = y * self.stride;
        self.cells.get(at..at + self.width as usize).unwrap_or(&[])
    }
}

/// Header offsets of a version 5 `MapLayout` (discriminator included).
pub mod hdr {
    pub const VERSION: usize = 8;
    pub const MAP: usize = VERSION + 1;
    pub const WIDTH: usize = MAP + 32;
    pub const HEIGHT: usize = WIDTH + 1;
    pub const CLASS: usize = HEIGHT + 1;
    pub const BOSS_TABLE: usize = CLASS + 1;
    pub const BUMP: usize = BOSS_TABLE + 1;
    pub const SPAWN_COUNT: usize = BUMP + 1;
    pub const POINT_COUNT: usize = SPAWN_COUNT + 1;
    pub const BODY: usize = POINT_COUNT + 1;
}

/// Where everything lies in a version 5 `MapLayout` of one size (discriminator included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutGeom {
    pub width: u8,
    pub height: u8,
    pub class: u8,
    /// Pack numbers, points, secrets and blob bytes this class stores.
    pub packs: usize,
    pub points: usize,
    pub secrets: usize,
    pub blob_cap: usize,
    /// `u64` words per row of the check's bitsets.
    pub row_words: usize,
    pub spawns_at: usize,
    pub points_at: usize,
    pub packs_at: usize,
    pub threat_at: usize,
    pub declared_at: usize,
    pub arenas_at: usize,
    pub cells_at: usize,
    pub blob_len_at: usize,
    pub blob_at: usize,
    pub check_at: usize,
    pub pass_at: usize,
    pub reach_at: usize,
    /// Account size.
    pub end: usize,
}

impl LayoutGeom {
    /// Bytes of the check's cursor (`deadroll_mapcheck::wide::CheckState`).
    pub const CHECK_LEN: usize = 8;

    /// The geometry of a `width` x `height` map of size class `class`. `None` for a class past
    /// the last or a side outside `1..=MAP_SIDE_CAP` (the builder limits are mapcheck's).
    pub const fn new(width: u8, height: u8, class: u8) -> Option<Self> {
        let c = class as usize;
        if c >= MAP_CLASSES
            || width == 0
            || height == 0
            || width as usize > MAP_SIDE_CAP
            || height as usize > MAP_SIDE_CAP
        {
            return None;
        }
        let packs = CLASS_PACKS[c];
        let points = 2 + CLASS_CHESTS[c];
        let blob_cap = CLASS_BLOB[c];
        let row_words = (width as usize).div_ceil(64);
        let spawns_at = hdr::BODY;
        let points_at = spawns_at + packs * Spawn::SIZE;
        let packs_at = points_at + points * MapPoint::SIZE;
        let threat_at = packs_at + packs * PackLayout::SIZE;
        let declared_at = threat_at + packs;
        let arenas_at = declared_at + SECRET_TYPES;
        let cells_at = arenas_at + packs * ARENA_CELLS;
        let blob_len_at = cells_at + width as usize * height as usize;
        let blob_at = blob_len_at + 2;
        let check_at = blob_at + blob_cap;
        let pass_at = check_at + Self::CHECK_LEN;
        let reach_at = pass_at + height as usize * row_words * 8;
        let end = reach_at + height as usize * row_words * 8;
        Some(Self {
            width,
            height,
            class,
            packs,
            points,
            secrets: CLASS_SECRETS[c],
            blob_cap,
            row_words,
            spawns_at,
            points_at,
            packs_at,
            threat_at,
            declared_at,
            arenas_at,
            cells_at,
            blob_len_at,
            blob_at,
            check_at,
            pass_at,
            reach_at,
            end,
        })
    }

    /// The geometry a version 5 layout's header names, if the account holds all of it.
    pub fn read(data: &[u8]) -> Option<Self> {
        if data.len() < hdr::BODY || data[hdr::VERSION] != LAYOUT_V5 {
            return None;
        }
        let g = Self::new(data[hdr::WIDTH], data[hdr::HEIGHT], data[hdr::CLASS])?;
        (data.len() >= g.end).then_some(g)
    }

    /// Bytes of one bitset of the check (`pass` or `reach`).
    pub const fn bits_len(&self) -> usize {
        self.height as usize * self.row_words * 8
    }

    /// Cells of the map.
    pub const fn cell_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// What `layout_hash` covers.
    pub const fn hash_range(&self) -> core::ops::Range<usize> {
        8..self.blob_len_at
    }

    /// The map's cells in the account.
    pub fn board<'a>(&self, data: &'a [u8]) -> Board<'a> {
        Board::new(
            self.width,
            self.height,
            &data[self.cells_at..self.blob_len_at],
        )
    }

    /// Writes the header of an empty layout (no spawns, points or packs, no boss table).
    pub fn write_header(&self, data: &mut [u8], map: &[u8; 32], boss_table: u8, bump: u8) {
        data[hdr::VERSION] = LAYOUT_V5;
        data[hdr::MAP..hdr::MAP + 32].copy_from_slice(map);
        data[hdr::WIDTH] = self.width;
        data[hdr::HEIGHT] = self.height;
        data[hdr::CLASS] = self.class;
        data[hdr::BOSS_TABLE] = boss_table;
        data[hdr::BUMP] = bump;
    }

    pub fn spawn(&self, data: &[u8], index: usize) -> Option<Spawn> {
        (index < self.packs).then(|| Spawn::read(&data[self.spawns_at + index * Spawn::SIZE..]))
    }

    pub fn point(&self, data: &[u8], index: usize) -> Option<MapPoint> {
        if index >= self.points {
            return None;
        }
        MapPoint::read(&data[self.points_at + index * MapPoint::SIZE..])
    }

    pub fn pack(&self, data: &[u8], pack: usize) -> Option<PackLayout> {
        if pack >= self.packs {
            return None;
        }
        PackLayout::read(&data[self.packs_at + pack * PackLayout::SIZE..])
    }

    /// The cell byte at `(x, y)` of pack `pack`'s arena (`None` outside the storage).
    pub fn arena_byte(&self, data: &[u8], pack: usize, x: usize, y: usize) -> Option<u8> {
        (pack < self.packs && x < ARENA_SIDE_MAX && y < ARENA_SIDE_MAX)
            .then(|| data[self.arenas_at + pack * ARENA_CELLS + y * ARENA_SIDE_MAX + x])
    }

    /// Replaces the spawns (the slots past them are cleared). `false`: too many.
    pub fn write_spawns(&self, data: &mut [u8], spawns: &[Spawn]) -> bool {
        if spawns.len() > self.packs {
            return false;
        }
        data[self.spawns_at..self.points_at].fill(0);
        for (i, s) in spawns.iter().enumerate() {
            s.write(&mut data[self.spawns_at + i * Spawn::SIZE..]);
        }
        data[hdr::SPAWN_COUNT] = spawns.len() as u8;
        true
    }

    /// Replaces the points (the slots past them are cleared) and the boss table. `false`: too
    /// many.
    pub fn write_points(&self, data: &mut [u8], points: &[MapPoint], boss_table: u8) -> bool {
        if points.len() > self.points {
            return false;
        }
        data[self.points_at..self.packs_at].fill(0);
        for (i, p) in points.iter().enumerate() {
            p.write(&mut data[self.points_at + i * MapPoint::SIZE..]);
        }
        data[hdr::POINT_COUNT] = points.len() as u8;
        data[hdr::BOSS_TABLE] = boss_table;
        true
    }

    /// Replaces pack `pack`'s record and its arena, `cells` row by row (`record.width` per row).
    /// `false`: no such pack number, or the cells do not fit the record.
    pub fn write_arena(
        &self,
        data: &mut [u8],
        pack: usize,
        record: &PackLayout,
        cells: &[u8],
    ) -> bool {
        let (w, h) = (record.width as usize, record.height as usize);
        if pack >= self.packs || w > ARENA_SIDE_MAX || h > ARENA_SIDE_MAX || cells.len() != w * h {
            return false;
        }
        record.write(&mut data[self.packs_at + pack * PackLayout::SIZE..]);
        let arena = self.arenas_at + pack * ARENA_CELLS;
        data[arena..arena + ARENA_CELLS].fill(0);
        for (y, row) in cells.chunks(w.max(1)).take(h).enumerate() {
            let at = arena + y * ARENA_SIDE_MAX;
            data[at..at + row.len()].copy_from_slice(row);
        }
        true
    }

    /// Writes `cells` into the map from cell `start` (row-major, `width` per row). `false`: past
    /// the map.
    pub fn write_cells(&self, data: &mut [u8], start: usize, cells: &[u8]) -> bool {
        match start.checked_add(cells.len()) {
            Some(end) if end <= self.cell_count() => {
                data[self.cells_at + start..self.cells_at + end].copy_from_slice(cells);
                true
            }
            _ => false,
        }
    }

    /// Writes the public part of the secrets (`threat` per pack number of this class, the
    /// declared counts) and `bytes` of the encrypted copy from `offset` (`len`: the whole copy's
    /// length). `false`: past the copy or the class's cap, or a threat list of another length.
    pub fn write_secrets(
        &self,
        data: &mut [u8],
        offset: usize,
        bytes: &[u8],
        len: usize,
        threat: &[u8],
        declared: &[u8; SECRET_TYPES],
    ) -> bool {
        let end = match offset.checked_add(bytes.len()) {
            Some(end) if end <= len && len <= self.blob_cap && threat.len() == self.packs => end,
            _ => return false,
        };
        data[self.threat_at..self.declared_at].copy_from_slice(threat);
        data[self.declared_at..self.arenas_at].copy_from_slice(declared);
        data[self.blob_len_at..self.blob_at].copy_from_slice(&(len as u16).to_le_bytes());
        data[self.blob_at + offset..self.blob_at + end].copy_from_slice(bytes);
        true
    }

    pub fn blob_len(&self, data: &[u8]) -> u16 {
        u16::from_le_bytes([data[self.blob_len_at], data[self.blob_len_at + 1]])
    }

    /// The spawns, points and boss table.
    pub fn core(&self, data: &[u8]) -> LayoutCore {
        let mut out = LayoutCore::EMPTY;
        out.spawn_count = data[hdr::SPAWN_COUNT];
        out.point_count = data[hdr::POINT_COUNT];
        out.boss_table = data[hdr::BOSS_TABLE];
        for i in 0..self.packs {
            out.spawns[i] = Spawn::read(&data[self.spawns_at + i * Spawn::SIZE..]);
        }
        for i in 0..self.points {
            // A bad point kind reads as the default; mapcheck refuses it by position.
            out.points[i] =
                MapPoint::read(&data[self.points_at + i * MapPoint::SIZE..]).unwrap_or_default();
        }
        out
    }

    /// The packs and their arenas into `out` (big: a caller boxes it).
    pub fn arenas_into(&self, data: &[u8], out: &mut LayoutArenas) {
        *out = LayoutArenas::EMPTY;
        for p in 0..self.packs {
            out.packs[p] =
                PackLayout::read(&data[self.packs_at + p * PackLayout::SIZE..]).unwrap_or_default();
            let at = self.arenas_at + p * ARENA_CELLS;
            out.cells[p].copy_from_slice(&data[at..at + ARENA_CELLS]);
        }
    }

    /// The threats, the declared counts and the encrypted copy's length.
    pub fn secrets(&self, data: &[u8]) -> LayoutSecrets {
        let mut out = LayoutSecrets::EMPTY;
        out.threat[..self.packs].copy_from_slice(&data[self.threat_at..self.declared_at]);
        out.declared
            .copy_from_slice(&data[self.declared_at..self.arenas_at]);
        out.blob_len = self.blob_len(data);
        out
    }
}

/// The spawns, points and boss table of a layout, at the largest class's size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutCore {
    /// Each pack's visible monster (patch C), `0..spawn_count` used.
    pub spawns: [Spawn; PACKS_CAP],
    pub spawn_count: u8,
    /// The entrance, the exit and the visible chests, `0..point_count` used.
    pub points: [MapPoint; POINTS_CAP],
    pub point_count: u8,
    /// The reward table of the boss (`NO_TABLE` = none).
    pub boss_table: u8,
}

impl LayoutCore {
    pub const EMPTY: LayoutCore = LayoutCore {
        spawns: [Spawn::ZERO; PACKS_CAP],
        spawn_count: 0,
        points: [MapPoint::ZERO; POINTS_CAP],
        point_count: 0,
        boss_table: crate::NO_TABLE,
    };

    /// A version 4 core (a Small map's spawns and points).
    pub fn from_v2(core: &MapLayoutDataV2) -> Self {
        let mut out = Self::EMPTY;
        let n = core.spawns.len().min(PACKS_CAP);
        out.spawns[..n].copy_from_slice(&core.spawns[..n]);
        out.spawn_count = core.spawn_count;
        out.points[..core.points.len()].copy_from_slice(&core.points);
        out.point_count = core.point_count;
        out.boss_table = core.boss_table;
        out
    }

    pub fn spawn_list(&self) -> &[Spawn] {
        &self.spawns[..(self.spawn_count as usize).min(PACKS_CAP)]
    }

    pub fn point_list(&self) -> &[MapPoint] {
        &self.points[..(self.point_count as usize).min(POINTS_CAP)]
    }

    pub fn point(&self, kind: PointKind) -> Option<Cell> {
        self.point_list()
            .iter()
            .find(|p| p.kind == kind)
            .map(MapPoint::cell)
    }

    pub fn entrance(&self) -> Option<Cell> {
        self.point(PointKind::Entrance)
    }

    pub fn exit(&self) -> Option<Cell> {
        self.point(PointKind::Exit)
    }

    /// The visible chests in point order.
    pub fn chests(&self) -> impl Iterator<Item = &MapPoint> {
        self.point_list()
            .iter()
            .filter(|p| p.kind == PointKind::Chest)
    }
}

/// The packs of a layout and their arenas, at the largest class's size (~4.8 KB).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutArenas {
    pub packs: [PackLayout; PACKS_CAP],
    /// Row-major cell bytes of each pack's arena, `ARENA_SIDE_MAX` per row.
    pub cells: [[u8; ARENA_CELLS]; PACKS_CAP],
}

impl LayoutArenas {
    pub const EMPTY: LayoutArenas = LayoutArenas {
        packs: [PackLayout::EMPTY; PACKS_CAP],
        cells: [[0; ARENA_CELLS]; PACKS_CAP],
    };

    /// Version 4 arenas (a Small map's 8 pack numbers).
    pub fn from_v1(arenas: &MapArenasData) -> Self {
        let mut out = Self::EMPTY;
        out.packs[..arenas.packs.len()].copy_from_slice(&arenas.packs);
        out.cells[..arenas.cells.len()].copy_from_slice(&arenas.cells);
        out
    }

    /// The cell byte at `(x, y)` of pack `pack`'s arena (`None` outside the storage).
    pub fn cell(&self, pack: u8, x: u8, y: u8) -> Option<u8> {
        if x as usize >= ARENA_SIDE_MAX || y as usize >= ARENA_SIDE_MAX {
            return None;
        }
        self.cells
            .get(pack as usize)
            .map(|arena| arena[y as usize * ARENA_SIDE_MAX + x as usize])
    }

    /// A used pack number's record.
    pub fn pack(&self, pack: u8) -> Option<&PackLayout> {
        self.packs.get(pack as usize).filter(|p| p.is_used())
    }
}

/// The public part of a layout's secrets, at the largest class's size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutSecrets {
    /// Each pack's declared threat (0 for an unused pack number).
    pub threat: [u8; PACKS_CAP],
    pub declared: [u8; SECRET_TYPES],
    pub blob_len: u16,
}

impl LayoutSecrets {
    pub const EMPTY: LayoutSecrets = LayoutSecrets {
        threat: [0; PACKS_CAP],
        declared: [0; SECRET_TYPES],
        blob_len: 0,
    };

    /// Version 4 secrets.
    pub fn from_v1(secrets: &MapSecretsData) -> Self {
        let mut out = Self::EMPTY;
        out.threat[..secrets.threat.len()].copy_from_slice(&secrets.threat);
        out.declared = secrets.declared;
        out.blob_len = secrets.blob_len;
        out
    }
}

impl Spawn {
    pub const ZERO: Spawn = Spawn {
        bestiary_id: 0,
        x: 0,
        y: 0,
        level: 0,
        skills: 0,
        flags: 0,
        aspects: 0,
        pack: 0,
    };

    /// Borsh bytes, without borsh (`SIZE` bytes).
    pub fn read(b: &[u8]) -> Spawn {
        Spawn {
            bestiary_id: b[0],
            x: b[1],
            y: b[2],
            level: b[3],
            skills: u16::from_le_bytes([b[4], b[5]]),
            flags: b[6],
            aspects: b[7],
            pack: b[8],
        }
    }

    pub fn write(&self, b: &mut [u8]) {
        let [s0, s1] = self.skills.to_le_bytes();
        b[..Self::SIZE].copy_from_slice(&[
            self.bestiary_id,
            self.x,
            self.y,
            self.level,
            s0,
            s1,
            self.flags,
            self.aspects,
            self.pack,
        ]);
    }
}

impl MapPoint {
    pub const ZERO: MapPoint = MapPoint {
        kind: PointKind::Chest,
        x: 0,
        y: 0,
        data: 0,
    };

    /// Borsh bytes, without borsh; `None` for an unknown kind.
    pub fn read(b: &[u8]) -> Option<MapPoint> {
        Some(MapPoint {
            kind: PointKind::try_from(b[0]).ok()?,
            x: b[1],
            y: b[2],
            data: b[3],
        })
    }

    pub fn write(&self, b: &mut [u8]) {
        b[..Self::SIZE].copy_from_slice(&[self.kind as u8, self.x, self.y, self.data]);
    }
}

impl PackLayout {
    /// Borsh bytes, without borsh; `None` for an unknown facing.
    pub fn read(b: &[u8]) -> Option<PackLayout> {
        Some(PackLayout {
            visible: b[0],
            x: b[1],
            y: b[2],
            facing: Facing::try_from(b[3]).ok()?,
            width: b[4],
            height: b[5],
        })
    }

    pub fn write(&self, b: &mut [u8]) {
        b[..Self::SIZE].copy_from_slice(&[
            self.visible,
            self.x,
            self.y,
            self.facing as u8,
            self.width,
            self.height,
        ]);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::{MAP_CELLS_CAP, NO_TABLE};
    use std::vec;

    #[test]
    fn geometry_of_every_class() {
        let dungeon = LayoutGeom::new(143, 143, 3).unwrap();
        assert_eq!(dungeon.cell_count(), MAP_CELLS_CAP);
        assert_eq!(dungeon.row_words, 3);
        assert_eq!(dungeon.packs, 40);
        let small = LayoutGeom::new(32, 32, 0).unwrap();
        assert_eq!(small.row_words, 1);
        assert!(small.end < dungeon.end);
        assert!(dungeon.end < 40_000, "{}", dungeon.end);
        assert_eq!(LayoutGeom::new(144, 8, 3), None);
        assert_eq!(LayoutGeom::new(8, 8, MAP_CLASSES as u8), None);
        assert_eq!(LayoutGeom::new(0, 8, 0), None);
    }

    #[test]
    fn writes_and_reads_back() {
        let g = LayoutGeom::new(40, 30, 1).unwrap();
        let mut data = vec![0u8; g.end];
        g.write_header(&mut data, &[7; 32], NO_TABLE, 254);
        assert_eq!(LayoutGeom::read(&data), Some(g));
        assert_eq!(LayoutGeom::read(&data[..g.end - 1]), None);
        let spawn = Spawn {
            bestiary_id: 3,
            skills: 0x1234,
            pack: 9,
            ..Spawn::ZERO
        };
        assert!(g.write_spawns(&mut data, &[spawn; 2]));
        assert!(!g.write_spawns(&mut data, &[spawn; 17]));
        let point = MapPoint {
            kind: PointKind::Exit,
            x: 39,
            y: 29,
            data: 0,
        };
        assert!(g.write_points(&mut data, &[point], 4));
        assert!(!g.write_points(&mut data, &[point; 11], 4));
        let record = PackLayout {
            visible: 1,
            x: 5,
            y: 6,
            facing: Facing::SW,
            width: 3,
            height: 2,
        };
        assert!(g.write_arena(&mut data, 15, &record, &[1, 2, 3, 4, 5, 6]));
        assert!(!g.write_arena(&mut data, 16, &record, &[0; 6]));
        assert!(!g.write_arena(&mut data, 15, &record, &[0; 5]));
        assert!(g.write_cells(&mut data, 40 * 29, &[9; 40]));
        assert!(!g.write_cells(&mut data, 40 * 29 + 1, &[9; 40]));
        let mut threat = [0u8; 16];
        threat[15] = 4;
        assert!(g.write_secrets(&mut data, 0, &[1, 2], 2, &threat, &[1, 0, 0, 2]));
        assert!(!g.write_secrets(&mut data, 0, &[], 1601, &threat, &[0; 4]));
        assert!(!g.write_secrets(&mut data, 0, &[], 2, &threat[..8], &[0; 4]));

        let core = g.core(&data);
        assert_eq!(core.spawn_list(), &[spawn, spawn]);
        assert_eq!(core.exit(), Some(Cell { x: 39, y: 29 }));
        assert_eq!(core.boss_table, 4);
        let mut arenas = LayoutArenas::EMPTY;
        g.arenas_into(&data, &mut arenas);
        assert_eq!(arenas.pack(15), Some(&record));
        assert_eq!(arenas.cell(15, 2, 1), Some(6));
        assert_eq!(g.arena_byte(&data, 15, 2, 1), Some(6));
        let secrets = g.secrets(&data);
        assert_eq!(secrets.threat[15], 4);
        assert_eq!(secrets.declared, [1, 0, 0, 2]);
        assert_eq!(secrets.blob_len, 2);
        let board = g.board(&data);
        assert_eq!(board.byte(39, 29), 9);
        assert_eq!(board.byte(40, 29), 0);
        assert_eq!(board.row(29), &[9; 40]);
    }
}
