//! Deadroll map checks and pathfinding (docs/MAP_DESCRIPTION.md, section 10).
//!
//! Pure, `no_std`, fixed-size buffers. `programs/map` runs [`check_map`] in `finalize_map`;
//! the website builder runs the same function through WASM.
#![no_std]

pub mod limits;
pub mod path;
pub mod roster;
pub mod tiles;

use deadroll_types::{Cell, MapLayoutData, Tile, SPAWNS_MAX};

use limits::{tier_limits, BOSSES, BOSS_HELPERS, PACK_RADIUS, SIDE_MAX, SIDE_MIN};
use roster::Role;
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
}

impl MapCheckError {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Runs every builder rule on a map. `role_of` maps a bestiary id to its role
/// (`roster::p0_role` until the registry `Bestiary` exists).
pub fn check_map(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutData,
    role_of: impl Fn(u8) -> Option<Role>,
) -> Result<(), MapCheckError> {
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
        let role = role_of(spawn.bestiary_id).ok_or(E::UnknownMonster)?;
        if !(limits.level_min..=limits.level_max).contains(&spawn.level) {
            return Err(E::LevelOutOfTier);
        }
        if role == Role::Boss {
            bosses += 1;
            boss_pack = Some(spawn.pack);
        }
    }
    if bosses != BOSSES {
        return Err(E::BossCount);
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

    let reach = path::reachable_from(&layout.tiles, width, height, layout.entrance);
    if !reach.contains(layout.exit) {
        return Err(E::ExitUnreachable);
    }
    if spawns
        .iter()
        .any(|s| !reach.contains(Cell { x: s.x, y: s.y }))
    {
        return Err(E::SpawnUnreachable);
    }
    Ok(())
}

/// Every cell inside the map is a `Tile`, with exactly one entrance and one exit that match
/// the layout points.
fn check_tiles(width: u8, height: u8, layout: &MapLayoutData) -> Result<(), MapCheckError> {
    use MapCheckError as E;

    let (mut entrances, mut exits) = (0u16, 0u16);
    let (mut entrance, mut exit) = (Cell::default(), Cell::default());
    for y in 0..height {
        for x in 0..width {
            match Tile::try_from(layout.tiles[cell_index(x, y)]).map_err(|_| E::InvalidTile)? {
                Tile::Entrance => {
                    entrances += 1;
                    entrance = Cell { x, y };
                }
                Tile::Exit => {
                    exits += 1;
                    exit = Cell { x, y };
                }
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
    use deadroll_types::{Pubkey, Spawn, MAP_CELLS};
    use roster::{ids, p0_role};

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
        }
    }

    fn check(layout: &MapLayoutData) -> Result<(), MapCheckError> {
        check_map(16, 16, 1, layout, p0_role)
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
        assert_eq!(check_map(32, 32, 1, &layout, p0_role), Ok(()));
    }

    #[test]
    fn rejects_size() {
        let layout = demo();
        assert_eq!(
            check_map(7, 16, 1, &layout, p0_role),
            Err(MapCheckError::SizeOutOfRange)
        );
        assert_eq!(
            check_map(16, 33, 1, &layout, p0_role),
            Err(MapCheckError::SizeOutOfRange)
        );
    }

    #[test]
    fn rejects_tier() {
        let layout = demo();
        assert_eq!(
            check_map(16, 16, 0, &layout, p0_role),
            Err(MapCheckError::TierOutOfRange)
        );
        assert_eq!(
            check_map(16, 16, 3, &layout, p0_role),
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
        assert_eq!(check_map(16, 16, 2, &layout, p0_role), Ok(()));
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
                Some(Role::Loner)
            } else {
                p0_role(id)
            }
        };
        assert_eq!(
            check_map(16, 16, 1, &layout, wolves_are_loners),
            Err(MapCheckError::TwoLoners)
        );
        // One loner per pack is fine.
        let mut layout = demo();
        layout.spawns[1].bestiary_id = ids::GOBLIN;
        assert_eq!(check_map(16, 16, 1, &layout, wolves_are_loners), Ok(()));
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
    }
}
