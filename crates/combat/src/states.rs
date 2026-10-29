//! States on a unit (docs/COMBAT_MATH.md, section 7; docs/DAMAGE_TYPES_AND_MIXES.md, section 5).
//!
//! P0: Burning, Chill -> Freeze, Bleeding. One instance per kind: a new application keeps
//! the stronger power and refreshes the duration to the longer one.
//!
//! `ActiveState.power` means: damage per target turn for Burning and Bleeding,
//! buildup in bp for Chill, vulnerability bp for Shock, armor counterweight bp for Penetration,
//! unused (0) for Freeze and Stun.

use deadroll_types::{ActiveState, Aspect, Form, Source, StateKind, BP, STATES_MAX};

use crate::balance::Balance;

/// A unit's state slots.
pub type States = [ActiveState; STATES_MAX];

/// `ActiveState.source` is a turn-order slot (below `TURN_ORDER_MAX`, 28): its low 5 bits. The
/// two top bits are marks that triples put on a state, so a state needs no extra byte.
pub const SOURCE_MASK: u8 = 0x1F;
/// The state waits: no damage and no countdown while its bearer is frozen or stunned (triples,
/// `Delayed`). It starts on the first turn start without them.
pub const MARK_DORMANT: u8 = 0x80;
/// The damage over time grows by `Balance::escalate_step_bp` after every tick (`Escalate`).
pub const MARK_ESCALATE: u8 = 0x40;
const MARKS: u8 = MARK_DORMANT | MARK_ESCALATE;

/// The unit that applied a state: its `source` without the marks. `0xFF` (nobody) stays.
pub const fn source_unit(source: u8) -> u8 {
    if source == u8::MAX {
        source
    } else {
        source & SOURCE_MASK
    }
}

/// `source` with `mark` set (nobody, `0xFF`, takes no mark).
pub const fn marked(source: u8, mark: u8) -> u8 {
    if source == u8::MAX {
        source
    } else {
        source | mark
    }
}

/// Source after a stronger application replaces an older one: the new unit, the marks of both.
const fn merged_source(old: u8, new: u8) -> u8 {
    if new == u8::MAX || old == u8::MAX {
        return new;
    }
    (new & SOURCE_MASK) | ((old | new) & MARKS)
}

/// Bit of a state kind in an immunity mask.
pub const fn state_bit(kind: StateKind) -> u8 {
    1 << kind as u8
}

/// Bit of an aspect in an immunity mask.
pub const fn aspect_bit(aspect: Aspect) -> u8 {
    1 << aspect as u8
}

/// Defense tags (docs/DAMAGE_TYPES_AND_MIXES.md 2 and 9: "immunity to magical"): the power
/// sources, then the forms. Indexes `Defender::tag_resist_bp` and the bits of a tag mask.
pub const TAGS: usize = Source::COUNT + Form::COUNT;

/// Bit of a power source in a tag mask.
pub const fn source_bit(source: Source) -> u8 {
    1 << source as u8
}

/// Bit of a form in a tag mask.
pub const fn form_bit(form: Form) -> u8 {
    1 << (Source::COUNT + form as usize)
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
            old.source = merged_source(old.source, new.source);
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
/// the threshold the target is frozen and the buildup resets. `can_freeze == false` (the target
/// is immune to Freeze, docs/BESTIARY.md 7.4): the buildup stays, nothing freezes.
pub fn apply_chill(
    states: &mut States,
    buildup_bp: u32,
    source: u8,
    can_freeze: bool,
    balance: &Balance,
) -> Applied {
    apply_chill_for(
        states,
        buildup_bp,
        balance.turns(StateKind::Chill),
        source,
        can_freeze,
        balance,
    )
}

/// [`apply_chill`] lasting `turns` (a resonance can lengthen it).
pub fn apply_chill_for(
    states: &mut States,
    buildup_bp: u32,
    turns: u8,
    source: u8,
    can_freeze: bool,
    balance: &Balance,
) -> Applied {
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
    if can_freeze && applied != Applied::None && total >= balance.freeze_threshold_bp {
        return freeze(states, source, balance);
    }
    applied
}

/// Applies Freeze directly (threshold reached or a resonance effect); clears Chill buildup.
/// TODO(post-demo): temporary tenacity against repeated hard control (section 7.4).
pub fn freeze(states: &mut States, source: u8, balance: &Balance) -> Applied {
    freeze_for(states, balance.turns(StateKind::Freeze), source)
}

/// [`freeze`] lasting `turns` (a resonance can lengthen it).
pub fn freeze_for(states: &mut States, turns: u8, source: u8) -> Applied {
    remove(states, StateKind::Chill);
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
    /// Damage over time dealt now (Burning + Bleeding + a thawing Freeze).
    pub dot_damage: u32,
    /// The part of `dot_damage` from a Freeze that thawed (its power).
    pub thaw_damage: u32,
    /// Hard control: the unit skips its action this turn.
    pub skip_action: bool,
    /// Bits (`state_bit`) of the states that ended with this tick.
    pub expired: u8,
}

/// [`tick_turn_start_with`] with the default balance (tests, tools without a config).
pub fn tick_turn_start(states: &mut States) -> Tick {
    tick_turn_start_with(states, &Balance::DEFAULT)
}

/// Start of the target's turn (section 7.5): over-time damage, hard control, then every
/// duration goes down by one and finished states are removed. A Freeze with power deals it as it
/// thaws (DAMAGE_TYPES_AND_MIXES 6.4, Burning Frost).
///
/// Triples: a dormant state ([`MARK_DORMANT`]) does nothing while a Freeze or Stun was on the unit
/// at the start of the tick (it wakes on the first tick without one); an escalating damage over
/// time ([`MARK_ESCALATE`]) grows by `balance.escalate_step_bp` after it hits.
pub fn tick_turn_start_with(states: &mut States, balance: &Balance) -> Tick {
    let mut tick = Tick::default();
    let held = states
        .iter()
        .any(|s| s.turns > 0 && matches!(s.kind, StateKind::Freeze | StateKind::Stun));
    for s in states.iter_mut().filter(|s| s.turns > 0) {
        if s.source != u8::MAX && s.source & MARK_DORMANT != 0 {
            if held {
                continue;
            }
            s.source &= !MARK_DORMANT;
        }
        match s.kind {
            StateKind::Burning | StateKind::Bleeding => {
                tick.dot_damage = tick.dot_damage.saturating_add(s.power);
                if s.source != u8::MAX && s.source & MARK_ESCALATE != 0 {
                    let step = (s.power as u64 * balance.escalate_step_bp as u64 / BP as u64)
                        .clamp(1, u32::MAX as u64) as u32;
                    s.power = s.power.saturating_add(step);
                }
            }
            StateKind::Freeze | StateKind::Stun => tick.skip_action = true,
            StateKind::Chill | StateKind::Shock | StateKind::Penetration => {}
        }
        s.turns -= 1;
        if s.turns == 0 {
            if s.kind == StateKind::Freeze && s.power > 0 {
                tick.thaw_damage = tick.thaw_damage.saturating_add(s.power);
                tick.dot_damage = tick.dot_damage.saturating_add(s.power);
            }
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
    fn bits() {
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
        assert_eq!(apply_chill(&mut s, 5_000, 2, true, &B), Applied::Added);
        assert_eq!(chill_slow_bp(&s, &B), 5_000);
        assert_eq!(apply_chill(&mut s, 5_000, 2, true, &B), Applied::Froze);
        assert!(!has(&s, StateKind::Chill));
        let i = find(&s, StateKind::Freeze).unwrap();
        assert_eq!(s[i].turns, B.turns(StateKind::Freeze));
        assert_eq!(chill_slow_bp(&s, &B), 0);
        // Buildup restarts from zero after a freeze.
        assert_eq!(apply_chill(&mut s, 5_000, 2, true, &B), Applied::Added);
        assert!(has(&s, StateKind::Freeze) && has(&s, StateKind::Chill));
    }

    #[test]
    fn big_single_buildup_freezes_at_once() {
        let mut s = States::default();
        assert_eq!(apply_chill(&mut s, 12_000, 0, true, &B), Applied::Froze);
        // Immune to Freeze: the buildup passes the threshold and stays Chill.
        let mut t = States::default();
        assert_eq!(apply_chill(&mut t, 12_000, 0, false, &B), Applied::Added);
        assert!(has(&t, StateKind::Chill) && !has(&t, StateKind::Freeze));
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
                thaw_damage: 0,
                skip_action: false,
                expired: state_bit(StateKind::Burning)
            }
        );
        assert_eq!(tick_turn_start(&mut s), Tick::default());
        assert!(s.iter().all(|x| x.turns == 0));
    }

    #[test]
    fn a_powered_freeze_deals_its_power_when_it_thaws() {
        let mut s = States::default();
        apply_state(&mut s, st(StateKind::Freeze, 2, 6));
        let t = tick_turn_start(&mut s);
        assert_eq!((t.dot_damage, t.thaw_damage, t.skip_action), (0, 0, true));
        let t = tick_turn_start(&mut s);
        assert_eq!((t.dot_damage, t.thaw_damage), (6, 6));
        assert_eq!(t.expired, state_bit(StateKind::Freeze));
        // A plain Freeze thaws for nothing.
        freeze(&mut s, 0, &B);
        assert_eq!(tick_turn_start(&mut s).thaw_damage, 0);
    }

    #[test]
    fn dot_saturates() {
        let mut s = States::default();
        apply_state(&mut s, st(StateKind::Burning, 1, u32::MAX));
        apply_state(&mut s, st(StateKind::Bleeding, 1, u32::MAX));
        assert_eq!(tick_turn_start(&mut s).dot_damage, u32::MAX);
    }
}
