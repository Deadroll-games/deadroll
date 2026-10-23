//! Deadroll map checks and pathfinding (docs/MAP_DESCRIPTION.md, section 10).
//!
//! Pure, `no_std`, fixed-size buffers. `programs/map` runs [`check_map`] in `finalize_map`;
//! the website builder runs the same function through WASM.
#![no_std]

pub mod limits;
pub mod loot;
pub mod path;
pub mod roster;
pub mod tiles;

use deadroll_skillcheck::{check_monster_selection, monster::RECORDS_MAX, SkillError};
use deadroll_types::{Cell, MapLayoutData, Tile, SESSION_SKILLS_MAX, SPAWNS_MAX, SPAWN_ELITE};

use limits::{tier_limits, BOSSES, BOSS_HELPERS, PACK_RADIUS, SIDE_MAX, SIDE_MIN};
use loot::{MapLoot, TableRules};
use roster::{Role, SpawnRules};
use tiles::{cell_index, in_bounds, is_passable};

/// The first broken builder rule. One variant per rule; `code()` is stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MapCheckError {
    /// A side is outside `SIDE_MIN..=SIDE_MAX`.
    SizeOutOfRange,
    /// No limits for this map tier.
    TierOutOfRange,
    /// A cell inside the map holds a byte that is not a `Tile`.
    InvalidTile,
    /// Not exactly one `Entrance` tile.
    EntranceCount,
    /// Not exactly one `Exit` tile.
    ExitCount,
    /// `layout.entrance` is not the `Entrance` tile.
    EntranceMismatch,
    /// `layout.exit` is not the `Exit` tile.
    ExitMismatch,
    /// `spawn_count` is above `SPAWNS_MAX`.
    TooManySpawns,
    SpawnOutOfBounds,
    SpawnNotPassable,
    /// Two spawns on one cell, or a spawn on the entrance or the exit.
    SpawnOverlap,
    /// The bestiary id has no role.
    UnknownMonster,
    /// The level is outside the map tier's range.
    LevelOutOfTier,
    /// Not exactly `BOSSES` bosses.
    BossCount,
    /// A boss helper outside the boss pack.
    HelperOutsideBossPack,
    /// A regular monster in the boss pack.
    BossPackMixed,
    /// More than `BOSS_HELPERS` helpers.
    TooManyHelpers,
    /// More regular monsters than the tier allows.
    TooManyMonsters,
    /// More regular packs than the tier allows.
    TooManyPacks,
    /// Two loners in one pack.
    TwoLoners,
    /// Two monsters of one pack farther apart than `PACK_RADIUS`.
    PackTooSpread,
    /// No path from the entrance to the exit.
    ExitUnreachable,
    /// A spawn cannot be reached from the entrance.
    SpawnUnreachable,
    /// Patch 6. The map tier is below the template's minimum tier (BESTIARY 4, MAP 7.2).
    TierBelowTemplate,
    /// A chosen skill bit past the template's pool.
    MonsterSkillUnknown,
    /// Not exactly min(slots, pool) chosen skills of a category (BESTIARY 3.2, MAP 7.2).
    MonsterSkillCount,
    /// A boss marked elite (BESTIARY 5: elite is a regular monster).
    EliteBoss,
    /// More elite spawns than the tier allows.
    TooManyElites,
    /// More distinct chosen skills than a session copies (`SESSION_SKILLS_MAX`).
    TooManySessionSkills,
    // Patch 11: chests and rewards (docs/MAP_DESCRIPTION.md 7.1, 10; PARTY_AND_TURNS.md 9.2).
    /// The map's minimum players is outside `1..=PARTY_MAX`.
    MinPlayersOutOfRange,
    /// More visible chests than the tier allows.
    TooManyChests,
    /// A point whose type is not a visible chest.
    InvalidPoint,
    ChestOutOfBounds,
    ChestNotPassable,
    /// A chest on the entrance, the exit, a spawn or another chest.
    ChestOverlap,
    /// No reward table with this id.
    UnknownTable,
    /// A reward table not allowed on this tier (not listed, or levels above the tier).
    TableNotAllowed,
    /// The boss has no reward table.
    BossTableMissing,
    /// The boss table has one level: no room for the boss guarantee (PARTY_AND_TURNS 9.4).
    BossTableOneLevel,
    /// Chests and boss cost more than the budget for the map's minimum players.
    OverBudget,
    /// No cell next to a chest can be reached from the entrance.
    ChestUnreachable,
}

impl MapCheckError {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Runs every builder rule on a map. `template_of` gives a bestiary id's rules from the registry
/// (`None`: no such template); `loot` the chest and reward table rules (patch 11).
pub fn check_map<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutData,
    template_of: impl Fn(u8) -> Option<SpawnRules>,
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    let role_of = |id: u8| template_of(id).map(|t| t.role);
    use MapCheckError as E;

    let side_ok = |side: u8| (SIDE_MIN..=SIDE_MAX).contains(&side);
    if !side_ok(width) || !side_ok(height) {
        return Err(E::SizeOutOfRange);
    }
    let limits = tier_limits(tier).ok_or(E::TierOutOfRange)?;

    check_tiles(width, height, layout)?;

    let count = layout.spawn_count as usize;
    if count > SPAWNS_MAX {
        return Err(E::TooManySpawns);
    }
    let spawns = &layout.spawns[..count];

    let mut boss_pack = None;
    let mut bosses = 0u8;
    let mut elites = 0u8;
    // Distinct chosen skills, bit per `monster_skills` index.
    let mut skills = [0u64; RECORDS_MAX / 64];
    for (i, spawn) in spawns.iter().enumerate() {
        let cell = Cell {
            x: spawn.x,
            y: spawn.y,
        };
        if !in_bounds(cell, width, height) {
            return Err(E::SpawnOutOfBounds);
        }
        let tile =
            Tile::try_from(layout.tiles[cell_index(cell.x, cell.y)]).map_err(|_| E::InvalidTile)?;
        if !is_passable(tile) {
            return Err(E::SpawnNotPassable);
        }
        let taken = cell == layout.entrance
            || cell == layout.exit
            || spawns[..i].iter().any(|s| s.x == spawn.x && s.y == spawn.y);
        if taken {
            return Err(E::SpawnOverlap);
        }
        let t = template_of(spawn.bestiary_id).ok_or(E::UnknownMonster)?;
        let role = t.role;
        if !(limits.level_min..=limits.level_max).contains(&spawn.level) {
            return Err(E::LevelOutOfTier);
        }
        if tier < t.min_tier {
            return Err(E::TierBelowTemplate);
        }
        let elite = spawn.flags & SPAWN_ELITE != 0;
        if elite && role == Role::Boss {
            return Err(E::EliteBoss);
        }
        elites += elite as u8;
        check_monster_selection(t.pool(), spawn.skills, t.slots_for(elite)).map_err(
            |e| match e {
                SkillError::MonsterSkillUnknown => E::MonsterSkillUnknown,
                _ => E::MonsterSkillCount,
            },
        )?;
        for bit in 0..t.pool_len.min(16) {
            if spawn.skills & (1 << bit) != 0 {
                let at = t.pool_start as usize + bit as usize;
                if at < RECORDS_MAX {
                    skills[at / 64] |= 1 << (at % 64);
                }
            }
        }
        if role == Role::Boss {
            bosses += 1;
            boss_pack = Some(spawn.pack);
        }
    }
    if bosses != BOSSES {
        return Err(E::BossCount);
    }
    if elites > limits.elites {
        return Err(E::TooManyElites);
    }
    if skills.iter().map(|w| w.count_ones()).sum::<u32>() > SESSION_SKILLS_MAX as u32 {
        return Err(E::TooManySessionSkills);
    }
    let boss_pack = boss_pack.ok_or(E::BossCount)?;

    // Roles are known for every spawn here, so `unwrap_or` never hides an id.
    let role = |id: u8| role_of(id).unwrap_or(Role::Swarm);
    let mut helpers = 0u8;
    let mut monsters = 0u8;
    let mut packs = [0u32; 8];
    for spawn in spawns {
        let in_boss_pack = spawn.pack == boss_pack;
        match role(spawn.bestiary_id) {
            Role::Boss => {}
            Role::Helper if in_boss_pack => helpers += 1,
            Role::Helper => return Err(E::HelperOutsideBossPack),
            _ if in_boss_pack => return Err(E::BossPackMixed),
            _ => {
                monsters += 1;
                packs[spawn.pack as usize / 32] |= 1 << (spawn.pack % 32);
            }
        }
    }
    if helpers > BOSS_HELPERS {
        return Err(E::TooManyHelpers);
    }
    if monsters > limits.monsters {
        return Err(E::TooManyMonsters);
    }
    if packs.iter().map(|w| w.count_ones()).sum::<u32>() > limits.packs as u32 {
        return Err(E::TooManyPacks);
    }

    for (i, a) in spawns.iter().enumerate() {
        for b in &spawns[i + 1..] {
            if a.pack != b.pack {
                continue;
            }
            if role(a.bestiary_id) == Role::Loner && role(b.bestiary_id) == Role::Loner {
                return Err(E::TwoLoners);
            }
            if a.x.abs_diff(b.x).max(a.y.abs_diff(b.y)) > PACK_RADIUS {
                return Err(E::PackTooSpread);
            }
        }
    }

    loot::check_chests(width, height, limits.level_max, layout, loot)?;

    // Closed chests occupy their cell: the paths go around them.
    let (chests, chest_count) = loot::chest_cells(layout);
    let reach = path::reachable_avoiding(
        &layout.tiles,
        width,
        height,
        layout.entrance,
        &chests[..chest_count],
    );
    if !reach.contains(layout.exit) {
        return Err(E::ExitUnreachable);
    }
    if spawns
        .iter()
        .any(|s| !reach.contains(Cell { x: s.x, y: s.y }))
    {
        return Err(E::SpawnUnreachable);
    }
    loot::check_chest_access(width, height, layout, &reach)
}

/// Every cell inside the map is a `Tile`, with exactly one entrance and one exit that match
/// the layout points.
fn check_tiles(width: u8, height: u8, layout: &MapLayoutData) -> Result<(), MapCheckError> {
    use MapCheckError as E;

    let (mut entrances, mut exits) = (0u16, 0u16);
    let (mut entrance, mut exit) = (Cell::default(), Cell::default());
    // Raw byte compares: this runs per cell on-chain.
    const ENTRANCE: u8 = Tile::Entrance as u8;
    const EXIT: u8 = Tile::Exit as u8;
    for y in 0..height {
        let row = cell_index(0, y);
        for (x, &byte) in layout.tiles[row..row + width as usize].iter().enumerate() {
            match byte {
                ENTRANCE => {
                    entrances += 1;
                    entrance = Cell { x: x as u8, y };
                }
                EXIT => {
                    exits += 1;
                    exit = Cell { x: x as u8, y };
                }
                b if b as usize >= Tile::COUNT => return Err(E::InvalidTile),
                _ => {}
            }
        }
    }
    if entrances != 1 {
        return Err(E::EntranceCount);
    }
    if exits != 1 {
        return Err(E::ExitCount);
    }
    if layout.entrance != entrance {
        return Err(E::EntranceMismatch);
    }
    if layout.exit != exit {
        return Err(E::ExitMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_skillcheck::Category;
    use deadroll_types::{MapPoint, PointKind, Pubkey, Spawn, CHESTS_MAX, MAP_CELLS, NO_TABLE};

    /// Test bestiary ids (the shipped registry order).
    mod ids {
        pub const WOLF: u8 = 0;
        pub const GOBLIN: u8 = 1;
        pub const GOBLIN_CHIEFTAIN: u8 = 2;
        pub const GOBLIN_WARRIOR: u8 = 3;
        pub const GOBLIN_SHAMAN: u8 = 4;
    }

    /// Roles of the test ids; no skill pools.
    fn p0_role(id: u8) -> Option<SpawnRules> {
        let role = match id {
            ids::WOLF => Role::Fast,
            ids::GOBLIN => Role::Swarm,
            ids::GOBLIN_CHIEFTAIN => Role::Boss,
            ids::GOBLIN_WARRIOR | ids::GOBLIN_SHAMAN => Role::Helper,
            _ => return None,
        };
        Some(SpawnRules::plain(role))
    }

    const ENTRANCE: Cell = Cell { x: 1, y: 1 };
    const EXIT: Cell = Cell { x: 14, y: 14 };

    fn spawn(bestiary_id: u8, x: u8, y: u8, pack: u8) -> Spawn {
        Spawn {
            bestiary_id,
            x,
            y,
            level: 1,
            pack,
            ..Spawn::default()
        }
    }

    /// A valid tier 1, 16x16 map: open room in a wall ring, wolves, goblins, boss pack.
    fn demo() -> MapLayoutData {
        let mut tiles = [Tile::Wall as u8; MAP_CELLS];
        for y in 1..15 {
            for x in 1..15 {
                tiles[cell_index(x, y)] = Tile::Floor as u8;
            }
        }
        tiles[cell_index(ENTRANCE.x, ENTRANCE.y)] = Tile::Entrance as u8;
        tiles[cell_index(EXIT.x, EXIT.y)] = Tile::Exit as u8;
        let mut spawns = [Spawn::default(); SPAWNS_MAX];
        let list = [
            spawn(ids::WOLF, 6, 2, 1),
            spawn(ids::WOLF, 7, 3, 1),
            spawn(ids::GOBLIN, 2, 10, 2),
            spawn(ids::GOBLIN, 3, 11, 2),
            spawn(ids::GOBLIN_CHIEFTAIN, 12, 12, 3),
            spawn(ids::GOBLIN_WARRIOR, 11, 12, 3),
            spawn(ids::GOBLIN_SHAMAN, 12, 11, 3),
        ];
        spawns[..list.len()].copy_from_slice(&list);
        MapLayoutData {
            version: 1,
            map: Pubkey::default(),
            tiles,
            spawns,
            spawn_count: list.len() as u8,
            entrance: ENTRANCE,
            exit: EXIT,
            bump: 0,
            chests: [chest(5, 5, tables::SMALL); CHESTS_MAX],
            chest_count: 1,
            boss_table: tables::BOSS,
        }
    }

    /// Test reward tables.
    mod tables {
        /// Levels 1, cost 2.
        pub const SMALL: u8 = 0;
        /// Levels 1-2, cost 4.
        pub const BOSS: u8 = 1;
        /// Levels 3-4 (tier 2), cost 5.
        pub const HIGH: u8 = 2;
        /// Levels 1-2, cost 3, not in the allowed mask.
        pub const UNLISTED: u8 = 3;
    }

    fn table_of(id: u8) -> Option<TableRules> {
        let (level_base, level_max, cost) = match id {
            tables::SMALL => (1, 1, 2),
            tables::BOSS => (1, 2, 4),
            tables::HIGH => (3, 4, 5),
            tables::UNLISTED => (1, 2, 3),
            _ => return None,
        };
        Some(TableRules {
            level_base,
            level_max,
            cost,
        })
    }

    type TestLoot = MapLoot<fn(u8) -> Option<TableRules>>;

    fn test_loot() -> TestLoot {
        loot_for(1)
    }

    fn loot_for(min_players: u8) -> TestLoot {
        MapLoot {
            min_players,
            rules: loot::LootRules {
                chests: 2,
                budget: [6, 8, 10, 12],
                allowed: 0b0111,
            },
            table_of,
        }
    }

    fn chest(x: u8, y: u8, table: u8) -> MapPoint {
        MapPoint {
            kind: PointKind::Chest,
            x,
            y,
            data: table,
        }
    }

    fn check(layout: &MapLayoutData) -> Result<(), MapCheckError> {
        check_map(16, 16, 1, layout, p0_role, &test_loot())
    }

    fn set(layout: &mut MapLayoutData, x: u8, y: u8, tile: Tile) {
        layout.tiles[cell_index(x, y)] = tile as u8;
    }

    #[test]
    fn demo_is_valid() {
        assert_eq!(check(&demo()), Ok(()));
    }

    #[test]
    fn largest_map_is_valid() {
        let mut layout = demo();
        layout.tiles = [Tile::Floor as u8; MAP_CELLS];
        set(&mut layout, 0, 0, Tile::Entrance);
        set(&mut layout, 31, 31, Tile::Exit);
        layout.entrance = Cell { x: 0, y: 0 };
        layout.exit = Cell { x: 31, y: 31 };
        assert_eq!(check_map(32, 32, 1, &layout, p0_role, &test_loot()), Ok(()));
    }

    #[test]
    fn rejects_size() {
        let layout = demo();
        assert_eq!(
            check_map(7, 16, 1, &layout, p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
        assert_eq!(
            check_map(16, 33, 1, &layout, p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
    }

    #[test]
    fn rejects_tier() {
        let layout = demo();
        assert_eq!(
            check_map(16, 16, 0, &layout, p0_role, &test_loot()),
            Err(MapCheckError::TierOutOfRange)
        );
        assert_eq!(
            check_map(16, 16, 3, &layout, p0_role, &test_loot()),
            Err(MapCheckError::TierOutOfRange)
        );
    }

    #[test]
    fn rejects_unknown_tile_byte_inside_the_map() {
        let mut layout = demo();
        layout.tiles[cell_index(5, 5)] = Tile::COUNT as u8;
        assert_eq!(check(&layout), Err(MapCheckError::InvalidTile));
    }

    #[test]
    fn ignores_bytes_outside_the_map() {
        let mut layout = demo();
        layout.tiles[cell_index(20, 20)] = u8::MAX;
        layout.tiles[cell_index(16, 0)] = Tile::Entrance as u8;
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_entrance_count() {
        let mut layout = demo();
        set(&mut layout, 1, 1, Tile::Floor);
        assert_eq!(check(&layout), Err(MapCheckError::EntranceCount));
        set(&mut layout, 1, 1, Tile::Entrance);
        set(&mut layout, 2, 1, Tile::Entrance);
        assert_eq!(check(&layout), Err(MapCheckError::EntranceCount));
    }

    #[test]
    fn rejects_exit_count() {
        let mut layout = demo();
        set(&mut layout, 14, 14, Tile::Floor);
        assert_eq!(check(&layout), Err(MapCheckError::ExitCount));
        set(&mut layout, 14, 14, Tile::Exit);
        set(&mut layout, 13, 14, Tile::Exit);
        assert_eq!(check(&layout), Err(MapCheckError::ExitCount));
    }

    #[test]
    fn rejects_point_mismatch() {
        let mut layout = demo();
        layout.entrance = Cell { x: 2, y: 1 };
        assert_eq!(check(&layout), Err(MapCheckError::EntranceMismatch));
        let mut layout = demo();
        layout.exit = Cell { x: 14, y: 13 };
        assert_eq!(check(&layout), Err(MapCheckError::ExitMismatch));
    }

    #[test]
    fn rejects_too_many_spawns() {
        let mut layout = demo();
        layout.spawn_count = SPAWNS_MAX as u8 + 1;
        assert_eq!(check(&layout), Err(MapCheckError::TooManySpawns));
    }

    #[test]
    fn rejects_spawn_out_of_bounds() {
        let mut layout = demo();
        layout.spawns[0].x = 16;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOutOfBounds));
    }

    #[test]
    fn rejects_spawn_on_wall() {
        let mut layout = demo();
        set(&mut layout, 6, 2, Tile::Wall);
        assert_eq!(check(&layout), Err(MapCheckError::SpawnNotPassable));
    }

    #[test]
    fn rejects_overlap() {
        let mut layout = demo();
        layout.spawns[1].x = 6;
        layout.spawns[1].y = 2;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOverlap));
        let mut layout = demo();
        layout.spawns[0].x = ENTRANCE.x;
        layout.spawns[0].y = ENTRANCE.y;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOverlap));
        let mut layout = demo();
        layout.spawns[4].x = EXIT.x;
        layout.spawns[4].y = EXIT.y;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOverlap));
    }

    #[test]
    fn rejects_unknown_monster() {
        let mut layout = demo();
        layout.spawns[0].bestiary_id = 200;
        assert_eq!(check(&layout), Err(MapCheckError::UnknownMonster));
    }

    #[test]
    fn rejects_level_outside_tier() {
        let mut layout = demo();
        layout.spawns[0].level = 3;
        assert_eq!(check(&layout), Err(MapCheckError::LevelOutOfTier));
        layout.spawns[0].level = 0;
        assert_eq!(check(&layout), Err(MapCheckError::LevelOutOfTier));
        // Level 3 is fine on tier 2, level 1 is not.
        let mut layout = demo();
        for s in layout.spawns.iter_mut() {
            s.level = 3;
        }
        assert_eq!(check_map(16, 16, 2, &layout, p0_role, &test_loot()), Ok(()));
    }

    #[test]
    fn rejects_boss_count() {
        let mut layout = demo();
        layout.spawns[4].bestiary_id = ids::GOBLIN;
        layout.spawns[4].pack = 2;
        layout.spawns[4].x = 4;
        layout.spawns[4].y = 10;
        assert_eq!(check(&layout), Err(MapCheckError::BossCount));
        let mut layout = demo();
        layout.spawns[7] = spawn(ids::GOBLIN_CHIEFTAIN, 13, 12, 3);
        layout.spawn_count = 8;
        assert_eq!(check(&layout), Err(MapCheckError::BossCount));
    }

    #[test]
    fn rejects_helper_outside_boss_pack() {
        let mut layout = demo();
        layout.spawns[5].pack = 2;
        layout.spawns[5].x = 4;
        layout.spawns[5].y = 10;
        assert_eq!(check(&layout), Err(MapCheckError::HelperOutsideBossPack));
    }

    #[test]
    fn rejects_regular_monster_in_boss_pack() {
        let mut layout = demo();
        layout.spawns[3].pack = 3;
        layout.spawns[3].x = 13;
        layout.spawns[3].y = 11;
        assert_eq!(check(&layout), Err(MapCheckError::BossPackMixed));
    }

    #[test]
    fn rejects_too_many_helpers() {
        let mut layout = demo();
        layout.spawns[7] = spawn(ids::GOBLIN_WARRIOR, 13, 12, 3);
        layout.spawn_count = 8;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyHelpers));
    }

    #[test]
    fn rejects_too_many_monsters() {
        let mut layout = demo();
        layout.spawns[7] = spawn(ids::WOLF, 8, 2, 1);
        layout.spawn_count = 8;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyMonsters));
    }

    #[test]
    fn rejects_too_many_packs() {
        let mut layout = demo();
        // Four single-monster packs: 4 monsters fit tier 1, 4 packs do not.
        layout.spawns[1].pack = 4;
        layout.spawns[3].pack = 5;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyPacks));
    }

    #[test]
    fn rejects_two_loners_in_a_pack() {
        let layout = demo();
        let wolves_are_loners = |id| {
            if id == ids::WOLF {
                Some(SpawnRules::plain(Role::Loner))
            } else {
                p0_role(id)
            }
        };
        assert_eq!(
            check_map(16, 16, 1, &layout, wolves_are_loners, &test_loot()),
            Err(MapCheckError::TwoLoners)
        );
        // One loner per pack is fine.
        let mut layout = demo();
        layout.spawns[1].bestiary_id = ids::GOBLIN;
        assert_eq!(
            check_map(16, 16, 1, &layout, wolves_are_loners, &test_loot()),
            Ok(())
        );
    }

    #[test]
    fn rejects_spread_pack() {
        let mut layout = demo();
        layout.spawns[1].x = 6 + PACK_RADIUS + 1;
        assert_eq!(check(&layout), Err(MapCheckError::PackTooSpread));
        layout.spawns[1].x = 6 + PACK_RADIUS;
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_map_without_path_to_exit() {
        let mut layout = demo();
        for x in 1..15 {
            set(&mut layout, x, 13, Tile::Wall);
        }
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
    }

    #[test]
    fn rejects_unreachable_spawn() {
        let mut layout = demo();
        // Wall in the first wolf.
        for (x, y) in [(5, 2), (7, 2), (6, 1), (6, 3)] {
            set(&mut layout, x, y, Tile::Wall);
        }
        layout.spawns[1].x = 7;
        layout.spawns[1].y = 4;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnUnreachable));
    }

    #[test]
    fn codes_are_sequential() {
        assert_eq!(MapCheckError::SizeOutOfRange.code(), 0);
        assert_eq!(MapCheckError::SpawnUnreachable.code(), 22);
        assert_eq!(MapCheckError::TooManySessionSkills.code(), 28);
        assert_eq!(MapCheckError::MinPlayersOutOfRange.code(), 29);
        assert_eq!(MapCheckError::ChestUnreachable.code(), 40);
    }

    #[test]
    fn map_without_chests_is_valid() {
        let mut layout = demo();
        layout.chest_count = 0;
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_min_players_out_of_range() {
        let layout = demo();
        for bad in [0, 5] {
            assert_eq!(
                check_map(16, 16, 1, &layout, p0_role, &loot_for(bad)),
                Err(MapCheckError::MinPlayersOutOfRange)
            );
        }
        assert_eq!(check_map(16, 16, 1, &layout, p0_role, &loot_for(4)), Ok(()));
    }

    #[test]
    fn rejects_too_many_chests() {
        let mut layout = demo();
        layout.chests[1] = chest(6, 6, tables::SMALL);
        layout.chest_count = 2;
        assert_eq!(
            check_map(16, 16, 1, &layout, p0_role, &loot_for(2)),
            Ok(()),
            "the tier allows two"
        );
        layout.chests[2] = chest(7, 7, tables::SMALL);
        layout.chest_count = 3;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyChests));
        layout.chest_count = CHESTS_MAX as u8 + 1;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyChests));
    }

    #[test]
    fn rejects_chest_placement() {
        let mut layout = demo();
        layout.chests[0] = chest(16, 5, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ChestOutOfBounds));
        layout.chests[0] = chest(0, 5, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ChestNotPassable));
        for cell in [ENTRANCE, EXIT, Cell { x: 6, y: 2 }] {
            layout.chests[0] = chest(cell.x, cell.y, tables::SMALL);
            assert_eq!(check(&layout), Err(MapCheckError::ChestOverlap));
        }
        layout.chests[0] = chest(5, 5, tables::SMALL);
        layout.chests[1] = chest(5, 5, tables::SMALL);
        layout.chest_count = 2;
        assert_eq!(check(&layout), Err(MapCheckError::ChestOverlap));
    }

    #[test]
    fn rejects_point_that_is_not_a_chest() {
        let mut layout = demo();
        // A byte past `PointKind` cannot be decoded; a future variant would be rejected here too.
        layout.chests[0].kind = PointKind::Chest;
        assert_eq!(check(&layout), Ok(()));
        assert_eq!(PointKind::COUNT, 1);
    }

    #[test]
    fn rejects_tables() {
        let mut layout = demo();
        layout.chests[0].data = 9;
        assert_eq!(check(&layout), Err(MapCheckError::UnknownTable));
        layout.chests[0].data = tables::UNLISTED;
        assert_eq!(check(&layout), Err(MapCheckError::TableNotAllowed));
        layout.chests[0].data = tables::HIGH;
        assert_eq!(
            check(&layout),
            Err(MapCheckError::TableNotAllowed),
            "levels above tier 1"
        );
        layout.chests[0].data = tables::SMALL;
        layout.boss_table = NO_TABLE;
        assert_eq!(check(&layout), Err(MapCheckError::BossTableMissing));
        layout.boss_table = tables::SMALL;
        assert_eq!(check(&layout), Err(MapCheckError::BossTableOneLevel));
        layout.boss_table = tables::UNLISTED;
        assert_eq!(check(&layout), Err(MapCheckError::TableNotAllowed));
    }

    #[test]
    fn budget_grows_with_min_players() {
        let mut layout = demo();
        // Two chests (2 + 2) and the boss (4): 8 points.
        layout.chests[1] = chest(6, 6, tables::SMALL);
        layout.chest_count = 2;
        assert_eq!(
            check_map(16, 16, 1, &layout, p0_role, &loot_for(1)),
            Err(MapCheckError::OverBudget)
        );
        assert_eq!(check_map(16, 16, 1, &layout, p0_role, &loot_for(2)), Ok(()));
    }

    #[test]
    fn closed_chest_blocks_the_path() {
        let mut layout = demo();
        // A wall at x = 13 with one gap at (13, 7) between the room and the exit (14, 14).
        for y in 1..15 {
            set(&mut layout, 13, y, Tile::Wall);
        }
        set(&mut layout, 13, 7, Tile::Floor);
        assert_eq!(check(&layout), Ok(()));
        layout.chests[0] = chest(13, 7, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
    }

    #[test]
    fn closed_chest_hides_a_spawn() {
        let mut layout = demo();
        // Wall off the wolves' corner but one gap, then put the chest in the gap.
        for x in 1..=8 {
            set(&mut layout, x, 4, Tile::Wall);
        }
        for y in 1..4 {
            set(&mut layout, 8, y, Tile::Wall);
        }
        set(&mut layout, 3, 4, Tile::Floor);
        layout.entrance = Cell { x: 1, y: 6 };
        set(&mut layout, ENTRANCE.x, ENTRANCE.y, Tile::Floor);
        set(&mut layout, 1, 6, Tile::Entrance);
        assert_eq!(check(&layout), Ok(()));
        layout.chests[0] = chest(3, 4, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::SpawnUnreachable));
    }

    #[test]
    fn chest_needs_a_reachable_neighbor() {
        let mut layout = demo();
        // A chest in a walled pocket: every neighbor is a wall.
        for (x, y) in [
            (4, 4),
            (5, 4),
            (6, 4),
            (4, 5),
            (6, 5),
            (4, 6),
            (5, 6),
            (6, 6),
        ] {
            set(&mut layout, x, y, Tile::Wall);
        }
        assert_eq!(check(&layout), Err(MapCheckError::ChestUnreachable));
        // One open diagonal neighbor is enough.
        set(&mut layout, 6, 6, Tile::Floor);
        assert_eq!(check(&layout), Ok(()));
    }

    /// Wolves with a pool of 4 actives and a passive at `start`.
    fn with_pool(start: u8) -> impl Fn(u8) -> Option<SpawnRules> {
        move |id| {
            let mut t = p0_role(id)?;
            if id == ids::WOLF {
                t.pool_start = start;
                t.pool_len = 5;
                t.pool[4] = Category::Passive;
            }
            Some(t)
        }
    }

    #[test]
    fn spawn_skills_must_fill_their_slots() {
        let mut layout = demo();
        let roster = with_pool(0);
        // No skills chosen while the pool has some.
        assert_eq!(
            check_map(16, 16, 1, &layout, &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[0].skills = 0b1_0111;
        layout.spawns[1].skills = 0b1_1110;
        assert_eq!(check_map(16, 16, 1, &layout, &roster, &test_loot()), Ok(()));
        layout.spawns[1].skills = 0b10_0111;
        assert_eq!(
            check_map(16, 16, 1, &layout, &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillUnknown)
        );
        // An elite takes all four actives.
        layout.spawns[1].skills = 0b1_1111;
        assert_eq!(
            check_map(16, 16, 1, &layout, &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[1].flags = SPAWN_ELITE;
        assert_eq!(check_map(16, 16, 1, &layout, &roster, &test_loot()), Ok(()));
    }

    #[test]
    fn elites_are_limited_and_never_bosses() {
        let mut layout = demo();
        layout.spawns[0].flags = SPAWN_ELITE;
        assert_eq!(check(&layout), Ok(()));
        layout.spawns[1].flags = SPAWN_ELITE;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyElites));
        assert_eq!(
            check_map(16, 16, 2, &layout, p0_role, &test_loot()),
            Err(MapCheckError::LevelOutOfTier)
        );
        let mut layout = demo();
        layout.spawns[4].flags = SPAWN_ELITE;
        assert_eq!(check(&layout), Err(MapCheckError::EliteBoss));
    }

    #[test]
    fn the_template_minimum_tier_holds() {
        let layout = demo();
        let high = |id| {
            let mut t = p0_role(id)?;
            t.min_tier = 2;
            Some(t)
        };
        assert_eq!(
            check_map(16, 16, 1, &layout, high, &test_loot()),
            Err(MapCheckError::TierBelowTemplate)
        );
    }

    #[test]
    fn a_session_copies_at_most_its_skill_table() {
        let mut layout = demo();
        // Each goblin and helper with a pool of 16 actives at its own place: 2 x 3 + ... skills.
        let roster = |id: u8| {
            let mut t = p0_role(id)?;
            t.pool_len = 16;
            t.pool_start = id * 16;
            t.slots = [[16, 0, 0], [16, 0, 0], [16, 0, 0]];
            Some(t)
        };
        for s in layout.spawns[..7].iter_mut() {
            s.skills = u16::MAX;
        }
        // 5 templates x 16 distinct skills = 80 > 32.
        assert_eq!(
            check_map(16, 16, 1, &layout, roster, &test_loot()),
            Err(MapCheckError::TooManySessionSkills)
        );
        // The same two templates share their skills: 2 x 16 = 32 fit.
        for s in layout.spawns[..7].iter_mut() {
            if s.bestiary_id > ids::GOBLIN {
                s.skills = 0;
            }
        }
        let two = |id: u8| {
            let mut t = roster(id)?;
            if id > ids::GOBLIN {
                t.pool_len = 0;
            }
            Some(t)
        };
        assert_eq!(check_map(16, 16, 1, &layout, two, &test_loot()), Ok(()));
    }
}
