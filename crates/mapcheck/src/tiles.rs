//! Tile properties (docs/MAP_DESCRIPTION.md, sections 4.1 and 4.2) and cell addressing.
//!
//! Patch 13: the properties are data, the registry `Tileset` records, never code. Callers build
//! [`TileRules`] with `deadroll_rules::registry::tile_rules` (the same function for `finalize_map`,
//! `open_session` and the website), so the map check, movement and the builder agree.

use deadroll_types::{cell_kind, Cell, MAP_MAX, TILE_KINDS_MAX};

/// The `Tileset` as the checks see it: per tile type id (a cell byte's bits 7-3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileRules {
    /// Action points to step onto a tile of this type, `0` = it cannot be entered.
    pub cost: [u8; TILE_KINDS_MAX],
    /// Bit `1 << id`: the `Tileset` has this tile type. A cell of any other type is invalid.
    pub known: u32,
    /// Bit `1 << id`: the tile blocks vision (a door: while closed).
    pub blocks_vision: u32,
}

const _: () = assert!(TILE_KINDS_MAX <= u32::BITS as usize);

impl TileRules {
    /// The `Tileset` has the type of this cell byte (any variant).
    #[inline(always)]
    pub const fn knows(&self, byte: u8) -> bool {
        self.known & (1 << cell_kind(byte)) != 0
    }

    /// Action points to step onto a cell with this byte, `None` if it cannot be entered (an
    /// impassable or unknown type).
    #[inline(always)]
    pub const fn step_cost(&self, byte: u8) -> Option<u8> {
        match self.cost[cell_kind(byte) as usize] {
            0 => None,
            cost if self.knows(byte) => Some(cost),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn is_passable(&self, byte: u8) -> bool {
        self.step_cost(byte).is_some()
    }

    /// Bit `1 << id` for every tile type that can be entered: one shift per cell in the hot loops.
    pub const fn passable_mask(&self) -> u32 {
        let mut mask = 0;
        let mut id = 0;
        while id < TILE_KINDS_MAX {
            if self.cost[id] != 0 {
                mask |= 1 << id;
            }
            id += 1;
        }
        mask & self.known
    }

    pub const fn blocks_vision(&self, byte: u8) -> bool {
        self.blocks_vision & (1 << cell_kind(byte)) != 0
    }
}

/// Index of `(x, y)` in `MapLayoutDataV2.cells` (row-major, `MAP_MAX` cells per row).
pub const fn cell_index(x: u8, y: u8) -> usize {
    y as usize * MAP_MAX + x as usize
}

pub const fn in_bounds(cell: Cell, width: u8, height: u8) -> bool {
    cell.x < width && cell.y < height
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use deadroll_types::{cell_byte, TileKind};

    /// The shipped `assets/registry/tileset.json` (docs/MAP_DESCRIPTION.md 4.2).
    pub(crate) fn starter() -> TileRules {
        use TileKind as T;
        let mut rules = TileRules::default();
        for &kind in TileKind::ALL {
            let cost = match kind {
                T::Void | T::Wall | T::DeepWater | T::Chasm => 0,
                T::ShallowWater | T::Rubble => 2,
                _ => 1,
            };
            rules.cost[kind as usize] = cost;
            rules.known |= 1 << kind as u8;
            if matches!(kind, T::Void | T::Wall | T::Door) {
                rules.blocks_vision |= 1 << kind as u8;
            }
        }
        rules
    }

    #[test]
    fn costs_come_from_the_rules() {
        let rules = starter();
        let byte = |kind: TileKind, variant| cell_byte(kind as u8, variant);
        assert_eq!(rules.step_cost(byte(TileKind::StoneFloor, 0)), Some(1));
        assert_eq!(rules.step_cost(byte(TileKind::ShallowWater, 0)), Some(2));
        assert_eq!(rules.step_cost(byte(TileKind::Rubble, 7)), Some(2));
        for kind in [
            TileKind::Void,
            TileKind::Wall,
            TileKind::DeepWater,
            TileKind::Chasm,
        ] {
            assert_eq!(rules.step_cost(byte(kind, 3)), None, "{kind:?}");
            assert!(rules.knows(byte(kind, 3)));
        }
        assert!(rules.blocks_vision(byte(TileKind::Door, 0)));
        assert!(!rules.blocks_vision(byte(TileKind::DeepWater, 0)));
    }

    #[test]
    fn the_variant_does_not_change_the_cost() {
        let rules = starter();
        for variant in 0..8 {
            let b = cell_byte(TileKind::Dirt as u8, variant);
            assert_eq!(rules.step_cost(b), Some(1));
        }
    }

    #[test]
    fn unknown_types_block() {
        let mut rules = starter();
        // A cost without a record (a reserved id) still blocks.
        rules.cost[20] = 1;
        assert!(!rules.knows(cell_byte(20, 0)));
        assert_eq!(rules.step_cost(cell_byte(20, 0)), None);
        assert_eq!(rules.step_cost(u8::MAX), None);
    }

    #[test]
    fn row_major_with_full_stride() {
        assert_eq!(cell_index(0, 0), 0);
        assert_eq!(cell_index(5, 0), 5);
        assert_eq!(cell_index(0, 1), MAP_MAX);
        assert_eq!(cell_index(31, 31), MAP_MAX * MAP_MAX - 1);
    }

    #[test]
    fn bounds() {
        assert!(in_bounds(Cell { x: 15, y: 15 }, 16, 16));
        assert!(!in_bounds(Cell { x: 16, y: 0 }, 16, 16));
        assert!(!in_bounds(Cell { x: 0, y: 16 }, 16, 16));
    }
}
