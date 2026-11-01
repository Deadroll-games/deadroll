//! The line target on the session's map (`deadroll_rules::line`): walls cut it.
use deadroll_rules::line::{line_cells, Line};
use deadroll_types::{Cell, RangeTag};

use crate::{
    rules::{reach, step_cost},
    state::{external::MapLayoutView, GameSession},
};

/// The cells of a line from `from` through `through`, to the reach of `range`; the first cell
/// that cannot be entered (registry tile costs) ends it.
pub fn skill_line(
    session: &GameSession,
    layout: &MapLayoutView,
    from: Cell,
    through: Cell,
    range: RangeTag,
) -> Line {
    line_cells(from, through, reach(range), |c| {
        c.x < session.width
            && c.y < session.height
            && layout
                .tile(c.x, c.y)
                .is_ok_and(|t| step_cost(t, &session.tile_cost).is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{layout_account, session, WALL, WATER, WIDTH};

    fn cells(from: Cell, through: Cell, range: RangeTag) -> Vec<Cell> {
        let s = session(&[from]);
        let mut layout = layout_account(s.core.map);
        let info = layout.info();
        let view = MapLayoutView::load(&info, None).unwrap();
        skill_line(&s, &view, from, through, range).cells().to_vec()
    }

    const fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    #[test]
    fn a_wall_ends_the_line() {
        // (0, 2) is a wall: the line south from (0, 0) stops on (0, 1).
        assert_eq!(WALL, c(0, 2));
        assert_eq!(cells(c(0, 0), c(0, 1), RangeTag::Ranged), vec![c(0, 1)]);
    }

    #[test]
    fn water_is_passable_and_the_line_runs_to_the_reach() {
        // Water costs 2 to walk but does not stop a line: six cells east of (0, 0).
        assert_eq!(WATER, c(1, 0));
        let east = cells(c(0, 0), c(1, 0), RangeTag::Ranged);
        assert_eq!(east, (1..=6).map(|x| c(x, 0)).collect::<Vec<_>>());
        // A melee line reaches one cell.
        assert_eq!(cells(c(0, 0), c(1, 0), RangeTag::Melee), vec![c(1, 0)]);
    }

    #[test]
    fn the_map_edge_ends_the_line() {
        let east = cells(c(3, 3), c(4, 3), RangeTag::Ranged);
        assert_eq!(east.last(), Some(&c(WIDTH - 1, 3)));
        assert_eq!(east.len(), (WIDTH - 4) as usize);
    }

    #[test]
    fn diagonal_through_the_target() {
        // The Arc test on the validator: (5, 1) through (6, 2) also takes (7, 3).
        let diag = cells(c(4, 0), c(5, 1), RangeTag::Ranged);
        assert_eq!(&diag[..3], &[c(5, 1), c(6, 2), c(7, 3)]);
        assert_eq!(diag.len(), 3, "(8, 4) is off the 8 x 8 map");
    }
}
