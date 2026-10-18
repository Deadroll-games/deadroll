//! Unit ids, the initiative roll, cooldowns and reach.
use deadroll_combat::{d20, RollSource};
use deadroll_types::{Cell, Form, RangeTag, PARTY_MAX, TURN_ORDER_MAX};

use crate::config::PlayerTuning;

/// Unused `turn_order` entry.
pub const EMPTY_UNIT: u8 = 0xFF;
/// `turn_order` ids `0..PARTY_MAX` are player slots, `PARTY_MAX + i` is monster `i`.
pub const MONSTER_UNIT_BASE: u8 = PARTY_MAX as u8;

/// A participant in the turn order. The id is also `ActiveState.source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Player(u8),
    Monster(u8),
}

impl Unit {
    pub fn from_id(id: u8) -> Option<Unit> {
        match id {
            EMPTY_UNIT => None,
            id if id < MONSTER_UNIT_BASE => Some(Unit::Player(id)),
            id => Some(Unit::Monster(id - MONSTER_UNIT_BASE)),
        }
    }

    pub fn id(self) -> u8 {
        match self {
            Unit::Player(slot) => slot,
            Unit::Monster(index) => MONSTER_UNIT_BASE + index,
        }
    }
}

/// The turn order rolled from one VRF value (docs/PARTY_AND_TURNS.md, 6; the timeline is P1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Initiative {
    pub len: u8,
    /// Turn-order ids, first to act first; `EMPTY_UNIT` after `len`.
    pub order: [u8; TURN_ORDER_MAX],
    pub d20: [u8; TURN_ORDER_MAX],
    pub initiative: [u16; TURN_ORDER_MAX],
}

/// Every unit in `units` (players by slot, then monsters by index) rolls a d20 in that order
/// (roll `i` of the seed's stream); the higher `d20 + initiative` acts first, ties go to the
/// higher initiative, then to the lower unit id.
pub fn roll_initiative(units: &[(u8, u16)], src: &mut impl RollSource) -> Initiative {
    let mut out = Initiative {
        len: units.len().min(TURN_ORDER_MAX) as u8,
        order: [EMPTY_UNIT; TURN_ORDER_MAX],
        d20: [0; TURN_ORDER_MAX],
        initiative: [0; TURN_ORDER_MAX],
    };
    let len = out.len as usize;
    for (i, &(unit, initiative)) in units[..len].iter().enumerate() {
        out.order[i] = unit;
        out.d20[i] = d20(src);
        out.initiative[i] = initiative;
    }
    // Insertion sort: at most 28 units, stable, no allocation.
    let key = |o: &Initiative, i: usize| {
        (
            o.d20[i] as u32 + o.initiative[i] as u32,
            o.initiative[i],
            u8::MAX - o.order[i],
        )
    };
    for i in 1..len {
        let mut j = i;
        while j > 0 && key(&out, j) > key(&out, j - 1) {
            out.order.swap(j, j - 1);
            out.d20.swap(j, j - 1);
            out.initiative.swap(j, j - 1);
            j -= 1;
        }
    }
    out
}

/// Cooldown value stored when a skill with `cooldown` (own turns, `SkillRecord.cooldown`) is
/// used: own turn starts until it is usable again. Each start counts it down by one, so the
/// skill misses exactly its next `cooldown` own turns (docs/SKILL_BUILDER.md, section 8).
pub fn cooldown_after_use(cooldown: u8) -> u8 {
    match cooldown {
        0 => 0,
        c => c.saturating_add(1),
    }
}

/// Start of the unit's own turn: every cooldown goes down by one.
pub fn tick_cooldowns(cooldowns: &mut [u8]) {
    for c in cooldowns {
        *c = c.saturating_sub(1);
    }
}

/// Whether `range` reaches from `from` to `to` (Chebyshev distance, never the own cell).
pub fn in_range(range: RangeTag, from: Cell, to: Cell, t: &PlayerTuning) -> bool {
    let distance = chebyshev(from, to);
    distance >= 1 && distance <= reach(range, t)
}

/// Whether a buff of `range` reaches an ally: the same, but allies may share a cell (the whole
/// party stands on the entrance when it joins).
pub fn in_ally_range(range: RangeTag, from: Cell, to: Cell, t: &PlayerTuning) -> bool {
    chebyshev(from, to) <= reach(range, t)
}

pub fn chebyshev(from: Cell, to: Cell) -> u8 {
    from.x.abs_diff(to.x).max(from.y.abs_diff(to.y))
}

/// Reach of `range` in cells (docs/DAMAGE_TYPES_AND_MIXES.md, section 2: the range axis).
pub fn reach(range: RangeTag, t: &PlayerTuning) -> u8 {
    match range {
        RangeTag::Melee => t.melee_range,
        RangeTag::Ranged => t.ranged_range,
    }
}

/// Range of a monster's natural attack. The bestiary gives it a form, not a range
/// (docs/BESTIARY.md 2): a bite or a blade strikes in melee, an orb flies (a projectile).
pub const fn form_range(form: Form) -> RangeTag {
    match form {
        Form::Strike => RangeTag::Melee,
        Form::Projectile => RangeTag::Ranged,
    }
}
