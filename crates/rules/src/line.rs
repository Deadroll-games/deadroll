//! The line target (docs/SKILL_BUILDER.md, sections 2 and 5.3): one use hits every unit on the
//! line, not one target.
//!
//! The line runs from the caster through the chosen target and on to the skill's reach
//! (Chebyshev distance), along a Bresenham line. The first cell that cannot be entered (a wall,
//! a closed door) ends it; units do not. The caster's own cell is not on it. A damage or debuff
//! line hits the monsters on it, a buff line the allies on it, in line order.
use deadroll_types::{Cell, MAP_MAX};

use crate::units::chebyshev;

/// Most cells of one line.
pub const LINE_MAX: usize = MAP_MAX;
/// Most units one line hits.
pub const LINE_TARGETS_MAX: usize = LINE_MAX;

/// The cells of a line, nearest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub cells: [Cell; LINE_MAX],
    pub len: u8,
}

impl Line {
    pub const EMPTY: Line = Line {
        cells: [Cell { x: 0, y: 0 }; LINE_MAX],
        len: 0,
    };

    pub fn cells(&self) -> &[Cell] {
        &self.cells[..self.len as usize]
    }

    /// Position of `cell` on the line, nearest first.
    pub fn index_of(&self, cell: Cell) -> Option<usize> {
        self.cells().iter().position(|c| *c == cell)
    }
}

/// The line from `from` through `through`, up to `reach` cells away; `passable` says which cells
/// can be entered (and are on the map). Empty when `through == from`.
pub fn line_cells(from: Cell, through: Cell, reach: u8, passable: impl Fn(Cell) -> bool) -> Line {
    let mut out = Line::EMPTY;
    if from == through {
        return out;
    }
    let (x1, y1) = (through.x as i32, through.y as i32);
    let (mut x, mut y) = (from.x as i32, from.y as i32);
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let (sx, sy) = ((x1 - x).signum(), (y1 - y).signum());
    // All-octant Bresenham, kept going past the target: up to it, the same cells as the line
    // from `from` to `through`.
    let mut err = dx + dy;
    while (out.len as usize) < LINE_MAX {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        if !(0..=u8::MAX as i32).contains(&x) || !(0..=u8::MAX as i32).contains(&y) {
            break;
        }
        let cell = Cell {
            x: x as u8,
            y: y as u8,
        };
        if chebyshev(from, cell) > reach || !passable(cell) {
            break;
        }
        out.cells[out.len as usize] = cell;
        out.len += 1;
    }
    out
}

/// The units on `line`, in line order: `units` gives each candidate's index and cell (living
/// units of the skill's side only).
pub fn targets_on_line(
    line: &Line,
    units: impl Iterator<Item = (u8, Cell)> + Clone,
) -> ([u8; LINE_TARGETS_MAX], usize) {
    let mut out = [0u8; LINE_TARGETS_MAX];
    let mut n = 0;
    for cell in line.cells() {
        for (index, at) in units.clone() {
            if at == *cell && n < LINE_TARGETS_MAX {
                out[n] = index;
                n += 1;
            }
        }
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    fn open(_: Cell) -> bool {
        true
    }

    #[test]
    fn straight_and_diagonal_lines_go_through_the_target_to_the_reach() {
        let l = line_cells(c(2, 2), c(4, 2), 4, open);
        assert_eq!(l.cells(), &[c(3, 2), c(4, 2), c(5, 2), c(6, 2)]);
        let l = line_cells(c(2, 2), c(3, 3), 3, open);
        assert_eq!(l.cells(), &[c(3, 3), c(4, 4), c(5, 5)]);
        let l = line_cells(c(5, 5), c(5, 3), 2, open);
        assert_eq!(l.cells(), &[c(5, 4), c(5, 3)]);
    }

    #[test]
    fn a_slanted_line_passes_its_target() {
        for (to, reach) in [(c(7, 4), 6), (c(3, 9), 6), (c(10, 2), 6), (c(5, 11), 6)] {
            let from = c(6, 6);
            let l = line_cells(from, to, reach, open);
            assert!(l.index_of(to).is_some(), "{to:?}: {:?}", l.cells());
            // One step per cell: every next cell touches the previous one.
            let mut prev = from;
            for cell in l.cells() {
                assert_eq!(chebyshev(prev, *cell), 1);
                prev = *cell;
            }
            assert!(l.cells().iter().all(|x| chebyshev(from, *x) <= reach));
            assert_eq!(chebyshev(from, *l.cells().last().unwrap()), reach);
        }
    }

    #[test]
    fn a_wall_and_the_map_edge_end_the_line() {
        let l = line_cells(c(0, 0), c(1, 0), 6, |x| x.x != 3);
        assert_eq!(l.cells(), &[c(1, 0), c(2, 0)]);
        let l = line_cells(c(2, 0), c(1, 0), 6, open);
        assert_eq!(l.cells(), &[c(1, 0), c(0, 0)]);
        assert_eq!(line_cells(c(2, 2), c(2, 2), 6, open).len, 0);
    }

    #[test]
    fn targets_come_in_line_order() {
        let l = line_cells(c(0, 0), c(1, 0), 6, open);
        let units = [(7u8, c(4, 0)), (2, c(1, 0)), (5, c(1, 1)), (9, c(6, 0))];
        let (t, n) = targets_on_line(&l, units.iter().copied());
        assert_eq!(&t[..n], &[2, 7, 9]);
    }
}
