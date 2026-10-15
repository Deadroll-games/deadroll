//! States on a unit (docs/COMBAT_MATH.md, section 7; docs/DAMAGE_TYPES_AND_MIXES.md, section 5).
//!
//! P0: Burning, Chill -> Freeze, Bleeding. One instance per kind: a new application keeps
//! the stronger power and refreshes the duration to the longer one.
//!
//! `ActiveState.power` means: damage per target turn for Burning and Bleeding,
//! buildup in bp for Chill, vulnerability bp for Shock, armor counterweight bp for Penetration,
//! unused (0) for Freeze and Stun.

use deadroll_types::{ActiveState, Aspect, StateKind, BP, STATES_MAX};

use crate::balance::Balance;

/// A unit's state slots.
pub type States = [ActiveState; STATES_MAX];

/// Native state of an aspect (docs/DAMAGE_TYPES_AND_MIXES.md, sections 3.2 and 5).
pub const fn native_state(aspect: Aspect) -> Option<StateKind> {
    match aspect {
        Aspect::Fire => Some(StateKind::Burning),
        Aspect::Cold => Some(StateKind::Chill),
        Aspect::Lightning => Some(StateKind::Shock),
        Aspect::Blood => Some(StateKind::Bleeding),
        Aspect::Blunt => Some(StateKind::Stun),
        Aspect::Piercing => Some(StateKind::Penetration),
    }
}

/// Bit of a state kind in an immunity mask.
pub const fn state_bit(kind: StateKind) -> u8 {
    1 << kind as u8
}

/// Bit of an aspect in an immunity mask.
pub const fn aspect_bit(aspect: Aspect) -> u8 {
    1 << aspect as u8
}

pub fn find(states: &States, kind: StateKind) -> Option<usize> {
    states.iter().position(|s| s.turns > 0 && s.kind == kind)
}

pub fn has(states: &States, kind: StateKind) -> bool {
    find(states, kind).is_some()
}

pub fn remove(states: &mut States, kind: StateKind) -> bool {
    match find(states, kind) {
        Some(i) => {
            states[i] = ActiveState::default();
            true
        }
        None => false,
    }
}

/// What an application did to the target.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Applied {
    /// Nothing changed (zero duration or no free slot).
    #[default]
    None,
    /// A new instance.
    Added,
    /// The existing instance was kept, made stronger and/or longer.
    Refreshed,
    /// Chill buildup reached the threshold: Chill removed, Freeze applied.
    Froze,
}

/// One instance per kind: the strongest power stays, duration refreshed to the longer one.
pub fn apply_state(states: &mut States, new: ActiveState) -> Applied {
    if new.turns == 0 {
        return Applied::None;
    }
    if let Some(i) = find(states, new.kind) {
        let old = &mut states[i];
        if new.power > old.power {
            old.power = new.power;
            old.source = new.source;
        }
        old.turns = old.turns.max(new.turns);
        return Applied::Refreshed;
    }
    match states.iter().position(|s| s.turns == 0) {
        Some(i) => {
            states[i] = new;
            Applied::Added
        }
        None => Applied::None,
    }
}

/// Adds Chill buildup (docs/COMBAT_MATH.md, section 7.3). Buildup accumulates; when it reaches
/// the threshold the target is frozen and the buildup resets.
pub fn apply_chill(states: &mut States, buildup_bp: u32, source: u8, balance: &Balance) -> Applied {
    let turns = balance.turns(StateKind::Chill);
    if turns == 0 {
        return Applied::None;
    }
    let (total, applied) = match find(states, StateKind::Chill) {
        Some(i) => {
            let s = &mut states[i];
            s.power = s.power.saturating_add(buildup_bp);
            s.turns = s.turns.max(turns);
            s.source = source;
            (s.power, Applied::Refreshed)
        }
        None => {
            let applied = apply_state(
                states,
                ActiveState {
                    kind: StateKind::Chill,
                    turns,
                    power: buildup_bp,
                    source,
                },
            );
            (buildup_bp, applied)
        }
    };
    if applied != Applied::None && total >= balance.freeze_threshold_bp {
        return freeze(states, source, balance);
    }
    applied
}

/// Applies Freeze directly (threshold reached or a resonance effect); clears Chill buildup.
/// TODO(post-demo): temporary tenacity against repeated hard control (section 7.4).
pub fn freeze(states: &mut States, source: u8, balance: &Balance) -> Applied {
    remove(states, StateKind::Chill);
    let turns = balance.turns(StateKind::Freeze);
    match apply_state(
        states,
        ActiveState {
            kind: StateKind::Freeze,
            turns,
            power: 0,
            source,
        },
    ) {
        Applied::None => Applied::None,
        _ => Applied::Froze,
    }
}

/// Result of the start of a unit's turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// Damage over time dealt now (Burning + Bleeding).
    pub dot_damage: u32,
    /// Hard control: the unit skips its action this turn.
    pub skip_action: bool,
    /// Bits (`state_bit`) of the states that ended with this tick.
    pub expired: u8,
}

/// Start of the target's turn (section 7.5): over-time damage, hard control, then every
/// duration goes down by one and finished states are removed.
pub fn tick_turn_start(states: &mut States) -> Tick {
    let mut tick = Tick::default();
    for s in states.iter_mut().filter(|s| s.turns > 0) {
        match s.kind {
            StateKind::Burning | StateKind::Bleeding => {
                tick.dot_damage = tick.dot_damage.saturating_add(s.power);
            }
            StateKind::Freeze | StateKind::Stun => tick.skip_action = true,
            StateKind::Chill | StateKind::Shock | StateKind::Penetration => {}
        }
        s.turns -= 1;
        if s.turns == 0 {
            tick.expired |= state_bit(s.kind);
            *s = ActiveState::default();
        }
    }
    tick
}

/// Power of the live instance of `kind`, or 0.
pub fn power_of(states: &States, kind: StateKind) -> u32 {
    find(states, kind).map(|i| states[i].power).unwrap_or(0)
}

/// Speed counterweight from Chill (soft control, section 7.4), bp.
pub fn chill_slow_bp(states: &States, balance: &Balance) -> u32 {
    find(states, StateKind::Chill)
        .map(|i| {
            (states[i].power as u64 * balance.chill_slow_share_bp as u64 / BP as u64)
                .min(u32::MAX as u64) as u32
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Balance = Balance::DEFAULT;

    fn st(kind: StateKind, turns: u8, power: u32) -> ActiveState {
        ActiveState {
            kind,
            turns,
            power,
            source: 1,
        }
    }

    #[test]
    fn natives() {
        assert_eq!(native_state(Aspect::Fire), Some(StateKind::Burning));
        assert_eq!(native_state(Aspect::Cold), Some(StateKind::Chill));
        assert_eq!(native_state(Aspect::Lightning), Some(StateKind::Shock));
        assert_eq!(native_state(Aspect::Blood), Some(StateKind::Bleeding));
        assert_eq!(native_state(Aspect::Blunt), Some(StateKind::Stun));
        assert_eq!(native_state(Aspect::Piercing), Some(StateKind::Penetration));
        assert_eq!(state_bit(StateKind::Chill), 0b10);
        assert_eq!(aspect_bit(Aspect::Piercing), 0b10_0000);
    }

    #[test]
    fn one_instance_strongest_stays_longest_duration() {
        let mut s = States::default();
        assert_eq!(
            apply_state(&mut s, st(StateKind::Burning, 2, 5)),
            Applied::Added
        );
        assert_eq!(
            apply_state(&mut s, st(StateKind::Burning, 4, 3)),
            Applied::Refreshed
        );
        let i = find(&s, StateKind::Burning).unwrap();
        assert_eq!((s[i].power, s[i].turns), (5, 4));
        let stronger = ActiveState {
            source: 9,
            ..st(StateKind::Burning, 1, 8)
        };
        apply_state(&mut s, stronger);
        assert_eq!((s[i].power, s[i].turns, s[i].source), (8, 4, 9));
        assert_eq!(s.iter().filter(|x| x.turns > 0).count(), 1);
    }

    #[test]
    fn zero_turns_is_ignored() {
        let mut s = States::default();
        assert_eq!(
            apply_state(&mut s, st(StateKind::Bleeding, 0, 5)),
            Applied::None
        );
        assert!(!has(&s, StateKind::Bleeding));
    }

    #[test]
    fn full_slots_reject_new_kind() {
        let mut s = States::default();
        for k in [
            StateKind::Burning,
            StateKind::Chill,
            StateKind::Bleeding,
            StateKind::Shock,
            StateKind::Stun,
        ] {
            apply_state(&mut s, st(k, 1, 1));
        }
        // Six kinds fit six slots; fill the last by hand with a duplicate kind to force "full".
        s[5] = st(StateKind::Shock, 1, 1);
        assert_eq!(
            apply_state(&mut s, st(StateKind::Freeze, 1, 0)),
            Applied::None
        );
    }

    #[test]
    fn chill_builds_up_to_freeze() {
        let mut s = States::default();
        assert_eq!(apply_chill(&mut s, 5_000, 2, &B), Applied::Added);
        assert_eq!(chill_slow_bp(&s, &B), 5_000);
        assert_eq!(apply_chill(&mut s, 5_000, 2, &B), Applied::Froze);
        assert!(!has(&s, StateKind::Chill));
        let i = find(&s, StateKind::Freeze).unwrap();
        assert_eq!(s[i].turns, B.turns(StateKind::Freeze));
        assert_eq!(chill_slow_bp(&s, &B), 0);
        // Buildup restarts from zero after a freeze.
        assert_eq!(apply_chill(&mut s, 5_000, 2, &B), Applied::Added);
        assert!(has(&s, StateKind::Freeze) && has(&s, StateKind::Chill));
    }

    #[test]
    fn big_single_buildup_freezes_at_once() {
        let mut s = States::default();
        assert_eq!(apply_chill(&mut s, 12_000, 0, &B), Applied::Froze);
    }

    #[test]
    fn tick_deals_dot_skips_and_expires() {
        let mut s = States::default();
        apply_state(&mut s, st(StateKind::Burning, 2, 4));
        apply_state(&mut s, st(StateKind::Bleeding, 1, 3));
        freeze(&mut s, 0, &B);
        let t = tick_turn_start(&mut s);
        assert_eq!(t.dot_damage, 7);
        assert!(t.skip_action);
        assert_eq!(
            t.expired,
            state_bit(StateKind::Bleeding) | state_bit(StateKind::Freeze)
        );
        let t = tick_turn_start(&mut s);
        assert_eq!(
            t,
            Tick {
                dot_damage: 4,
                skip_action: false,
                expired: state_bit(StateKind::Burning)
            }
        );
        assert_eq!(tick_turn_start(&mut s), Tick::default());
        assert!(s.iter().all(|x| x.turns == 0));
    }

    #[test]
    fn dot_saturates() {
        let mut s = States::default();
        apply_state(&mut s, st(StateKind::Burning, 1, u32::MAX));
        apply_state(&mut s, st(StateKind::Bleeding, 1, u32::MAX));
        assert_eq!(tick_turn_start(&mut s).dot_damage, u32::MAX);
    }
}
