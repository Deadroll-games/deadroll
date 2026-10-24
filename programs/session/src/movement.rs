//! The map as the ground of `move_path` (patch 13): `deadroll_mapcheck::walk` checks the path, this
//! file says what a cell costs (the session's copy of the `Tileset`) and what stands on it.
use anchor_lang::prelude::*;
use deadroll_mapcheck::walk::{Blocker, Field, WalkError};
use deadroll_rules::spread::{step_into, BurningCell, BURNING_CELLS_MAX};
use deadroll_types::{ActiveState, Cell, MONSTERS_MAX, STATES_MAX};

use crate::{
    errors::SessionError,
    events::CellBurned,
    rules::{step_cost, Unit, BALANCE},
    state::{external::MapLayoutView, GameSession, Monsters},
};

/// The party's map for one mover: the layout's cells, the living units but the mover, the closed
/// chests.
pub struct MapField<'a, 'b> {
    session: &'a GameSession,
    layout: &'a MapLayoutView<'b>,
    mover: Unit,
    /// Cells of the living monsters other than the mover.
    monsters: [Option<Cell>; MONSTERS_MAX],
}

impl<'a, 'b> MapField<'a, 'b> {
    pub fn new(
        session: &'a GameSession,
        layout: &'a MapLayoutView<'b>,
        monsters: &Monsters,
        mover: Unit,
    ) -> Result<Self> {
        let mut cells = [None; MONSTERS_MAX];
        for index in 0..monsters.count()?.min(MONSTERS_MAX as u8) {
            let m = monsters.get(index)?;
            if m.alive && mover != Unit::Monster(index) {
                cells[index as usize] = Some(Cell { x: m.x, y: m.y });
            }
        }
        Ok(Self {
            session,
            layout,
            mover,
            monsters: cells,
        })
    }
}

impl Field for MapField<'_, '_> {
    fn contains(&self, cell: Cell) -> bool {
        cell.x < self.session.width && cell.y < self.session.height
    }

    fn step_cost(&self, cell: Cell) -> Option<u8> {
        let byte = self.layout.tile(cell.x, cell.y).ok()?;
        step_cost(byte, &self.session.tile_cost)
    }

    fn blocker(&self, cell: Cell) -> Option<Blocker> {
        let s = self.session;
        let player = (0..s.core.player_count).any(|slot| {
            self.mover != Unit::Player(slot)
                && s.player_alive(slot)
                && s.player_pos[slot as usize] == cell
        });
        if player || self.monsters.contains(&Some(cell)) {
            return Some(Blocker::Unit);
        }
        // Patch 11: a closed chest occupies its cell.
        s.closed_chest_at(cell).then_some(Blocker::ClosedChest)
    }
}

/// The program error of a broken path.
pub fn walk_error(e: WalkError) -> Error {
    match e {
        WalkError::PathLength => error!(SessionError::InvalidPathLength),
        WalkError::NotAdjacent => error!(SessionError::PathNotAdjacent),
        WalkError::OutOfBounds => error!(SessionError::OutOfBounds),
        WalkError::Blocked => error!(SessionError::CellBlocked),
        WalkError::Occupied => error!(SessionError::CellOccupied),
        WalkError::ChestInTheWay => error!(SessionError::ChestInTheWay),
        WalkError::NotEnoughAp => error!(SessionError::NotEnoughActionPoints),
        WalkError::CornerBlocked => error!(SessionError::CornerBlocked),
    }
}

/// Every cell of a walked path sets the mover on fire if it burns (docs/DAMAGE_TYPES_AND_MIXES.md
/// 6.4, Blasting Flame; 7.7: burning cells set on fire those who step on them). Writes
/// `CellBurned` per cell.
pub fn burn_on_path(
    session: &GameSession,
    session_key: Pubkey,
    cells: &[BurningCell; BURNING_CELLS_MAX],
    mover: Unit,
    walked: &[Cell],
    states: &mut [ActiveState; STATES_MAX],
    immune_states: u8,
) {
    if cells.iter().all(|c| !c.burns(session.core.round)) {
        return;
    }
    for &at in walked {
        if let Some((_, applied)) = step_into(
            cells,
            at,
            session.core.round,
            states,
            immune_states,
            BALANCE,
        ) {
            emit!(CellBurned {
                session: session_key,
                unit: mover.id(),
                x: at.x,
                y: at.y,
                applied: applied.into(),
                states: *states,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{
        layout_account, monster, monsters_account, session, HEIGHT, WALL, WATER, WIDTH,
    };
    use deadroll_mapcheck::walk::{walk as walk_cells, Walk, MAX_PATH};
    use deadroll_types::Monster;

    const ORIGIN: Cell = Cell { x: 0, y: 0 };
    const P0: Unit = Unit::Player(0);

    const fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    fn code(err: Error) -> u32 {
        match err {
            Error::AnchorError(e) => e.error_code_number,
            other => panic!("unexpected {other:?}"),
        }
    }

    /// `mover` walks `path` from `start` with `ap` on the test map; players at `players` (bits of
    /// `dead` are dead), monsters `ms`, `edit` changes the session (chests).
    #[allow(clippy::too_many_arguments)]
    fn walk_with(
        players: &[Cell],
        dead: u8,
        ms: &[Monster],
        mover: Unit,
        start: Cell,
        path: &[Cell],
        ap: u8,
        edit: impl FnOnce(&mut GameSession),
    ) -> Result<Walk> {
        let mut s = session(players);
        s.alive_mask &= !dead;
        edit(&mut s);
        let monsters_session = Pubkey::new_unique();
        let mut layout = layout_account(s.core.map);
        let mut monsters = monsters_account(monsters_session, ms);
        let (layout_info, monsters_info) = (layout.info(), monsters.info());
        let layout_view = MapLayoutView::load(&layout_info)?;
        let monsters_view = Monsters::load(&monsters_info, &monsters_session)?;
        let field = MapField::new(&s, &layout_view, &monsters_view, mover)?;
        walk_cells(&field, start, path, ap, |_, _| false).map_err(walk_error)
    }

    fn walk(
        players: &[Cell],
        dead: u8,
        ms: &[Monster],
        mover: Unit,
        start: Cell,
        path: &[Cell],
        ap: u8,
    ) -> Result<Walk> {
        walk_with(players, dead, ms, mover, start, path, ap, |_| {})
    }

    fn walk_alone(start: Cell, path: &[Cell], ap: u8) -> Result<Walk> {
        walk(&[start], 0, &[], P0, start, path, ap)
    }

    fn end(w: Walk) -> (Cell, u8) {
        (w.end, w.ap_left)
    }

    #[test]
    fn a_closed_chest_blocks_an_opened_one_does_not() {
        let with_chest = |opened: u8| {
            walk_with(&[ORIGIN], 0, &[], P0, ORIGIN, &[c(1, 1)], 5, |s| {
                s.chests[0] = deadroll_types::MapPoint {
                    kind: deadroll_types::PointKind::Chest,
                    x: 1,
                    y: 1,
                    data: 0,
                };
                s.chest_count = 1;
                s.chests_opened = opened;
            })
        };
        let err = with_chest(0).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::ChestInTheWay));
        assert_eq!(end(with_chest(1).unwrap()), (c(1, 1), 4));
    }

    #[test]
    fn floor_costs_one_water_two_diagonals_too() {
        assert_eq!(WATER, c(1, 0));
        assert_eq!(
            end(walk_alone(ORIGIN, &[c(1, 1)], 5).unwrap()),
            (c(1, 1), 4)
        );
        assert_eq!(
            end(walk_alone(ORIGIN, &[c(1, 0), c(2, 0)], 5).unwrap()),
            (c(2, 0), 2)
        );
        assert_eq!(
            end(walk_alone(c(0, 1), &[c(1, 0)], 5).unwrap()),
            (c(1, 0), 3)
        );
    }

    #[test]
    fn spends_exactly_all_action_points() {
        let path = [c(1, 1), c(2, 2), c(3, 3), c(4, 4), c(5, 5)];
        assert_eq!(end(walk_alone(ORIGIN, &path, 5).unwrap()), (c(5, 5), 0));
        let err = walk_alone(ORIGIN, &[path.as_slice(), &[c(6, 6)]].concat(), 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::NotEnoughActionPoints));
    }

    #[test]
    fn rejects_walls_and_edges() {
        assert_eq!(WALL, c(0, 2));
        let err = walk_alone(c(0, 1), &[c(0, 2)], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellBlocked));
        // The rest of the 32x32 grid is wall, but outside the 8x8 map it is out of bounds first.
        let corner = c(WIDTH - 1, HEIGHT - 1);
        for next in [c(WIDTH, HEIGHT - 1), c(WIDTH - 1, HEIGHT), c(WIDTH, HEIGHT)] {
            let err = walk_alone(corner, &[next], 5).unwrap_err();
            assert_eq!(code(err), u32::from(SessionError::OutOfBounds));
        }
    }

    #[test]
    fn rejects_bad_paths() {
        let too_long: Vec<Cell> = (1..=MAX_PATH as u8 + 1)
            .map(|x| c(x % 2, x / 2 % 2))
            .collect();
        let cases: [(&[Cell], SessionError); 4] = [
            (&[], SessionError::InvalidPathLength),
            (&too_long, SessionError::InvalidPathLength),
            (&[c(2, 0)], SessionError::PathNotAdjacent),
            (&[ORIGIN], SessionError::PathNotAdjacent),
        ];
        for (path, expected) in cases {
            let err = walk_alone(ORIGIN, path, 30).unwrap_err();
            assert_eq!(code(err), u32::from(expected));
        }
    }

    #[test]
    fn living_players_block_dead_do_not() {
        // Player 1 still stands on the entrance.
        let both = [ORIGIN, ORIGIN];
        let path = [c(0, 1), ORIGIN];
        let err = walk(&both, 0, &[], P0, ORIGIN, &path, 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
        walk(&both, 0b10, &[], P0, ORIGIN, &path, 5).unwrap();
    }

    #[test]
    fn a_diagonal_never_cuts_a_corner() {
        // The wall at (0, 2): diagonals past it either way are refused.
        assert_eq!(WALL, c(0, 2));
        for (from, to) in [(c(0, 1), c(1, 2)), (c(0, 3), c(1, 2)), (c(1, 2), c(0, 1))] {
            let err = walk_alone(from, &[to], 5).unwrap_err();
            assert_eq!(
                code(err),
                u32::from(SessionError::CornerBlocked),
                "{from:?} -> {to:?}"
            );
        }
        // An open diagonal passes; a monster beside it does not block it.
        let w = walk(&[c(2, 2)], 0, &[monster(3, 2)], P0, c(2, 2), &[c(3, 3)], 5).unwrap();
        assert_eq!(end(w), (c(3, 3), 4));
    }

    #[test]
    fn living_monsters_block_dead_do_not() {
        let start = c(4, 5);
        let err = walk(&[start], 0, &[monster(5, 5)], P0, start, &[c(5, 5)], 5).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
        let mut dead = monster(5, 5);
        dead.alive = false;
        walk(&[start], 0, &[dead], P0, start, &[c(5, 5)], 5).unwrap();
    }

    #[test]
    fn monster_may_pass_its_own_cell_but_not_others() {
        let at = c(5, 5);
        let pack = [monster(5, 5), monster(6, 5)];
        let mover = Unit::Monster(0);
        let w = walk(&[ORIGIN], 0, &pack, mover, at, &[c(5, 4), at], 4).unwrap();
        assert_eq!(end(w), (at, 2));
        let err = walk(&[ORIGIN], 0, &pack, mover, at, &[c(6, 5)], 4).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::CellOccupied));
    }
}
