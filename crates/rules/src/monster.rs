//! Monster templates and their numbers at a level (docs/BESTIARY.md, sections 7-8).
use deadroll_combat::DamageRange;
use deadroll_types::{Aspect, Form, Source, BP};

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
    /// Resistance (counterweight) per aspect, indexed by `Aspect`.
    pub resist_bp: [u32; Aspect::COUNT],
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
    /// Experience for killing it.
    pub xp: u32,
}

impl MonsterBase {
    /// The template at `level`: the weapon and the attack bonus grow with it.
    pub fn profile(&self, level: u8, s: &MonsterScaling) -> MonsterProfile {
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
            resist_bp: self.resist_bp,
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

/// The P0 bestiary the program uses until the registry holds combat profiles.
pub mod p0 {
    use deadroll_mapcheck::roster::{ids, p0_role, Role};

    use super::*;

    const fn resist(aspect: Aspect, bp: u32) -> [u32; Aspect::COUNT] {
        let mut out = [0; Aspect::COUNT];
        out[aspect as usize] = bp;
        out
    }

    const NONE: [u32; Aspect::COUNT] = [0; Aspect::COUNT];

    /// P0 bestiary ids: 0 wolf, 1 goblin, 2 goblin chieftain, 3 goblin warrior, 4 goblin shaman.
    pub fn monster_base(bestiary_id: u8) -> Option<MonsterBase> {
        let strike = |aspect, min, max| DamageRange::new(aspect, min, max);
        let base = match bestiary_id {
            // Bite: martial, strike, Piercing; thick fur resists Cold.
            ids::WOLF => MonsterBase {
                weapon: strike(Aspect::Piercing, 2, 4),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 2,
                defense: 10,
                armor: 5,
                resist_bp: resist(Aspect::Cold, 5_000),
                xp_per_level: 10,
                boss: false,
                hp: 12,
                hp_per_level: 3,
                speed: 6,
                initiative: 6,
            },
            // Rusty blade: martial, strike, Piercing; no special defenses.
            ids::GOBLIN => MonsterBase {
                weapon: strike(Aspect::Piercing, 1, 3),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 1,
                defense: 9,
                armor: 5,
                resist_bp: NONE,
                xp_per_level: 6,
                boss: false,
                hp: 8,
                hp_per_level: 2,
                speed: 4,
                initiative: 6,
            },
            // Heavy blade: martial, strike, Piercing; trophy armor resists Piercing.
            ids::GOBLIN_CHIEFTAIN => MonsterBase {
                weapon: strike(Aspect::Piercing, 4, 8),
                source: Source::Combat,
                form: Form::Strike,
                attack_bonus: 4,
                defense: 13,
                armor: 40,
                resist_bp: resist(Aspect::Piercing, 5_000),
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
                resist_bp: NONE,
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
                resist_bp: resist(Aspect::Fire, 5_000),
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
