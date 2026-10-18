//! What a resonance does beyond its target (docs/DAMAGE_TYPES_AND_MIXES.md 6.4): states that jump
//! to other monsters, a knockback, cells set on fire. `crates/combat` returns them with the
//! `HitResult`; this module plays them on the map, the same way for the program and the
//! simulator.
//!
//! - Adjacent: Chebyshev distance 1. Of several candidates the lowest monster index is taken.
//! - A jump lands only on a living monster that is not immune to the state and was not reached
//!   yet by this jump.
//! - "Behind the target": one step from the target along the attacker-to-target direction
//!   (each axis -1, 0 or 1).
//! - A knockback moves the target one cell at a time while the next cell is passable and free.
//! - Cells around the target (after the knockback) that are passable burn for
//!   `Balance::burning_cell_rounds` rounds; whoever steps in catches Burning (section 7.7: such
//!   cells "set on fire those who step on them").
use deadroll_combat::{
    apply_chill, apply_state, states::freeze_for, Applied, HitResult, JumpMode, JUMPS_MAX,
};
use deadroll_types::{ActiveState, Cell, Monster, StateKind};

use crate::config::BalanceConfig;
use crate::monster::MonsterDefense;

/// Cells burning at once in one fight.
pub const BURNING_CELLS_MAX: usize = 16;
/// Monsters one hit can pass states on to.
pub const SPREAD_HITS_MAX: usize = 8;

/// The map as the spread sees it.
pub trait Field {
    /// Inside the map and walkable.
    fn passable(&self, cell: Cell) -> bool;
    /// A living unit (player or monster) stands there.
    fn occupied(&self, cell: Cell) -> bool;
}

/// A cell on fire. `power == 0` is a free slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BurningCell {
    pub cell: Cell,
    /// Burns through this round.
    pub until_round: u16,
    /// Burning damage per turn on whoever steps in.
    pub power: u32,
    /// Turn-order id of the unit that set it on fire.
    pub source: u8,
}

impl BurningCell {
    pub fn burns(&self, round: u16) -> bool {
        self.power > 0 && self.until_round >= round
    }
}

/// One monster a state jumped to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpreadHit {
    pub monster: u8,
    pub kind: StateKind,
    pub applied: Applied,
}

/// What [`spread`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpreadOutcome {
    pub hits: [SpreadHit; SPREAD_HITS_MAX],
    pub hit_count: u8,
    /// The target was pushed from one cell to another.
    pub moved: Option<(Cell, Cell)>,
    /// Cells set on fire (or refreshed) by this hit.
    pub ignited: [Cell; 8],
    pub ignited_count: u8,
}

impl SpreadOutcome {
    pub fn hits(&self) -> &[SpreadHit] {
        &self.hits[..self.hit_count as usize]
    }

    pub fn ignited(&self) -> &[Cell] {
        &self.ignited[..self.ignited_count as usize]
    }

    pub fn is_empty(&self) -> bool {
        self.hit_count == 0 && self.moved.is_none() && self.ignited_count == 0
    }
}

fn chebyshev(a: Cell, b: Cell) -> u8 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

fn cell_of(m: &Monster) -> Cell {
    Cell { x: m.x, y: m.y }
}

/// One step from `from` in direction `(dx, dy)`, `None` off the grid.
fn step(from: Cell, dx: i8, dy: i8) -> Option<Cell> {
    let x = from.x as i16 + dx as i16;
    let y = from.y as i16 + dy as i16;
    (0..=u8::MAX as i16).contains(&x).then_some(())?;
    (0..=u8::MAX as i16).contains(&y).then_some(())?;
    Some(Cell {
        x: x as u8,
        y: y as u8,
    })
}

/// Lands `state` on a unit's `states`: Chill adds its buildup, Freeze clears it.
pub fn land_state(
    states: &mut [ActiveState; deadroll_types::STATES_MAX],
    state: ActiveState,
    cfg: &BalanceConfig,
) -> Applied {
    match state.kind {
        StateKind::Chill => apply_chill(states, state.power, state.source, &cfg.combat),
        StateKind::Freeze => freeze_for(states, state.turns, state.source),
        _ => apply_state(states, state),
    }
}

/// Plays the jumps, the knockback and the burning cells of `result` (a hit by a unit at
/// `attacker` on monster `target`). `defense[i]` is monster `i`'s; `round` the current one.
#[allow(clippy::too_many_arguments)]
pub fn spread(
    cfg: &BalanceConfig,
    result: &HitResult,
    attacker: Cell,
    target: usize,
    monsters: &mut [Monster],
    defense: &[MonsterDefense],
    field: &impl Field,
    cells: &mut [BurningCell; BURNING_CELLS_MAX],
    round: u16,
    source: u8,
) -> SpreadOutcome {
    let mut out = SpreadOutcome::default();
    let Some(origin) = monsters.get(target).map(cell_of) else {
        return out;
    };
    let can = |monsters: &[Monster], i: usize, kind: StateKind| {
        monsters[i].alive
            && defense
                .get(i)
                .is_none_or(|d| d.immune_states & (1 << kind as u8) == 0)
    };
    let dir = (
        (origin.x as i16 - attacker.x as i16).signum() as i8,
        (origin.y as i16 - attacker.y as i16).signum() as i8,
    );

    for jump in result.jumps().iter().take(JUMPS_MAX) {
        let kind = jump.state.kind;
        // Reached by this jump: the target and every monster it passed through.
        let mut reached = 1u32 << target.min(31);
        let mut from = origin;
        let mut left = jump.limit;
        while left > 0 {
            let next = match jump.mode {
                JumpMode::Adjacent | JumpMode::Chain => (0..monsters.len()).find(|&i| {
                    reached & (1 << i.min(31)) == 0
                        && can(monsters, i, kind)
                        && chebyshev(cell_of(&monsters[i]), from) == 1
                        && (jump.mode != JumpMode::Chain
                            || deadroll_combat::states::has(&monsters[i].states, jump.filter))
                }),
                JumpMode::Line => step(origin, dir.0, dir.1).and_then(|behind| {
                    (0..monsters.len()).find(|&i| {
                        i != target && can(monsters, i, kind) && cell_of(&monsters[i]) == behind
                    })
                }),
            };
            let Some(i) = next else {
                break;
            };
            let applied = land_state(&mut monsters[i].states, jump.state, cfg);
            if (out.hit_count as usize) < SPREAD_HITS_MAX {
                out.hits[out.hit_count as usize] = SpreadHit {
                    monster: i as u8,
                    kind,
                    applied,
                };
                out.hit_count += 1;
            }
            reached |= 1 << i.min(31);
            from = cell_of(&monsters[i]);
            left -= 1;
            if jump.mode != JumpMode::Chain {
                break;
            }
        }
    }

    // Knockback, away from the attacker.
    let mut at = origin;
    if dir != (0, 0) && monsters[target].alive {
        for _ in 0..result.knockback {
            match step(at, dir.0, dir.1) {
                Some(next) if field.passable(next) && !field.occupied(next) => at = next,
                _ => break,
            }
        }
        if at != origin {
            monsters[target].x = at.x;
            monsters[target].y = at.y;
            out.moved = Some((origin, at));
        }
    }

    // Burning cells around the target.
    if result.ignite_power > 0 {
        let until_round = round.saturating_add(cfg.combat.burning_cell_rounds as u16);
        for (dx, dy) in NEIGHBORS {
            let Some(cell) = step(at, dx, dy).filter(|c| field.passable(*c)) else {
                continue;
            };
            if ignite(cells, cell, until_round, result.ignite_power, source, round) {
                out.ignited[out.ignited_count as usize] = cell;
                out.ignited_count += 1;
            }
        }
    }
    out
}

/// The eight cells around one, row by row.
const NEIGHBORS: [(i8, i8); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Sets `cell` on fire: a burning one keeps the stronger fire and the later end, a new one takes
/// a free or burnt-out slot. False when every slot burns.
pub fn ignite(
    cells: &mut [BurningCell; BURNING_CELLS_MAX],
    cell: Cell,
    until_round: u16,
    power: u32,
    source: u8,
    round: u16,
) -> bool {
    if let Some(c) = cells.iter_mut().find(|c| c.burns(round) && c.cell == cell) {
        if power > c.power {
            c.power = power;
            c.source = source;
        }
        c.until_round = c.until_round.max(until_round);
        return true;
    }
    match cells.iter_mut().find(|c| !c.burns(round)) {
        Some(slot) => {
            *slot = BurningCell {
                cell,
                until_round,
                power,
                source,
            };
            true
        }
        None => false,
    }
}

/// A unit steps into `cell`: Burning from a burning cell, unless it is immune (`immune_states`
/// bits). `None` when the cell does not burn.
pub fn step_into(
    cells: &[BurningCell; BURNING_CELLS_MAX],
    cell: Cell,
    round: u16,
    states: &mut [ActiveState; deadroll_types::STATES_MAX],
    immune_states: u8,
    cfg: &BalanceConfig,
) -> Option<(ActiveState, Applied)> {
    let c = cells.iter().find(|c| c.burns(round) && c.cell == cell)?;
    if immune_states & (1 << StateKind::Burning as u8) != 0 {
        return None;
    }
    let state = ActiveState {
        kind: StateKind::Burning,
        turns: cfg.combat.turns(StateKind::Burning),
        power: c.power,
        source: c.source,
    };
    Some((state, apply_state(states, state)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::Jump;
    use deadroll_types::Aspect;

    const CFG: &BalanceConfig = &BalanceConfig::DEFAULT;

    /// A 10x10 open map with walls on the listed cells.
    struct Map<'a> {
        walls: &'a [Cell],
        units: &'a [Cell],
    }

    impl Field for Map<'_> {
        fn passable(&self, c: Cell) -> bool {
            c.x < 10 && c.y < 10 && !self.walls.contains(&c)
        }
        fn occupied(&self, c: Cell) -> bool {
            self.units.contains(&c)
        }
    }

    fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    fn monster(x: u8, y: u8) -> Monster {
        Monster {
            x,
            y,
            hp: 10,
            hp_max: 10,
            alive: true,
            ..Monster::default()
        }
    }

    fn burning(power: u32) -> ActiveState {
        ActiveState {
            kind: StateKind::Burning,
            turns: 3,
            power,
            source: 0,
        }
    }

    fn with_jump(mode: JumpMode, state: ActiveState, filter: StateKind, limit: u8) -> HitResult {
        let mut r = HitResult::default();
        r.jumps[0] = Jump {
            mode,
            state,
            filter,
            limit,
            base: Aspect::Fire,
        };
        r.jump_count = 1;
        r
    }

    const OPEN: Map = Map {
        walls: &[],
        units: &[],
    };

    fn has(m: &Monster, kind: StateKind) -> bool {
        deadroll_combat::states::has(&m.states, kind)
    }

    #[test]
    fn a_jump_takes_the_lowest_adjacent_living_monster_that_is_not_immune() {
        let mut ms = [
            monster(5, 5),
            monster(7, 5), // too far
            monster(6, 6), // adjacent (diagonal)
            monster(4, 5), // adjacent
        ];
        let defense = [MonsterDefense::NONE; 4];
        let r = with_jump(JumpMode::Adjacent, burning(3), StateKind::Burning, 1);
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        let out = spread(
            CFG,
            &r,
            c(0, 5),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert_eq!(
            out.hits(),
            [SpreadHit {
                monster: 2,
                kind: StateKind::Burning,
                applied: Applied::Added
            }]
        );
        assert!(has(&ms[2], StateKind::Burning) && !has(&ms[3], StateKind::Burning));
        // Monster 2 immune, monster 3 dead: nobody.
        let mut ms = [monster(5, 5), monster(6, 6), monster(4, 5)];
        ms[2].alive = false;
        let defense = [
            MonsterDefense::NONE,
            MonsterDefense {
                immune_states: 1 << StateKind::Burning as u8,
                ..MonsterDefense::NONE
            },
            MonsterDefense::NONE,
        ];
        let out = spread(
            CFG,
            &r,
            c(0, 5),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_chain_follows_bleeding_monsters_up_to_its_limit() {
        let bleed = ActiveState {
            kind: StateKind::Bleeding,
            turns: 2,
            power: 1,
            source: 0,
        };
        let mut ms = [
            monster(1, 1),
            monster(2, 1),
            monster(3, 1),
            monster(4, 1),
            monster(1, 2),
        ];
        for i in [1, 2, 3] {
            ms[i].states[0] = bleed;
        }
        let shock = ActiveState {
            kind: StateKind::Shock,
            turns: 2,
            power: 2_000,
            source: 0,
        };
        let r = with_jump(JumpMode::Chain, shock, StateKind::Bleeding, 2);
        let defense = [MonsterDefense::NONE; 5];
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        let out = spread(
            CFG,
            &r,
            c(0, 1),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        // Monster 4 is adjacent but not bleeding; the chain stops after two.
        assert_eq!(out.hit_count, 2);
        assert_eq!((out.hits()[0].monster, out.hits()[1].monster), (1, 2));
        assert!(!has(&ms[3], StateKind::Shock) && !has(&ms[4], StateKind::Shock));
    }

    #[test]
    fn a_line_hits_only_the_cell_right_behind_the_target() {
        let shock = ActiveState {
            kind: StateKind::Shock,
            turns: 2,
            power: 2_000,
            source: 0,
        };
        let r = with_jump(JumpMode::Line, shock, StateKind::Shock, 1);
        let defense = [MonsterDefense::NONE; 3];
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        // Attacker at (2,2), target at (4,4): behind is (5,5); (5,4) is beside it.
        let mut ms = [monster(4, 4), monster(5, 4), monster(5, 5)];
        let out = spread(
            CFG,
            &r,
            c(2, 2),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert_eq!(out.hits()[0].monster, 2);
        assert_eq!(out.hit_count, 1);
        let mut ms = [monster(4, 4), monster(5, 4)];
        let out = spread(
            CFG,
            &r,
            c(2, 2),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn knockback_stops_at_walls_units_and_the_edge() {
        let mut r = HitResult {
            knockback: 2,
            ..HitResult::default()
        };
        let defense = [MonsterDefense::NONE; 1];
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        let mut ms = [monster(5, 5)];
        let out = spread(
            CFG,
            &r,
            c(4, 5),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert_eq!(out.moved, Some((c(5, 5), c(7, 5))));
        assert_eq!((ms[0].x, ms[0].y), (7, 5));
        // A wall two cells away: one step.
        let walled = Map {
            walls: &[c(7, 5)],
            units: &[],
        };
        let mut ms = [monster(5, 5)];
        spread(
            CFG,
            &r,
            c(4, 5),
            0,
            &mut ms,
            &defense,
            &walled,
            &mut cells,
            1,
            0,
        );
        assert_eq!(ms[0].x, 6);
        // A unit right behind: no move. The map edge: no move.
        let crowded = Map {
            walls: &[],
            units: &[c(6, 5)],
        };
        let mut ms = [monster(5, 5)];
        let out = spread(
            CFG,
            &r,
            c(4, 5),
            0,
            &mut ms,
            &defense,
            &crowded,
            &mut cells,
            1,
            0,
        );
        assert_eq!(out.moved, None);
        let mut ms = [monster(9, 9)];
        r.knockback = 1;
        let out = spread(
            CFG,
            &r,
            c(8, 8),
            0,
            &mut ms,
            &defense,
            &OPEN,
            &mut cells,
            1,
            0,
        );
        assert_eq!(out.moved, None);
    }

    #[test]
    fn cells_around_the_target_burn_and_set_on_fire_whoever_steps_in() {
        let r = HitResult {
            knockback: 1,
            ignite_power: 4,
            ..HitResult::default()
        };
        let walled = Map {
            walls: &[c(7, 4)],
            units: &[],
        };
        let defense = [MonsterDefense::NONE; 1];
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        let mut ms = [monster(5, 5)];
        let out = spread(
            CFG,
            &r,
            c(4, 5),
            0,
            &mut ms,
            &defense,
            &walled,
            &mut cells,
            3,
            1,
        );
        // Pushed to (6,5); its 8 neighbours but the wall burn.
        assert_eq!(out.ignited_count, 7);
        assert!(!out.ignited().contains(&c(7, 4)));
        let until = 3 + CFG.combat.burning_cell_rounds as u16;
        assert!(cells.iter().filter(|c| c.burns(until)).count() == 7);
        assert!(cells.iter().all(|c| !c.burns(until + 1)));
        let mut states = [ActiveState::default(); deadroll_types::STATES_MAX];
        let (s, applied) = step_into(&cells, c(5, 5), 4, &mut states, 0, CFG).unwrap();
        assert_eq!((s.power, s.source, applied), (4, 1, Applied::Added));
        assert!(step_into(&cells, c(6, 5), 4, &mut states, 0, CFG).is_none());
        assert!(step_into(&cells, c(5, 5), until + 1, &mut states, 0, CFG).is_none());
        let immune = 1 << StateKind::Burning as u8;
        assert!(step_into(&cells, c(5, 5), 4, &mut states, immune, CFG).is_none());
        // A stronger fire on a burning cell keeps one slot; a full set refuses new cells.
        assert!(ignite(&mut cells, c(5, 5), until + 2, 9, 2, 4));
        assert_eq!(cells.iter().filter(|c| c.burns(4)).count(), 7);
        for i in 0..9 {
            ignite(&mut cells, c(i, 9), until, 1, 0, 4);
        }
        assert!(!ignite(&mut cells, c(9, 0), until, 1, 0, 4));
    }
}
