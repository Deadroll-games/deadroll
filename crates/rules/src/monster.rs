//! Monster templates: the registry `Bestiary` record (docs/BESTIARY.md, sections 2, 4, 5, 12)
//! decoded once for the program, the simulator and the website, and its numbers at a level.
//!
//! Every monster number is registry data (`assets/registry/bestiary.json`, uploaded by
//! `tools/scripts/seed-registry.ts`, copied into `MonsterState` at `open_session`); how a
//! template grows with its level and what an elite gets is `MonsterScaling`.
use deadroll_combat::{modifiers::more, DamageRange, TAGS};
use deadroll_mapcheck::roster::{Role, SpawnRules};
use deadroll_skillcheck::{monster::MonsterSkill, Category};
use deadroll_types::{Aspect, Form, Source, StateKind, BP, MONSTER_SLOTS, SPAWN_ELITE};

use crate::{config::MonsterScaling, input::SkillStats};

/// Bestiary record bytes (programs/registry README).
pub mod record {
    pub const SIZE: usize = 48;
    pub const PRESENT: usize = 0;
    pub const ROLE: usize = 1;
    pub const BIOME: usize = 2;
    pub const CELLS: usize = 3;
    pub const MIN_TIER: usize = 4;
    pub const SPEED: usize = 5;
    pub const ATTACK_ASPECT: usize = 6;
    pub const ATTACK_SOURCE: usize = 7;
    pub const ATTACK_FORM: usize = 8;
    pub const PREDISPOSITION: usize = 9;
    pub const RESIST: usize = 10;
    pub const IMMUNE: usize = 11;
    pub const TRAITS: usize = 12;
    pub const ARMOR: usize = 14;
    pub const EVASION: usize = 16;
    pub const ACCURACY: usize = 18;
    pub const INITIATIVE: usize = 20;
    pub const HP: usize = 22;
    pub const HP_PER_LEVEL: usize = 26;
    pub const DAMAGE_MIN: usize = 30;
    pub const DAMAGE_MAX: usize = 32;
    pub const XP: usize = 34;
    pub const IMMUNE_STATES: usize = 38;
    /// Patch 6: the skill pool, a run of `pool_len` records of the registry `monster_skills`
    /// table from `pool_start`, and the kin (`NO_KIN` = none).
    pub const POOL_START: usize = 39;
    pub const POOL_LEN: usize = 40;
    pub const KIN: usize = 41;
    /// Patch 9: defense tag masks (`deadroll_combat::source_bit` / `form_bit`), resisted and
    /// immune (docs/DAMAGE_TYPES_AND_MIXES.md 9: "immunity to magical").
    pub const RESIST_TAGS: usize = 42;
    pub const IMMUNE_TAGS: usize = 43;
    /// `[44..48]` reserved (zero).
    pub const RESERVED: usize = 44;
    /// Predisposition / kin byte for "none".
    pub const NONE: u8 = 0xFF;
}

/// No kin: a kin-only skill of this monster reaches nobody but itself.
pub const NO_KIN: u8 = record::NONE;

/// Roles in record order (`record::ROLE`).
pub const ROLES: [Role; 6] = [
    Role::Swarm,
    Role::Fast,
    Role::Tank,
    Role::Loner,
    Role::Boss,
    Role::Helper,
];

/// A monster's defenses from its registry bestiary record (docs/BESTIARY.md 10,
/// DAMAGE_TYPES_AND_MIXES.md 9): which aspects it resists, which it is immune to, and the states
/// it never gets, and since patch 9 the tags (power source, form) it resists or is immune to.
/// The level of "resistance" is one number, `MonsterScaling::resist_bp`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MonsterDefense {
    /// Bit `1 << aspect`.
    pub resist: u8,
    pub immune_aspects: u8,
    /// Bit `1 << state`.
    pub immune_states: u8,
    /// Bits `deadroll_combat::source_bit` / `form_bit`.
    pub resist_tags: u8,
    pub immune_tags: u8,
}

impl MonsterDefense {
    pub const NONE: MonsterDefense = MonsterDefense {
        resist: 0,
        immune_aspects: 0,
        immune_states: 0,
        resist_tags: 0,
        immune_tags: 0,
    };

    /// From a bestiary record; `None` for an empty slot or bits past the known aspects / states.
    pub fn from_record(r: &[u8]) -> Option<MonsterDefense> {
        MonsterTemplate::from_record(r).map(|t| t.defense)
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

    /// Resistance per defense tag at the scaling's level.
    pub fn tag_resist_bp(&self, s: &MonsterScaling) -> [u32; TAGS] {
        let mut out = [0; TAGS];
        for (i, r) in out.iter_mut().enumerate() {
            if self.resist_tags & (1 << i) != 0 {
                *r = s.resist_bp;
            }
        }
        out
    }
}

/// How a spawn is ranked for its skill slots and HP (docs/BESTIARY.md 3.2, 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Rank {
    Regular,
    Elite,
    Boss,
}

impl Rank {
    pub const COUNT: usize = 3;
}

/// One registry bestiary record (docs/BESTIARY.md 2, 12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterTemplate {
    pub role: Role,
    /// 0 Forest, 1 Caves and Ruins.
    pub biome: u8,
    /// Cells per side (1 or 2).
    pub size: u8,
    pub min_tier: u8,
    /// Action points per turn.
    pub speed: u8,
    /// Natural attack damage at level 0 and its aspect.
    pub weapon: DamageRange,
    pub source: Source,
    pub form: Form,
    pub predisposition: Option<Aspect>,
    pub defense: MonsterDefense,
    /// Trait bits (swarm, regeneration, leader, hunt): rules of a later patch.
    pub traits: u16,
    pub armor: u32,
    /// The hit threshold of an attack on it.
    pub evasion: u32,
    /// The `d20` bonus of its attacks and hostile effects at level 0.
    pub accuracy: u32,
    pub initiative: u16,
    pub hp: u32,
    pub hp_per_level: u32,
    /// Experience per level (level 0 counts as 1).
    pub xp: u32,
    pub pool_start: u8,
    pub pool_len: u8,
    /// [`NO_KIN`] for none.
    pub kin: u8,
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
    /// Patch 9: resistance per defense tag and the immune tags.
    pub tag_resist_bp: [u32; TAGS],
    pub immune_tags: u8,
    /// Patch 9: the template's predisposition, added to its Resistance skills.
    pub predisposition: Option<Aspect>,
    /// Experience for killing it.
    pub xp: u32,
    /// Killing it wins the party (docs/PARTY_AND_TURNS.md, 7.1).
    pub boss: bool,
    pub kin: u8,
}

fn u16_at(r: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([r[at], r[at + 1]])
}

fn u32_at(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

impl MonsterTemplate {
    /// Decodes a bestiary record; `None` for an empty slot or a record `write_table` would refuse.
    pub fn from_record(r: &[u8]) -> Option<MonsterTemplate> {
        use record::*;
        if r.len() < SIZE || r[PRESENT] != 1 {
            return None;
        }
        let r = &r[..SIZE];
        let aspects = (1u16 << Aspect::COUNT) - 1;
        let states = (1u16 << StateKind::COUNT) - 1;
        let fits = |v: u8, mask: u16| u16::from(v) & !mask == 0;
        let tags = (1u16 << TAGS) - 1;
        let defense = MonsterDefense {
            resist: r[RESIST],
            immune_aspects: r[IMMUNE],
            immune_states: r[IMMUNE_STATES],
            resist_tags: r[RESIST_TAGS],
            immune_tags: r[IMMUNE_TAGS],
        };
        if !(fits(defense.resist, aspects)
            && fits(defense.immune_aspects, aspects)
            && fits(defense.immune_states, states)
            && fits(defense.resist_tags, tags)
            && fits(defense.immune_tags, tags))
        {
            return None;
        }
        let (min, max) = (u16_at(r, DAMAGE_MIN), u16_at(r, DAMAGE_MAX));
        if min > max
            || !matches!(r[CELLS], 1 | 2)
            || r[SPEED] == 0
            || r[POOL_LEN] as usize > deadroll_skillcheck::monster::POOL_MAX
            || r[RESERVED..].iter().any(|b| *b != 0)
        {
            return None;
        }
        Some(MonsterTemplate {
            role: *ROLES.get(r[ROLE] as usize)?,
            biome: r[BIOME],
            size: r[CELLS],
            min_tier: r[MIN_TIER],
            speed: r[SPEED],
            weapon: DamageRange::new(
                Aspect::try_from(r[ATTACK_ASPECT]).ok()?,
                min as u32,
                max as u32,
            ),
            source: Source::try_from(r[ATTACK_SOURCE]).ok()?,
            form: Form::try_from(r[ATTACK_FORM]).ok()?,
            predisposition: match r[PREDISPOSITION] {
                NONE => None,
                a => Some(Aspect::try_from(a).ok()?),
            },
            defense,
            traits: u16_at(r, TRAITS),
            armor: u16_at(r, ARMOR) as u32,
            evasion: u16_at(r, EVASION) as u32,
            accuracy: u16_at(r, ACCURACY) as u32,
            initiative: u16_at(r, INITIATIVE),
            hp: u32_at(r, HP),
            hp_per_level: u32_at(r, HP_PER_LEVEL),
            xp: u32_at(r, XP),
            pool_start: r[POOL_START],
            pool_len: r[POOL_LEN],
            kin: r[KIN],
        })
    }

    pub fn is_boss(&self) -> bool {
        self.role == Role::Boss
    }

    /// The rank of a spawn of this template with `flags` (`SPAWN_ELITE`): a boss is always a
    /// boss; any other template may be elite (BESTIARY 5: "a regular monster").
    pub fn rank(&self, flags: u8) -> Rank {
        if self.is_boss() {
            Rank::Boss
        } else if flags & SPAWN_ELITE != 0 {
            Rank::Elite
        } else {
            Rank::Regular
        }
    }

    /// The template at `level` with its registry defenses: the weapon and the accuracy grow with
    /// the level (BESTIARY 4).
    pub fn profile(&self, level: u8, s: &MonsterScaling) -> MonsterProfile {
        let l = level as u32;
        let grow = |v: u32, bp: u32| v.saturating_add((l as u64 * bp as u64 / BP as u64) as u32);
        let min = grow(self.weapon.min, s.weapon_min_per_level_bp);
        let max = grow(self.weapon.max, s.weapon_max_per_level_bp);
        MonsterProfile {
            weapon: DamageRange::new(self.weapon.aspect, min, max.max(min)),
            source: self.source,
            form: self.form,
            attack_bonus: grow(self.accuracy, s.attack_per_level_bp),
            defense: self.evasion,
            armor: self.armor,
            resist_bp: self.defense.resist_bp(s),
            immune_aspects: self.defense.immune_aspects,
            immune_states: self.defense.immune_states,
            tag_resist_bp: self.defense.tag_resist_bp(s),
            immune_tags: self.defense.immune_tags,
            predisposition: self.predisposition,
            xp: self.xp.saturating_mul(l.max(1)),
            boss: self.is_boss(),
            kin: self.kin,
        }
    }

    /// HP at spawn: the level's HP, more for an elite (BESTIARY 5).
    pub fn hp_at(&self, level: u8, flags: u8, s: &MonsterScaling) -> u32 {
        let hp = monster_hp(self.hp, self.hp_per_level, level);
        match self.rank(flags) {
            Rank::Elite => more(hp as u64, s.elite_hp_bp).min(u32::MAX as u64) as u32,
            Rank::Regular | Rank::Boss => hp,
        }
    }

    /// The builder rules of this template for `deadroll_mapcheck::check_map`: `category_of(i)`
    /// is the category of `monster_skills` record `i` (`None` for an empty or missing record: then
    /// the template is unusable and `None` comes back).
    pub fn spawn_rules(
        &self,
        category_of: impl Fn(usize) -> Option<Category>,
        s: &MonsterScaling,
    ) -> Option<SpawnRules> {
        let mut r = SpawnRules::plain(self.role);
        r.min_tier = self.min_tier;
        r.pool_start = self.pool_start;
        r.pool_len = self.pool_len;
        for (i, c) in r.pool.iter_mut().enumerate().take(self.pool_len as usize) {
            *c = category_of(self.pool_start as usize + i)?;
        }
        r.slots = s.slots;
        Some(r)
    }

    /// Skill slots by `Category` for a spawn with `flags` (BESTIARY 3.2).
    pub fn slots(&self, flags: u8, s: &MonsterScaling) -> [u8; Category::COUNT] {
        s.slots[self.rank(flags) as usize]
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

/// Slot of each category's first skill in a monster's [`MONSTER_SLOTS`]: `0..5` active, `5`
/// reactive, `6` passive.
pub const ACTIVE_SLOT: usize = 0;
pub const ACTIVE_SLOTS: usize = 5;
pub const REACTIVE_SLOT: usize = 5;
pub const PASSIVE_SLOT: usize = 6;
/// An empty skill slot.
pub const NO_SKILL: u8 = 0xFF;

/// The skills a spawn takes into the fight, as pool indices by slot ([`NO_SKILL`] = empty):
/// the chosen pool bits of each category in pool order, at most the rank's slots of it. Bits past
/// the pool and past the slots are skipped (`finalize_map` already refused them; old maps open).
pub fn select_skills(
    pool: &[Category],
    mask: u16,
    slots: [u8; Category::COUNT],
) -> [u8; MONSTER_SLOTS] {
    let mut out = [NO_SKILL; MONSTER_SLOTS];
    let mut taken = [0usize; Category::COUNT];
    for (i, c) in pool.iter().enumerate().take(16) {
        if mask & (1 << i) == 0 {
            continue;
        }
        let k = *c as usize;
        let (first, room) = match c {
            Category::Active => (ACTIVE_SLOT, ACTIVE_SLOTS),
            Category::Reactive => (REACTIVE_SLOT, 1),
            Category::Passive => (PASSIVE_SLOT, 1),
        };
        if taken[k] < (slots[k] as usize).min(room) {
            out[first + taken[k]] = i as u8;
            taken[k] += 1;
        }
    }
    out
}

/// A monster skill with what is derived from its blocks (`SkillTuning`, like a player's at
/// creation) and its monster-only flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterSkillStats {
    pub stats: SkillStats,
    pub kin_only: bool,
}

impl MonsterSkillStats {
    pub fn new(skill: &MonsterSkill, t: &deadroll_skillcheck::SkillTuning) -> Self {
        MonsterSkillStats {
            stats: SkillStats::derive(skill.blocks, skill.ext, t),
            kin_only: skill.kin_only(),
        }
    }
}

/// Whether a kin-only ally effect of a bearer of kin `bearer` reaches an ally of kin `ally`
/// (the bearer itself always).
pub fn kin_reaches(kin_only: bool, bearer: u8, ally: u8, self_target: bool) -> bool {
    self_target || !kin_only || (bearer != NO_KIN && bearer == ally)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wolf() -> [u8; record::SIZE] {
        let mut r = [0u8; record::SIZE];
        r[0] = 1;
        r[1] = 1;
        r[3] = 1;
        r[4] = 1;
        r[5] = 6;
        r[6] = Aspect::Piercing as u8;
        r[9] = record::NONE;
        r[10] = 1 << Aspect::Cold as u8;
        r[14..16].copy_from_slice(&5u16.to_le_bytes());
        r[16..18].copy_from_slice(&10u16.to_le_bytes());
        r[18..20].copy_from_slice(&2u16.to_le_bytes());
        r[20..22].copy_from_slice(&6u16.to_le_bytes());
        r[22..26].copy_from_slice(&12u32.to_le_bytes());
        r[26..30].copy_from_slice(&3u32.to_le_bytes());
        r[30..32].copy_from_slice(&2u16.to_le_bytes());
        r[32..34].copy_from_slice(&4u16.to_le_bytes());
        r[34..38].copy_from_slice(&10u32.to_le_bytes());
        r[39] = 0;
        r[40] = 4;
        r[41] = 0;
        r
    }

    #[test]
    fn a_record_decodes_and_scales() {
        let s = MonsterScaling::DEFAULT;
        let t = MonsterTemplate::from_record(&wolf()).unwrap();
        assert_eq!(t.role, Role::Fast);
        assert_eq!((t.pool_start, t.pool_len, t.kin), (0, 4, 0));
        let p = t.profile(2, &s);
        assert_eq!((p.weapon.min, p.weapon.max), (3, 6));
        assert_eq!((p.attack_bonus, p.defense, p.armor, p.xp), (4, 10, 5, 20));
        assert_eq!(p.resist_bp[Aspect::Cold as usize], s.resist_bp);
        assert!(!p.boss);
        assert_eq!(t.hp_at(2, 0, &s), 18);
        assert_eq!(t.rank(SPAWN_ELITE), Rank::Elite);
        assert!(t.hp_at(2, SPAWN_ELITE, &s) > 18);
        assert_eq!(t.slots(0, &s), s.slots[Rank::Regular as usize]);
        assert_eq!(t.slots(SPAWN_ELITE, &s), s.slots[Rank::Elite as usize]);
    }

    #[test]
    fn bad_records_are_refused() {
        let ok = wolf();
        for (at, v) in [
            (record::PRESENT, 0),
            (record::ROLE, 6),
            (record::CELLS, 3),
            (record::SPEED, 0),
            (record::ATTACK_ASPECT, 6),
            (record::ATTACK_SOURCE, 2),
            (record::ATTACK_FORM, 2),
            (record::PREDISPOSITION, 6),
            (record::RESIST, 0x40),
            (record::IMMUNE_STATES, 0x80),
            (record::POOL_LEN, 17),
            (record::RESERVED, 1),
        ] {
            let mut r = ok;
            r[at] = v;
            assert_eq!(MonsterTemplate::from_record(&r), None, "byte {at} = {v}");
        }
        let mut r = ok;
        r[30..32].copy_from_slice(&9u16.to_le_bytes());
        assert_eq!(MonsterTemplate::from_record(&r), None);
        assert_eq!(MonsterTemplate::from_record(&ok[..47]), None);
    }

    #[test]
    fn skills_fill_their_slots_in_pool_order() {
        use Category::*;
        let pool = [Active, Passive, Active, Active, Active, Reactive];
        // Regular: 3 active, the passive; the 4th active bit and the reactive are skipped.
        let s = select_skills(&pool, 0b11_1111, [3, 0, 1]);
        assert_eq!(s, [0, 2, 3, NO_SKILL, NO_SKILL, NO_SKILL, 1]);
        let s = select_skills(&pool, 0b11_1111, [5, 1, 1]);
        assert_eq!(s, [0, 2, 3, 4, NO_SKILL, 5, 1]);
        assert_eq!(
            select_skills(&pool, 0, [5, 1, 1]),
            [NO_SKILL; MONSTER_SLOTS]
        );
        // Bits past the pool are ignored.
        assert_eq!(select_skills(&[Active], 0xFFFF, [3, 0, 1])[0], 0);
    }

    #[test]
    fn kin_only_reaches_the_own_kin_and_self() {
        assert!(kin_reaches(false, 0, 1, false));
        assert!(kin_reaches(true, 1, 1, false));
        assert!(!kin_reaches(true, 1, 0, false));
        assert!(!kin_reaches(true, NO_KIN, NO_KIN, false));
        assert!(kin_reaches(true, NO_KIN, NO_KIN, true));
    }
}
