//! C ABI over `deadroll-mapcheck` for the website map builder (docs/MAP_DESCRIPTION.md 1, 10): the
//! verdict `finalize_map` gives a draft, before any transaction. See `README.md` next to this crate.
//!
//! The rules are not copied: the page loads the registry table bodies the program reads
//! (`set_table`), and [`check`] decodes them with `deadroll_rules` (`tiles`, `map_tables`, the tier
//! loot of `BalanceConfig::DEFAULT`) and runs `check_map`, as `finalize_map` does.
//!
//! Static buffers, no imports. The page writes the borsh [`MapCheckRequest`] into the input buffer
//! and calls `check_map`. Patch C: `check_design` takes a [`DesignRequest`] (the same map, the
//! declared secret counts and every secret) and runs `deadroll_mapcheck::design::check_design`,
//! the public part `finalize_map` checks and each pack and secret as the session checks them when
//! revealed. Built with `std` for the same reason as the combat module
//! (`solana-pubkey` links `std` off SBF).

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_mapcheck::{check_map, loot::MapLoot, roster::SpawnRules, MapCheckError};
use deadroll_rules::{loot::map_rules, map_tables, tiles::tile_rules, BalanceConfig};
use deadroll_types::{FoundSecret, MapArenasData, MapLayoutDataV2, SECRET_TYPES};

/// Bumped on any change of the exports or the byte layouts (2: the arenas, arena update).
pub const ABI_VERSION: u32 = 3;
/// Size of the input buffer: a request is 2505 bytes, a table body at most 4096.
pub const BUFFER_LEN: usize = 4096;
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

/// A draft as `finalize_map` sees it: the `Map` numbers, the layout core and (arena update) the
/// packs' arenas that follow it in the `MapLayout` account.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapCheckRequest {
    pub width: u8,
    pub height: u8,
    pub tier: u8,
    pub min_players: u8,
    pub layout: MapLayoutDataV2,
    pub arenas: MapArenasData,
}

/// The registry table bodies (as far as written), by [`Table`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tables {
    pub bestiary: Vec<u8>,
    pub monster_skills: Vec<u8>,
    pub rewards: Vec<u8>,
    pub tileset: Vec<u8>,
}

/// Every builder rule on a draft, with the registry tables: the `MapCheckError` `finalize_map`
/// would return, or `Ok`.
pub fn check(request: &MapCheckRequest, tables: &Tables) -> Result<(), MapCheckError> {
    let cfg = &BalanceConfig::DEFAULT;
    let layout = &request.layout;
    // The templates the spawns name, decoded once each (as `finalize_map` caches them).
    let mut templates: [Option<SpawnRules>; BESTIARY_MAX] = [None; BESTIARY_MAX];
    for spawn in &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())] {
        if let Some(slot) = templates.get_mut(spawn.bestiary_id as usize) {
            *slot = map_tables::spawn_rules(
                &tables.bestiary,
                &tables.monster_skills,
                spawn.bestiary_id,
                &cfg.monsters,
            );
        }
    }
    let loot = MapLoot {
        min_players: request.min_players,
        rules: map_rules(&cfg.loot, request.tier),
        table_of: |id: u8| map_tables::table_rules(&tables.rewards, id),
    };
    check_map(
        request.width,
        request.height,
        request.tier,
        layout,
        &request.arenas,
        cfg.arena.limits(),
        &tile_rules(&tables.tileset),
        |id| templates.get(id as usize).copied().flatten(),
        &loot,
    )
}

/// Decodes a borsh `MapCheckRequest` and checks it: `0` for a valid map, `1 + code` for the first
/// broken rule (`MapCheckError::code`), or a negative [`Error`].
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
    let layout = &m.layout;
    let mut templates: [Option<SpawnRules>; BESTIARY_MAX] = [None; BESTIARY_MAX];
    for spawn in &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())] {
        if let Some(slot) = templates.get_mut(spawn.bestiary_id as usize) {
            *slot = map_tables::spawn_rules(
                &tables.bestiary,
                &tables.monster_skills,
                spawn.bestiary_id,
                &cfg.monsters,
            );
        }
    }
    let loot = MapLoot {
        min_players: m.min_players,
        rules: map_rules(&cfg.loot, m.tier),
        table_of: |id: u8| map_tables::table_rules(&tables.rewards, id),
    };
    let limits = cfg
        .secrets
        .limits(m.tier)
        .ok_or(MapCheckError::TierOutOfRange)?;
    deadroll_mapcheck::design::check_design(
        m.width,
        m.height,
        m.tier,
        layout,
        &m.arenas,
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

pub fn check_bytes(input: &[u8], tables: &Tables) -> i32 {
    let Ok(request) = MapCheckRequest::try_from_slice(input) else {
        return Error::BadInput as i32;
    };
    match check(&request, tables) {
        Ok(()) => 0,
        Err(e) => 1 + e.code() as i32,
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
}
