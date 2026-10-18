//! Passive skills and auras (docs/SKILL_BUILDER.md, sections 3.3 and 3.5).
//!
//! - A passive skill works all the time while it is taken onto the map. It rolls nothing: its
//!   strength is a fixed point of its effect's range (`SupportBalance::passive_point_bp`). A
//!   passive skill without an aura works on its bearer only.
//! - An aura works on its bearer and the allies within its radius (Chebyshev distance, no line
//!   of sight). A debuff aura works on the enemies within its radius instead.
//! - A permanent aura comes from a passive skill; a temporary one from a reactive skill, for the
//!   effect's duration in the bearer's own turns.
//! - One aura = one effect, a timed modifier kind. Identical auras do not stack: per kind the
//!   strongest one acts, and an aura and a buff of the same kind follow the same rule as two buffs
//!   (one instance per kind, the stronger acts).
//! - The basic attack "is not strengthened by passive skills" (3.1): a passive Empowerment, an
//!   ally's permanent aura included, does not reach it.
//! - The aura keeps its aspect for the reactions of allies (docs/DAMAGE_TYPES_AND_MIXES.md 7,
//!   a later patch).
//!
//! The merged modifiers are a view for one calculation (a hit, the action points of a turn);
//! the stored ones never take the auras in.
use deadroll_combat::{mod_turns, support::find_mod, ModKind, TimedMod, TimedMods};
use deadroll_skillcheck::{Category, Purpose};
use deadroll_types::{Aspect, Cell, PARTY_MAX};

use crate::{
    config::BalanceConfig,
    input::{effect_action, effect_range, EffectAction, HeroStats, SkillStats},
    units::chebyshev,
};

/// Aura slots of one player: its passive skill's, then its reactive skill's.
pub const AURAS_PER_PLAYER: usize = 2;
pub const PASSIVE_AURA: usize = 0;
pub const REACTIVE_AURA: usize = 1;
/// `Aura::turns` of a passive skill: it never ends.
pub const PERMANENT: u8 = u8::MAX;

/// One aura, or a passive skill on its bearer. `turns == 0` is an empty slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aura {
    pub kind: ModKind,
    /// The skill's aspect.
    pub aspect: Aspect,
    pub power_bp: u32,
    /// Cells around the bearer.
    pub radius: u8,
    /// Own turn starts of the bearer until it ends ([`mod_turns`]), [`PERMANENT`] for a passive.
    pub turns: u8,
    /// A debuff aura: on the enemies around the bearer.
    pub hostile: bool,
    /// From a passive skill.
    pub passive: bool,
    /// A passive skill without an aura: on the bearer only.
    pub self_only: bool,
}

impl Aura {
    pub const fn is_empty(&self) -> bool {
        self.turns == 0
    }

    /// Whether it reaches a unit at `at` from a bearer at `bearer`.
    pub fn reaches(&self, bearer: Cell, at: Cell) -> bool {
        !self.self_only && chebyshev(bearer, at) <= self.radius
    }
}

pub type PlayerAuras = [Aura; AURAS_PER_PLAYER];
pub type PartyAuras = [PlayerAuras; PARTY_MAX];

/// The party as the auras see it.
#[derive(Clone, Copy, Debug)]
pub struct AuraField<'a> {
    pub auras: &'a PartyAuras,
    /// Each player slot's cell.
    pub cells: &'a [Cell; PARTY_MAX],
    /// Bit per living player slot (a dead bearer's auras are gone).
    pub alive: u8,
}

impl AuraField<'_> {
    fn active(&self) -> impl Iterator<Item = (u8, Cell, &Aura)> + '_ {
        (0..PARTY_MAX as u8)
            .filter(|s| self.alive & (1 << s) != 0)
            .flat_map(move |s| {
                let cell = self.cells[s as usize];
                self.auras[s as usize]
                    .iter()
                    .filter(|a| !a.is_empty())
                    .map(move |a| (s, cell, a))
            })
    }

    /// Player `slot` at `at` with its own modifiers `own`, plus every friendly aura over it (its
    /// own passive skill included). `basic_attack`: for the attacker of a basic attack, which no
    /// passive Empowerment strengthens.
    pub fn hero_mods(&self, own: &TimedMods, slot: u8, at: Cell, basic_attack: bool) -> TimedMods {
        let mut out = *own;
        for (bearer, cell, a) in self.active() {
            if a.hostile {
                continue;
            }
            // A self-only aura (a passive skill without an aura) works on its bearer alone.
            let over = if a.self_only {
                bearer == slot
            } else {
                a.reaches(cell, at)
            };
            if !over {
                continue;
            }
            if basic_attack && a.passive && a.kind == ModKind::Empowerment {
                continue;
            }
            merge(&mut out, a, bearer);
        }
        out
    }

    /// A monster at `at` with its own modifiers, plus every debuff aura over it.
    pub fn monster_mods(&self, own: &TimedMods, at: Cell) -> TimedMods {
        let mut out = *own;
        for (bearer, cell, a) in self.active() {
            if a.hostile && a.reaches(cell, at) {
                merge(&mut out, a, bearer);
            }
        }
        out
    }
}

/// One instance per kind: the stronger of the modifier and the aura acts.
fn merge(mods: &mut TimedMods, a: &Aura, bearer: u8) {
    let view = TimedMod {
        kind: a.kind,
        aspect: a.aspect,
        // A view: present while it is computed, never counted down.
        turns: 1,
        power_bp: a.power_bp,
        source: bearer,
    };
    match find_mod(mods, a.kind) {
        Some(i) if mods[i].power_bp >= a.power_bp => {}
        Some(i) => {
            mods[i] = TimedMod {
                turns: mods[i].turns,
                ..view
            }
        }
        None => {
            if let Some(free) = mods.iter_mut().find(|m| m.is_empty()) {
                *free = view;
            }
        }
    }
}

/// The modifier kind of a passive or aura skill (validated skills always have one).
pub fn aura_kind(skill: &SkillStats) -> Option<ModKind> {
    match effect_action(skill.blocks.effect)? {
        EffectAction::Mod(kind) => Some(kind),
        EffectAction::Heal | EffectAction::Stun | EffectAction::Freeze => None,
    }
}

/// The aura of `skill` with strength `power_bp` lasting `turns`.
fn aura_of(skill: &SkillStats, kind: ModKind, power_bp: u32, turns: u8) -> Aura {
    let b = &skill.blocks;
    Aura {
        kind,
        aspect: b.aspects[0],
        power_bp,
        radius: b.aura_radius,
        turns,
        hostile: b.purpose == Purpose::Debuff,
        passive: b.category == Category::Passive,
        self_only: !b.is_aura(),
    }
}

/// A passive skill as it works on the map: a permanent aura (or a buff on the bearer only) at the
/// fixed point of its range. `None` for a skill that is not passive.
pub fn passive_aura(cfg: &BalanceConfig, hero: &HeroStats, skill: &SkillStats) -> Option<Aura> {
    if skill.blocks.category != Category::Passive {
        return None;
    }
    let kind = aura_kind(skill)?;
    let (min, max) = effect_range(cfg, hero, skill.power_bp, EffectAction::Mod(kind));
    let power = cfg.support.passive_point(min, max);
    Some(aura_of(skill, kind, power, PERMANENT))
}

/// A reactive skill's temporary aura of the rolled strength `amount`, for its effect's duration.
/// `None` when the skill is not an aura.
pub fn reactive_aura(cfg: &BalanceConfig, skill: &SkillStats, amount: u32) -> Option<Aura> {
    if !skill.blocks.is_aura() || skill.blocks.category != Category::Reactive {
        return None;
    }
    let kind = aura_kind(skill)?;
    Some(aura_of(
        skill,
        kind,
        amount,
        mod_turns(cfg.support.duration(kind)),
    ))
}

/// Start of the bearer's own turn: a temporary aura counts down; returns the slots that ended
/// (bit per aura slot). Permanent auras never end.
pub fn tick_auras(auras: &mut PlayerAuras) -> u8 {
    let mut ended = 0;
    for (i, a) in auras.iter_mut().enumerate() {
        if a.turns == 0 || a.turns == PERMANENT {
            continue;
        }
        a.turns -= 1;
        if a.turns == 0 {
            *a = Aura::default();
            ended |= 1 << i;
        }
    }
    ended
}
