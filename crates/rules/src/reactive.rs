//! Reactive skills (docs/SKILL_BUILDER.md, section 3.2): a skill that triggers by itself when its
//! condition is met, then waits out its cooldown.
//!
//! - Conditions: the bearer was hit; hit by a crit; its HP dropped below the skill's threshold; a
//!   nearby ally's HP dropped below it (`PlayerTuning::nearby_ally_cells`); it was frozen or
//!   stunned; an enemy entered a cell next to it.
//! - "Dropped below": the HP was at or above the threshold before this event and is below it
//!   after, and the unit lives.
//! - A reactive is not an action: it costs no action points and does not use up the turn's one
//!   attack or active skill (docs/PARTY_AND_TURNS.md 5.1). It triggers on frozen or stunned
//!   bearers too (hard control stops actions and movement).
//! - What it does: damage (a counterattack, a hit like an active skill's, a line runs on through
//!   the enemy) or a debuff lands on the enemy that caused the trigger; a buff lands on the
//!   bearer, or on the ally whose HP dropped; an aura starts around the bearer (a temporary
//!   aura, section 3.5). Damage, a debuff and a buff on an ally need the skill's reach: an enemy
//!   or ally out of reach does not trigger it and the cooldown is not spent.
//! - Several bearers react to one event in player slot order. Their rolls follow the rolls of
//!   the action that caused it, on the same VRF value.
//! - Reactive damage is an attack: it uses the attacking resonances. A reactive buff or aura has
//!   no resonance yet (docs/DAMAGE_TYPES_AND_MIXES.md 6.2).
use deadroll_combat::{states::state_bit, HitResult, Outcome};
use deadroll_skillcheck::{Category, Purpose, Trigger};
use deadroll_types::{Cell, StateKind};

use crate::{
    config::BalanceConfig,
    input::SkillStats,
    units::{chebyshev, in_ally_range, in_range},
};

/// What happened, as a reactive trigger sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// An enemy's attack hit the bearer (`crit`: with a crit).
    Hit { crit: bool },
    /// The bearer's HP went from `before` to `after` of `hp_max`.
    HpDrop {
        before: u32,
        after: u32,
        hp_max: u32,
    },
    /// The HP of the bearer's ally in `ally` went from `before` to `after` of `hp_max`.
    AllyHpDrop {
        ally: u8,
        before: u32,
        after: u32,
        hp_max: u32,
    },
    /// Freeze or Stun landed on the bearer.
    Controlled,
    /// An enemy entered a cell next to the bearer.
    EnemyEntered,
}

/// The HP went below `pct` percent of `hp_max` now, and the unit lives.
pub fn dropped_below(before: u32, after: u32, hp_max: u32, pct: u8) -> bool {
    let line = hp_max as u64 * pct as u64;
    after > 0 && before as u64 * 100 >= line && (after as u64) * 100 < line
}

/// Whether `skill`'s condition is met by `event`.
pub fn fires(skill: &SkillStats, event: &Event) -> bool {
    if skill.blocks.category != Category::Reactive {
        return false;
    }
    let pct = skill.ext.hp_below_pct;
    match skill.blocks.trigger {
        Trigger::None => false,
        Trigger::Hit => matches!(event, Event::Hit { .. }),
        Trigger::CritHit => matches!(event, Event::Hit { crit: true }),
        Trigger::HpBelow => match *event {
            Event::HpDrop {
                before,
                after,
                hp_max,
            } => dropped_below(before, after, hp_max, pct),
            _ => false,
        },
        Trigger::AllyHpBelow => match *event {
            Event::AllyHpDrop {
                before,
                after,
                hp_max,
                ..
            } => dropped_below(before, after, hp_max, pct),
            _ => false,
        },
        Trigger::Controlled => matches!(event, Event::Controlled),
        Trigger::EnemyAdjacent => matches!(event, Event::EnemyEntered),
    }
}

/// The events an enemy's hit on a player makes for that player: `before` / `after` its HP.
pub fn hit_events(result: &HitResult, before: u32, after: u32, hp_max: u32) -> [Option<Event>; 3] {
    let hit = result.outcome == Outcome::Hit;
    let controlled = result.landed & (state_bit(StateKind::Freeze) | state_bit(StateKind::Stun));
    [
        hit.then_some(Event::Hit { crit: result.crit }),
        (after < before).then_some(Event::HpDrop {
            before,
            after,
            hp_max,
        }),
        (hit && controlled != 0).then_some(Event::Controlled),
    ]
}

/// An enemy that moved from `from` to `to` entered the cell next to `bearer` (it was not next to
/// it before).
pub fn entered_next_to(from: Cell, to: Cell, bearer: Cell) -> bool {
    chebyshev(to, bearer) == 1 && chebyshev(from, bearer) > 1
}

/// Whether `bearer` counts `ally` as nearby for `AllyHpBelow`.
pub fn nearby(cfg: &BalanceConfig, bearer: Cell, ally: Cell) -> bool {
    chebyshev(bearer, ally) <= cfg.players.nearby_ally_cells
}

/// What a triggered reactive does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    /// A counterattack on monster `monster` (a line runs on through it).
    Strike { monster: u8 },
    /// A debuff on monster `monster`.
    Debuff { monster: u8 },
    /// A buff on player slot `slot`: the bearer, or the ally whose HP dropped.
    Buff { slot: u8 },
    /// A temporary aura around the bearer.
    Aura,
}

/// The reaction of `skill` (borne by player `bearer` at `at`) to `event`, or `None` when it does
/// not trigger: not its condition, still on cooldown (`cooldown_left > 0`), or the enemy or ally
/// out of reach. `source`: the monster that caused the event and its cell; `ally_cell`: the cell
/// of the ally of an `AllyHpDrop`.
#[allow(clippy::too_many_arguments)]
pub fn reaction(
    cfg: &BalanceConfig,
    skill: &SkillStats,
    cooldown_left: u8,
    bearer: u8,
    at: Cell,
    event: &Event,
    source: Option<(u8, Cell)>,
    ally_cell: Option<Cell>,
) -> Option<Reaction> {
    if cooldown_left > 0 || !fires(skill, event) {
        return None;
    }
    if skill.blocks.is_aura() {
        return Some(Reaction::Aura);
    }
    let range = skill.ext.range;
    let enemy = || {
        let (monster, cell) = source?;
        in_range(range, at, cell, &cfg.players).then_some(monster)
    };
    match skill.blocks.purpose {
        Purpose::Damage => enemy().map(|monster| Reaction::Strike { monster }),
        Purpose::Debuff => enemy().map(|monster| Reaction::Debuff { monster }),
        Purpose::Buff => match *event {
            Event::AllyHpDrop { ally, .. } => {
                let cell = ally_cell?;
                in_ally_range(range, at, cell, &cfg.players)
                    .then_some(Reaction::Buff { slot: ally })
            }
            Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => {
                Some(Reaction::Buff { slot: bearer })
            }
        },
    }
}
