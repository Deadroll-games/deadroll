//! Path validation shared by `move_unit` and `monster_move`.
use anchor_lang::prelude::*;
use deadroll_types::Cell;

use crate::{
    errors::SessionError,
    rules::{step_cost, Unit, DIRECTIONS, MAX_PATH},
    state::{external::MapLayoutView, GameSession, Monsters},
};

/// A unit's path through the grid: `len` steps from `steps`, each a direction `0..4`.
pub struct Path<'a> {
    pub steps: &'a [u8; MAX_PATH],
    pub len: u8,
}

/// Walks `path` from `start` and returns the end cell and the action points left.
///
/// Every step must stay on the map, enter a passable tile, avoid cells held by any other living
/// unit and be paid for from `action_points`. Nothing is written: the caller stores the result.
pub fn walk_path(
    session: &GameSession,
    layout: &MapLayoutView,
    monsters: &Monsters,
    mover: Unit,
    start: Cell,
    path: Path,
    mut action_points: u8,
) -> Result<(Cell, u8)> {
    require!(
        path.len > 0 && path.len as usize <= MAX_PATH,
        SessionError::InvalidPathLength
    );
    let mut at = start;
    for &dir in &path.steps[..path.len as usize] {
        require!(dir < DIRECTIONS, SessionError::InvalidDirection);
        at = step(at, dir, session.width, session.height)?;
        let cost = step_cost(layout.tile(at.x, at.y)?).ok_or(SessionError::CellBlocked)?;
        require!(
            !occupied(session, monsters, mover, at)?,
            SessionError::CellOccupied
        );
        action_points = action_points
            .checked_sub(cost)
            .ok_or(SessionError::NotEnoughActionPoints)?;
    }
    Ok((at, action_points))
}

fn step(at: Cell, dir: u8, width: u8, height: u8) -> Result<Cell> {
    let next = match dir {
        0 => at.y.checked_sub(1).map(|y| Cell { x: at.x, y }),
        1 => at.x.checked_add(1).map(|x| Cell { x, y: at.y }),
        2 => at.y.checked_add(1).map(|y| Cell { x: at.x, y }),
        _ => at.x.checked_sub(1).map(|x| Cell { x, y: at.y }),
    };
    next.filter(|c| c.x < width && c.y < height)
        .ok_or(error!(SessionError::OutOfBounds))
}

/// Whether a living unit other than `mover` stands on `cell`.
fn occupied(session: &GameSession, monsters: &Monsters, mover: Unit, cell: Cell) -> Result<bool> {
    let player_there = (0..session.core.player_count).any(|slot| {
        mover != Unit::Player(slot)
            && session.player_alive(slot)
            && session.player_pos[slot as usize] == cell
    });
    if player_there {
        return Ok(true);
    }
    for index in 0..monsters.count()? {
        if mover == Unit::Monster(index) {
            continue;
        }
        let m = monsters.get(index)?;
        if m.alive && m.x == cell.x && m.y == cell.y {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{
        layout_account, monster, monsters_account, session, HEIGHT, WALL, WATER, WIDTH,
    };
    use deadroll_types::Monster;

    const N: u8 = 0;
    const E: u8 = 1;
    const S: u8 = 2;
    const W: u8 = 3;
    const ORIGIN: Cell = Cell { x: 0, y: 0 };
    const P0: Unit = Unit::Player(0);

    fn code(err: Error) -> u32 {
        match err {
            Error::AnchorError(e) => e.error_code_number,
            other => panic!("unexpected {other:?}"),
        }
    }

    /// `mover` walks `steps` from `start` with `ap`; players at `players` (bits of `dead` are
    /// dead), monsters `ms`.
    fn walk(
        players: &[Cell],
        dead: u8,
        ms: &[Monster],
        mover: Unit,
        start: Cell,
        steps: &[u8],
        ap: u8,
    ) -> Result<(Cell, u8)> {
        let mut s = session(players);
        s.alive_mask &= !dead;
        let monsters_session = Pubkey::new_unique();
        let mut layout = layout_account(s.core.map);
        let mut monsters = monsters_account(monsters_session, ms);
        let layout_info = layout.info();
        let monsters_info = monsters.info();
        let layout_view = MapLayoutView::load(&layout_info)?;
        let monsters_view = Monsters::load(&monsters_info, &monsters_session)?;
        let mut buf = [0u8; MAX_PATH];
        let n = steps.len().min(MAX_PATH);
        buf[..n].copy_from_slice(&steps[..n]);
        let path = Path {
            steps: &buf,
            len: steps.len() as u8,
        };
        walk_path(&s, &layout_view, &monsters_view, mover, start, path, ap)
    }

    fn walk_alone(start: Cell, steps: &[u8], ap: u8) -> Result<(Cell, u8)> {
        walk(&[start], 0, &[], P0, start, steps, ap)
    }

    #[test]
    fn floor_costs_one_water_two() {
        assert_eq!(
            walk_alone(ORIGIN, &[S, E], 5).unwrap(),
            (Cell { x: 1, y: 1 }, 3)
        );
        assert_eq!(
            walk_alone(ORIGIN, &[E, E], 5).unwrap(),
            (Cell { x: 2, y: 0 }, 2)
        );
        assert_eq!(WATER, Cell { x: 1, y: 0 });
    }

    #[test]
    fn spends_exactly_all_action_points() {
        let path = [S, E, E, E, E];
        assert_eq!(
            walk_alone(ORIGIN, &path, 5).unwrap(),
            (Cell { x: 4, y: 1 }, 0)
        );
        let err = walk_alone(ORIGIN, &[S, E, E, E, E, E], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::NotEnoughActionPoints));
    }

    #[test]
    fn rejects_walls_and_edges() {
        assert_eq!(WALL, Cell { x: 0, y: 2 });
        let err = walk_alone(ORIGIN, &[S, S], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellBlocked));
        for dir in [N, W] {
            let err = walk_alone(ORIGIN, &[dir], 5).unwrap_err();
            assert_eq!(code(err), u32::from(SessionError::OutOfBounds));
        }
        let corner = Cell {
            x: WIDTH - 1,
            y: HEIGHT - 1,
        };
        for dir in [E, S] {
            let err = walk_alone(corner, &[dir], 5).unwrap_err();
            assert_eq!(code(err), u32::from(SessionError::OutOfBounds));
        }
    }

    #[test]
    fn rejects_bad_paths() {
        let too_long = [S; MAX_PATH + 1];
        let cases: [(&[u8], SessionError); 3] = [
            (&[], SessionError::InvalidPathLength),
            (&too_long, SessionError::InvalidPathLength),
            (&[4], SessionError::InvalidDirection),
        ];
        for (steps, expected) in cases {
            let err = walk_alone(ORIGIN, steps, 30).unwrap_err();
            assert_eq!(code(err), u32::from(expected));
        }
    }

    #[test]
    fn living_players_block_dead_do_not() {
        // Player 1 still stands on the entrance.
        let both = [ORIGIN, ORIGIN];
        let err = walk(&both, 0, &[], P0, ORIGIN, &[S, N], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
        walk(&both, 0b10, &[], P0, ORIGIN, &[S, N], 5).unwrap();
    }

    #[test]
    fn living_monsters_block_dead_do_not() {
        let start = Cell { x: 4, y: 5 };
        let err = walk(&[start], 0, &[monster(5, 5)], P0, start, &[E], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
        let mut dead = monster(5, 5);
        dead.alive = false;
        walk(&[start], 0, &[dead], P0, start, &[E], 5).unwrap();
    }

    #[test]
    fn monster_may_pass_its_own_cell_but_not_others() {
        let at = Cell { x: 5, y: 5 };
        let pack = [monster(5, 5), monster(6, 5)];
        let mover = Unit::Monster(0);
        assert_eq!(
            walk(&[ORIGIN], 0, &pack, mover, at, &[N, S], 4).unwrap(),
            (at, 2)
        );
        let err = walk(&[ORIGIN], 0, &pack, mover, at, &[E], 4).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
    }
}
