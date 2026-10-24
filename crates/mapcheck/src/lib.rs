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
pub mod walk;

use deadroll_skillcheck::{check_monster_selection, monster::RECORDS_MAX, SkillError};
use deadroll_types::{
    Cell, MapLayoutDataV2, PointKind, POINTS_MAX, SESSION_SKILLS_MAX, SPAWNS_MAX, SPAWN_ELITE,
};

use limits::{tier_limits, BOSSES, BOSS_HELPERS, PACK_RADIUS, SIDE_MAX, SIDE_MIN};
use loot::{MapLoot, TableRules};
use roster::{Role, SpawnRules};
use tiles::{cell_index, in_bounds, TileRules};

/// The first broken builder rule. One variant per rule; `code()` is stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MapCheckError {
    /// A side is outside `SIDE_MIN..=SIDE_MAX`.
    SizeOutOfRange,
    /// No limits for this map tier.
    TierOutOfRange,
    /// A cell inside the map has a tile type the `Tileset` does not have (patch 13; before it,
    /// a byte that is not a `Tile`).
    InvalidTile,
    /// Not exactly one entrance point (patch 13; before it, the `Entrance` tile).
    EntranceCount,
    /// Not exactly one exit point (patch 13; before it, the `Exit` tile).
    ExitCount,
    /// Unused since patch 13 (the entrance tile and field are gone).
    EntranceMismatch,
    /// Unused since patch 13 (the exit tile and field are gone).
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
    /// Unused since patch 13 (a point may be the entrance, the exit or a chest).
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
    // Patch 13: the entrance and the exit are points (docs/MAP_DESCRIPTION.md 7.1).
    /// `point_count` is above `POINTS_MAX`.
    TooManyPoints,
    /// The entrance or the exit is outside the map.
    PointOutOfBounds,
    /// The entrance or the exit is on a cell that cannot be entered.
    PointNotPassable,
    /// The entrance and the exit on one cell.
    PointOverlap,
}

impl MapCheckError {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Runs every builder rule on a map. `tiles` are the registry `Tileset` (patch 13);
/// `template_of` gives a bestiary id's rules from the registry (`None`: no such template);
/// `loot` the chest and reward table rules (patch 11).
pub fn check_map<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutDataV2,
    tiles: &TileRules,
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

    check_tiles(width, height, layout, tiles)?;
    let ends = check_points(width, height, layout, tiles)?;
    let [entrance, exit] = ends;

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
        if !tiles.is_passable(layout.cells[cell_index(cell.x, cell.y)]) {
            return Err(E::SpawnNotPassable);
        }
        let taken =
            ends.contains(&cell) || spawns[..i].iter().any(|s| s.x == spawn.x && s.y == spawn.y);
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

    loot::check_chests(width, height, limits.level_max, layout, tiles, ends, loot)?;

    // Closed chests occupy their cell: the paths go around them.
    let (chests, chest_count) = loot::chest_cells(layout);
    let reach = path::reachable_avoiding(
        &layout.cells,
        tiles,
        width,
        height,
        entrance,
        &chests[..chest_count],
    );
    if !reach.contains(exit) {
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

/// Every cell inside the map has a tile type the `Tileset` has (any variant).
fn check_tiles(
    width: u8,
    height: u8,
    layout: &MapLayoutDataV2,
    tiles: &TileRules,
) -> Result<(), MapCheckError> {
    for y in 0..height {
        let row = cell_index(0, y);
        if !layout.cells[row..row + width as usize]
            .iter()
            .all(|&b| tiles.knows(b))
        {
            return Err(MapCheckError::InvalidTile);
        }
    }
    Ok(())
}

/// Exactly one entrance and one exit point, inside the map, on cells that can be entered and
/// apart. Returns `[entrance, exit]`. Chests are `loot::check_chests`'s.
fn check_points(
    width: u8,
    height: u8,
    layout: &MapLayoutDataV2,
    tiles: &TileRules,
) -> Result<[Cell; 2], MapCheckError> {
    use MapCheckError as E;

    if layout.point_count as usize > POINTS_MAX {
        return Err(E::TooManyPoints);
    }
    let mut ends = [None; 2];
    for point in layout.point_list() {
        let (end, twice) = match point.kind {
            PointKind::Entrance => (0, E::EntranceCount),
            PointKind::Exit => (1, E::ExitCount),
            PointKind::Chest => continue,
        };
        if ends[end].is_some() {
            return Err(twice);
        }
        let cell = point.cell();
        if !in_bounds(cell, width, height) {
            return Err(E::PointOutOfBounds);
        }
        if !tiles.is_passable(layout.cells[cell_index(cell.x, cell.y)]) {
            return Err(E::PointNotPassable);
        }
        ends[end] = Some(cell);
    }
    let entrance = ends[0].ok_or(E::EntranceCount)?;
    let exit = ends[1].ok_or(E::ExitCount)?;
    if entrance == exit {
        return Err(E::PointOverlap);
    }
    Ok([entrance, exit])
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_skillcheck::Category;
    use deadroll_types::{cell_byte, MapPoint, Pubkey, Spawn, TileKind, MAP_CELLS, NO_TABLE};

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

    fn tiles() -> TileRules {
        tiles::tests::starter()
    }

    const FLOOR: u8 = TileKind::StoneFloor as u8 * 8;
    const WALL: u8 = TileKind::Wall as u8 * 8;

    fn point(kind: PointKind, cell: Cell, data: u8) -> MapPoint {
        MapPoint {
            kind,
            x: cell.x,
            y: cell.y,
            data,
        }
    }

    /// A valid tier 1, 16x16 map: open room in a wall ring, wolves, goblins, boss pack.
    /// Points: the entrance, the exit, then the chests.
    fn demo() -> MapLayoutDataV2 {
        let mut cells = [WALL; MAP_CELLS];
        for y in 1..15 {
            for x in 1..15 {
                cells[cell_index(x, y)] = FLOOR;
            }
        }
        let mut points = [MapPoint::default(); POINTS_MAX];
        points[..3].copy_from_slice(&[
            point(PointKind::Entrance, ENTRANCE, 0),
            point(PointKind::Exit, EXIT, 0),
            chest(5, 5, tables::SMALL),
        ]);
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
        MapLayoutDataV2 {
            version: 1,
            map: Pubkey::default(),
            cells,
            spawns,
            spawn_count: list.len() as u8,
            points,
            point_count: 3,
            boss_table: tables::BOSS,
            bump: 0,
        }
    }

    /// Replaces the chests (the points after the entrance and the exit).
    fn set_chests(layout: &mut MapLayoutDataV2, chests: &[MapPoint]) {
        layout.points[2..2 + chests.len()].copy_from_slice(chests);
        layout.point_count = 2 + chests.len() as u8;
    }

    fn chest0(layout: &mut MapLayoutDataV2) -> &mut MapPoint {
        &mut layout.points[2]
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

    fn check(layout: &MapLayoutDataV2) -> Result<(), MapCheckError> {
        check_map(16, 16, 1, layout, &tiles(), p0_role, &test_loot())
    }

    fn set(layout: &mut MapLayoutDataV2, x: u8, y: u8, kind: TileKind) {
        layout.cells[cell_index(x, y)] = kind as u8 * 8;
    }

    #[test]
    fn demo_is_valid() {
        assert_eq!(check(&demo()), Ok(()));
    }

    #[test]
    fn largest_map_is_valid() {
        let mut layout = demo();
        layout.cells = [FLOOR; MAP_CELLS];
        layout.points[0] = point(PointKind::Entrance, Cell { x: 0, y: 0 }, 0);
        layout.points[1] = point(PointKind::Exit, Cell { x: 31, y: 31 }, 0);
        assert_eq!(
            check_map(32, 32, 1, &layout, &tiles(), p0_role, &test_loot()),
            Ok(())
        );
    }

    #[test]
    fn rejects_size() {
        let layout = demo();
        assert_eq!(
            check_map(7, 16, 1, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
        assert_eq!(
            check_map(16, 33, 1, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
    }

    #[test]
    fn rejects_tier() {
        let layout = demo();
        assert_eq!(
            check_map(16, 16, 0, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::TierOutOfRange)
        );
        assert_eq!(
            check_map(16, 16, 3, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::TierOutOfRange)
        );
    }

    #[test]
    fn rejects_unknown_tile_type_inside_the_map() {
        let mut layout = demo();
        layout.cells[cell_index(5, 7)] = cell_byte(TileKind::COUNT as u8, 0);
        assert_eq!(check(&layout), Err(MapCheckError::InvalidTile));
        // A reserved id the Tileset gets later is fine from then on.
        let mut more = tiles();
        more.known |= 1 << TileKind::COUNT;
        more.cost[TileKind::COUNT] = 1;
        assert_eq!(
            check_map(16, 16, 1, &layout, &more, p0_role, &test_loot()),
            Ok(())
        );
    }

    #[test]
    fn every_starter_tile_and_variant_is_known() {
        let mut layout = demo();
        for (i, &kind) in TileKind::ALL.iter().enumerate() {
            layout.cells[cell_index(1 + i as u8, 8)] = cell_byte(kind as u8, 7);
        }
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn ignores_bytes_outside_the_map() {
        let mut layout = demo();
        layout.cells[cell_index(20, 20)] = u8::MAX;
        layout.cells[cell_index(16, 0)] = u8::MAX;
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn the_variant_does_not_change_passability() {
        let mut layout = demo();
        for x in 1..15 {
            layout.cells[cell_index(x, 13)] = cell_byte(TileKind::Dirt as u8, x % 8);
        }
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_entrance_count() {
        let mut layout = demo();
        layout.points[0] = chest(2, 2, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::EntranceCount));
        let mut layout = demo();
        layout.points[3] = point(PointKind::Entrance, Cell { x: 2, y: 1 }, 0);
        layout.point_count = 4;
        assert_eq!(check(&layout), Err(MapCheckError::EntranceCount));
    }

    #[test]
    fn rejects_exit_count() {
        let mut layout = demo();
        layout.point_count = 1;
        assert_eq!(check(&layout), Err(MapCheckError::ExitCount));
        let mut layout = demo();
        layout.points[3] = point(PointKind::Exit, Cell { x: 13, y: 14 }, 0);
        layout.point_count = 4;
        assert_eq!(check(&layout), Err(MapCheckError::ExitCount));
    }

    #[test]
    fn points_may_come_in_any_order() {
        let mut layout = demo();
        layout.points.swap(0, 2);
        layout.points.swap(1, 2);
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_entrance_and_exit_placement() {
        let mut layout = demo();
        layout.point_count = POINTS_MAX as u8 + 1;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyPoints));
        let mut layout = demo();
        layout.points[0].x = 16;
        assert_eq!(check(&layout), Err(MapCheckError::PointOutOfBounds));
        let mut layout = demo();
        layout.points[1].y = 16;
        assert_eq!(check(&layout), Err(MapCheckError::PointOutOfBounds));
        let mut layout = demo();
        layout.points[0] = point(PointKind::Entrance, Cell { x: 0, y: 1 }, 0);
        assert_eq!(check(&layout), Err(MapCheckError::PointNotPassable));
        for kind in [TileKind::Void, TileKind::DeepWater, TileKind::Chasm] {
            let mut layout = demo();
            set(&mut layout, EXIT.x, EXIT.y, kind);
            assert_eq!(
                check(&layout),
                Err(MapCheckError::PointNotPassable),
                "{kind:?}"
            );
        }
        let mut layout = demo();
        layout.points[1] = point(PointKind::Exit, ENTRANCE, 0);
        assert_eq!(check(&layout), Err(MapCheckError::PointOverlap));
    }

    #[test]
    fn exit_may_be_on_stairs_or_water() {
        let mut layout = demo();
        set(&mut layout, EXIT.x, EXIT.y, TileKind::Stairs);
        assert_eq!(check(&layout), Ok(()));
        set(&mut layout, EXIT.x, EXIT.y, TileKind::ShallowWater);
        assert_eq!(check(&layout), Ok(()));
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
        set(&mut layout, 6, 2, TileKind::Wall);
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
        assert_eq!(
            check_map(16, 16, 2, &layout, &tiles(), p0_role, &test_loot()),
            Ok(())
        );
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
            check_map(
                16,
                16,
                1,
                &layout,
                &tiles(),
                wolves_are_loners,
                &test_loot()
            ),
            Err(MapCheckError::TwoLoners)
        );
        // One loner per pack is fine.
        let mut layout = demo();
        layout.spawns[1].bestiary_id = ids::GOBLIN;
        assert_eq!(
            check_map(
                16,
                16,
                1,
                &layout,
                &tiles(),
                wolves_are_loners,
                &test_loot()
            ),
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
            set(&mut layout, x, 13, TileKind::Wall);
        }
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
    }

    #[test]
    fn rejects_unreachable_spawn() {
        let mut layout = demo();
        // Wall in the first wolf.
        for (x, y) in [
            (5, 1),
            (6, 1),
            (7, 1),
            (5, 2),
            (7, 2),
            (5, 3),
            (6, 3),
            (7, 3),
        ] {
            set(&mut layout, x, y, TileKind::Wall);
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
        assert_eq!(MapCheckError::PointOverlap.code(), 44);
    }

    #[test]
    fn diagonals_do_not_cut_corners() {
        // Two wall rows across the room, open at (7, 8) and (8, 9): the diagonal step
        // (7, 8) -> (8, 9) passes between the walls (8, 8) and (7, 9), so the halves stay apart.
        let mut layout = demo();
        for x in 1..15 {
            set(&mut layout, x, 8, TileKind::Wall);
            set(&mut layout, x, 9, TileKind::Wall);
        }
        set(&mut layout, 7, 8, TileKind::StoneFloor);
        set(&mut layout, 8, 9, TileKind::StoneFloor);
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
        // One impassable corner is enough to block: (8, 8) deep water, (7, 9) still a wall.
        set(&mut layout, 8, 8, TileKind::DeepWater);
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
        // Both corners passable: the halves link.
        set(&mut layout, 8, 8, TileKind::Rubble);
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn deep_water_and_chasm_cut_the_map_rubble_does_not() {
        for (kind, verdict) in [
            (TileKind::DeepWater, Err(MapCheckError::ExitUnreachable)),
            (TileKind::Chasm, Err(MapCheckError::ExitUnreachable)),
            (TileKind::Void, Err(MapCheckError::ExitUnreachable)),
            (TileKind::Rubble, Ok(())),
            (TileKind::ShallowWater, Ok(())),
        ] {
            let mut layout = demo();
            for x in 1..15 {
                set(&mut layout, x, 13, kind);
            }
            assert_eq!(check(&layout), verdict, "{kind:?}");
        }
    }

    #[test]
    fn map_without_chests_is_valid() {
        let mut layout = demo();
        set_chests(&mut layout, &[]);
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_min_players_out_of_range() {
        let layout = demo();
        for bad in [0, 5] {
            assert_eq!(
                check_map(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(bad)),
                Err(MapCheckError::MinPlayersOutOfRange)
            );
        }
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(4)),
            Ok(())
        );
    }

    #[test]
    fn rejects_too_many_chests() {
        let mut layout = demo();
        let small = |x, y| chest(x, y, tables::SMALL);
        set_chests(&mut layout, &[small(5, 5), small(6, 6)]);
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(2)),
            Ok(()),
            "the tier allows two"
        );
        set_chests(&mut layout, &[small(5, 5), small(6, 6), small(7, 7)]);
        assert_eq!(check(&layout), Err(MapCheckError::TooManyChests));
        set_chests(
            &mut layout,
            &[small(5, 5), small(6, 6), small(7, 7), small(8, 8)],
        );
        assert_eq!(check(&layout), Err(MapCheckError::TooManyChests));
    }

    #[test]
    fn rejects_chest_placement() {
        let mut layout = demo();
        *chest0(&mut layout) = chest(16, 5, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ChestOutOfBounds));
        *chest0(&mut layout) = chest(0, 5, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ChestNotPassable));
        set(&mut layout, 0, 5, TileKind::DeepWater);
        assert_eq!(check(&layout), Err(MapCheckError::ChestNotPassable));
        for cell in [ENTRANCE, EXIT, Cell { x: 6, y: 2 }] {
            *chest0(&mut layout) = chest(cell.x, cell.y, tables::SMALL);
            assert_eq!(check(&layout), Err(MapCheckError::ChestOverlap));
        }
        let small = chest(5, 5, tables::SMALL);
        set_chests(&mut layout, &[small, small]);
        assert_eq!(check(&layout), Err(MapCheckError::ChestOverlap));
    }

    #[test]
    fn rejects_tables() {
        let mut layout = demo();
        chest0(&mut layout).data = 9;
        assert_eq!(check(&layout), Err(MapCheckError::UnknownTable));
        chest0(&mut layout).data = tables::UNLISTED;
        assert_eq!(check(&layout), Err(MapCheckError::TableNotAllowed));
        chest0(&mut layout).data = tables::HIGH;
        assert_eq!(
            check(&layout),
            Err(MapCheckError::TableNotAllowed),
            "levels above tier 1"
        );
        chest0(&mut layout).data = tables::SMALL;
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
        set_chests(
            &mut layout,
            &[chest(5, 5, tables::SMALL), chest(6, 6, tables::SMALL)],
        );
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(1)),
            Err(MapCheckError::OverBudget)
        );
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(2)),
            Ok(())
        );
    }

    #[test]
    fn closed_chest_blocks_the_path() {
        let mut layout = demo();
        // A wall at x = 13 with one gap at (13, 7) between the room and the exit (14, 14).
        for y in 1..15 {
            set(&mut layout, 13, y, TileKind::Wall);
        }
        set(&mut layout, 13, 7, TileKind::StoneFloor);
        assert_eq!(check(&layout), Ok(()));
        *chest0(&mut layout) = chest(13, 7, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::ExitUnreachable));
    }

    #[test]
    fn closed_chest_hides_a_spawn() {
        let mut layout = demo();
        // Wall off the wolves' corner but one gap, then put the chest in the gap.
        for x in 1..=8 {
            set(&mut layout, x, 4, TileKind::Wall);
        }
        for y in 1..4 {
            set(&mut layout, 8, y, TileKind::Wall);
        }
        set(&mut layout, 3, 4, TileKind::StoneFloor);
        layout.points[0] = point(PointKind::Entrance, Cell { x: 1, y: 6 }, 0);
        assert_eq!(check(&layout), Ok(()));
        *chest0(&mut layout) = chest(3, 4, tables::SMALL);
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
            set(&mut layout, x, y, TileKind::Wall);
        }
        assert_eq!(check(&layout), Err(MapCheckError::ChestUnreachable));
        // One open diagonal neighbor is enough.
        set(&mut layout, 6, 6, TileKind::StoneFloor);
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
            check_map(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[0].skills = 0b1_0111;
        layout.spawns[1].skills = 0b1_1110;
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Ok(())
        );
        layout.spawns[1].skills = 0b10_0111;
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillUnknown)
        );
        // An elite takes all four actives.
        layout.spawns[1].skills = 0b1_1111;
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[1].flags = SPAWN_ELITE;
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Ok(())
        );
    }

    #[test]
    fn elites_are_limited_and_never_bosses() {
        let mut layout = demo();
        layout.spawns[0].flags = SPAWN_ELITE;
        assert_eq!(check(&layout), Ok(()));
        layout.spawns[1].flags = SPAWN_ELITE;
        assert_eq!(check(&layout), Err(MapCheckError::TooManyElites));
        assert_eq!(
            check_map(16, 16, 2, &layout, &tiles(), p0_role, &test_loot()),
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
            check_map(16, 16, 1, &layout, &tiles(), high, &test_loot()),
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
            check_map(16, 16, 1, &layout, &tiles(), roster, &test_loot()),
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
        assert_eq!(
            check_map(16, 16, 1, &layout, &tiles(), two, &test_loot()),
            Ok(())
        );
    }
}
