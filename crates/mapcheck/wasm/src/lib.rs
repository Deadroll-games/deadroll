//! C ABI over `deadroll-mapcheck` for the website map builder (docs/MAP_DESCRIPTION.md 1, 10): the
//! verdict `finalize_map` gives a draft, before any transaction. See `README.md` next to this crate.
//!
//! The rules are not copied: the page loads the registry table bodies the program reads
//! (`set_table`), and [`check`] decodes them with `deadroll_rules` (`tiles`, `map_tables`, the tier
//! loot and the size multipliers of `BalanceConfig::DEFAULT`) and runs
//! `deadroll_mapcheck::check_design_map`, the rules of the whole design.
//!
//! Static buffers, no imports. The page writes the borsh [`MapCheckRequest`] into the input buffer
//! and calls `check_map`. Patch C: `check_design` takes a [`DesignRequest`] (the same map, the
//! declared secret counts and every secret) and runs `deadroll_mapcheck::design::check_design`,
//! the public part `finalize_map` checks and each pack and secret as the session checks them when
//! revealed. Patch D: a map of any size class; `check_steps` gives how many `check_step`
//! transactions the map needs before `finalize_map`. Built with `std` for the same reason as the
//! combat module (`solana-pubkey` links `std` off SBF).

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_mapcheck::{
    check_design_map,
    limits::SizeScale,
    loot::{LootRules, MapLoot},
    roster::SpawnRules,
    wide, with_reach, MapCheckError, MapParts,
};
use deadroll_rules::{loot::map_rules, map_tables, tiles::tile_rules, BalanceConfig};
use deadroll_types::{
    Board, FoundSecret, LayoutArenas, LayoutCore, MapPoint, PackLayout, Spawn, ARENA_CELLS,
    MAP_ROW_WORDS, MAP_SIDE_CAP, PACKS_CAP, POINTS_CAP, SECRET_TYPES,
};

/// Bumped on any change of the exports or the byte layouts (2: the arenas, arena update; 4: map
/// sizes, patch D).
pub const ABI_VERSION: u32 = 4;
/// Size of the input buffer: a Dungeon request is ~28 KB, a table body at most 4096.
pub const BUFFER_LEN: usize = 40_960;
/// Bestiary ids a spawn can name (registry `BESTIARY_MAX`).
const BESTIARY_MAX: usize = 32;

/// Export error codes (negative return values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    /// `input_len` is larger than the input buffer.
    InputTooLong = -1,
    /// The input is not a borsh `MapCheckRequest` (or has trailing bytes).
    BadInput = -2,
    /// `set_table` with an unknown table kind.
    UnknownTable = -3,
    /// Patch D, `check_steps`: a side outside the size classes or a cell the `Tileset` lacks.
    BadMap = -4,
}

/// The registry tables `finalize_map` reads, by `set_table` kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Table {
    Bestiary = 0,
    MonsterSkills = 1,
    Rewards = 2,
    Tileset = 3,
}

/// A draft as the builder holds it (patch D, any size class): the `Map` numbers and what the
/// `MapLayout` stores, with every spawn of the design.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapCheckRequest {
    pub width: u8,
    pub height: u8,
    pub tier: u8,
    pub min_players: u8,
    /// `width * height` cell bytes, row by row.
    pub cells: Vec<u8>,
    /// Every monster of the design; a spawn's `pack` is its pack number.
    pub spawns: Vec<Spawn>,
    /// The entrance, the exit and the visible chests.
    pub points: Vec<MapPoint>,
    pub boss_table: u8,
    /// The packs by number (`visible` into `spawns`), at most `PACKS_CAP`.
    pub packs: Vec<PackLayout>,
    /// Each pack's arena, `ARENA_CELLS` bytes, `ARENA_SIDE_MAX` per row, in pack order.
    pub arenas: Vec<u8>,
}

/// The registry table bodies (as far as written), by [`Table`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tables {
    pub bestiary: Vec<u8>,
    pub monster_skills: Vec<u8>,
    pub rewards: Vec<u8>,
    pub tileset: Vec<u8>,
}

/// A request in the shapes the rules read; `None` when it cannot be one (lists too long, cells of
/// another count).
struct Parts {
    core: LayoutCore,
    arenas: Box<LayoutArenas>,
}

fn parts(r: &MapCheckRequest) -> Option<Parts> {
    if r.cells.len() != r.width as usize * r.height as usize
        || r.points.len() > POINTS_CAP
        || r.packs.len() > PACKS_CAP
        || r.arenas.len() != r.packs.len() * ARENA_CELLS
    {
        return None;
    }
    let mut core = LayoutCore::EMPTY;
    core.points[..r.points.len()].copy_from_slice(&r.points);
    core.point_count = r.points.len() as u8;
    core.boss_table = r.boss_table;
    let mut arenas = Box::new(LayoutArenas::EMPTY);
    arenas.packs[..r.packs.len()].copy_from_slice(&r.packs);
    for (n, cells) in r.arenas.chunks_exact(ARENA_CELLS).enumerate() {
        arenas.cells[n].copy_from_slice(cells);
    }
    Some(Parts { core, arenas })
}

/// The size class of a request and its scale (`None`: no class holds it).
fn scale_of(r: &MapCheckRequest) -> Option<SizeScale> {
    let cfg = &BalanceConfig::DEFAULT;
    cfg.map_size
        .class_of(r.width, r.height)
        .map(|c| cfg.map_size.scale(c))
}

/// The templates the spawns name, decoded once each (as `finalize_map` caches them).
fn templates(spawns: &[Spawn], tables: &Tables) -> [Option<SpawnRules>; BESTIARY_MAX] {
    let mut out = [None; BESTIARY_MAX];
    for spawn in spawns {
        if let Some(slot) = out.get_mut(spawn.bestiary_id as usize) {
            *slot = map_tables::spawn_rules(
                &tables.bestiary,
                &tables.monster_skills,
                spawn.bestiary_id,
                &BalanceConfig::DEFAULT.monsters,
            );
        }
    }
    out
}

/// The tier's loot limits at the request's size.
fn loot_rules(r: &MapCheckRequest, scale: SizeScale) -> LootRules {
    let cfg = &BalanceConfig::DEFAULT;
    cfg.map_size.loot(map_rules(&cfg.loot, r.tier), scale.class)
}

/// Every builder rule on a whole design, with the registry tables: the first `MapCheckError`, or
/// `Ok`. A request no class holds is `SizeOutOfRange`.
pub fn check(request: &MapCheckRequest, tables: &Tables) -> Result<(), MapCheckError> {
    let cfg = &BalanceConfig::DEFAULT;
    let scale = scale_of(request).ok_or(MapCheckError::SizeOutOfRange)?;
    let p = parts(request).ok_or(MapCheckError::SizeOutOfRange)?;
    let templates = templates(&request.spawns, tables);
    let loot = MapLoot {
        min_players: request.min_players,
        rules: loot_rules(request, scale),
        table_of: |id: u8| map_tables::table_rules(&tables.rewards, id),
    };
    let tiles = tile_rules(&tables.tileset);
    let map = MapParts {
        board: Board::new(request.width, request.height, &request.cells),
        core: &p.core,
        spawns: &request.spawns,
        arenas: &p.arenas,
    };
    with_reach(&map.board, &p.core, &tiles, |reach| {
        check_design_map(
            &map,
            request.tier,
            scale,
            cfg.arena.limits(),
            &tiles,
            |id| templates.get(id as usize).copied().flatten(),
            &loot,
            reach,
        )
    })?
}

/// Patch D: the `check_step` transactions the map needs before `finalize_map`
/// (`deadroll_mapcheck::wide::steps_needed`), or the rule that stops the check.
pub fn check_steps(request: &MapCheckRequest, tables: &Tables) -> Result<u32, MapCheckError> {
    scale_of(request).ok_or(MapCheckError::SizeOutOfRange)?;
    let p = parts(request).ok_or(MapCheckError::SizeOutOfRange)?;
    let board = Board::new(request.width, request.height, &request.cells);
    let z = vec![[0u64; MAP_ROW_WORDS]; MAP_SIDE_CAP];
    let (mut pass, mut open, mut reach) = (z.clone(), z.clone(), z.clone());
    let (mut tpass, mut topen, mut treach) = (z.clone(), z.clone(), z);
    let (chests, count) = deadroll_mapcheck::loot::chest_cells(&p.core);
    let start = p.core.entrance().unwrap_or(deadroll_types::Cell {
        x: u8::MAX,
        y: u8::MAX,
    });
    wide::steps_needed(
        &board,
        &tile_rules(&tables.tileset),
        start,
        &chests[..count],
        (&mut pass, &mut open, &mut reach),
        (&mut tpass, &mut topen, &mut treach),
    )
}

/// Patch C: a whole design: the map with every spawn (`visible` into them), the declared counts
/// and the secrets by leaf.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct DesignRequest {
    pub map: MapCheckRequest,
    pub declared: [u8; SECRET_TYPES],
    pub secrets: Vec<FoundSecret>,
}

/// Patch C: every rule of a design (`deadroll_mapcheck::design::check_design`).
pub fn check_design(request: &DesignRequest, tables: &Tables) -> Result<(), MapCheckError> {
    let cfg = &BalanceConfig::DEFAULT;
    let m = &request.map;
    let scale = scale_of(m).ok_or(MapCheckError::SizeOutOfRange)?;
    let p = parts(m).ok_or(MapCheckError::SizeOutOfRange)?;
    let templates = templates(&m.spawns, tables);
    let loot = MapLoot {
        min_players: m.min_players,
        rules: loot_rules(m, scale),
        table_of: |id: u8| map_tables::table_rules(&tables.rewards, id),
    };
    let limits = cfg.map_size.secrets(
        cfg.secrets
            .limits(m.tier)
            .ok_or(MapCheckError::TierOutOfRange)?,
        scale.class,
    );
    let map = MapParts {
        board: Board::new(m.width, m.height, &m.cells),
        core: &p.core,
        spawns: &m.spawns,
        arenas: &p.arenas,
    };
    deadroll_mapcheck::design::check_design(
        &map,
        m.tier,
        scale,
        &request.declared,
        &request.secrets,
        cfg.arena.limits(),
        &limits,
        cfg.secrets.threat(),
        &tile_rules(&tables.tileset),
        |id| templates.get(id as usize).copied().flatten(),
        &loot,
    )
    .map(|_| ())
}

/// Patch C: [`check_design`] on a borsh [`DesignRequest`]: 0, `1 + code`, or an [`Error`].
pub fn check_design_bytes(input: &[u8], tables: &Tables) -> i32 {
    let Ok(request) = DesignRequest::try_from_slice(input) else {
        return Error::BadInput as i32;
    };
    match check_design(&request, tables) {
        Ok(()) => 0,
        Err(e) => 1 + e.code() as i32,
    }
}

/// Decodes a borsh `MapCheckRequest` and checks it: `0` for a valid map, `1 + code` for the first
/// broken rule (`MapCheckError::code`), or a negative [`Error`].
pub fn check_bytes(input: &[u8], tables: &Tables) -> i32 {
    let Ok(request) = MapCheckRequest::try_from_slice(input) else {
        return Error::BadInput as i32;
    };
    match check(&request, tables) {
        Ok(()) => 0,
        Err(e) => 1 + e.code() as i32,
    }
}

/// Patch D: [`check_steps`] on a borsh `MapCheckRequest`: the count, or a negative [`Error`].
pub fn check_steps_bytes(input: &[u8], tables: &Tables) -> i32 {
    let Ok(request) = MapCheckRequest::try_from_slice(input) else {
        return Error::BadInput as i32;
    };
    match check_steps(&request, tables) {
        Ok(n) => n as i32,
        Err(_) => Error::BadMap as i32,
    }
}

mod abi {
    use core::cell::UnsafeCell;

    use super::*;

    struct Buffers {
        input: [u8; BUFFER_LEN],
        tables: Tables,
    }

    /// wasm32 runs one thread: the exports never overlap.
    struct Shared(UnsafeCell<Option<Buffers>>);
    unsafe impl Sync for Shared {}

    static BUFFERS: Shared = Shared(UnsafeCell::new(None));

    fn buffers() -> &'static mut Buffers {
        // SAFETY: single-threaded, no other borrow of the buffers is alive during an export.
        let slot = unsafe { &mut *BUFFERS.0.get() };
        slot.get_or_insert_with(|| Buffers {
            input: [0; BUFFER_LEN],
            tables: Tables::default(),
        })
    }

    #[no_mangle]
    pub extern "C" fn abi_version() -> u32 {
        ABI_VERSION
    }

    #[no_mangle]
    pub extern "C" fn buffer_len() -> u32 {
        BUFFER_LEN as u32
    }

    #[no_mangle]
    pub extern "C" fn input_ptr() -> *mut u8 {
        buffers().input.as_mut_ptr()
    }

    /// Keeps `input[..input_len]` as the body of registry table `kind` ([`Table`]). Returns 0 or
    /// an [`Error`] code.
    #[no_mangle]
    pub extern "C" fn set_table(kind: u32, input_len: u32) -> i32 {
        let b = buffers();
        let Some(body) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        let slot = match kind {
            k if k == Table::Bestiary as u32 => &mut b.tables.bestiary,
            k if k == Table::MonsterSkills as u32 => &mut b.tables.monster_skills,
            k if k == Table::Rewards as u32 => &mut b.tables.rewards,
            k if k == Table::Tileset as u32 => &mut b.tables.tileset,
            _ => return Error::UnknownTable as i32,
        };
        *slot = body.to_vec();
        0
    }

    /// Checks the `MapCheckRequest` in `input[..input_len]` (see [`check_bytes`]).
    #[no_mangle]
    pub extern "C" fn check_map(input_len: u32) -> i32 {
        let b = buffers();
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        check_bytes(input, &b.tables)
    }

    /// Patch C: the verdict of a whole design (`super::check_design`).
    #[no_mangle]
    pub extern "C" fn check_design(input_len: u32) -> i32 {
        let b = buffers();
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        check_design_bytes(input, &b.tables)
    }

    /// Patch D: the `check_step` transactions of the map in `input[..input_len]`
    /// (`super::check_steps`).
    #[no_mangle]
    pub extern "C" fn check_steps(input_len: u32) -> i32 {
        let b = buffers();
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        check_steps_bytes(input, &b.tables)
    }
}
