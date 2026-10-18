//! Monster templates and their numbers at a level (docs/BESTIARY.md, sections 7-8).
use deadroll_combat::DamageRange;
use deadroll_types::{Aspect, Form, Source, StateKind, BP};

use crate::config::MonsterScaling;

/// A monster template at level 0: content, not balance. The program reads the P0 table in
/// [`p0`]; `tools/balance` reads JSON presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterBase {
    /// Natural attack damage at level 0.
    pub weapon: DamageRange,
    pub source: Source,
    pub form: Form,
    pub attack_bonus: u32,
    pub defense: u32,
    pub armor: u32,
    /// Experience for a kill, per level (level 0 counts as 1).
    pub xp_per_level: u32,
    /// Killing it wins the party (docs/PARTY_AND_TURNS.md, 7.1).
    pub boss: bool,
    /// Registry `Bestiary` numbers: HP at level 0 and per level, speed (action points),
    /// initiative. On-chain these come from the registry table copied at `open_session`.
    pub hp: u32,
    pub hp_per_level: u32,
    pub speed: u8,
    pub initiative: u16,
}

/// One template at a level: what the hit inputs need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterProfile {
    pub weapon: DamageRange,
    pub source: Source,
    pub form: Form,
    pub attack_bonus: u32,
    pub defense: u32,
    pub armor: u32,
    pub resist_bp: [u32; Aspect::COUNT],
    /// Immunity per aspect and per state, bit `1 << aspect` / `1 << state`.
    pub immune_aspects: u8,
    pub immune_states: u8,
    /// Experience for killing it.
    pub xp: u32,
}

/// A monster's defenses from its registry bestiary record (docs/BESTIARY.md 10,
/// DAMAGE_TYPES_AND_MIXES.md 9): which aspects it resists, which it is immune to, and the states
/// it never gets. The level of "resistance" is one number, `MonsterScaling::resist_bp`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MonsterDefense {
    /// Bit `1 << aspect`.
    pub resist: u8,
    pub immune_aspects: u8,
    /// Bit `1 << state`.
    pub immune_states: u8,
}

/// Bestiary record bytes (programs/registry README): resistance and immunity masks, the state
/// immunity mask.
pub mod record {
    pub const SIZE: usize = 48;
    pub const PRESENT: usize = 0;
    pub const RESIST: usize = 10;
    pub const IMMUNE: usize = 11;
    pub const IMMUNE_STATES: usize = 38;
}

impl MonsterDefense {
    pub const NONE: MonsterDefense = MonsterDefense {
        resist: 0,
        immune_aspects: 0,
        immune_states: 0,
    };

    /// From a bestiary record; `None` for an empty slot or bits past the known aspects / states.
    pub fn from_record(r: &[u8]) -> Option<MonsterDefense> {
        if r.len() < record::SIZE || r[record::PRESENT] != 1 {
            return None;
        }
        let aspects = (1u16 << Aspect::COUNT) - 1;
        let states = (1u16 << StateKind::COUNT) - 1;
        let d = MonsterDefense {
            resist: r[record::RESIST],
            immune_aspects: r[record::IMMUNE],
            immune_states: r[record::IMMUNE_STATES],
        };
        let fits = |v: u8, mask: u16| u16::from(v) & !mask == 0;
        (fits(d.resist, aspects)
            && fits(d.immune_aspects, aspects)
            && fits(d.immune_states, states))
        .then_some(d)
    }

    /// Resistance per aspect at the scaling's level.
    pub fn resist_bp(&self, s: &MonsterScaling) -> [u32; Aspect::COUNT] {
        let mut out = [0; Aspect::COUNT];
        for (i, r) in out.iter_mut().enumerate() {
            if self.resist & (1 << i) != 0 {
                *r = s.resist_bp;
            }
        }
        out
    }
}

impl MonsterBase {
    /// The template at `level` with its registry defenses: the weapon and the attack bonus grow
    /// with the level.
    pub fn profile(
        &self,
        level: u8,
        s: &MonsterScaling,
        defense: &MonsterDefense,
    ) -> MonsterProfile {
        let l = level as u32;
        let grow = |v: u32, bp: u32| v.saturating_add((l as u64 * bp as u64 / BP as u64) as u32);
        let min = grow(self.weapon.min, s.weapon_min_per_level_bp);
        let max = grow(self.weapon.max, s.weapon_max_per_level_bp);
        MonsterProfile {
            weapon: DamageRange::new(self.weapon.aspect, min, max.max(min)),
            source: self.source,
            form: self.form,
            attack_bonus: grow(self.attack_bonus, s.attack_per_level_bp),
            defense: self.defense,
            armor: self.armor,
            resist_bp: defense.resist_bp(s),
            immune_aspects: defense.immune_aspects,
            immune_states: defense.immune_states,
            xp: self.xp_per_level.saturating_mul(l.max(1)),
        }
    }

    /// HP at spawn.
    pub fn hp_at(&self, level: u8) -> u32 {
        monster_hp(self.hp, self.hp_per_level, level)
    }
}

/// Action points per turn for a monster: its bestiary speed.
pub fn monster_ap(speed: u8, s: &MonsterScaling) -> u8 {
    speed.clamp(s.ap_min, s.ap_max.max(s.ap_min))
}

/// Monster HP at spawn from its bestiary numbers (at least 1).
pub fn monster_hp(hp: u32, hp_per_level: u32, level: u8) -> u32 {
    hp.saturating_add(hp_per_level.saturating_mul(level as u32))
        .max(1)
}

/// The P0 bestiary the program uses until the registry holds combat profiles. Defenses come
/// from the registry (`MonsterDefense`), not from here.
pub mod p0 {
    use deadroll_mapcheck::roster::{ids, p0_role, Role};

    use super::*;

    /// P0 bestiary ids: 0 wolf, 1 goblin, 2 goblin chieftain, 3 goblin warrior, 4 goblin shaman.
    pub fn monster_base(bestiary_id: u8) -> Option<MonsterBase> {
        let strike = |aspect, min, max| DamageRange::new(aspect, min, max);
        let base = match bestiary_id {
            // Bite: martial, strike, Piercing.
            ids::WOLF => MonsterBase {
                weapon: strike(Aspect::Piercing, 2, 4),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 2,
                defense: 10,
                armor: 5,
                xp_per_level: 10,
                boss: false,
                hp: 12,
                hp_per_level: 3,
                speed: 6,
                initiative: 6,
            },
            // Rusty blade: martial, strike, Piercing.
            ids::GOBLIN => MonsterBase {
                weapon: strike(Aspect::Piercing, 1, 3),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 1,
                defense: 9,
                armor: 5,
                xp_per_level: 6,
                boss: false,
                hp: 8,
                hp_per_level: 2,
                speed: 4,
                initiative: 6,
            },
            // Heavy blade: martial, strike, Piercing.
            ids::GOBLIN_CHIEFTAIN => MonsterBase {
                weapon: strike(Aspect::Piercing, 4, 8),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 4,
                defense: 13,
                armor: 40,
                xp_per_level: 50,
                boss: false,
                hp: 28,
                hp_per_level: 6,
                speed: 5,
                initiative: 8,
            },
            ids::GOBLIN_WARRIOR => MonsterBase {
                weapon: strike(Aspect::Blunt, 2, 5),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 3,
                defense: 11,
                armor: 25,
                xp_per_level: 15,
                boss: false,
                hp: 14,
                hp_per_level: 3,
                speed: 4,
                initiative: 5,
            },
            // Support caster: a magical bolt from range.
            ids::GOBLIN_SHAMAN => MonsterBase {
                weapon: strike(Aspect::Fire, 2, 4),
                source: Source::Magical,
                form: Form::Projectile,
                attack_bonus: 2,
                defense: 10,
                armor: 5,
                xp_per_level: 15,
                boss: false,
                hp: 10,
                hp_per_level: 2,
                speed: 3,
                initiative: 6,
            },
            _ => return None,
        };
        Some(MonsterBase {
            boss: p0_role(bestiary_id) == Some(Role::Boss),
            ..base
        })
    }
}
