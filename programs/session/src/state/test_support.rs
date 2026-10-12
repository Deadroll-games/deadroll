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

/// A `MonsterState` account for `session`, serialized exactly as Anchor would.
pub fn monsters_account(session: Pubkey, monsters: &[Monster]) -> TestAccount {
    let mut all = [Monster::default(); MONSTERS_MAX];
    all[..monsters.len()].copy_from_slice(monsters);
    let state = MonsterState {
        core: MonsterStateData {
            version: 1,
            session,
            monsters: all,
            count: monsters.len() as u8,
            bump: 254,
        },
        cooldowns: [[0; super::MONSTER_SKILLS_MAX]; MONSTERS_MAX],
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
    }
}

/// `assets/registry/tileset.json` in `Tile` order: Floor, Wall, Water, Door, Entrance, Exit, Trap.
pub const TILE_COST: [u8; Tile::COUNT] = [1, 0, 2, 1, 1, 1, 1];
