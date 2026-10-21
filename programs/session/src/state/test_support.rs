//! In-memory accounts for unit tests of the in-place views (no validator needed).
use anchor_lang::prelude::*;
use anchor_lang::AccountSerialize;
use deadroll_types::{
    Cell, GameSessionData, MapLayoutData, Monster, MonsterStateData, SessionStatus, Spawn, Tile,
    MAP_CELLS, MAP_MAX, MONSTERS_MAX, PARTY_MAX, SPAWNS_MAX, TURN_ORDER_MAX,
};

use super::{external::MAP_LAYOUT_DISCRIMINATOR, GameSession, MonsterState};
use crate::rules::EMPTY_UNIT;

/// Owns the key, lamports and bytes an `AccountInfo` borrows.
pub struct TestAccount {
    pub key: Pubkey,
    pub owner: Pubkey,
    pub lamports: u64,
    pub data: Vec<u8>,
}

impl TestAccount {
    pub fn new(owner: Pubkey, data: Vec<u8>) -> Self {
        Self {
            key: Pubkey::new_unique(),
            owner,
            lamports: 1,
            data,
        }
    }

    pub fn info(&mut self) -> AccountInfo<'_> {
        AccountInfo::new(
            &self.key,
            false,
            true,
            &mut self.lamports,
            &mut self.data,
            &self.owner,
            false,
        )
    }
}

pub const WIDTH: u8 = 8;
pub const HEIGHT: u8 = 8;
pub const WATER: Cell = Cell { x: 1, y: 0 };
pub const WALL: Cell = Cell { x: 0, y: 2 };

/// 8x8 floor (rest of the 32x32 grid is wall), water at (1,0), wall at (0,2).
pub fn layout_account(map: Pubkey) -> TestAccount {
    let mut tiles = [Tile::Wall as u8; MAP_CELLS];
    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            tiles[y * MAP_MAX + x] = Tile::Floor as u8;
        }
    }
    tiles[WATER.y as usize * MAP_MAX + WATER.x as usize] = Tile::Water as u8;
    tiles[WALL.y as usize * MAP_MAX + WALL.x as usize] = Tile::Wall as u8;
    let mut spawns = [Spawn::default(); SPAWNS_MAX];
    spawns[0] = Spawn {
        bestiary_id: 1,
        x: 5,
        y: 5,
        level: 2,
        ..Spawn::default()
    };
    let layout = MapLayoutData {
        version: 1,
        map,
        tiles,
        spawns,
        spawn_count: 1,
        entrance: Cell { x: 0, y: 0 },
        exit: Cell { x: 7, y: 7 },
        bump: 255,
    };
    let mut data = MAP_LAYOUT_DISCRIMINATOR.to_vec();
    layout.serialize(&mut data).unwrap();
    data.extend_from_slice(&[0; 16]);
    TestAccount::new(deadroll_map::ID, data)
}

pub fn monster(x: u8, y: u8) -> Monster {
    Monster {
        bestiary_id: 1,
        level: 2,
        x,
        y,
        hp: 20,
        hp_max: 20,
        action_points: 4,
        alive: true,
        ..Monster::default()
    }
}

/// A `MonsterState` account for `session`, serialized exactly as Anchor would; each monster's
/// bestiary record is the shipped one of its `bestiary_id` (as `open_session` copies it).
pub fn monsters_account(session: Pubkey, monsters: &[Monster]) -> TestAccount {
    let mut all = [Monster::default(); MONSTERS_MAX];
    all[..monsters.len()].copy_from_slice(monsters);
    let reg = registry();
    let mut records = [[0; super::MONSTER_RECORD_LEN]; MONSTERS_MAX];
    for (r, m) in records.iter_mut().zip(monsters) {
        if let Some(record) = reg.record(m.bestiary_id) {
            *r = record;
        }
    }
    let state = MonsterState {
        core: MonsterStateData {
            version: 1,
            session,
            monsters: all,
            count: monsters.len() as u8,
            bump: 254,
        },
        cooldowns: [[0; super::MONSTER_SKILLS_MAX]; MONSTERS_MAX],
        mods: [Default::default(); MONSTERS_MAX],
        defense: [Default::default(); MONSTERS_MAX],
        res_aspects: [0; super::RES_ASPECTS_LEN],
        res_traits: [0; super::RES_TRAITS_LEN],
        res_pairs: [0; super::RES_PAIRS_LEN],
        burning_cells: [Default::default(); deadroll_rules::spread::BURNING_CELLS_MAX],
        records,
        skill_table: [[0; super::SKILL_COPY_LEN]; deadroll_types::SESSION_SKILLS_MAX],
        skill_count: 0,
        skills: [[deadroll_rules::monster::NO_SKILL; deadroll_types::MONSTER_SLOTS]; MONSTERS_MAX],
        skill_cooldowns: [[0; deadroll_types::MONSTER_SLOTS]; MONSTERS_MAX],
        auras: [Default::default(); MONSTERS_MAX],
        resist_mods: Default::default(),
        aura_weights: Default::default(),
    };
    let mut data = Vec::new();
    state.try_serialize(&mut data).unwrap();
    TestAccount::new(crate::ID, data)
}

/// An active session on the test map with `players` at the given cells, all alive.
pub fn session(players: &[Cell]) -> GameSession {
    let mut player_pos = [Cell::default(); PARTY_MAX];
    player_pos[..players.len()].copy_from_slice(players);
    GameSession {
        core: GameSessionData {
            version: 1,
            map: Pubkey::new_unique(),
            dm: Pubkey::new_unique(),
            status: SessionStatus::Active,
            price_lamports: 0,
            players: [Pubkey::default(); PARTY_MAX],
            player_count: players.len() as u8,
            turn_order: [EMPTY_UNIT; TURN_ORDER_MAX],
            turn_index: 0,
            round: 1,
            action_counter: 0,
            pending_vrf: false,
            last_seed: [0; 32],
            bump: 255,
        },
        layout: Pubkey::new_unique(),
        width: WIDTH,
        height: HEIGHT,
        entrance: Cell { x: 0, y: 0 },
        dm_actor: Pubkey::default(),
        turn_order_len: 0,
        turn_seq: 1,
        turn_started_at: 0,
        player_pos,
        alive_mask: ((1u16 << players.len()) - 1) as u8,
        ready_mask: 0,
        pending: Default::default(),
        flee_mask: 0,
        settled_mask: 0,
        xp_pool: 0,
        victory_seed: [0; 32],
        monster_speed: [4; MONSTERS_MAX],
        monster_initiative: [0; MONSTERS_MAX],
        player_initiative: [0; PARTY_MAX],
        tile_cost: TILE_COST,
        acted_seq: 0,
        started_seq: 0,
        pending_target: Pubkey::default(),
        participations: [Pubkey::default(); PARTY_MAX],
        reactives: Default::default(),
        auras: Default::default(),
        pending_reactive: Default::default(),
        pending_monster_reactive: Default::default(),
        pending_deaths: 0,
        aura_weights: Default::default(),
    }
}

/// `assets/registry/tileset.json` in `Tile` order: Floor, Wall, Water, Door, Entrance, Exit, Trap.
pub const TILE_COST: [u8; Tile::COUNT] = [1, 0, 2, 1, 1, 1, 1];

/// The shipped registry resonance tables (assets/registry), as `open_session` copies them.
pub fn tables() -> deadroll_combat::ResonanceTables {
    deadroll_rules::registry::Registry::shipped()
        .expect("assets/registry")
        .tables
}

/// Test bestiary ids (the shipped `assets/registry/bestiary.json` order).
#[allow(dead_code)]
pub mod ids {
    pub const WOLF: u8 = 0;
    pub const GOBLIN: u8 = 1;
    pub const GOBLIN_CHIEFTAIN: u8 = 2;
    pub const GOBLIN_WARRIOR: u8 = 3;
    pub const GOBLIN_SHAMAN: u8 = 4;
}

/// The shipped registry (assets/registry).
pub fn registry() -> deadroll_rules::registry::Registry {
    deadroll_rules::registry::Registry::shipped().expect("assets/registry")
}

/// The shipped template of bestiary id `id` at level 1, with its registry defenses.
pub fn profile(id: u8) -> deadroll_rules::MonsterProfile {
    registry()
        .template(id)
        .expect("a shipped bestiary id")
        .profile(1, &crate::rules::BALANCE.monsters)
}
