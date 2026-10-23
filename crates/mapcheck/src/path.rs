//! Breadth-first search over passable cells (docs/MAP_DESCRIPTION.md, section 10.1).
//!
//! Steps are orthogonal, the same four directions as `programs/session` movement.
//! Fixed-size buffers only: a 2 KB queue and a 128-byte bitset, small enough for one
//! SBF stack frame (4 KB).

use deadroll_types::{Cell, MAP_CELLS, MAP_MAX};

use crate::tiles::{byte_is_passable, cell_index, in_bounds};

/// Cells reachable from a start cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    bits: [u32; MAP_CELLS / 32],
}

impl Reach {
    const fn empty() -> Self {
        Self {
            bits: [0; MAP_CELLS / 32],
        }
    }

    fn get(&self, index: usize) -> bool {
        self.bits[index / 32] & (1 << (index % 32)) != 0
    }

    fn set(&mut self, index: usize) {
        self.bits[index / 32] |= 1 << (index % 32);
    }

    /// `false` for cells outside the map.
    pub fn contains(&self, cell: Cell) -> bool {
        (cell.x as usize) < MAP_MAX
            && (cell.y as usize) < MAP_MAX
            && self.get(cell_index(cell.x, cell.y))
    }

    /// Number of reachable cells.
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }
}

// Row stride is a power of two: `index >> ROW_SHIFT` is the row, `index & ROW_MASK` the column.
const ROW_SHIFT: u32 = MAP_MAX.trailing_zeros();
const ROW_MASK: usize = MAP_MAX - 1;
const _: () = assert!(MAP_MAX.is_power_of_two());

/// Every cell reachable from `start` through passable in-bounds cells, `start` included.
/// An impassable or out-of-bounds `start` reaches nothing.
///
/// Hot loop on-chain (`finalize_map`): `closed` holds visited and impassable cells together,
/// so each neighbor costs one bit test.
pub fn reachable_from(tiles: &[u8; MAP_CELLS], width: u8, height: u8, start: Cell) -> Reach {
    reachable_avoiding(tiles, width, height, start, &[])
}

/// [`reachable_from`] with `blocked` cells closed as well (patch 11: closed chests occupy their
/// cell). A blocked `start` reaches nothing.
#[inline(never)]
pub fn reachable_avoiding(
    tiles: &[u8; MAP_CELLS],
    width: u8,
    height: u8,
    start: Cell,
    blocked: &[Cell],
) -> Reach {
    let mut reach = Reach::empty();
    if !in_bounds(start, width, height)
        || !byte_is_passable(tiles[cell_index(start.x, start.y)])
        || blocked.contains(&start)
    {
        return reach;
    }
    let (w, h) = (width as usize, height as usize);
    let mut closed = Reach::empty();
    for y in 0..h {
        let row = y << ROW_SHIFT;
        for (i, &byte) in tiles[row..row + w].iter().enumerate() {
            if !byte_is_passable(byte) {
                closed.set(row + i);
            }
        }
    }
    for c in blocked {
        if in_bounds(*c, width, height) {
            closed.set(cell_index(c.x, c.y));
        }
    }
    // Every cell enters the queue at most once, so a plain array never overflows.
    let mut queue = [0u16; MAP_CELLS];
    let (mut head, mut tail) = (0usize, 0usize);
    macro_rules! push {
        ($next:expr) => {{
            let next = $next;
            if !closed.get(next) {
                closed.set(next);
                reach.set(next);
                queue[tail] = next as u16;
                tail += 1;
            }
        }};
    }
    push!(cell_index(start.x, start.y));
    while head < tail {
        let index = queue[head] as usize;
        head += 1;
        let (x, y) = (index & ROW_MASK, index >> ROW_SHIFT);
        if y > 0 {
            push!(index - MAP_MAX);
        }
        if x + 1 < w {
            push!(index + 1);
        }
        if y + 1 < h {
            push!(index + MAP_MAX);
        }
        if x > 0 {
            push!(index - 1);
        }
    }
    reach
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::Tile;

    fn grid(rows: &[&str]) -> [u8; MAP_CELLS] {
        let mut tiles = [Tile::Wall as u8; MAP_CELLS];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                tiles[cell_index(x as u8, y as u8)] = match ch {
                    b'#' => Tile::Wall,
                    b'~' => Tile::Water,
                    _ => Tile::Floor,
                } as u8;
            }
        }
        tiles
    }

    #[test]
    fn open_room_reaches_everything() {
        let tiles = [Tile::Floor as u8; MAP_CELLS];
        let reach = reachable_from(&tiles, 16, 16, Cell { x: 0, y: 0 });
        assert_eq!(reach.count(), 256);
        assert!(reach.contains(Cell { x: 15, y: 15 }));
        assert!(!reach.contains(Cell { x: 16, y: 0 }), "outside the map");
    }

    #[test]
    fn walls_split_the_map() {
        let tiles = grid(&["..#..", "..#..", "..#.."]);
        let reach = reachable_from(&tiles, 5, 3, Cell { x: 0, y: 0 });
        assert_eq!(reach.count(), 6);
        assert!(!reach.contains(Cell { x: 3, y: 0 }));
    }

    #[test]
    fn water_is_walkable() {
        let tiles = grid(&[".~~~."]);
        let reach = reachable_from(&tiles, 5, 1, Cell { x: 0, y: 0 });
        assert!(reach.contains(Cell { x: 4, y: 0 }));
    }

    #[test]
    fn blocked_cells_close_a_corridor() {
        let tiles = grid(&["....."]);
        let gate = Cell { x: 2, y: 0 };
        let reach = reachable_avoiding(&tiles, 5, 1, Cell { x: 0, y: 0 }, &[gate]);
        assert_eq!(reach.count(), 2);
        assert!(!reach.contains(gate));
        assert_eq!(
            reachable_avoiding(&tiles, 5, 1, gate, &[gate]).count(),
            0,
            "blocked start"
        );
        // Out-of-map blocked cells are ignored.
        let far = Cell { x: 9, y: 9 };
        assert_eq!(
            reachable_avoiding(&tiles, 5, 1, Cell { x: 0, y: 0 }, &[far]).count(),
            5
        );
    }

    #[test]
    fn no_diagonal_steps() {
        let tiles = grid(&[".#", "#."]);
        let reach = reachable_from(&tiles, 2, 2, Cell { x: 0, y: 0 });
        assert_eq!(reach.count(), 1);
    }

    #[test]
    fn blocked_start_reaches_nothing() {
        let tiles = grid(&["#."]);
        assert_eq!(reachable_from(&tiles, 2, 1, Cell { x: 0, y: 0 }).count(), 0);
        assert_eq!(reachable_from(&tiles, 2, 1, Cell { x: 5, y: 0 }).count(), 0);
    }

    #[test]
    fn ignores_cells_outside_width_and_height() {
        // Floor everywhere, but the map is only 8x8.
        let tiles = [Tile::Floor as u8; MAP_CELLS];
        assert_eq!(
            reachable_from(&tiles, 8, 8, Cell { x: 3, y: 3 }).count(),
            64
        );
    }

    #[test]
    fn winding_corridor_reaches_exit_but_not_sealed_pocket() {
        // 8x8. The exit (7,7) is one corridor away. (3,2) and (4,3) sit in a walled pocket.
        let tiles = grid(&[
            "........", "#######.", "###..##.", "###..##.", "#######.", "#.......", "#.######",
            "#.......",
        ]);
        let reach = reachable_from(&tiles, 8, 8, Cell { x: 0, y: 0 });
        assert!(reach.contains(Cell { x: 7, y: 7 }));
        assert!(!reach.contains(Cell { x: 3, y: 2 }));
        assert!(!reach.contains(Cell { x: 4, y: 3 }));
    }

    #[test]
    fn serpentine_32_reaches_far_corner() {
        // Even rows are open. Each odd row is a wall with one gap, left then right.
        let mut tiles = grid(&[]);
        let mut floors = 0u32;
        for y in 0..MAP_MAX as u8 {
            let gap = if y % 2 == 0 {
                None
            } else if (y / 2) % 2 == 0 {
                Some(0)
            } else {
                Some(MAP_MAX as u8 - 1)
            };
            for x in 0..MAP_MAX as u8 {
                if gap.is_none_or(|g| x == g) {
                    tiles[cell_index(x, y)] = Tile::Floor as u8;
                    floors += 1;
                }
            }
        }
        let reach = reachable_from(&tiles, MAP_MAX as u8, MAP_MAX as u8, Cell { x: 0, y: 0 });
        assert!(reach.contains(Cell {
            x: MAP_MAX as u8 - 1,
            y: MAP_MAX as u8 - 1,
        }));
        assert_eq!(reach.count(), floors);
    }

    #[test]
    fn closed_room_exit_is_unreachable() {
        let tiles = grid(&[
            "........", ".######.", ".#....#.", ".#....#.", ".#....#.", ".######.", "........",
        ]);
        let reach = reachable_from(&tiles, 8, 7, Cell { x: 0, y: 0 });
        assert!(reach.contains(Cell { x: 0, y: 6 }));
        assert!(!reach.contains(Cell { x: 3, y: 3 }));
        assert!(!reach.contains(Cell { x: 4, y: 2 }));
    }

    #[test]
    fn water_corridor_is_the_only_link() {
        let open = grid(&["...#...", "...~...", "...#..."]);
        let reach = reachable_from(&open, 7, 3, Cell { x: 0, y: 0 });
        assert!(reach.contains(Cell { x: 6, y: 2 }));
        assert_eq!(reach.count(), 19);

        let mut walled = open;
        walled[cell_index(3, 1)] = Tile::Wall as u8;
        let blocked = reachable_from(&walled, 7, 3, Cell { x: 0, y: 0 });
        assert!(!blocked.contains(Cell { x: 6, y: 0 }));
        assert!(!blocked.contains(Cell { x: 6, y: 2 }));
        assert_eq!(blocked.count(), 9);
    }

    #[test]
    fn start_boxed_by_four_walls_counts_one() {
        let tiles = grid(&["###", "#.#", "###"]);
        let reach = reachable_from(&tiles, 3, 3, Cell { x: 1, y: 1 });
        assert_eq!(reach.count(), 1);
        assert!(reach.contains(Cell { x: 1, y: 1 }));
        assert!(!reach.contains(Cell { x: 1, y: 0 }));
        assert!(!reach.contains(Cell { x: 2, y: 1 }));
    }

    #[test]
    fn width_and_height_bound_an_open_floor() {
        let tiles = [Tile::Floor as u8; MAP_CELLS];
        for (width, height) in [(8u8, 8u8), (16, 8), (32, 32), (8, 32)] {
            let reach = reachable_from(&tiles, width, height, Cell { x: 0, y: 0 });
            assert_eq!(
                reach.count(),
                u32::from(width) * u32::from(height),
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn start_on_last_row_and_column() {
        let tiles = [Tile::Floor as u8; MAP_CELLS];
        let reach = reachable_from(&tiles, 32, 32, Cell { x: 31, y: 31 });
        assert_eq!(reach.count(), 32 * 32);
        assert!(reach.contains(Cell { x: 0, y: 0 }));

        let narrow = reachable_from(&tiles, 8, 32, Cell { x: 7, y: 31 });
        assert_eq!(narrow.count(), 8 * 32);
        assert!(narrow.contains(Cell { x: 0, y: 0 }));
    }

    #[test]
    fn unknown_tile_bytes_act_as_walls() {
        let mut tiles = [Tile::Floor as u8; MAP_CELLS];
        tiles[cell_index(1, 0)] = 7;
        tiles[cell_index(1, 1)] = 200;
        tiles[cell_index(1, 2)] = 255;
        let reach = reachable_from(&tiles, 3, 3, Cell { x: 0, y: 1 });
        assert_eq!(reach.count(), 3);
        assert!(reach.contains(Cell { x: 0, y: 0 }));
        assert!(reach.contains(Cell { x: 0, y: 2 }));
        assert!(!reach.contains(Cell { x: 1, y: 0 }));
        assert!(!reach.contains(Cell { x: 1, y: 1 }));
        assert!(!reach.contains(Cell { x: 1, y: 2 }));
        assert!(!reach.contains(Cell { x: 2, y: 1 }));
    }
}
