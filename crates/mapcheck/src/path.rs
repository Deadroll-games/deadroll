//! Reachability over passable cells (docs/MAP_DESCRIPTION.md, section 10.1).
//!
//! Steps go to the 8 neighbors, the same as `programs/session` movement (patch 13), with the
//! passability of the registry `Tileset` ([`TileRules`]). A diagonal step does not cut a corner:
//! both orthogonal cells it passes between must be passable tiles (units and closed chests do not
//! count, only the `Tileset`). The search is a flood fill over row
//! bitsets: a row of the layout is one `u32` (`MAP_MAX` = 32), so a whole row grows at once. It
//! finds the same cells as a breadth-first search, with fixed buffers of 256 bytes.

use deadroll_types::{cell_kind, Cell, MAP_CELLS, MAP_MAX};

use crate::tiles::{in_bounds, TileRules};

/// Cells reachable from a start cell: bit `x` of word `y`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    bits: [u32; MAP_MAX],
}

const _: () = assert!(MAP_MAX == u32::BITS as usize && MAP_CELLS == MAP_MAX * MAP_MAX);

impl Reach {
    const fn empty() -> Self {
        Self { bits: [0; MAP_MAX] }
    }

    /// `false` for cells outside the map.
    pub fn contains(&self, cell: Cell) -> bool {
        (cell.x as usize) < MAP_MAX
            && (cell.y as usize) < MAP_MAX
            && self.bits[cell.y as usize] & (1 << cell.x) != 0
    }

    /// Number of reachable cells.
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }
}

/// Every cell reachable from `start` through passable in-bounds cells, `start` included.
/// An impassable or out-of-bounds `start` reaches nothing.
pub fn reachable_from(
    cells: &[u8; MAP_CELLS],
    rules: &TileRules,
    width: u8,
    height: u8,
    start: Cell,
) -> Reach {
    reachable_avoiding(cells, rules, width, height, start, &[])
}

/// The cells of `row` that are `open`, joined to them along the row (Kogge-Stone fill: each step
/// doubles the reach, so 5 steps per side cover 32 cells). `row` must be a subset of `open`.
#[inline(always)]
fn fill_row(row: u32, open: u32) -> u32 {
    let (mut up, mut g) = (row, open);
    up |= g & (up << 1);
    g &= g << 1;
    up |= g & (up << 2);
    g &= g << 2;
    up |= g & (up << 4);
    g &= g << 4;
    up |= g & (up << 8);
    g &= g << 8;
    up |= g & (up << 16);
    let (mut down, mut g) = (row, open);
    down |= g & (down >> 1);
    g &= g >> 1;
    down |= g & (down >> 2);
    g &= g >> 2;
    down |= g & (down >> 4);
    g &= g >> 4;
    down |= g & (down >> 8);
    g &= g >> 8;
    down |= g & (down >> 16);
    up | down
}

/// What a step from the cells `from` of one row enters in the next row: straight, or diagonal
/// when both orthogonal cells are passable tiles (`from_pass`: the source row's, `to_pass`: the
/// target row's). A diagonal to the right needs the target column in the source row and the source
/// column in the target row.
#[inline(always)]
const fn step_rows(from: u32, from_pass: u32, to_pass: u32) -> u32 {
    from | ((from << 1) & from_pass & (to_pass << 1)) | ((from >> 1) & from_pass & (to_pass >> 1))
}

/// [`reachable_from`] with `blocked` cells closed as well (patch 11: closed chests occupy their
/// cell). A blocked `start` reaches nothing.
///
/// Hot loop on-chain (`finalize_map`): one pass builds the open rows (a passability mask per tile
/// type, one shift per cell), then sweeps down and up grow every row from its neighbors until
/// nothing changes. A sweep pair costs about the same for any map; the number of pairs is the
/// number of times a path turns back up or down.
pub fn reachable_avoiding(
    cells: &[u8; MAP_CELLS],
    rules: &TileRules,
    width: u8,
    height: u8,
    start: Cell,
    blocked: &[Cell],
) -> Reach {
    let mut reach = Reach::empty();
    let (w, h) = (width as usize, height as usize);
    if !in_bounds(start, width, height) || w > MAP_MAX || h > MAP_MAX {
        return reach;
    }
    let passable = rules.passable_mask();
    // `pass`: passable tiles (what a corner needs); `open`: those without a blocked cell (what a
    // step can enter).
    let mut pass = [0u32; MAP_MAX];
    for (y, row) in pass.iter_mut().enumerate().take(h) {
        let line = &cells[y * MAP_MAX..y * MAP_MAX + w];
        for (x, &byte) in line.iter().enumerate() {
            *row |= ((passable >> cell_kind(byte)) & 1) << x;
        }
    }
    let mut open = pass;
    for c in blocked {
        if in_bounds(*c, width, height) {
            open[c.y as usize] &= !(1 << c.x);
        }
    }
    let first = 1u32 << start.x;
    if open[start.y as usize] & first == 0 {
        return reach;
    }
    reach.bits[start.y as usize] = fill_row(first, open[start.y as usize]);
    let rows = &mut reach.bits;
    let grow = |rows: &mut [u32; MAP_MAX], y: usize| -> bool {
        let above = if y > 0 {
            step_rows(rows[y - 1], pass[y - 1], pass[y])
        } else {
            0
        };
        let below = if y + 1 < h {
            step_rows(rows[y + 1], pass[y + 1], pass[y])
        } else {
            0
        };
        let seed = (rows[y] | above | below) & open[y];
        if seed == rows[y] {
            return false;
        }
        rows[y] = fill_row(seed, open[y]);
        true
    };
    loop {
        let mut changed = false;
        for y in 0..h {
            changed |= grow(rows, y);
        }
        for y in (0..h).rev() {
            changed |= grow(rows, y);
        }
        if !changed {
            break;
        }
    }
    reach
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiles::{cell_index, tests::starter};
    use deadroll_types::TileKind;

    const FLOOR: u8 = TileKind::StoneFloor as u8 * 8;
    const WALL: u8 = TileKind::Wall as u8 * 8;

    fn grid(rows: &[&str]) -> [u8; MAP_CELLS] {
        let mut tiles = [WALL; MAP_CELLS];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                tiles[cell_index(x as u8, y as u8)] = match ch {
                    b'#' => WALL,
                    b'~' => TileKind::ShallowWater as u8 * 8,
                    _ => FLOOR,
                };
            }
        }
        tiles
    }

    fn reachable_from(tiles: &[u8; MAP_CELLS], width: u8, height: u8, start: Cell) -> Reach {
        super::reachable_from(tiles, &starter(), width, height, start)
    }

    fn reachable_avoiding(
        tiles: &[u8; MAP_CELLS],
        width: u8,
        height: u8,
        start: Cell,
        blocked: &[Cell],
    ) -> Reach {
        super::reachable_avoiding(tiles, &starter(), width, height, start, blocked)
    }

    #[test]
    fn open_room_reaches_everything() {
        let tiles = [FLOOR; MAP_CELLS];
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
    fn diagonals_do_not_cut_corners() {
        // Two walls touching at the corner.
        let tiles = grid(&[".#", "#."]);
        let reach = reachable_from(&tiles, 2, 2, Cell { x: 0, y: 0 });
        assert_eq!(reach.count(), 1);
        // A single wall next to the diagonal blocks it too: (0, 0) -> (1, 1) past the wall (1, 0).
        let tiles = grid(&[".#", "##", "..."]);
        let reach = reachable_from(&tiles, 3, 3, Cell { x: 0, y: 2 });
        assert!(!reach.contains(Cell { x: 0, y: 0 }));
        let tiles = grid(&[".#.", "..#"]);
        let reach = reachable_from(&tiles, 3, 2, Cell { x: 1, y: 1 });
        assert!(
            !reach.contains(Cell { x: 2, y: 0 }),
            "past (1, 0) and (2, 1)"
        );
        // Deep water, a chasm or void at the corner block like a wall.
        for kind in [TileKind::DeepWater, TileKind::Chasm, TileKind::Void] {
            let mut tiles = grid(&["..", ".."]);
            tiles[cell_index(1, 0)] = kind as u8 * 8;
            tiles[cell_index(0, 1)] = WALL;
            let reach = reachable_from(&tiles, 2, 2, Cell { x: 0, y: 0 });
            assert!(!reach.contains(Cell { x: 1, y: 1 }), "{kind:?}");
        }
        // An open diagonal passes: shallow water and rubble are passable corners.
        let mut tiles = grid(&["..", ".."]);
        tiles[cell_index(1, 0)] = TileKind::ShallowWater as u8 * 8;
        tiles[cell_index(0, 1)] = TileKind::Rubble as u8 * 8;
        let reach = reachable_avoiding(
            &tiles,
            2,
            2,
            Cell { x: 0, y: 0 },
            &[Cell { x: 1, y: 0 }, Cell { x: 0, y: 1 }],
        );
        assert!(
            reach.contains(Cell { x: 1, y: 1 }),
            "blocked cells (chests) do not block the corner"
        );
        // All 8 neighbors of a middle cell.
        let open = grid(&["...", "...", "..."]);
        let center = Cell { x: 1, y: 1 };
        let reach = reachable_avoiding(
            &open,
            3,
            3,
            center,
            &[
                Cell { x: 0, y: 1 },
                Cell { x: 2, y: 1 },
                Cell { x: 1, y: 0 },
                Cell { x: 1, y: 2 },
            ],
        );
        assert_eq!(reach.count(), 5, "the four corners and the start");
    }

    #[test]
    fn deep_water_and_chasm_block_rubble_does_not() {
        let mut tiles = grid(&["....."]);
        tiles[cell_index(2, 0)] = TileKind::DeepWater as u8 * 8;
        assert_eq!(reachable_from(&tiles, 5, 1, Cell { x: 0, y: 0 }).count(), 2);
        tiles[cell_index(2, 0)] = TileKind::Chasm as u8 * 8 + 3;
        assert_eq!(reachable_from(&tiles, 5, 1, Cell { x: 0, y: 0 }).count(), 2);
        tiles[cell_index(2, 0)] = TileKind::Rubble as u8 * 8 + 7;
        assert_eq!(reachable_from(&tiles, 5, 1, Cell { x: 0, y: 0 }).count(), 5);
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
        let tiles = [FLOOR; MAP_CELLS];
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
                    tiles[cell_index(x, y)] = FLOOR;
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
        walled[cell_index(3, 1)] = WALL;
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
        let tiles = [FLOOR; MAP_CELLS];
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
        let tiles = [FLOOR; MAP_CELLS];
        let reach = reachable_from(&tiles, 32, 32, Cell { x: 31, y: 31 });
        assert_eq!(reach.count(), 32 * 32);
        assert!(reach.contains(Cell { x: 0, y: 0 }));

        let narrow = reachable_from(&tiles, 8, 32, Cell { x: 7, y: 31 });
        assert_eq!(narrow.count(), 8 * 32);
        assert!(narrow.contains(Cell { x: 0, y: 0 }));
    }

    #[test]
    fn unknown_tile_bytes_act_as_walls() {
        let mut tiles = [FLOOR; MAP_CELLS];
        tiles[cell_index(1, 0)] = 7; // void, variant 7
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

#[cfg(test)]
mod same_as_bfs {
    use super::*;
    use crate::tiles::{cell_index, tests::starter};
    use deadroll_types::TileKind;

    /// A plain queue BFS over the 8 neighbors: the reference the row fill must match.
    fn bfs(cells: &[u8; MAP_CELLS], w: u8, h: u8, start: Cell, blocked: &[Cell]) -> Reach {
        let rules = starter();
        let pass = |c: Cell| in_bounds(c, w, h) && rules.is_passable(cells[cell_index(c.x, c.y)]);
        let open = |c: Cell| pass(c) && !blocked.contains(&c);
        let mut reach = Reach::empty();
        if !open(start) {
            return reach;
        }
        let mut queue = [Cell::default(); MAP_CELLS];
        let (mut head, mut tail) = (0, 1);
        queue[0] = start;
        reach.bits[start.y as usize] |= 1 << start.x;
        while head < tail {
            let c = queue[head];
            head += 1;
            for dy in -1i16..=1 {
                for dx in -1i16..=1 {
                    let (x, y) = (c.x as i16 + dx, c.y as i16 + dy);
                    if x < 0 || y < 0 {
                        continue;
                    }
                    let n = Cell {
                        x: x as u8,
                        y: y as u8,
                    };
                    // A diagonal needs both orthogonal cells to be passable tiles.
                    let corner = dx == 0
                        || dy == 0
                        || (pass(Cell { x: n.x, y: c.y }) && pass(Cell { x: c.x, y: n.y }));
                    if corner && open(n) && !reach.contains(n) {
                        reach.bits[n.y as usize] |= 1 << n.x;
                        queue[tail] = n;
                        tail += 1;
                    }
                }
            }
        }
        reach
    }

    #[test]
    fn random_maps_match_a_queue_bfs() {
        // A fixed LCG: walls, deep water and floors at random, several densities and sizes.
        let mut seed = 0x2545_f491_u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 8
        };
        let floor = TileKind::StoneFloor as u8 * 8;
        for round in 0..300 {
            let (w, h) = (8 + (next() % 25) as u8, 8 + (next() % 25) as u8);
            let density = 20 + round % 45;
            let mut cells = [floor; MAP_CELLS];
            for c in cells.iter_mut() {
                let r = next() % 100;
                if r < density {
                    *c = if r % 3 == 0 {
                        TileKind::DeepWater as u8 * 8
                    } else {
                        TileKind::Wall as u8 * 8
                    };
                }
            }
            let start = Cell {
                x: (next() % w as u32) as u8,
                y: (next() % h as u32) as u8,
            };
            let blocked = [Cell {
                x: (next() % 32) as u8,
                y: (next() % 32) as u8,
            }];
            let fast = reachable_avoiding(&cells, &starter(), w, h, start, &blocked);
            assert_eq!(
                fast,
                bfs(&cells, w, h, start, &blocked),
                "round {round}: {w}x{h}"
            );
        }
    }

    #[test]
    fn a_vertical_serpentine_turns_32_times() {
        // Columns alternate: open, wall with a gap at the bottom, open, wall with a gap at the top.
        let wall = TileKind::Wall as u8 * 8;
        let mut cells = [TileKind::StoneFloor as u8 * 8; MAP_CELLS];
        for x in (1..31u8).step_by(2) {
            let gap = if (x / 2) % 2 == 0 { 31 } else { 0 };
            for y in 0..32u8 {
                if y != gap {
                    cells[cell_index(x, y)] = wall;
                }
            }
        }
        let start = Cell { x: 0, y: 0 };
        let fast = reachable_from(&cells, &starter(), 32, 32, start);
        assert!(fast.contains(Cell { x: 31, y: 0 }) && fast.contains(Cell { x: 31, y: 31 }));
        assert_eq!(fast, bfs(&cells, 32, 32, start, &[]));
    }
}
