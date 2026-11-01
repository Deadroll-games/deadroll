//! In-memory accounts for unit tests of the in-place views (no validator needed).
use anchor_lang::prelude::*;
use anchor_lang::AccountSerialize;
use deadroll_types::{
    cell_byte, Cell, GameSessionData, MapArenasData, MapLayoutDataV2, MapPoint, Monster,
    MonsterStateData, PackLayout, PointKind, SessionStatus, Spawn, TileKind, ARENA_MONSTERS_MAX,
    ARENA_SIDE_MAX, MAP_CELLS, MAP_MAX, MONSTERS_MAX, PARTY_MAX, POINTS_MAX, SESSION_TEMPLATES_MAX,
    SPAWNS_MAX, TILE_KINDS_MAX, TURN_ORDER_MAX,
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
    let byte = |kind: TileKind| cell_byte(kind as u8, 0);
    let mut cells = [byte(TileKind::Wall); MAP_CELLS];
    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            cells[y * MAP_MAX + x] = byte(TileKind::StoneFloor);
        }
    }
    cells[WATER.y as usize * MAP_MAX + WATER.x as usize] = byte(TileKind::ShallowWater);
    cells[WALL.y as usize * MAP_MAX + WALL.x as usize] = byte(TileKind::Wall);
    let mut spawns = [Spawn::default(); SPAWNS_MAX];
    spawns[0] = Spawn {
        bestiary_id: 1,
        x: 5,
        y: 5,
        level: 2,
        ..Spawn::default()
    };
    let point = |kind, x, y| MapPoint {
        kind,
        x,
        y,
        data: 0,
    };
    let mut points = [MapPoint::default(); POINTS_MAX];
    points[0] = point(PointKind::Entrance, 0, 0);
    points[1] = point(PointKind::Exit, 7, 7);
    let layout = MapLayoutDataV2 {
        version: crate::state::external::LAYOUT_VERSION,
        map,
        cells,
        spawns,
        spawn_count: 1,
        points,
        point_count: 2,
        boss_table: deadroll_types::NO_TABLE,
        bump: 255,
    };
    // Arena update: pack 0's arena is the same 8x8 ground (the map's corner), so the views read
    // the same cells on the map and in that arena.
    let mut arenas = MapArenasData::EMPTY;
    arenas.packs[0] = PackLayout {
        visible: 0,
        x: 5,
        y: 5,
        facing: deadroll_types::Facing::N,
        width: WIDTH,
        height: HEIGHT,
    };
    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            arenas.cells[0][y * ARENA_SIDE_MAX + x] = cells[y * MAP_MAX + x];
        }
    }
    let mut data = MAP_LAYOUT_DISCRIMINATOR.to_vec();
    layout.serialize(&mut data).unwrap();
    arenas.serialize(&mut data).unwrap();
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

/// A `MonsterState` account for `session` with `monsters` in the arena slots, serialized exactly
/// as Anchor would; each monster's bestiary record is the shipped one of its `bestiary_id` (as
/// `enter_arena` copies it), and the party's templates are every shipped one (as `open_session`
/// copies the map's).
pub fn monsters_account(session: Pubkey, monsters: &[Monster]) -> TestAccount {
    assert!(monsters.len() <= ARENA_MONSTERS_MAX);
    let mut all = [Monster::default(); MONSTERS_MAX];
    all[..monsters.len()].copy_from_slice(monsters);
    let reg = registry();
    let mut records = [[0; super::MONSTER_RECORD_LEN]; ARENA_MONSTERS_MAX];
    for (r, m) in records.iter_mut().zip(monsters) {
        if let Some(record) = reg.record(m.bestiary_id) {
            *r = record;
        }
    }
    let mut template_ids = [super::NO_TEMPLATE; SESSION_TEMPLATES_MAX];
    let mut templates = [[0; super::MONSTER_RECORD_LEN]; SESSION_TEMPLATES_MAX];
    let mut t = 0;
    for id in 0..SESSION_TEMPLATES_MAX as u8 {
        if let Some(record) = reg.record(id) {
            template_ids[t] = id;
            templates[t] = record;
            t += 1;
        }
    }
    // Patch 11: every shipped reward table, by id, as `open_session` copies the ones a map needs.
    let mut loot_ids = [deadroll_types::NO_TABLE; super::LOOT_TABLES_MAX];
    let mut loot_tables = [[0; super::LOOT_RECORD_LEN]; super::LOOT_TABLES_MAX];
    let mut n = 0;
    for id in 0..super::LOOT_TABLES_MAX as u8 {
        if let Some(t) = reg.reward_table(id) {
            loot_ids[n] = id;
            loot_tables[n] = t.to_record();
            n += 1;
        }
    }
    let state = MonsterState {
        core: MonsterStateData {
            version: super::MONSTER_STATE_VERSION,
            session,
            monsters: all,
            count: monsters.len() as u8,
            bump: 254,
        },
        mods: [Default::default(); ARENA_MONSTERS_MAX],
        defense: [Default::default(); ARENA_MONSTERS_MAX],
        res_aspects: [0; super::RES_ASPECTS_LEN],
        res_traits: [0; super::RES_TRAITS_LEN],
        res_pairs: [0; super::RES_PAIRS_LEN],
        burning_cells: [Default::default(); deadroll_rules::spread::BURNING_CELLS_MAX],
        records,
        skill_table: [[0; super::SKILL_COPY_LEN]; deadroll_types::SESSION_SKILLS_MAX],
        skill_count: 0,
        skills: [[deadroll_rules::monster::NO_SKILL; deadroll_types::MONSTER_SLOTS];
            ARENA_MONSTERS_MAX],
        skill_cooldowns: [[0; deadroll_types::MONSTER_SLOTS]; ARENA_MONSTERS_MAX],
        auras: [Default::default(); ARENA_MONSTERS_MAX],
        resist_mods: Default::default(),
        aura_weights: Default::default(),
        loot_ids,
        loot_tables,
        res_triples: [0; super::RES_TRIPLES_LEN],
        spawn_of: [0; ARENA_MONSTERS_MAX],
        template_ids,
        templates,
        skill_ids: [0; deadroll_types::SESSION_SKILLS_MAX],
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
        tier: 1,
        min_players: 1,
        chests: Default::default(),
        chest_count: 0,
        boss_table: deadroll_types::NO_TABLE,
        chests_opened: 0,
        pending_chest: super::NO_CHEST,
        // Full experience, rewards rolled for everyone.
        xp_bp: [deadroll_types::BP as u16; PARTY_MAX],
        loot_mask: 0b1111,
        unpaid_kills: 0,
        ended_at: 0,
        chest_radius: 3,
        settle_timeout_secs: 600,
        // In an arena by default: the views see the test monsters as its pack, on the test
        // map's cells (pack 0's arena is the map's 8x8 corner, `layout_account`).
        mode: super::ArenaMode::Arena,
        arena_pack: 0,
        engaged_by: 0,
        packs: [deadroll_types::PackLayout::EMPTY; deadroll_types::PACKS_MAX],
        packs_alive: 0,
        map_width: WIDTH,
        map_height: HEIGHT,
        map_pos: [Cell::default(); PARTY_MAX],
        map_order: [EMPTY_UNIT; PARTY_MAX],
        map_order_len: 0,
        map_turn_index: 0,
        map_round: 1,
        unit_time: [0; super::ARENA_UNITS],
        player_dex: [0; PARTY_MAX],
    }
}

/// `assets/registry/tileset.json` by tile type id (`TileKind` order: void, stone floor, wooden
/// floor, dirt, wall, door, shallow water, deep water, stairs, rubble, chasm), as
/// `read_tile_costs` copies it.
pub const TILE_COST: [u8; TILE_KINDS_MAX] = {
    let mut costs = [0; TILE_KINDS_MAX];
    let starter = [0, 1, 1, 1, 0, 1, 2, 0, 1, 2, 0];
    let mut i = 0;
    while i < starter.len() {
        costs[i] = starter[i];
        i += 1;
    }
    costs
};

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
