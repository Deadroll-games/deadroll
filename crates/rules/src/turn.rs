//! The start of a unit's own turn.
use deadroll_combat::{
    states::{has, tick_turn_start, States},
    tick_mods, TimedMods,
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
        .filter(|s| match s.kind {
            StateKind::Burning | StateKind::Bleeding => s.turns > 0,
            // A Freeze that thaws now and deals damage (DAMAGE_TYPES_AND_MIXES 6.4).
            StateKind::Freeze => s.turns == 1 && s.power > 0,
            _ => false,
        })
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
        thaw_damage: tick.thaw_damage,
        hp,
        died,
        skip: if died { None } else { hard },
        expired: tick.expired,
        expired_mods,
        dot_source,
    }
}
