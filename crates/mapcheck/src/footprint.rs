//! Patch B: the cells a unit stands on (docs/BESTIARY.md 2: regular monsters 1×1, large monsters
//! and bosses 2×2; docs/PARTY_AND_TURNS.md 4.1.3: a boss may occupy 2×2 cells in the arena).
//!
//! A unit's cell is the top-left one of its footprint, `size` cells per side. Distances between
//! units are Chebyshev distances between their nearest cells, so adjacency, range, auras and the
//! party's reach count every cell of a large monster.

use deadroll_types::Cell;

/// The cells a unit stands on: `size` x `size` from `at` (its top-left cell).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footprint {
    pub at: Cell,
    /// Cells per side, at least 1 (a 0 counts as 1).
    pub size: u8,
}

impl From<Cell> for Footprint {
    fn from(at: Cell) -> Self {
        Footprint::cell(at)
    }
}

/// The gap between two spans `[a, a + an)` and `[b, b + bn)` on one axis: 0 when they touch or
/// overlap.
const fn gap(a: u8, an: u8, b: u8, bn: u8) -> u16 {
    let (a, b) = (a as u16, b as u16);
    let (a_end, b_end) = (
        (a + an as u16).saturating_sub(1),
        (b + bn as u16).saturating_sub(1),
    );
    // At most one of the two is not 0.
    b.saturating_sub(a_end) + a.saturating_sub(b_end)
}

/// `v` brought into the span `[lo, lo + n)`, within the byte range.
const fn clamp(v: u8, lo: u8, n: u8) -> u8 {
    let (v, lo) = (v as u16, lo as u16);
    let hi = lo + n as u16 - 1;
    let out = if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    };
    if out > u8::MAX as u16 {
        u8::MAX
    } else {
        out as u8
    }
}

impl Footprint {
    pub const fn new(at: Cell, size: u8) -> Self {
        Footprint {
            at,
            size: if size == 0 { 1 } else { size },
        }
    }

    /// A one-cell unit.
    pub const fn cell(at: Cell) -> Self {
        Footprint { at, size: 1 }
    }

    const fn side(&self) -> u8 {
        if self.size == 0 {
            1
        } else {
            self.size
        }
    }

    /// `c` is one of its cells.
    pub const fn covers(&self, c: Cell) -> bool {
        gap(self.at.x, self.side(), c.x, 1) == 0 && gap(self.at.y, self.side(), c.y, 1) == 0
    }

    /// Its cells, row by row (cells past 255 are left out).
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        let n = self.side() as u16;
        let (x0, y0) = (self.at.x as u16, self.at.y as u16);
        (0..n * n).filter_map(move |i| {
            let (x, y) = (x0 + i % n, y0 + i / n);
            (x <= u8::MAX as u16 && y <= u8::MAX as u16).then_some(Cell {
                x: x as u8,
                y: y as u8,
            })
        })
    }

    /// Chebyshev distance between the nearest cells of the two (0 when they overlap).
    pub const fn distance(&self, other: &Footprint) -> u16 {
        let dx = gap(self.at.x, self.side(), other.at.x, other.side());
        let dy = gap(self.at.y, self.side(), other.at.y, other.side());
        if dx > dy {
            dx
        } else {
            dy
        }
    }

    /// Chebyshev distance from its nearest cell to `c`.
    pub const fn distance_to(&self, c: Cell) -> u16 {
        self.distance(&Footprint::cell(c))
    }

    /// The two share a cell.
    pub const fn overlaps(&self, other: &Footprint) -> bool {
        self.distance(other) == 0
    }

    /// Its cell nearest to `c` (`c` itself when covered): where a line of its own starts.
    pub const fn nearest(&self, c: Cell) -> Cell {
        Cell {
            x: clamp(c.x, self.at.x, self.side()),
            y: clamp(c.y, self.at.y, self.side()),
        }
    }

    /// The footprint fits inside a `width` x `height` field.
    pub const fn inside(&self, width: u8, height: u8) -> bool {
        let n = self.side() as u16;
        self.at.x as u16 + n <= width as u16 && self.at.y as u16 + n <= height as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    #[test]
    fn a_large_unit_covers_its_square() {
        let boss = Footprint::new(c(3, 4), 2);
        let mut cells = [c(0, 0); 4];
        for (o, cell) in cells.iter_mut().zip(boss.cells()) {
            *o = cell;
        }
        assert_eq!(cells, [c(3, 4), c(4, 4), c(3, 5), c(4, 5)]);
        assert_eq!(boss.cells().count(), 4);
        assert!(boss.covers(c(4, 5)) && boss.covers(c(3, 4)));
        assert!(!boss.covers(c(5, 4)) && !boss.covers(c(2, 4)) && !boss.covers(c(3, 6)));
        assert_eq!(Footprint::cell(c(1, 1)).cells().count(), 1);
        // A 0 size counts as one cell.
        assert_eq!(Footprint::new(c(1, 1), 0), Footprint::cell(c(1, 1)));
        // At the edge of the byte range nothing wraps.
        assert_eq!(Footprint::new(c(255, 255), 2).cells().count(), 1);
    }

    #[test]
    fn distances_count_the_nearest_cells() {
        let boss = Footprint::new(c(3, 3), 2);
        // Every cell around the square is next to it.
        for (x, y) in [
            (2, 2),
            (5, 2),
            (2, 5),
            (5, 5),
            (4, 2),
            (2, 4),
            (5, 3),
            (3, 5),
        ] {
            assert_eq!(boss.distance_to(c(x, y)), 1, "({x}, {y})");
        }
        assert_eq!(boss.distance_to(c(4, 4)), 0);
        assert_eq!(boss.distance_to(c(7, 3)), 3);
        assert_eq!(boss.distance_to(c(0, 0)), 3);
        // Two squares.
        let other = Footprint::new(c(6, 1), 2);
        assert_eq!(boss.distance(&other), 2);
        assert_eq!(other.distance(&boss), 2);
        assert!(boss.overlaps(&Footprint::new(c(4, 4), 2)));
        assert!(!boss.overlaps(&Footprint::new(c(5, 3), 2)));
        // One-cell units: the plain Chebyshev distance.
        assert_eq!(Footprint::cell(c(0, 0)).distance_to(c(3, 1)), 3);
        assert_eq!(Footprint::cell(c(0, 0)).distance_to(c(255, 0)), 255);
    }

    #[test]
    fn the_nearest_cell_and_the_field() {
        let boss = Footprint::new(c(3, 3), 2);
        assert_eq!(boss.nearest(c(0, 0)), c(3, 3));
        assert_eq!(boss.nearest(c(9, 4)), c(4, 4));
        assert_eq!(boss.nearest(c(3, 9)), c(3, 4));
        assert_eq!(boss.nearest(c(4, 3)), c(4, 3));
        assert!(boss.inside(5, 5));
        assert!(!boss.inside(4, 5) && !boss.inside(5, 4));
        assert!(Footprint::cell(c(4, 4)).inside(5, 5));
    }
}
