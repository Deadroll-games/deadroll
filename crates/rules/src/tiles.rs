//! The registry `Tileset` (docs/MAP_DESCRIPTION.md 4.2, 5), patch 13: one decoder of its records
//! for the registry, the map check (`finalize_map`), movement (`open_session`) and the website,
//! so the properties of a tile live only in the table.

use deadroll_mapcheck::tiles::TileRules;
use deadroll_types::TILE_KINDS_MAX;

/// A `Tileset` record: `[passable, cost, blocks_los, flags, reserved x4]`. The record index is
/// the tile type id (`deadroll_types::TileKind`, the cell byte's bits 7-3).
pub mod record {
    pub const SIZE: usize = 8;
    /// `0` or `1`.
    pub const PASSABLE: usize = 0;
    /// Action points to step onto the tile, `>= 1` when passable.
    pub const COST: usize = 1;
    /// `0` or `1` (a door: while closed).
    pub const BLOCKS_LOS: usize = 2;
    pub const FLAGS: usize = 3;
    /// `flags` bit: a door (opens; no door logic yet).
    pub const FLAG_DOOR: u8 = 1;
    /// Records the table may hold: one per tile type a cell byte can name.
    pub const RECORDS_MAX: usize = deadroll_types::TILE_KINDS_MAX;
}

/// A record the registry accepts.
pub fn validate_record(r: &[u8]) -> bool {
    use record::*;
    r.len() == SIZE
        && r[PASSABLE] <= 1
        && r[BLOCKS_LOS] <= 1
        && (r[PASSABLE] == 0 || r[COST] >= 1)
}

/// The `Tileset` table body as [`TileRules`]: every whole record is a known tile type, records
/// past `RECORDS_MAX` are ignored. An impassable record costs `0` (cannot be entered).
pub fn tile_rules(body: &[u8]) -> TileRules {
    let mut rules = TileRules::default();
    for (id, r) in body
        .chunks_exact(record::SIZE)
        .take(TILE_KINDS_MAX)
        .enumerate()
    {
        rules.known |= 1 << id;
        if r[record::PASSABLE] == 1 {
            rules.cost[id] = r[record::COST];
        }
        if r[record::BLOCKS_LOS] == 1 {
            rules.blocks_vision |= 1 << id;
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::{cell_byte, TileKind};

    fn rec(passable: u8, cost: u8, blocks: u8) -> [u8; record::SIZE] {
        [passable, cost, blocks, 0, 0, 0, 0, 0]
    }

    #[test]
    fn decodes_records_by_index() {
        let records = [
            rec(0, 0, 1), // void
            rec(1, 1, 0), // stone floor
            rec(1, 2, 0), // wooden floor, made difficult
            rec(0, 9, 0), // dirt, made impassable: its cost is ignored
        ];
        let rules = tile_rules(records.as_flattened());
        assert_eq!(rules.known, 0b1111);
        assert_eq!(rules.blocks_vision, 0b0001);
        assert_eq!(rules.step_cost(cell_byte(TileKind::Void as u8, 0)), None);
        assert_eq!(rules.step_cost(cell_byte(TileKind::StoneFloor as u8, 2)), Some(1));
        assert_eq!(rules.step_cost(cell_byte(TileKind::WoodenFloor as u8, 0)), Some(2));
        assert_eq!(rules.step_cost(cell_byte(TileKind::Dirt as u8, 0)), None);
        assert!(!rules.knows(cell_byte(TileKind::Wall as u8, 0)));
    }

    #[test]
    fn a_partial_record_and_extra_records_are_ignored() {
        let body = [1u8; (TILE_KINDS_MAX + 2) * record::SIZE + 3];
        let rules = tile_rules(&body);
        assert_eq!(rules.known, u32::MAX);
        assert_eq!(rules.cost, [1; TILE_KINDS_MAX]);
        assert_eq!(tile_rules(&[1, 1, 0]).known, 0);
    }

    #[test]
    fn validates_records() {
        assert!(validate_record(&rec(1, 1, 0)));
        assert!(validate_record(&rec(0, 0, 1)));
        assert!(!validate_record(&rec(1, 0, 0)), "passable needs a cost");
        assert!(!validate_record(&rec(2, 1, 0)));
        assert!(!validate_record(&rec(1, 1, 2)));
        assert!(!validate_record(&[1, 1, 0]));
    }
}
