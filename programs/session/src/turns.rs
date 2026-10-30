//! Turn changes: the first turn after the initiative roll, the advance shared by `end_turn`,
//! `flee` and `start_turn`, the start of a turn, and the escape rule.
//!
//! Arena update (docs/PARTY_AND_TURNS.md 4.1, 6): on the map the players move in a circle (the
//! order of the initiative roll); in an arena every unit is on the initiative timeline
//! (`deadroll_rules::arena`): the living unit with the lowest time acts, its turn adds its delay.
use anchor_lang::prelude::*;
use deadroll_combat::TimedMods;
use deadroll_rules::arena::{
    delay, next_unit, round_at, start_time, turn_delay, FirstSide, TimelineUnit,
};
use deadroll_types::TURN_ORDER_MAX;

/// The start of a unit's own turn lives in `deadroll-rules` (shared with `tools/balance`).
pub use deadroll_rules::turn::{start_of_turn, start_of_turn_with, TurnStart};

use crate::{
    errors::SessionError,
    events::{InitiativeRolled, TurnStarted},
    rules::{monster_ap, roll_initiative, Unit, BALANCE, EMPTY_UNIT},
    state::{ArenaMode, GameSession, Monsters},
    vrf::SyscallRoll,
};

/// Starts the next living unit's turn. Unspent AP are lost: players get fresh AP lazily through
/// `turn_seq`, monsters right here. On the map the next player of the circle (passing the end
/// starts a new round); in an arena the next unit of the timeline, after the current unit's delay
/// (also after a turn skipped by Freeze or Stun, docs/PARTY_AND_TURNS.md 6.2). Between the map
/// and an arena nobody's turn starts: `enter_arena` and `leave_arena` start the next one.
pub fn advance(session: &mut GameSession, session_key: Pubkey, monsters: &Monsters) -> Result<()> {
    match session.mode {
        ArenaMode::Map => advance_circle(session, session_key, monsters),
        ArenaMode::Arena => {
            // `start_turn` added the delay under the turn's buffs and debuffs; a turn passed
            // without it (a timeout) takes the delay of the bare initiative.
            if let Some(unit) = session.current_unit() {
                if !session.turn_started() {
                    let id = unit.id() as usize;
                    let step = delay(initiative_of(session, unit), &BALANCE.arena);
                    session.unit_time[id] = session.unit_time[id].saturating_add(step);
                }
            }
            next_on_timeline(session, session_key, monsters)
        }
        ArenaMode::Engaged | ArenaMode::Won => Ok(()),
    }
}

/// The next living player of the map circle.
fn advance_circle(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
) -> Result<()> {
    let len = session.turn_order_len;
    let mut index = session.core.turn_index;
    for _ in 0..len {
        index = (index + 1) % len;
        if index == 0 {
            session.core.round = session
                .core
                .round
                .checked_add(1)
                .ok_or(SessionError::Overflow)?;
        }
        let alive = match Unit::from_id(session.core.turn_order[index as usize]) {
            Some(Unit::Player(slot)) => session.player_alive(slot),
            Some(Unit::Monster(i)) => monsters.get(i)?.alive,
            None => false,
        };
        if alive {
            break;
        }
    }
    session.core.turn_index = index;
    start_unit(session, session_key, monsters)
}

/// Starts the turn of `turn_order[turn_index]`: a new `turn_seq`, the monster's AP.
fn start_unit(session: &mut GameSession, session_key: Pubkey, monsters: &Monsters) -> Result<()> {
    let index = session.core.turn_index;
    session.turn_seq = session
        .turn_seq
        .checked_add(1)
        .ok_or(SessionError::Overflow)?;
    session.turn_started_at = Clock::get()?.unix_timestamp;

    let unit = session.core.turn_order[index as usize];
    if let Some(Unit::Monster(i)) = Unit::from_id(unit) {
        let mut monster = monsters.get(i)?;
        monster.action_points = monster_ap(session.monster_speed[i as usize]);
        monsters.set(i, &monster)?;
    }

    emit!(TurnStarted {
        session: session_key,
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}

/// Writes the turn order rolled from the initiative VRF value (`start_session`'s request) and
/// starts the first turn. Players by slot, then living monsters by index, roll in that order.
/// Arena update: the party starts on the map, where no monster is live: the order is the
/// players' circle.
pub fn begin(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    seed: [u8; 32],
) -> Result<()> {
    let mut units = [(0u8, 0u16); TURN_ORDER_MAX];
    let mut len = 0usize;
    for slot in 0..session.core.player_count {
        units[len] = (
            Unit::Player(slot).id(),
            session.player_initiative[slot as usize],
        );
        len += 1;
    }
    for index in 0..monsters.count()? {
        if monsters.get(index)?.alive {
            units[len] = (
                Unit::Monster(index).id(),
                session.monster_initiative[index as usize],
            );
            len += 1;
        }
    }
    let rolled = roll_initiative(&units[..len], &mut SyscallRoll::new(seed));
    session.core.turn_order = rolled.order;
    session.turn_order_len = rolled.len;
    session.core.turn_index = 0;
    session.core.round = 1;
    session.turn_seq = 1;
    session.turn_started_at = Clock::get()?.unix_timestamp;

    emit!(InitiativeRolled {
        session: session_key,
        seed,
        action_counter: session.core.action_counter,
        len: rolled.len,
        order: rolled.order,
        d20: rolled.d20,
        initiative: rolled.initiative,
    });

    // Players get AP lazily on their first action; a monster that goes first gets them here.
    let unit = session.core.turn_order[0];
    if let Some(Unit::Monster(i)) = Unit::from_id(unit) {
        let mut monster = monsters.get(i)?;
        monster.action_points = monster_ap(session.monster_speed[i as usize]);
        monsters.set(i, &monster)?;
    }
    emit!(TurnStarted {
        session: session_key,
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}

/// Patch B: in an arena, the start of `unit`'s turn puts its next turn on the timeline: its time
/// grows by the delay under `mods` (its own with the auras over it, after the turn start's tick).
/// An effect that lands later in the turn changes the delay from the next turn on, and a turn
/// lost to Freeze or Stun takes the delay as well (docs/PARTY_AND_TURNS.md 6.2).
pub fn add_turn_delay(session: &mut GameSession, unit: Unit, mods: &TimedMods) {
    if session.mode != ArenaMode::Arena {
        return;
    }
    let id = unit.id() as usize;
    let step = turn_delay(initiative_of(session, unit), mods, &BALANCE.arena);
    if let Some(t) = session.unit_time.get_mut(id) {
        *t = t.saturating_add(step);
    }
}

/// Initiative of a unit (its delay on the timeline).
fn initiative_of(session: &GameSession, unit: Unit) -> u16 {
    match unit {
        Unit::Player(s) => session.player_initiative[s as usize],
        Unit::Monster(i) => session.monster_initiative[i as usize],
    }
}

/// What wins a tie on the timeline: a player's Dexterity, a monster's initiative (monsters have
/// no Dexterity).
fn tie_of(session: &GameSession, unit: Unit) -> u16 {
    match unit {
        Unit::Player(s) => session.player_dex[s as usize],
        Unit::Monster(i) => session.monster_initiative[i as usize],
    }
}

/// Starts the turn of the living unit with the lowest time (`deadroll_rules::arena::next_unit`);
/// the round is the one its time falls in. Nothing when nobody lives (the party ended).
fn next_on_timeline(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
) -> Result<()> {
    let len = (session.turn_order_len as usize).min(TURN_ORDER_MAX);
    let mut units = [TimelineUnit::default(); TURN_ORDER_MAX];
    for (i, u) in units.iter_mut().enumerate().take(len) {
        let id = session.core.turn_order[i];
        let Some(unit) = Unit::from_id(id) else {
            continue;
        };
        let alive = match unit {
            Unit::Player(s) => session.player_alive(s),
            Unit::Monster(m) => monsters.get(m)?.alive,
        };
        *u = TimelineUnit {
            id,
            time: session.unit_time[id as usize],
            tie: tie_of(session, unit),
            alive,
        };
    }
    let Some(k) = next_unit(&units[..len]) else {
        return Ok(());
    };
    session.core.turn_index = k as u8;
    session.core.round = round_at(units[k].time, &BALANCE.arena);
    start_unit(session, session_key, monsters)
}

/// Arena update: the timeline of a new arena (docs/PARTY_AND_TURNS.md 4.1.3, 6): every player
/// by slot, then the arena's monsters by slot; `first`'s units start at 0, the others at their
/// own delay. Starts the first turn.
pub fn enter_timeline(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    first: FirstSide,
) -> Result<()> {
    session.core.turn_order = [EMPTY_UNIT; TURN_ORDER_MAX];
    let mut len = 0usize;
    let players = (0..session.core.player_count).map(Unit::Player);
    let pack = (0..monsters.count()?).map(Unit::Monster);
    for unit in players.chain(pack) {
        let party = matches!(unit, Unit::Player(_));
        let leads = party == (first == FirstSide::Party);
        session.unit_time[unit.id() as usize] =
            start_time(leads, initiative_of(session, unit), &BALANCE.arena);
        session.core.turn_order[len] = unit.id();
        len += 1;
    }
    session.turn_order_len = len as u8;
    next_on_timeline(session, session_key, monsters)
}

/// The party flees when every living player has the escape intent (docs/PARTY_AND_TURNS.md,
/// section 7.3: the dead do not vote, unanimity is the check).
pub fn everyone_flees(flee_mask: u8, alive_mask: u8) -> bool {
    alive_mask != 0 && flee_mask & alive_mask == alive_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::rules::EMPTY_UNIT;
    use deadroll_combat::{
        mod_turns, states::state_bit, support::mod_bit, ModKind, States, TimedMod, TimedMods,
    };
    use deadroll_types::StateKind;
    use deadroll_types::{ActiveState, Aspect, STATES_MAX};

    const NONE: TimedMods = [TimedMod {
        kind: ModKind::Haste,
        aspect: Aspect::Fire,
        turns: 0,
        power_bp: 0,
        source: 0,
        weights_pct: [0; deadroll_types::Aspect::COUNT],
    }; deadroll_combat::MODS_MAX];

    fn with(states: &[(StateKind, u8, u32, u8)]) -> States {
        let mut out = [ActiveState::default(); STATES_MAX];
        for (i, &(kind, turns, power, source)) in states.iter().enumerate() {
            out[i] = ActiveState {
                kind,
                turns,
                power,
                source,
            };
        }
        out
    }

    #[test]
    fn quiet_turn_counts_cooldowns_only() {
        let mut states = with(&[]);
        let mut cooldowns = [0, 1, 3];
        let start = start_of_turn(&mut states, &mut NONE.clone(), &mut cooldowns, 10);
        assert_eq!(
            start,
            TurnStart {
                dot_damage: 0,
                thaw_damage: 0,
                hp: 10,
                died: false,
                skip: None,
                expired: 0,
                expired_mods: 0,
                dot_source: EMPTY_UNIT,
            }
        );
        assert_eq!(cooldowns, [0, 0, 2]);
    }

    #[test]
    fn burning_and_bleeding_tick_and_expire() {
        let mut states = with(&[
            (StateKind::Burning, 2, 3, 4),
            (StateKind::Bleeding, 1, 5, 5),
            (StateKind::Chill, 1, 2_000, 4),
        ]);
        let start = start_of_turn(&mut states, &mut NONE.clone(), &mut [], 20);
        assert_eq!((start.dot_damage, start.hp, start.died), (8, 12, false));
        // The strongest over-time state names its source.
        assert_eq!(start.dot_source, 5);
        assert_eq!(start.skip, None);
        assert_eq!(
            start.expired,
            state_bit(StateKind::Bleeding) | state_bit(StateKind::Chill)
        );
        assert_eq!((states[0].kind, states[0].turns), (StateKind::Burning, 1));
        assert!(states[1].is_empty() && states[2].is_empty());

        let next = start_of_turn(&mut states, &mut NONE.clone(), &mut [], start.hp);
        assert_eq!((next.dot_damage, next.hp), (3, 9));
        assert_eq!(next.expired, state_bit(StateKind::Burning));
        assert!(states.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn dot_kills_at_zero_hp_and_death_wins_over_a_skip() {
        let mut states = with(&[(StateKind::Bleeding, 3, 7, 6), (StateKind::Freeze, 2, 0, 6)]);
        let start = start_of_turn(&mut states, &mut NONE.clone(), &mut [], 7);
        assert_eq!((start.hp, start.died, start.skip), (0, true, None));
        assert_eq!(start.dot_source, 6);

        let mut states = with(&[(StateKind::Burning, 1, 100, 4)]);
        let start = start_of_turn(&mut states, &mut NONE.clone(), &mut [], 3);
        assert_eq!((start.dot_damage, start.hp, start.died), (100, 0, true));
    }

    #[test]
    fn freeze_and_stun_skip_each_of_their_turns() {
        let mut states = with(&[(StateKind::Freeze, 2, 0, 4)]);
        let mut cooldowns = [2];
        let first = start_of_turn(&mut states, &mut NONE.clone(), &mut cooldowns, 5);
        assert_eq!((first.skip, first.expired), (Some(StateKind::Freeze), 0));
        // A skipped turn still counts down cooldowns.
        assert_eq!(cooldowns, [1]);
        let second = start_of_turn(&mut states, &mut NONE.clone(), &mut cooldowns, 5);
        assert_eq!(
            (second.skip, second.expired),
            (Some(StateKind::Freeze), state_bit(StateKind::Freeze))
        );
        assert_eq!(
            start_of_turn(&mut states, &mut NONE.clone(), &mut cooldowns, 5).skip,
            None
        );

        let mut stunned = with(&[(StateKind::Stun, 1, 0, 0), (StateKind::Burning, 1, 2, 0)]);
        let start = start_of_turn(&mut stunned, &mut NONE.clone(), &mut [], 5);
        assert_eq!((start.skip, start.hp), (Some(StateKind::Stun), 3));
        let mut both = with(&[(StateKind::Stun, 1, 0, 0), (StateKind::Freeze, 1, 0, 0)]);
        assert_eq!(
            start_of_turn(&mut both, &mut NONE.clone(), &mut [], 5).skip,
            Some(StateKind::Freeze)
        );
    }

    #[test]
    fn buffs_and_debuffs_count_down_with_the_turn() {
        let mut mods = NONE;
        mods[0] = TimedMod {
            kind: ModKind::Haste,
            aspect: Aspect::Blood,
            turns: mod_turns(1),
            power_bp: 5_000,
            source: 1,
            weights_pct: [0; deadroll_types::Aspect::COUNT],
        };
        mods[1] = TimedMod {
            kind: ModKind::Slow,
            turns: mod_turns(2),
            ..mods[0]
        };
        let mut states = with(&[]);
        let first = start_of_turn(&mut states, &mut mods, &mut [], 10);
        // Both still cover this turn.
        assert_eq!(first.expired_mods, 0);
        assert_eq!((mods[0].turns, mods[1].turns), (1, 2));
        let second = start_of_turn(&mut states, &mut mods, &mut [], 10);
        assert_eq!(second.expired_mods, mod_bit(ModKind::Haste));
        assert!(mods[0].is_empty() && mods[1].turns == 1);
        let third = start_of_turn(&mut states, &mut mods, &mut [], 10);
        assert_eq!(third.expired_mods, mod_bit(ModKind::Slow));
        assert!(mods.iter().all(|m| m.is_empty()));
    }

    #[test]
    fn the_turn_start_puts_the_next_turn_on_the_timeline_under_its_mods() {
        use crate::state::test_support::session;
        use deadroll_rules::arena::{delay, turn_delay};
        use deadroll_types::Cell;
        let mut s = session(&[Cell::default(), Cell::default()]);
        s.player_initiative = [10, 4, 0, 0];
        let mut slow = NONE;
        slow[0] = TimedMod {
            kind: ModKind::Slow,
            turns: 1,
            power_bp: 10_000,
            ..NONE[0]
        };
        // On the map nothing moves.
        s.mode = ArenaMode::Map;
        add_turn_delay(&mut s, Unit::Player(0), &slow);
        assert_eq!(s.unit_time[0], 0);
        s.mode = ArenaMode::Arena;
        add_turn_delay(&mut s, Unit::Player(0), &NONE);
        assert_eq!(s.unit_time[0], delay(10, &BALANCE.arena));
        // Slowed 100%: initiative 5's delay.
        add_turn_delay(&mut s, Unit::Player(1), &slow);
        assert_eq!(s.unit_time[1], turn_delay(4, &slow, &BALANCE.arena));
        assert_eq!(s.unit_time[1], delay(2, &BALANCE.arena));
        // A Haste leaves the order alone.
        let mut haste = slow;
        haste[0].kind = ModKind::Haste;
        let before = s.unit_time[0];
        add_turn_delay(&mut s, Unit::Player(0), &haste);
        assert_eq!(s.unit_time[0], before + delay(10, &BALANCE.arena));
    }

    #[test]
    fn flee_needs_every_living_player() {
        assert!(!everyone_flees(0b0001, 0b0011));
        assert!(everyone_flees(0b0011, 0b0011));
        // The tank died: the other three decide.
        assert!(everyone_flees(0b1110, 0b1110));
        assert!(everyone_flees(0b1111, 0b1110));
        assert!(!everyone_flees(0b0110, 0b1110));
        assert!(!everyone_flees(0, 0));
    }
}
