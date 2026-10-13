//! Turn changes: the first turn after the initiative roll, the advance shared by `end_turn`,
//! `flee` and `start_turn`, the start of a turn, and the escape rule.
use anchor_lang::prelude::*;
use deadroll_combat::{
    states::{has, tick_turn_start, States},
    tick_mods, TimedMods,
};
use deadroll_types::{StateKind, TURN_ORDER_MAX};

use crate::{
    combat_input::take_damage,
    errors::SessionError,
    events::{InitiativeRolled, TurnStarted},
    rules::{monster_ap, roll_initiative, tick_cooldowns, Unit, EMPTY_UNIT},
    state::{GameSession, Monsters},
    vrf::SyscallRoll,
};

/// Starts the next living unit's turn; passing the end of the order starts a new round. Unspent
/// AP are lost: players get fresh AP lazily through `turn_seq`, monsters right here.
pub fn advance(session: &mut GameSession, session_key: Pubkey, monsters: &Monsters) -> Result<()> {
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

/// What the start of a unit's own turn did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnStart {
    /// Burning + Bleeding damage dealt now.
    pub dot_damage: u32,
    /// HP after it.
    pub hp: u32,
    /// The over-time damage killed the unit.
    pub died: bool,
    /// Hard control that skips this turn (Freeze before Stun); `None` when the unit died.
    pub skip: Option<StateKind>,
    /// `state_bit`s of the states that ended now.
    pub expired: u8,
    /// `deadroll_combat::support::mod_bit`s of the buffs and debuffs that ended now.
    pub expired_mods: u8,
    /// Unit that applied the strongest over-time state (the killer on a death), `EMPTY_UNIT` if none.
    pub dot_source: u8,
}

/// The start of a unit's own turn (docs/COMBAT_MATH.md, 7.5; SKILL_BUILDER.md, 8): over-time
/// damage, hard control, every state, buff and debuff duration and every cooldown down by one.
pub fn start_of_turn(
    states: &mut States,
    mods: &mut TimedMods,
    cooldowns: &mut [u8],
    hp: u32,
) -> TurnStart {
    let dot_source = states
        .iter()
        .filter(|s| s.turns > 0 && matches!(s.kind, StateKind::Burning | StateKind::Bleeding))
        .max_by_key(|s| s.power)
        .map_or(EMPTY_UNIT, |s| s.source);
    let hard = [StateKind::Freeze, StateKind::Stun]
        .into_iter()
        .find(|&kind| has(states, kind));
    let tick = tick_turn_start(states);
    let expired_mods = tick_mods(mods);
    tick_cooldowns(cooldowns);
    let (hp, died) = take_damage(hp, tick.dot_damage);
    TurnStart {
        dot_damage: tick.dot_damage,
        hp,
        died,
        skip: if died { None } else { hard },
        expired: tick.expired,
        expired_mods,
        dot_source,
    }
}

/// The party flees when every living player has the escape intent (docs/PARTY_AND_TURNS.md,
/// section 7.3: the dead do not vote, unanimity is the check).
pub fn everyone_flees(flee_mask: u8, alive_mask: u8) -> bool {
    alive_mask != 0 && flee_mask & alive_mask == alive_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    use deadroll_combat::{mod_turns, states::state_bit, support::mod_bit, ModKind, TimedMod};
    use deadroll_types::{ActiveState, Aspect, STATES_MAX};

    const NONE: TimedMods = [TimedMod {
        kind: ModKind::Haste,
        aspect: Aspect::Fire,
        turns: 0,
        power_bp: 0,
        source: 0,
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
