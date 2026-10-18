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
//! - Monsters (patch 6) bear auras the same way: a friendly one works on the allied monsters
//!   around (of the bearer's own kin for a kin-only skill, docs/BESTIARY.md 8.1 "nearby Goblins"),
//!   a hostile one on the players around.
//!
//! The merged modifiers are a view for one calculation (a hit, the action points of a turn);
//! the stored ones never take the auras in.
use deadroll_combat::{mod_turns, support::find_mod, ModKind, TimedMod, TimedMods};
use deadroll_skillcheck::{Category, Purpose};
use deadroll_types::{Aspect, Cell, PARTY_MAX};

use crate::{
    config::BalanceConfig,
    input::{
        effect_action, effect_range, monster_effect_range, EffectAction, HeroStats, SkillStats,
    },
    monster::{kin_reaches, MonsterProfile, MonsterSkillStats},
    units::{chebyshev, MONSTER_UNIT_BASE},
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
    /// Patch 6, monster bearers: a friendly aura on allies of the bearer's kin only.
    pub kin_only: bool,
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
/// A monster's auras: its passive skill's, then its reactive skill's (like a player's).
pub type MonsterAuras = PlayerAuras;

/// One living monster with an aura, as the auras see it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterBearer {
    pub index: u8,
    pub cell: Cell,
    /// `MonsterTemplate::kin`.
    pub kin: u8,
    pub auras: MonsterAuras,
}

impl MonsterBearer {
    pub fn has_aura(auras: &MonsterAuras) -> bool {
        auras.iter().any(|a| !a.is_empty())
    }
}

/// The monsters as the auras see them: the living ones that bear an aura (a dead bearer's auras
/// are gone), in any order.
#[derive(Clone, Copy, Debug)]
pub struct MonsterAuraField<'a> {
    pub bearers: &'a [MonsterBearer],
}

impl MonsterAuraField<'_> {
    /// No monster auras (a view without the monsters).
    pub const EMPTY: MonsterAuraField<'static> = MonsterAuraField { bearers: &[] };

    fn active(&self) -> impl Iterator<Item = (&MonsterBearer, &Aura)> + '_ {
        self.bearers.iter().flat_map(|b| {
            b.auras
                .iter()
                .filter(|a| !a.is_empty())
                .map(move |a| (b, a))
        })
    }
}

/// The party (and, since patch 6, the monsters) as the auras see them.
#[derive(Clone, Copy, Debug)]
pub struct AuraField<'a> {
    pub auras: &'a PartyAuras,
    /// Each player slot's cell.
    pub cells: &'a [Cell; PARTY_MAX],
    /// Bit per living player slot (a dead bearer's auras are gone).
    pub alive: u8,
    pub monsters: MonsterAuraField<'a>,
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
        // Debuff auras of the monsters around.
        for (b, a) in self.monsters.active() {
            if a.hostile && a.reaches(b.cell, at) {
                merge(&mut out, a, MONSTER_UNIT_BASE + b.index);
            }
        }
        out
    }

    /// Monster `index` (of kin `kin`) at `at` with its own modifiers, plus every debuff aura of the
    /// players over it and every friendly aura of the monsters (its own passive skill included).
    /// `basic_attack`: for its natural attack, which no passive Empowerment strengthens (one
    /// engine for everyone, docs/BESTIARY.md 1.2; SKILL_BUILDER.md 3.1).
    pub fn monster_mods(
        &self,
        own: &TimedMods,
        index: u8,
        at: Cell,
        kin: u8,
        basic_attack: bool,
    ) -> TimedMods {
        let mut out = *own;
        for (bearer, cell, a) in self.active() {
            if a.hostile && a.reaches(cell, at) {
                merge(&mut out, a, bearer);
            }
        }
        for (b, a) in self.monsters.active() {
            if a.hostile {
                continue;
            }
            let me = b.index == index;
            let over = if a.self_only {
                me
            } else {
                a.reaches(b.cell, at) && kin_reaches(a.kin_only, b.kin, kin, me)
            };
            if !over || (basic_attack && a.passive && a.kind == ModKind::Empowerment) {
                continue;
            }
            merge(&mut out, a, MONSTER_UNIT_BASE + b.index);
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
        kin_only: false,
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

/// A monster's passive skill as it works in the fight: like [`passive_aura`] with the monster's
/// numbers and its kin-only flag.
pub fn monster_passive_aura(
    cfg: &BalanceConfig,
    profile: &MonsterProfile,
    skill: &MonsterSkillStats,
) -> Option<Aura> {
    let s = &skill.stats;
    if s.blocks.category != Category::Passive {
        return None;
    }
    let kind = aura_kind(s)?;
    let (min, max) = monster_effect_range(cfg, profile, s.power_bp, EffectAction::Mod(kind));
    let power = cfg.support.passive_point(min, max);
    Some(Aura {
        kin_only: skill.kin_only,
        ..aura_of(s, kind, power, PERMANENT)
    })
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
