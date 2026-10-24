//! Walking a path of cells (`move_path`, docs/ARCHITECTURE.md 6: "actor; spends action points; no
//! rolls"; docs/PARTY_AND_TURNS.md 5.1). Patch 13.
//!
//! The checks know nothing of the map's layout or size: the caller's [`Field`] says what a cell
//! costs and what stands on it. The session program implements it over the map layout; a combat
//! arena can implement it over its own cells.

use deadroll_types::Cell;

/// Most cells one move walks (the instruction's path argument).
pub const MAX_PATH: usize = 16;

/// What stands on a cell and stops a step into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocker {
    /// A living unit other than the mover.
    Unit,
    /// A closed chest (patch 11: it occupies its cell until opened).
    ClosedChest,
}

/// The ground a unit walks on.
pub trait Field {
    /// `cell` is on the field.
    fn contains(&self, cell: Cell) -> bool;
    /// Action points to step onto `cell` (its tile type's cost in the `Tileset`), `None` if it
    /// cannot be entered.
    fn step_cost(&self, cell: Cell) -> Option<u8>;
    /// What holds `cell` for the mover, if anything.
    fn blocker(&self, cell: Cell) -> Option<Blocker>;
}

/// The first broken rule of a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkError {
    /// No cells, or more than [`MAX_PATH`].
    PathLength,
    /// A cell is not one of the 8 neighbors of the one before it (the start for the first).
    NotAdjacent,
    /// A cell outside the field.
    OutOfBounds,
    /// A cell that cannot be entered.
    Blocked,
    /// A living unit on the cell.
    Occupied,
    /// A closed chest on the cell.
    ChestInTheWay,
    /// The steps cost more than the action points.
    NotEnoughAp,
    /// A diagonal step past a corner: one of the two orthogonal cells it passes between cannot be
    /// entered (its tile type; units and closed chests do not block a diagonal).
    CornerBlocked,
}

/// Where a walk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    pub end: Cell,
    pub ap_left: u8,
    /// Cells of the path walked (the path is cut after a `stop`).
    pub steps: usize,
}

/// `b` is one of the 8 neighbors of `a` (Chebyshev distance 1): a diagonal step is a step.
pub const fn adjacent(a: Cell, b: Cell) -> bool {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    (dx | dy) != 0 && dx <= 1 && dy <= 1
}

/// Walks `path` from `start` with `ap` action points. Each cell must neighbor the one before, be on
/// the field, be enterable, be free, and be paid for. A diagonal step never cuts a corner: both
/// orthogonal cells it passes between must be enterable tiles (`Field::step_cost`). `stop(from, to)` is asked after each step:
/// `true` ends the walk on `to` (a reactive skill answers there); the rest of the path is not
/// walked and its action points stay. No dice: nothing here is random.
pub fn walk<F: Field>(
    field: &F,
    start: Cell,
    path: &[Cell],
    ap: u8,
    mut stop: impl FnMut(Cell, Cell) -> bool,
) -> Result<Walk, WalkError> {
    if path.is_empty() || path.len() > MAX_PATH {
        return Err(WalkError::PathLength);
    }
    let mut walked = Walk {
        end: start,
        ap_left: ap,
        steps: 0,
    };
    for &next in path {
        let from = walked.end;
        if !adjacent(from, next) {
            return Err(WalkError::NotAdjacent);
        }
        if !field.contains(next) {
            return Err(WalkError::OutOfBounds);
        }
        let cost = field.step_cost(next).ok_or(WalkError::Blocked)?;
        if from.x != next.x && from.y != next.y {
            let side = Cell {
                x: next.x,
                y: from.y,
            };
            let other = Cell {
                x: from.x,
                y: next.y,
            };
            if field.step_cost(side).is_none() || field.step_cost(other).is_none() {
                return Err(WalkError::CornerBlocked);
            }
        }
        match field.blocker(next) {
            Some(Blocker::Unit) => return Err(WalkError::Occupied),
            Some(Blocker::ClosedChest) => return Err(WalkError::ChestInTheWay),
            None => {}
        }
        walked.ap_left = walked
            .ap_left
            .checked_sub(cost)
            .ok_or(WalkError::NotEnoughAp)?;
        walked.end = next;
        walked.steps += 1;
        if stop(from, next) {
            break;
        }
    }
    Ok(walked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arena that is not a map layout: `w` x `h` cells given by rows of characters.
    /// `.` costs 1, `~` 2, `#` cannot be entered, `U` a unit, `C` a closed chest.
    struct Arena<'a> {
        rows: &'a [&'a str],
    }

    impl Arena<'_> {
        fn at(&self, c: Cell) -> u8 {
            self.rows
                .get(c.y as usize)
                .and_then(|r| r.as_bytes().get(c.x as usize))
                .copied()
                .unwrap_or(b'#')
        }
    }

    impl Field for Arena<'_> {
        fn contains(&self, c: Cell) -> bool {
            (c.y as usize) < self.rows.len() && (c.x as usize) < self.rows[0].len()
        }
        fn step_cost(&self, c: Cell) -> Option<u8> {
            match self.at(c) {
                b'#' => None,
                b'~' => Some(2),
                _ => Some(1),
            }
        }
        fn blocker(&self, c: Cell) -> Option<Blocker> {
            match self.at(c) {
                b'U' => Some(Blocker::Unit),
                b'C' => Some(Blocker::ClosedChest),
                _ => None,
            }
        }
    }

    const fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    fn go(rows: &[&str], start: Cell, path: &[Cell], ap: u8) -> Result<Walk, WalkError> {
        walk(&Arena { rows }, start, path, ap, |_, _| false)
    }

    /// A long thin arena (5 x 40): nothing assumes a square map of 32.
    const LONG: &[&str] = &[
        "........................................",
        "........................................",
        "..........~~~~..........................",
        "..........####..........................",
        "........................................",
    ];

    #[test]
    fn straight_and_diagonal_steps_cost_the_tile() {
        let w = go(LONG, c(0, 0), &[c(1, 1), c(2, 2), c(3, 1), c(4, 1)], 10).unwrap();
        assert_eq!(
            w,
            Walk {
                end: c(4, 1),
                ap_left: 6,
                steps: 4
            }
        );
        // Shallow water costs 2, diagonal or not.
        let w = go(LONG, c(9, 1), &[c(10, 2), c(11, 2)], 10).unwrap();
        assert_eq!(w.ap_left, 6);
        // Down past the end of the wall row: (13, 2) -> (14, 3) has (14, 2) open, (13, 3) a wall.
        assert_eq!(
            go(LONG, c(13, 2), &[c(14, 3)], 9),
            Err(WalkError::CornerBlocked)
        );
        // Far along the long side.
        let path: [Cell; 16] = core::array::from_fn(|i| c(21 + i as u8, 4));
        assert_eq!(go(LONG, c(20, 4), &path, 16).unwrap().end, c(36, 4));
    }

    #[test]
    fn a_diagonal_never_cuts_a_corner() {
        // Between two walls touching at the corner.
        let rows = &[".#", "#."];
        assert_eq!(
            go(rows, c(0, 0), &[c(1, 1)], 9),
            Err(WalkError::CornerBlocked)
        );
        // Next to a single wall, on either side.
        assert_eq!(
            go(&[".#", ".."], c(0, 0), &[c(1, 1)], 9),
            Err(WalkError::CornerBlocked)
        );
        assert_eq!(
            go(&["..", "#."], c(0, 0), &[c(1, 1)], 9),
            Err(WalkError::CornerBlocked)
        );
        assert_eq!(
            go(&["#.", ".."], c(1, 0), &[c(0, 1)], 9),
            Err(WalkError::CornerBlocked)
        );
        // An open diagonal passes, and so does one past shallow water (passable, cost 2).
        assert_eq!(
            go(&["..", ".."], c(0, 0), &[c(1, 1)], 9).unwrap().end,
            c(1, 1)
        );
        assert_eq!(
            go(&[".~", ".."], c(0, 0), &[c(1, 1)], 9).unwrap().ap_left,
            8
        );
        // A unit or a closed chest beside a diagonal does not block it.
        assert_eq!(
            go(&[".U", "C."], c(0, 0), &[c(1, 1)], 9).unwrap().end,
            c(1, 1)
        );
        // Straight steps never check corners.
        assert_eq!(
            go(&["#.#", "#.#"], c(1, 0), &[c(1, 1)], 9).unwrap().end,
            c(1, 1)
        );
        // A later corner fails the whole move.
        let rows = &["...", "..#", ".#."];
        assert_eq!(
            go(rows, c(0, 0), &[c(1, 0), c(1, 1), c(2, 2)], 9),
            Err(WalkError::CornerBlocked)
        );
    }

    #[test]
    fn rejects_paths_of_bad_length() {
        assert_eq!(go(LONG, c(0, 0), &[], 9), Err(WalkError::PathLength));
        let path: [Cell; MAX_PATH + 1] = core::array::from_fn(|i| c(1 + i as u8, 0));
        assert_eq!(go(LONG, c(0, 0), &path, 99), Err(WalkError::PathLength));
        assert_eq!(
            go(LONG, c(0, 0), &path[..MAX_PATH], 99).unwrap().steps,
            MAX_PATH
        );
    }

    #[test]
    fn rejects_cells_that_do_not_follow() {
        assert_eq!(
            go(LONG, c(0, 0), &[c(2, 0)], 9),
            Err(WalkError::NotAdjacent)
        );
        assert_eq!(
            go(LONG, c(0, 0), &[c(1, 0), c(1, 2)], 9),
            Err(WalkError::NotAdjacent)
        );
        // Standing still is not a step.
        assert_eq!(
            go(LONG, c(3, 3), &[c(3, 3)], 9),
            Err(WalkError::NotAdjacent)
        );
    }

    #[test]
    fn rejects_the_edge_walls_units_and_chests() {
        assert_eq!(
            go(LONG, c(39, 0), &[c(40, 0)], 9),
            Err(WalkError::OutOfBounds)
        );
        assert_eq!(go(LONG, c(10, 4), &[c(10, 3)], 9), Err(WalkError::Blocked));
        let rows = &["..U", "..C", "..."];
        assert_eq!(go(rows, c(1, 0), &[c(2, 0)], 9), Err(WalkError::Occupied));
        assert_eq!(
            go(rows, c(1, 0), &[c(2, 1)], 9),
            Err(WalkError::ChestInTheWay)
        );
        // A later bad cell fails the whole move: nothing is walked.
        assert_eq!(
            go(rows, c(0, 2), &[c(1, 2), c(2, 1)], 9),
            Err(WalkError::ChestInTheWay)
        );
    }

    #[test]
    fn action_points_pay_for_every_step() {
        let path = [c(1, 0), c(2, 0), c(3, 0)];
        assert_eq!(go(LONG, c(0, 0), &path, 3).unwrap().ap_left, 0);
        assert_eq!(go(LONG, c(0, 0), &path, 2), Err(WalkError::NotEnoughAp));
    }

    #[test]
    fn stop_ends_the_walk_and_keeps_the_points() {
        let path = [c(1, 0), c(2, 0), c(3, 0), c(4, 0)];
        let mut asked = 0;
        let w = walk(&Arena { rows: LONG }, c(0, 0), &path, 9, |from, to| {
            asked += 1;
            assert!(adjacent(from, to));
            to == c(2, 0)
        })
        .unwrap();
        assert_eq!(
            w,
            Walk {
                end: c(2, 0),
                ap_left: 7,
                steps: 2
            }
        );
        assert_eq!(asked, 2);
        // Cells after the stop are not checked.
        let bad = [c(1, 0), c(9, 9)];
        let w = walk(&Arena { rows: LONG }, c(0, 0), &bad, 9, |_, _| true).unwrap();
        assert_eq!(w.steps, 1);
    }

    #[test]
    fn adjacency() {
        assert!(adjacent(c(5, 5), c(6, 6)));
        assert!(adjacent(c(5, 5), c(4, 5)));
        assert!(!adjacent(c(5, 5), c(5, 5)));
        assert!(!adjacent(c(5, 5), c(7, 5)));
        assert!(!adjacent(c(0, 0), c(255, 0)));
    }
}
