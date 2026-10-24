//! Visible chests and the reward budget of a map (docs/MAP_DESCRIPTION.md 7.1 and 10,
//! docs/PARTY_AND_TURNS.md 9.2). Patch 11.
//!
//! The numbers come from `deadroll_rules::LootTuning` and the registry `Rewards` table: this crate
//! holds no balance numbers, the caller builds [`LootRules`] and `table_of`.

use deadroll_types::{Cell, MapLayoutDataV2, CHESTS_MAX, NO_TABLE, PARTY_MAX};

use crate::{
    path::Reach,
    tiles::{cell_index, in_bounds, TileRules},
    MapCheckError,
};

/// The loot limits of one map tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LootRules {
    /// Visible chests a map of this tier may have.
    pub chests: u8,
    /// Reward budget by the map's minimum players, index `min_players - 1`: the budget grows
    /// with party size (docs/PARTY_AND_TURNS.md 9.2).
    pub budget: [u16; PARTY_MAX],
    /// Reward table ids allowed on this tier, bit `1 << id`.
    pub allowed: u64,
}

/// A reward table as the builder sees it (a registry `Rewards` record).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRules {
    pub level_base: u8,
    pub level_max: u8,
    /// Budget points.
    pub cost: u16,
}

/// The loot part of a map check: the map's minimum players, the tier's limits and the registry
/// tables (`None`: no such table).
pub struct MapLoot<R: Fn(u8) -> Option<TableRules>> {
    pub min_players: u8,
    pub rules: LootRules,
    pub table_of: R,
}

/// The cells of the closed chests: they block movement (patch 11), so the path checks treat
/// them as walls. Cells past the count are left at the default and ignored by the caller.
pub fn chest_cells(layout: &MapLayoutDataV2) -> ([Cell; CHESTS_MAX], usize) {
    let mut cells = [Cell::default(); CHESTS_MAX];
    let mut count = 0;
    for (c, p) in cells.iter_mut().zip(layout.chests()) {
        *c = p.cell();
        count += 1;
    }
    (cells, count)
}

/// The tables part of the chest rules: the counts, every chest table and the boss table allowed
/// on the tier, the budget. Returns the budget points spent. `level_cap` is the tier's top level:
/// a table above it is not allowed (docs/PARTY_AND_TURNS.md 9.2). The website builder mirrors
/// this function (`app/src/chain/loot.ts`).
pub fn check_tables<R: Fn(u8) -> Option<TableRules>>(
    level_cap: u8,
    chest_tables: &[u8],
    boss_table: u8,
    loot: &MapLoot<R>,
) -> Result<u32, MapCheckError> {
    use MapCheckError as E;

    if loot.min_players == 0 || loot.min_players as usize > PARTY_MAX {
        return Err(E::MinPlayersOutOfRange);
    }
    let count = chest_tables.len();
    if count > CHESTS_MAX || count > loot.rules.chests as usize {
        return Err(E::TooManyChests);
    }
    let allowed = |id: u8| -> Result<TableRules, MapCheckError> {
        let t = (loot.table_of)(id).ok_or(E::UnknownTable)?;
        let in_mask = id < 64 && loot.rules.allowed & (1 << id) != 0;
        if !in_mask || t.level_max > level_cap {
            return Err(E::TableNotAllowed);
        }
        Ok(t)
    };
    let mut spent = 0u32;
    for &id in chest_tables {
        spent += allowed(id)?.cost as u32;
    }
    if boss_table == NO_TABLE {
        return Err(E::BossTableMissing);
    }
    let boss = allowed(boss_table)?;
    // The boss guarantee is "one level higher" (docs/PARTY_AND_TURNS.md 9.4).
    if boss.level_max <= boss.level_base {
        return Err(E::BossTableOneLevel);
    }
    spent += boss.cost as u32;
    if spent > loot.rules.budget[loot.min_players as usize - 1] as u32 {
        return Err(E::OverBudget);
    }
    Ok(spent)
}

/// Chest placement, then [`check_tables`]. `ends` are the entrance and the exit.
pub(crate) fn check_chests<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    level_cap: u8,
    layout: &MapLayoutDataV2,
    tiles: &TileRules,
    ends: [Cell; 2],
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;

    if loot.min_players == 0 || loot.min_players as usize > PARTY_MAX {
        return Err(E::MinPlayersOutOfRange);
    }
    let count = layout.chests().count();
    if count > CHESTS_MAX || count > loot.rules.chests as usize {
        return Err(E::TooManyChests);
    }

    let spawns = &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())];
    let (cells, _) = chest_cells(layout);
    let mut tables = [NO_TABLE; CHESTS_MAX];
    for (i, chest) in layout.chests().enumerate() {
        let cell = chest.cell();
        if !in_bounds(cell, width, height) {
            return Err(E::ChestOutOfBounds);
        }
        if !tiles.is_passable(layout.cells[cell_index(cell.x, cell.y)]) {
            return Err(E::ChestNotPassable);
        }
        let taken = ends.contains(&cell)
            || spawns.iter().any(|s| s.x == cell.x && s.y == cell.y)
            || cells[..i].contains(&cell);
        if taken {
            return Err(E::ChestOverlap);
        }
        tables[i] = chest.data;
    }
    check_tables(level_cap, &tables[..count], layout.boss_table, loot).map(|_| ())
}

/// Every chest has a neighbor (Chebyshev 1) the party can reach: it is opened from there.
pub(crate) fn check_chest_access(
    width: u8,
    height: u8,
    layout: &MapLayoutDataV2,
    reach: &Reach,
) -> Result<(), MapCheckError> {
    for chest in layout.chests() {
        let open = neighbors(chest.cell())
            .filter(|c| in_bounds(*c, width, height))
            .any(|c| reach.contains(c));
        if !open {
            return Err(MapCheckError::ChestUnreachable);
        }
    }
    Ok(())
}

/// The cells around `c` (Chebyshev distance 1).
fn neighbors(c: Cell) -> impl Iterator<Item = Cell> {
    let xs = c.x.saturating_sub(1)..=c.x.saturating_add(1);
    let ys = c.y.saturating_sub(1)..=c.y.saturating_add(1);
    ys.flat_map(move |y| xs.clone().map(move |x| Cell { x, y }))
        .filter(move |n| *n != c)
}
