//! Tile properties (docs/MAP_DESCRIPTION.md, section 4.2) and cell addressing.
//!
//! TODO(post-demo): read passability and step cost from the registry `Tileset`.
//! Until then these values must match `programs/session` `rules::step_cost`.

use deadroll_types::{Cell, Tile, MAP_MAX};

/// Action points to step onto `tile`, `None` if it cannot be entered.
pub const fn step_cost(tile: Tile) -> Option<u8> {
    match tile {
        Tile::Floor | Tile::Door | Tile::Entrance | Tile::Exit | Tile::Trap => Some(1),
        // Shallow water is difficult terrain.
        Tile::Water => Some(2),
        Tile::Wall => None,
    }
}

pub const fn is_passable(tile: Tile) -> bool {
    step_cost(tile).is_some()
}

/// A raw tile byte that is a known passable `Tile`. Byte compares only: this runs per cell
/// on-chain. `same_as_tile_rules` keeps it equal to `is_passable`.
#[inline(always)]
pub const fn byte_is_passable(byte: u8) -> bool {
    (byte as usize) < Tile::COUNT && byte != Tile::Wall as u8
}

/// Index of `(x, y)` in `MapLayout.tiles` (row-major, `MAP_MAX` cells per row).
pub const fn cell_index(x: u8, y: u8) -> usize {
    y as usize * MAP_MAX + x as usize
}

pub const fn in_bounds(cell: Cell, width: u8, height: u8) -> bool {
    cell.x < width && cell.y < height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wall_blocks() {
        for &tile in Tile::ALL {
            assert_eq!(is_passable(tile), tile != Tile::Wall, "{tile:?}");
        }
        assert_eq!(step_cost(Tile::Water), Some(2));
        assert_eq!(step_cost(Tile::Floor), Some(1));
    }

    #[test]
    fn same_as_tile_rules() {
        for byte in 0..=u8::MAX {
            let expected = Tile::try_from(byte).is_ok_and(is_passable);
            assert_eq!(byte_is_passable(byte), expected, "byte {byte}");
        }
    }

    #[test]
    fn unknown_byte_blocks() {
        assert!(byte_is_passable(Tile::Floor as u8));
        assert!(!byte_is_passable(Tile::Wall as u8));
        assert!(!byte_is_passable(Tile::COUNT as u8));
        assert!(!byte_is_passable(u8::MAX));
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
