//! Deadroll map checks and pathfinding (docs/MAP_DESCRIPTION.md, section 10).
//!
//! Pure, `no_std`, fixed-size buffers. `programs/map` runs [`check_map`] in `finalize_map`;
//! the website builder runs the same function through WASM.
#![no_std]

pub mod arena;
pub mod design;
pub mod footprint;
pub mod limits;
pub mod loot;
pub mod path;
pub mod roster;
pub mod secrets;
pub mod tiles;
pub mod walk;

use deadroll_skillcheck::{check_monster_selection, monster::RECORDS_MAX, SkillError};
use deadroll_types::{
    Cell, MapArenasData, MapLayoutDataV2, MapSecretsData, PointKind, Spawn, PACKS_MAX, POINTS_MAX,
    SESSION_SKILLS_MAX, SESSION_TEMPLATES_MAX, SPAWNS_MAX, SPAWN_ELITE,
};

use arena::ArenaLimits;
use limits::{tier_limits, TierLimits, BOSSES, BOSS_HELPERS, SIDE_MAX, SIDE_MIN};
use loot::{MapLoot, TableRules};
use roster::{Role, SpawnRules};
use secrets::SecretLimits;
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
    /// Arena update: a spawn outside its pack's arena (before it, outside the map).
    SpawnOutOfBounds,
    /// Arena update: a spawn on an arena cell that cannot be entered.
    SpawnNotPassable,
    /// Two spawns of one pack on one arena cell (before the arena update: two spawns on one map
    /// cell, or a spawn on the entrance or the exit).
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
    /// Unused since the arena update (a pack stands in its own arena).
    PackTooSpread,
    /// No path from the entrance to the exit.
    ExitUnreachable,
    /// Unused since the arena update (`PackUnreachable`, `ArenaUnreachable`).
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
    // Arena update (docs/MAP_DESCRIPTION.md 17.2, 17.3).
    /// A spawn's pack number is not below `PACKS_MAX`.
    PackOutOfRange,
    /// A spawn's pack has no arena.
    PackWithoutArena,
    /// More monsters in one pack than an arena holds (`ARENA_MONSTERS_MAX`).
    PackTooBig,
    /// An arena side outside the limits of `BalanceConfig.arena`.
    ArenaSizeOutOfRange,
    /// An arena cell with a tile type the `Tileset` does not have.
    ArenaInvalidTile,
    /// The monster shown on the map is not a spawn of its pack.
    VisibleNotInPack,
    /// The visible monster's map cell is outside the map.
    PackOutOfBounds,
    /// The visible monster's map cell cannot be entered.
    PackNotPassable,
    /// The visible monster stands on the entrance, the exit, a chest or another pack's cell.
    PackOverlap,
    /// The visible monster stands next to the entrance: the party would start in the fight.
    PackAtEntrance,
    /// No cell next to the visible monster can be reached from the entrance.
    PackUnreachable,
    /// From some side the arena has no free cells for a full party.
    ArenaNoRoom,
    /// From some side the entering party is split, or cannot reach a monster.
    ArenaUnreachable,
    /// More distinct bestiary templates than a session copies (`SESSION_TEMPLATES_MAX`).
    TooManyTemplates,
    // Patch C: secrets and hidden packs (docs/MAP_DESCRIPTION.md 8, 17.2, 17.4; `secrets`).
    /// A cell's contents with a bad kind for its place or nonzero unused bytes.
    SecretInvalid,
    /// A trap or an ambush on a cell that cannot be entered.
    SecretNotPassable,
    /// A trap or an ambush on the entrance, the exit, a chest or a pack's map cell.
    SecretOnPoint,
    /// Trap damage 0 or above the tier's `trap_damage_max`.
    TrapDamageOutOfRange,
    /// An ambush linked to a pack number that is not an ambush pack.
    AmbushPackInvalid,
    /// An ambush trap in an arena (it starts a fight from the map).
    AmbushInArena,
    /// A hidden stash in an arena.
    StashInArena,
    /// A secret door on a cell that can be entered.
    DoorNotWall,
    /// More declared secrets than the tier allows, or more secrets of a type than declared.
    TooManySecrets,
    /// Fewer declared traps than the tier needs.
    TooFewTraps,
    /// Not exactly one ambush trap per ambush pack.
    AmbushUnlinked,
    /// A used pack declared with threat 0, or an unused one with a threat.
    ThreatOutOfRange,
    /// A pack's composition does not give its declared threat.
    ThreatMismatch,
    /// A composition with no monster, too many, or one of another pack.
    RosterInvalid,
    /// A composition whose first monster is not the pack's public one.
    VisibleMismatch,
    /// A public spawn that is not the visible monster of its pack.
    SpawnNotVisible,
    /// Fewer secrets of a type than declared.
    TooFewSecrets,
    /// Two secrets on one leaf.
    DuplicateSecret,
}

impl MapCheckError {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Runs every builder rule on a map. `tiles` are the registry `Tileset` (patch 13);
/// `template_of` gives a bestiary id's rules from the registry (`None`: no such template);
/// `loot` the chest and reward table rules (patch 11); `arenas` and `arena_limits` the packs'
/// arenas (arena update, [`arena`]).
#[allow(clippy::too_many_arguments)]
pub fn check_map<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    arena_limits: ArenaLimits,
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
    // Distinct templates, bit per bestiary id.
    let mut templates = [0u64; 4];
    for spawn in spawns {
        let t = template_of(spawn.bestiary_id).ok_or(E::UnknownMonster)?;
        let id = spawn.bestiary_id as usize;
        templates[id / 64] |= 1 << (id % 64);
        let role = t.role;
        let elite = check_spawn(tier, &limits, spawn, &t)?;
        elites += elite as u8;
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
    if templates.iter().map(|w| w.count_ones()).sum::<u32>() > SESSION_TEMPLATES_MAX as u32 {
        return Err(E::TooManyTemplates);
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
            if a.pack == b.pack
                && role(a.bestiary_id) == Role::Loner
                && role(b.bestiary_id) == Role::Loner
            {
                return Err(E::TwoLoners);
            }
        }
    }

    check_ground(
        width,
        height,
        limits.level_max,
        layout,
        arenas,
        arena_limits,
        tiles,
        ends,
        &template_of,
        loot,
    )
}

/// Patch C: the public part of a map, what `finalize_map` checks (docs/MAP_DESCRIPTION.md 8.3,
/// 17.2, 17.5). `layout.spawns` holds only the monster each pack shows on the map; the packs'
/// compositions and the secrets are under the Merkle root, checked when revealed
/// ([`arena::check_roster`], [`secrets::check_secret`]). `secrets` gives the declared threats
/// and counts; `secret_limits` the tier's limits.
#[allow(clippy::too_many_arguments)]
pub fn check_public<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    secrets: &MapSecretsData,
    arena_limits: ArenaLimits,
    secret_limits: &SecretLimits,
    tiles: &TileRules,
    template_of: impl Fn(u8) -> Option<SpawnRules>,
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;

    let side_ok = |side: u8| (SIDE_MIN..=SIDE_MAX).contains(&side);
    if !side_ok(width) || !side_ok(height) {
        return Err(E::SizeOutOfRange);
    }
    let limits = tier_limits(tier).ok_or(E::TierOutOfRange)?;
    check_tiles(width, height, layout, tiles)?;
    let ends = check_points(width, height, layout, tiles)?;

    let count = layout.spawn_count as usize;
    if count > PACKS_MAX {
        return Err(E::TooManySpawns);
    }
    for (i, spawn) in layout.spawns[..count].iter().enumerate() {
        let t = template_of(spawn.bestiary_id).ok_or(E::UnknownMonster)?;
        check_spawn(tier, &limits, spawn, &t)?;
        let p = arenas
            .packs
            .get(spawn.pack as usize)
            .ok_or(E::PackOutOfRange)?;
        if !p.is_used() {
            return Err(E::PackWithoutArena);
        }
        if p.visible as usize != i {
            return Err(E::SpawnNotVisible);
        }
    }
    let used = arenas.packs.iter().filter(|p| p.is_used()).count();
    if used > (limits.packs + BOSSES) as usize {
        return Err(E::TooManyPacks);
    }
    for (p, threat) in arenas.packs.iter().zip(secrets.threat) {
        if p.is_used() != (threat > 0) {
            return Err(E::ThreatOutOfRange);
        }
    }
    let ambush = secrets::ambush_mask(&arenas.packs);
    secrets::check_declared(&secrets.declared, secret_limits, ambush.count_ones() as u8)?;
    check_ground(
        width,
        height,
        limits.level_max,
        layout,
        arenas,
        arena_limits,
        tiles,
        ends,
        &template_of,
        loot,
    )
}

/// The rules of one spawn by itself: level within the tier, the template's minimum tier, a boss
/// never elite, the chosen skills. Returns whether it is elite.
pub fn check_spawn(
    tier: u8,
    limits: &TierLimits,
    spawn: &Spawn,
    t: &SpawnRules,
) -> Result<bool, MapCheckError> {
    use MapCheckError as E;
    if !(limits.level_min..=limits.level_max).contains(&spawn.level) {
        return Err(E::LevelOutOfTier);
    }
    if tier < t.min_tier {
        return Err(E::TierBelowTemplate);
    }
    let elite = spawn.flags & SPAWN_ELITE != 0;
    if elite && t.role == Role::Boss {
        return Err(E::EliteBoss);
    }
    check_monster_selection(t.pool(), spawn.skills, t.slots_for(elite)).map_err(|e| match e {
        SkillError::MonsterSkillUnknown => E::MonsterSkillUnknown,
        _ => E::MonsterSkillCount,
    })?;
    Ok(elite)
}

/// The chests, the path to the exit and the arenas, the same for the whole design and its public
/// part.
#[allow(clippy::too_many_arguments)]
fn check_ground<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    level_cap: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    arena_limits: ArenaLimits,
    tiles: &TileRules,
    ends: [Cell; 2],
    template_of: &impl Fn(u8) -> Option<SpawnRules>,
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let [entrance, exit] = ends;
    loot::check_chests(width, height, level_cap, layout, tiles, ends, loot)?;

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
    loot::check_chest_access(width, height, layout, &reach)?;
    arena::check_arenas(
        width,
        height,
        layout,
        arenas,
        arena_limits,
        tiles,
        ends,
        &chests[..chest_count],
        &reach,
        &|id| template_of(id).map_or(1, |t| t.size),
    )
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
    use deadroll_types::{
        cell_byte, Facing, MapPoint, PackLayout, Pubkey, Spawn, TileKind, ARENA_CELLS,
        ARENA_SIDE_MAX, MAP_CELLS, NO_TABLE, PACKS_MAX,
    };

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
        // Arena update: the cells are in each pack's arena ([`arenas`]).
        let list = [
            spawn(ids::WOLF, 4, 4, 1),
            spawn(ids::WOLF, 5, 4, 1),
            spawn(ids::GOBLIN, 4, 4, 2),
            spawn(ids::GOBLIN, 4, 5, 2),
            spawn(ids::GOBLIN_CHIEFTAIN, 4, 4, 3),
            spawn(ids::GOBLIN_WARRIOR, 3, 4, 3),
            spawn(ids::GOBLIN_SHAMAN, 4, 3, 3),
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

    /// The arena sizes of the tests.
    const LIMITS: ArenaLimits = ArenaLimits {
        side_min: 6,
        side_max: 12,
    };

    fn pack(visible: u8, x: u8, y: u8, facing: Facing) -> PackLayout {
        PackLayout {
            visible,
            x,
            y,
            facing,
            width: 8,
            height: 8,
        }
    }

    /// The demo's packs: wolves (pack 1), goblins (2) and the boss pack (3), each in an open
    /// 8x8 arena.
    fn arenas() -> MapArenasData {
        let mut a = MapArenasData::EMPTY;
        a.packs[1] = pack(0, 6, 2, Facing::S);
        a.packs[2] = pack(2, 2, 10, Facing::E);
        a.packs[3] = pack(4, 12, 12, Facing::N);
        for p in 1..4 {
            a.cells[p] = [FLOOR; ARENA_CELLS];
        }
        a
    }

    /// [`check_map`] with the demo arenas.
    fn check_t(
        width: u8,
        height: u8,
        tier: u8,
        layout: &MapLayoutDataV2,
        tiles: &TileRules,
        template_of: impl Fn(u8) -> Option<SpawnRules>,
        loot: &TestLoot,
    ) -> Result<(), MapCheckError> {
        check_map(
            width,
            height,
            tier,
            layout,
            &arenas(),
            LIMITS,
            tiles,
            template_of,
            loot,
        )
    }

    fn check(layout: &MapLayoutDataV2) -> Result<(), MapCheckError> {
        check_t(16, 16, 1, layout, &tiles(), p0_role, &test_loot())
    }

    fn check_with(layout: &MapLayoutDataV2, arenas: &MapArenasData) -> Result<(), MapCheckError> {
        check_map(
            16,
            16,
            1,
            layout,
            arenas,
            LIMITS,
            &tiles(),
            p0_role,
            &test_loot(),
        )
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
            check_t(32, 32, 1, &layout, &tiles(), p0_role, &test_loot()),
            Ok(())
        );
    }

    #[test]
    fn rejects_size() {
        let layout = demo();
        assert_eq!(
            check_t(7, 16, 1, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
        assert_eq!(
            check_t(16, 33, 1, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::SizeOutOfRange)
        );
    }

    #[test]
    fn rejects_tier() {
        let layout = demo();
        assert_eq!(
            check_t(16, 16, 0, &layout, &tiles(), p0_role, &test_loot()),
            Err(MapCheckError::TierOutOfRange)
        );
        assert_eq!(
            check_t(16, 16, 3, &layout, &tiles(), p0_role, &test_loot()),
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
            check_t(16, 16, 1, &layout, &more, p0_role, &test_loot()),
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
    fn rejects_spawn_out_of_its_arena() {
        // Arena update: the cell is in the pack's 8x8 arena, not on the 16x16 map.
        let mut layout = demo();
        layout.spawns[0].x = 8;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOutOfBounds));
        layout.spawns[0].x = 7;
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_spawn_on_an_arena_wall() {
        let mut a = arenas();
        a.cells[1][4 * ARENA_SIDE_MAX + 4] = WALL;
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::SpawnNotPassable)
        );
        // A wall under the map cell of a hidden monster changes nothing now.
        let mut layout = demo();
        set(&mut layout, 7, 3, TileKind::Wall);
        assert_eq!(check(&layout), Ok(()));
    }

    #[test]
    fn rejects_overlap_within_a_pack_only() {
        let mut layout = demo();
        layout.spawns[1].x = 4;
        assert_eq!(check(&layout), Err(MapCheckError::SpawnOverlap));
        // Different packs may use the same arena cell: they never share an arena.
        let layout = demo();
        assert_eq!(
            (layout.spawns[0].x, layout.spawns[0].y),
            (layout.spawns[2].x, layout.spawns[2].y)
        );
        assert_eq!(check(&layout), Ok(()));
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
            check_t(16, 16, 2, &layout, &tiles(), p0_role, &test_loot()),
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
            check_t(
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
            check_t(
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
    fn a_pack_may_spread_over_its_arena() {
        // Arena update: no pack radius any more.
        let mut layout = demo();
        layout.spawns[1].x = 7;
        layout.spawns[1].y = 7;
        layout.spawns[0].x = 0;
        layout.spawns[0].y = 1;
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
    fn rejects_unreachable_pack() {
        let mut layout = demo();
        // Wall in the wolves' map cell (6, 2).
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
        assert_eq!(check(&layout), Err(MapCheckError::PackUnreachable));
    }

    #[test]
    fn every_used_pack_has_an_arena_and_every_spawn_a_used_pack() {
        let mut layout = demo();
        layout.spawns[1].pack = PACKS_MAX as u8;
        // Five packs on tier 1 fail the pack count first; drop one.
        layout.spawns[3].pack = 2;
        assert_eq!(check(&layout), Err(MapCheckError::PackOutOfRange));
        let mut layout = demo();
        layout.spawns[1].pack = 0;
        assert_eq!(check(&layout), Err(MapCheckError::PackWithoutArena));
        // An arena without monsters: its visible monster is not in it.
        let mut a = arenas();
        a.packs[5] = pack(0, 9, 9, Facing::N);
        a.cells[5] = [FLOOR; ARENA_CELLS];
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::VisibleNotInPack)
        );
        let mut a = arenas();
        a.packs[1].visible = 2;
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::VisibleNotInPack)
        );
        a.packs[1].visible = 1;
        assert_eq!(check_with(&demo(), &a), Ok(()));
        a.packs[1].visible = 40;
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::VisibleNotInPack)
        );
    }

    #[test]
    fn rejects_arena_size_and_tiles() {
        let mut a = arenas();
        a.packs[2].width = 5;
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::ArenaSizeOutOfRange)
        );
        a.packs[2].width = 13;
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::ArenaSizeOutOfRange)
        );
        a.packs[2].width = 12;
        assert_eq!(check_with(&demo(), &a), Ok(()));
        // The storage cap holds even if the limits allow more.
        let wide = ArenaLimits {
            side_min: 6,
            side_max: 40,
        };
        a.packs[2].width = 13;
        assert_eq!(
            check_map(
                16,
                16,
                1,
                &demo(),
                &a,
                wide,
                &tiles(),
                p0_role,
                &test_loot()
            ),
            Err(MapCheckError::ArenaSizeOutOfRange)
        );
        let mut a = arenas();
        a.cells[3][7 * ARENA_SIDE_MAX + 7] = cell_byte(TileKind::COUNT as u8, 0);
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::ArenaInvalidTile)
        );
        // Bytes past the arena's size are not looked at.
        let mut a = arenas();
        a.cells[3][7 * ARENA_SIDE_MAX + 8] = cell_byte(TileKind::COUNT as u8, 0);
        assert_eq!(check_with(&demo(), &a), Ok(()));
    }

    #[test]
    fn rejects_pack_map_cell() {
        let check_pack = |x: u8, y: u8| {
            let mut a = arenas();
            a.packs[1].x = x;
            a.packs[1].y = y;
            check_with(&demo(), &a)
        };
        assert_eq!(check_pack(16, 2), Err(MapCheckError::PackOutOfBounds));
        assert_eq!(check_pack(0, 5), Err(MapCheckError::PackNotPassable));
        assert_eq!(check_pack(EXIT.x, EXIT.y), Err(MapCheckError::PackOverlap));
        assert_eq!(check_pack(5, 5), Err(MapCheckError::PackOverlap));
        assert_eq!(check_pack(2, 10), Err(MapCheckError::PackOverlap));
        assert_eq!(check_pack(2, 2), Err(MapCheckError::PackAtEntrance));
        assert_eq!(
            check_pack(ENTRANCE.x, ENTRANCE.y),
            Err(MapCheckError::PackOverlap)
        );
        assert_eq!(check_pack(3, 3), Ok(()));
    }

    #[test]
    fn rejects_arena_without_room_or_path() {
        // A wall ring around the wolves: the party cannot reach them from any side.
        let mut a = arenas();
        for (x, y) in [
            (3, 3),
            (4, 3),
            (5, 3),
            (6, 3),
            (3, 4),
            (6, 4),
            (3, 5),
            (4, 5),
            (5, 5),
            (6, 5),
        ] {
            a.cells[1][y * ARENA_SIDE_MAX + x] = WALL;
        }
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::ArenaUnreachable)
        );
        // A wall across the arena splits the party coming from the north-west corner side.
        let mut a = arenas();
        for i in 0..8 {
            a.cells[2][(7 - i) * ARENA_SIDE_MAX + i] = WALL;
        }
        assert_eq!(
            check_with(&demo(), &a),
            Err(MapCheckError::ArenaUnreachable)
        );
        // A 6x6 arena of walls with 3 floor cells has no room for 4.
        let mut a = arenas();
        a.packs[3].width = 6;
        a.packs[3].height = 6;
        a.cells[3] = [WALL; ARENA_CELLS];
        for (x, y) in [(4, 4), (3, 4), (4, 3), (0, 0), (1, 0), (2, 0)] {
            a.cells[3][y * ARENA_SIDE_MAX + x] = FLOOR;
        }
        assert_eq!(check_with(&demo(), &a), Err(MapCheckError::ArenaNoRoom));
    }

    #[test]
    fn rejects_too_many_templates() {
        let many = |id: u8| {
            if id >= 10 {
                Some(SpawnRules::plain(Role::Fast))
            } else {
                p0_role(id)
            }
        };
        let mut layout = demo();
        // 8 more templates in the wolves' pack: 5 + 8 = 13 distinct.
        for i in 0..8u8 {
            layout.spawns[7 + i as usize] = Spawn {
                bestiary_id: 10 + i,
                ..spawn(ids::WOLF, i, 0, 1)
            };
        }
        layout.spawn_count = 15;
        for s in layout.spawns.iter_mut() {
            s.level = 3;
        }
        assert_eq!(
            check_t(16, 16, 2, &layout, &tiles(), many, &test_loot()),
            Err(MapCheckError::TooManyTemplates)
        );
        // 12 distinct pass this rule (and fail the tier's monster count next).
        layout.spawns[14].bestiary_id = ids::WOLF;
        assert_eq!(
            check_t(16, 16, 2, &layout, &tiles(), many, &test_loot()),
            Err(MapCheckError::TooManyMonsters)
        );
    }

    #[test]
    fn rejects_a_pack_bigger_than_an_arena() {
        // The tier limits keep packs smaller today; the arena rule holds on its own.
        let mut layout = demo();
        for i in 0..8u8 {
            layout.spawns[7 + i as usize] = spawn(ids::WOLF, i, 7, 1);
        }
        layout.spawn_count = 15;
        let (chests, count) = loot::chest_cells(&layout);
        let reach =
            path::reachable_avoiding(&layout.cells, &tiles(), 16, 16, ENTRANCE, &chests[..count]);
        let run = |layout: &MapLayoutDataV2| {
            arena::check_arenas(
                16,
                16,
                layout,
                &arenas(),
                LIMITS,
                &tiles(),
                [ENTRANCE, EXIT],
                &chests[..count],
                &reach,
                &|_| 1,
            )
        };
        assert_eq!(run(&layout), Err(MapCheckError::PackTooBig));
        // 2 wolves and 6 more: exactly an arena.
        layout.spawn_count = 13;
        assert_eq!(run(&layout), Ok(()));
    }

    #[test]
    fn a_large_monster_stands_on_its_whole_square() {
        // Patch B: the chieftain is 2x2 at (4, 4): (4..6, 4..6) of the boss pack's 8x8 arena.
        let big = |id: u8| {
            p0_role(id).map(|mut t| {
                if id == ids::GOBLIN_CHIEFTAIN {
                    t.size = 2;
                }
                t
            })
        };
        let run = |layout: &MapLayoutDataV2, arenas: &MapArenasData| {
            check_map(
                16,
                16,
                1,
                layout,
                arenas,
                LIMITS,
                &tiles(),
                big,
                &test_loot(),
            )
        };
        let boss = 4;
        let shaman = 6;
        assert_eq!(run(&demo(), &arenas()), Ok(()));
        // A helper on one of its cells.
        let mut layout = demo();
        layout.spawns[shaman].x = 5;
        layout.spawns[shaman].y = 5;
        assert_eq!(run(&layout, &arenas()), Err(MapCheckError::SpawnOverlap));
        // Its square past the arena's edge.
        let mut layout = demo();
        layout.spawns[boss].x = 7;
        assert_eq!(
            run(&layout, &arenas()),
            Err(MapCheckError::SpawnOutOfBounds)
        );
        // A wall under any of its cells.
        let mut a = arenas();
        a.cells[3][5 * ARENA_SIDE_MAX + 5] = WALL;
        assert_eq!(run(&demo(), &a), Err(MapCheckError::SpawnNotPassable));
        // A one-cell boss there passes.
        assert_eq!(check_with(&demo(), &a), Ok(()));
        // Its square closes the only way through a corridor 2 cells wide: the party entering on
        // one side cannot reach the other.
        let mut a = arenas();
        a.cells[3] = [WALL; ARENA_CELLS];
        for y in 0..8 {
            for x in 4..6 {
                a.cells[3][y * ARENA_SIDE_MAX + x] = FLOOR;
            }
        }
        for x in 0..8 {
            for y in [0, 1, 7] {
                a.cells[3][y * ARENA_SIDE_MAX + x] = FLOOR;
            }
        }
        let mut layout = demo();
        layout.spawns[5].x = 0;
        layout.spawns[5].y = 0;
        layout.spawns[shaman].x = 7;
        layout.spawns[shaman].y = 0;
        assert_eq!(check_with(&layout, &a), Ok(()));
        assert_eq!(run(&layout, &a), Err(MapCheckError::ArenaUnreachable));
    }

    #[test]
    fn codes_are_sequential() {
        assert_eq!(MapCheckError::SizeOutOfRange.code(), 0);
        assert_eq!(MapCheckError::SpawnUnreachable.code(), 22);
        assert_eq!(MapCheckError::TooManySessionSkills.code(), 28);
        assert_eq!(MapCheckError::MinPlayersOutOfRange.code(), 29);
        assert_eq!(MapCheckError::ChestUnreachable.code(), 40);
        assert_eq!(MapCheckError::PointOverlap.code(), 44);
        assert_eq!(MapCheckError::PackOutOfRange.code(), 45);
        assert_eq!(MapCheckError::TooManyTemplates.code(), 58);
        assert_eq!(MapCheckError::SecretInvalid.code(), 59);
        assert_eq!(MapCheckError::DuplicateSecret.code(), 76);
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
                check_t(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(bad)),
                Err(MapCheckError::MinPlayersOutOfRange)
            );
        }
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(4)),
            Ok(())
        );
    }

    #[test]
    fn rejects_too_many_chests() {
        let mut layout = demo();
        let small = |x, y| chest(x, y, tables::SMALL);
        set_chests(&mut layout, &[small(5, 5), small(6, 6)]);
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(2)),
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
        for cell in [ENTRANCE, EXIT] {
            *chest0(&mut layout) = chest(cell.x, cell.y, tables::SMALL);
            assert_eq!(check(&layout), Err(MapCheckError::ChestOverlap));
        }
        // Arena update: a chest on a pack's map cell is the pack's overlap.
        *chest0(&mut layout) = chest(6, 2, tables::SMALL);
        assert_eq!(check(&layout), Err(MapCheckError::PackOverlap));
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
            check_t(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(1)),
            Err(MapCheckError::OverBudget)
        );
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), p0_role, &loot_for(2)),
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
    fn closed_chest_hides_a_pack() {
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
        assert_eq!(check(&layout), Err(MapCheckError::PackUnreachable));
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
            check_t(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[0].skills = 0b1_0111;
        layout.spawns[1].skills = 0b1_1110;
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Ok(())
        );
        layout.spawns[1].skills = 0b10_0111;
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillUnknown)
        );
        // An elite takes all four actives.
        layout.spawns[1].skills = 0b1_1111;
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
            Err(MapCheckError::MonsterSkillCount)
        );
        layout.spawns[1].flags = SPAWN_ELITE;
        assert_eq!(
            check_t(16, 16, 1, &layout, &tiles(), &roster, &test_loot()),
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
            check_t(16, 16, 2, &layout, &tiles(), p0_role, &test_loot()),
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
            check_t(16, 16, 1, &layout, &tiles(), high, &test_loot()),
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
            check_t(16, 16, 1, &layout, &tiles(), roster, &test_loot()),
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
            check_t(16, 16, 1, &layout, &tiles(), two, &test_loot()),
            Ok(())
        );
    }

    // Patch C: the public part, a pack's composition when revealed, the whole design.
    mod fog {
        use super::*;
        use crate::{
            arena::check_roster,
            design::{check_design, split, Split},
            roster::{RosterCount, RosterTotals},
            secrets::{Board, Leaves, SecretLimits, ThreatWeights},
        };
        use deadroll_types::{
            CellSecret, FoundSecret, MapSecretsData, PackRoster, NO_SPAWN, SECRET_TYPES,
        };

        const SECRETS: SecretLimits = SecretLimits {
            traps_min: 1,
            traps_max: 2,
            declared_max: [2, 1, 1, 2],
            trap_damage_max: 8,
        };
        const WEIGHTS: ThreatWeights = ThreatWeights { elite: 2, boss: 3 };

        fn leaf(arenas: &MapArenasData, board: Board) -> u16 {
            Leaves::new(16, 16, &arenas.packs).of(board).unwrap()
        }

        /// A trap on the floor and a secret door in the west wall.
        fn secrets(arenas: &MapArenasData) -> [FoundSecret; 2] {
            [
                FoundSecret {
                    leaf: leaf(arenas, Board::Map(Cell { x: 3, y: 3 })),
                    secret: CellSecret::trap(5, 10),
                },
                FoundSecret {
                    leaf: leaf(arenas, Board::Map(Cell { x: 0, y: 5 })),
                    secret: CellSecret::door(),
                },
            ]
        }

        fn design(
            layout: &MapLayoutDataV2,
            arenas: &MapArenasData,
            declared: [u8; SECRET_TYPES],
            cells: &[FoundSecret],
            loot: &TestLoot,
        ) -> Result<Split, MapCheckError> {
            check_design(
                16,
                16,
                1,
                layout,
                arenas,
                &declared,
                cells,
                LIMITS,
                &SECRETS,
                WEIGHTS,
                &tiles(),
                p0_role,
                loot,
            )
        }

        fn public(
            s: &Split,
            threat: [u8; PACKS_MAX],
            declared: [u8; SECRET_TYPES],
        ) -> Result<(), MapCheckError> {
            let mut arenas = arenas();
            arenas.packs = s.packs;
            let mut secrets = MapSecretsData::EMPTY;
            secrets.threat = threat;
            secrets.declared = declared;
            check_public(
                16,
                16,
                1,
                &s.public,
                &arenas,
                &secrets,
                LIMITS,
                &SECRETS,
                &tiles(),
                p0_role,
                &test_loot(),
            )
        }

        fn roster(
            s: &Split,
            n: u8,
            r: &PackRoster,
            threat: u8,
        ) -> Result<RosterCount, MapCheckError> {
            let mut arenas = arenas();
            arenas.packs = s.packs;
            let p = arenas.packs[n as usize];
            let visible = (p.visible != NO_SPAWN).then(|| &s.public.spawns[p.visible as usize]);
            check_roster(
                1,
                n,
                r,
                visible,
                &arenas,
                &tiles(),
                p0_role,
                WEIGHTS,
                threat,
            )
        }

        fn demo_split() -> Split {
            let a = arenas();
            design(&demo(), &a, [1, 0, 0, 1], &secrets(&a), &test_loot()).unwrap()
        }

        #[test]
        fn the_demo_splits_into_visible_monsters_and_rosters() {
            let s = demo_split();
            assert_eq!(s.public.spawn_count, 3);
            assert_eq!(s.packs[1].visible, 0);
            assert_eq!(s.packs[3].visible, 2);
            assert_eq!(s.public.spawns[2].bestiary_id, ids::GOBLIN_CHIEFTAIN);
            let counts = [s.rosters[1].count, s.rosters[2].count, s.rosters[3].count];
            assert_eq!(counts, [2, 2, 3]);
            assert_eq!(s.rosters[0].count, 0);
            // Levels 1: two of a kind = 2; the boss counts 3 times plus two helpers = 5.
            assert_eq!(s.threat, [0, 2, 2, 5, 0, 0, 0, 0]);
            assert_eq!(public(&s, s.threat, [1, 0, 0, 1]), Ok(()));
            // The split puts the visible monster first.
            let layout = demo();
            let mut a = arenas();
            a.packs[1].visible = 1;
            let s2 = split(&layout, &a.packs, p0_role, WEIGHTS).unwrap();
            assert_eq!(s2.rosters[1].spawns[0], layout.spawns[1]);
            assert_eq!(s2.rosters[1].spawns[1], layout.spawns[0]);
        }

        #[test]
        fn the_public_part_rejects() {
            use MapCheckError as E;
            let s = demo_split();
            let mut threat = s.threat;
            threat[0] = 1;
            assert_eq!(public(&s, threat, [1, 0, 0, 1]), Err(E::ThreatOutOfRange));
            threat = s.threat;
            threat[1] = 0;
            assert_eq!(public(&s, threat, [1, 0, 0, 1]), Err(E::ThreatOutOfRange));
            assert_eq!(public(&s, s.threat, [0, 0, 0, 0]), Err(E::TooFewTraps));
            assert_eq!(public(&s, s.threat, [3, 0, 0, 0]), Err(E::TooManySecrets));
            assert_eq!(public(&s, s.threat, [1, 1, 0, 0]), Err(E::AmbushUnlinked));
            // A hidden monster left in the public list.
            let mut leak = s.clone();
            leak.public.spawns[3] = demo().spawns[1];
            leak.public.spawn_count = 4;
            assert_eq!(
                public(&leak, s.threat, [1, 0, 0, 0]),
                Err(E::SpawnNotVisible)
            );
        }

        #[test]
        fn a_revealed_roster_must_match() {
            use MapCheckError as E;
            let s = demo_split();
            let wolves = s.rosters[1];
            assert_eq!(roster(&s, 1, &wolves, 2).map(|c| c.regular), Ok(2));
            assert_eq!(roster(&s, 1, &wolves, 3), Err(E::ThreatMismatch));
            assert_eq!(
                roster(&s, 1, &PackRoster::default(), 0),
                Err(E::RosterInvalid)
            );
            let mut other = wolves;
            other.spawns[1].pack = 2;
            assert_eq!(roster(&s, 1, &other, 2), Err(E::RosterInvalid));
            let mut swapped = wolves;
            swapped.spawns.swap(0, 1);
            assert_eq!(roster(&s, 1, &swapped, 2), Err(E::VisibleMismatch));
            let mut mixed = wolves;
            mixed.spawns[1].bestiary_id = ids::GOBLIN_WARRIOR;
            assert_eq!(roster(&s, 1, &mixed, 2), Err(E::HelperOutsideBossPack));
            let mut boss = s.rosters[3];
            boss.spawns[1].bestiary_id = ids::GOBLIN;
            assert_eq!(roster(&s, 3, &boss, 5), Err(E::BossPackMixed));
            let mut wall = wolves;
            wall.spawns[1].x = 12;
            assert_eq!(roster(&s, 1, &wall, 2), Err(E::SpawnOutOfBounds));
            let mut high = wolves;
            high.spawns[1].level = 3;
            assert_eq!(roster(&s, 1, &high, 4), Err(E::LevelOutOfTier));

            // The totals over the packs.
            let limits = tier_limits(1).unwrap();
            let mut totals = RosterTotals::default();
            for n in 1..=3u8 {
                let c = roster(&s, n, &s.rosters[n as usize], s.threat[n as usize]).unwrap();
                totals.add(&c, &limits).unwrap();
            }
            assert_eq!(totals.finish(), Ok(()));
            assert_eq!(RosterTotals::default().finish(), Err(E::BossCount));
            let five = RosterCount {
                regular: 5,
                ..Default::default()
            };
            assert_eq!(
                RosterTotals::default().add(&five, &limits),
                Err(E::TooManyMonsters)
            );
        }

        #[test]
        fn the_secrets_of_a_design() {
            use MapCheckError as E;
            let a = arenas();
            let cells = secrets(&a);
            let loot = test_loot();
            let err = |declared, cells: &[FoundSecret], loot: &TestLoot| {
                design(&demo(), &a, declared, cells, loot).err()
            };
            assert_eq!(err([1, 0, 0, 0], &cells, &loot), Some(E::TooManySecrets));
            assert_eq!(err([1, 0, 0, 2], &cells, &loot), Some(E::TooFewSecrets));
            assert_eq!(
                err([2, 0, 0, 0], &[cells[0], cells[0]], &loot),
                Some(E::DuplicateSecret)
            );
            let past = FoundSecret {
                leaf: Leaves::new(16, 16, &a.packs).count,
                secret: CellSecret::trap(1, 1),
            };
            assert_eq!(err([1, 0, 0, 0], &[past], &loot), Some(E::SecretInvalid));
            let on_pack = FoundSecret {
                leaf: 2,
                secret: CellSecret::trap(1, 1),
            };
            assert_eq!(err([1, 0, 0, 0], &[on_pack], &loot), Some(E::SecretInvalid));
            let on_chest = FoundSecret {
                leaf: leaf(&a, Board::Map(Cell { x: 5, y: 5 })),
                secret: CellSecret::trap(1, 1),
            };
            assert_eq!(
                err([1, 0, 0, 0], &[on_chest], &loot),
                Some(E::SecretOnPoint)
            );
            // A trap in an arena is fine.
            let arena_trap = FoundSecret {
                leaf: leaf(&a, Board::Arena(2, Cell { x: 0, y: 0 })),
                secret: CellSecret::trap(2, 0),
            };
            assert_eq!(err([1, 0, 0, 0], &[arena_trap], &loot), None);

            // A hidden stash costs budget: chest 2 + stash 2 + boss 4 > 6 for one player.
            let stash = [
                cells[0],
                FoundSecret {
                    leaf: leaf(&a, Board::Map(Cell { x: 7, y: 7 })),
                    secret: CellSecret::stash(tables::SMALL),
                },
            ];
            assert_eq!(err([1, 0, 1, 0], &stash, &loot), Some(E::OverBudget));
            assert_eq!(err([1, 0, 1, 0], &stash, &loot_for(2)), None);
        }

        #[test]
        fn an_ambush_pack_has_no_map_cell_and_one_trap() {
            use MapCheckError as E;
            let mut a = arenas();
            a.packs[2].visible = NO_SPAWN;
            let ambush = FoundSecret {
                leaf: leaf(&a, Board::Map(Cell { x: 8, y: 8 })),
                secret: CellSecret::ambush(2),
            };
            let s = design(&demo(), &a, [0, 1, 0, 0], &[ambush], &test_loot()).unwrap();
            assert_eq!(s.packs[2].visible, NO_SPAWN);
            assert_eq!(s.public.spawn_count, 2);
            assert_eq!(s.rosters[2].count, 2);
            assert_eq!(roster(&s, 2, &s.rosters[2], 2).map(|c| c.regular), Ok(2));
            // Without its trap, or a trap on a pack that shows a monster.
            let trap = &secrets(&a)[..1];
            let err = design(&demo(), &a, [1, 0, 0, 0], trap, &test_loot()).err();
            assert_eq!(err, Some(E::AmbushUnlinked));
            let wrong = FoundSecret {
                secret: CellSecret::ambush(1),
                ..ambush
            };
            let err = design(&demo(), &a, [0, 1, 0, 0], &[wrong], &test_loot()).err();
            assert_eq!(err, Some(E::AmbushPackInvalid));
        }
    }
}
