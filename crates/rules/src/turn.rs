//! The start of a unit's own turn.
use deadroll_combat::{
    source_unit,
    states::{has, tick_turn_start_with, States, MARK_DORMANT},
    tick_mods, Balance, TimedMods,
};
use deadroll_types::StateKind;

use crate::{input::take_damage, units::tick_cooldowns, units::EMPTY_UNIT};

/// What the start of a unit's own turn did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnStart {
    /// Burning + Bleeding damage dealt now, with a Freeze that thawed (Burning Frost).
    pub dot_damage: u32,
    /// The part of `dot_damage` from the thawed Freeze.
    pub thaw_damage: u32,
    /// HP after it.
    pub hp: u32,
    /// The over-time damage killed the unit.
    pub died: bool,
    /// Hard control that skips this turn (Freeze before Stun); `None` when the unit died.
    pub skip: Option<StateKind>,
    /// `state_bit`s of the states that ended now.
    pub expired: u8,
    /// `deadroll_combat::support::mod_bit`s of the buffs and debuffs that ended now.
    pub expired_mods: u16,
    /// Unit that applied the strongest over-time state (the killer on a death), `EMPTY_UNIT` if none.
    pub dot_source: u8,
}

/// [`start_of_turn_with`] with the default balance (tests and tools without a config).
pub fn start_of_turn(
    states: &mut States,
    mods: &mut TimedMods,
    cooldowns: &mut [u8],
    hp: u32,
) -> TurnStart {
    start_of_turn_with(states, mods, cooldowns, hp, &Balance::DEFAULT)
}

/// The start of a unit's own turn (docs/COMBAT_MATH.md, 7.5; SKILL_BUILDER.md, 8): over-time
/// damage, hard control, every state, buff and debuff duration and every cooldown down by one.
/// `balance` gives the growth of an escalating damage over time.
pub fn start_of_turn_with(
    states: &mut States,
    mods: &mut TimedMods,
    cooldowns: &mut [u8],
    hp: u32,
    balance: &Balance,
) -> TurnStart {
    // A dormant state (a triple's delayed one) deals nothing while its bearer is held, so it is
    // never the killer of this tick.
    let held = states
        .iter()
        .any(|s| s.turns > 0 && matches!(s.kind, StateKind::Freeze | StateKind::Stun));
    let dot_source = states
        .iter()
        .filter(|s| !(held && s.source != u8::MAX && s.source & MARK_DORMANT != 0))
        .filter(|s| match s.kind {
            StateKind::Burning | StateKind::Bleeding => s.turns > 0,
            // A Freeze that thaws now and deals damage (DAMAGE_TYPES_AND_MIXES 6.4).
            StateKind::Freeze => s.turns == 1 && s.power > 0,
            _ => false,
        })
        .max_by_key(|s| s.power)
        .map_or(EMPTY_UNIT, |s| source_unit(s.source));
    let hard = [StateKind::Freeze, StateKind::Stun]
        .into_iter()
        .find(|&kind| has(states, kind));
    let tick = tick_turn_start_with(states, balance);
    let expired_mods = tick_mods(mods);
    tick_cooldowns(cooldowns);
    let (hp, died) = take_damage(hp, tick.dot_damage);
    TurnStart {
        dot_damage: tick.dot_damage,
        thaw_damage: tick.thaw_damage,
        hp,
        died,
        skip: if died { None } else { hard },
        expired: tick.expired,
        expired_mods,
        dot_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::states::{MARK_DORMANT, MARK_ESCALATE};
    use deadroll_types::ActiveState;

    fn state(kind: StateKind, turns: u8, power: u32, source: u8) -> ActiveState {
        ActiveState {
            kind,
            turns,
            power,
            source,
        }
    }

    #[test]
    fn an_escalating_burning_grows_every_turn_and_names_its_source() {
        let balance = Balance::DEFAULT;
        let mut states = States::default();
        states[0] = state(StateKind::Burning, 3, 10, 2 | MARK_ESCALATE);
        let mut mods = TimedMods::default();
        let first = start_of_turn_with(&mut states, &mut mods, &mut [], 100, &balance);
        let second = start_of_turn_with(&mut states, &mut mods, &mut [], first.hp, &balance);
        assert_eq!(first.dot_damage, 10);
        assert_eq!(
            second.dot_damage,
            10 + 10 * balance.escalate_step_bp / 10_000
        );
        // The killer is the unit, not the unit with the mark.
        assert_eq!((first.dot_source, second.dot_source), (2, 2));
        // The default-balance entry point agrees.
        let mut states = States::default();
        states[0] = state(StateKind::Burning, 2, 10, 2 | MARK_ESCALATE);
        let plain = start_of_turn(&mut states, &mut mods, &mut [], 100);
        assert_eq!(plain.dot_damage, 10);
    }

    #[test]
    fn a_dormant_bleeding_waits_for_the_stun_and_is_never_the_killer_meanwhile() {
        let balance = Balance::DEFAULT;
        let mut states = States::default();
        states[0] = state(StateKind::Stun, 1, 0, 1);
        states[1] = state(StateKind::Bleeding, 3, 7, 4 | MARK_DORMANT);
        states[2] = state(StateKind::Burning, 3, 1, 5);
        let mut mods = TimedMods::default();
        let held = start_of_turn_with(&mut states, &mut mods, &mut [], 100, &balance);
        // Only the Burning ticks; the dormant Bleeding did not count down.
        assert_eq!((held.dot_damage, held.dot_source), (1, 5));
        assert!(deadroll_combat::states::has(&states, StateKind::Bleeding));
        assert_eq!(states[1].turns, 3);
        let free = start_of_turn_with(&mut states, &mut mods, &mut [], held.hp, &balance);
        assert_eq!(free.dot_damage, 8);
        assert_eq!(free.dot_source, 4);
        assert_eq!(states[1].source & MARK_DORMANT, 0);
    }
}
