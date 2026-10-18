//! Monster skills (docs/BESTIARY.md, section 3): the registry `monster_skills` record and the
//! compact copy a session keeps. Skills are made by the game team with the monster builder
//! ([`crate::validate_monster_skill`]); a template's pool is a run of these records.
//!
//! Registry record (32 bytes, programs/registry README): `[0]` present (1), `[1..13]` the
//! [`SkillBlocks`] fields in order, `[13..16]` the [`SkillExt`] fields, `[16]` flags
//! ([`KIN_ONLY`]), `[17..32]` reserved (zero). The session copy is bytes `[1..17]`.

use deadroll_types::{Aspect, Form, RangeTag, Source, TimeTag};

use crate::{
    validate_monster_skill, Category, Effect, Purpose, SkillBlocks, SkillError, SkillExt,
    SkillTuning, Target, Trigger, SKILL_ASPECTS_MAX,
};

/// Bytes of one registry record.
pub const RECORD_SIZE: usize = 32;
/// Records in the registry table (4096-byte body).
pub const RECORDS_MAX: usize = 128;
/// Bytes of the session copy of one skill.
pub const SESSION_SIZE: usize = 16;
/// Most skills in one template's pool (`Spawn.skills` is a `u16` mask).
pub const POOL_MAX: usize = 16;
/// Flag: an ally-target skill (buff, heal, friendly aura) reaches allies of the bearer's own kin
/// only (docs/BESTIARY.md 7-8: "allied Wolves", "Goblins").
pub const KIN_ONLY: u8 = 1;

/// One monster skill: the blocks, the patch 5 blocks and the monster-only flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterSkill {
    pub blocks: SkillBlocks,
    pub ext: SkillExt,
    pub flags: u8,
}

impl MonsterSkill {
    pub const fn kin_only(&self) -> bool {
        self.flags & KIN_ONLY != 0
    }

    /// The session copy (16 bytes).
    pub fn encode_session(&self) -> [u8; SESSION_SIZE] {
        let b = &self.blocks;
        let mut out = [0u8; SESSION_SIZE];
        out[0] = b.category as u8;
        out[1] = b.purpose as u8;
        out[2] = b.source as u8;
        out[3] = b.form as u8;
        out[4] = b.target as u8;
        for (o, a) in out[5..5 + SKILL_ASPECTS_MAX].iter_mut().zip(b.aspects) {
            *o = a as u8;
        }
        out[8] = b.aspect_count;
        out[9] = b.effect as u8;
        out[10] = b.trigger as u8;
        out[11] = b.aura_radius;
        out[12] = self.ext.range as u8;
        out[13] = self.ext.timing as u8;
        out[14] = self.ext.hp_below_pct;
        out[15] = self.flags;
        out
    }

    /// Decodes a session copy; `None` for bytes that name no variant. Not validated: the session
    /// copies only records the registry validated.
    pub fn decode_session(r: &[u8]) -> Option<MonsterSkill> {
        if r.len() < SESSION_SIZE {
            return None;
        }
        let mut aspects = [Aspect::default(); SKILL_ASPECTS_MAX];
        for (a, &v) in aspects.iter_mut().zip(&r[5..5 + SKILL_ASPECTS_MAX]) {
            *a = Aspect::try_from(v).ok()?;
        }
        Some(MonsterSkill {
            blocks: SkillBlocks {
                category: Category::try_from(r[0]).ok()?,
                purpose: Purpose::try_from(r[1]).ok()?,
                source: Source::try_from(r[2]).ok()?,
                form: Form::try_from(r[3]).ok()?,
                target: Target::try_from(r[4]).ok()?,
                aspects,
                aspect_count: r[8],
                effect: Effect::try_from(r[9]).ok()?,
                trigger: Trigger::try_from(r[10]).ok()?,
                aura_radius: r[11],
            },
            ext: SkillExt {
                range: RangeTag::try_from(r[12]).ok()?,
                timing: TimeTag::try_from(r[13]).ok()?,
                hp_below_pct: r[14],
            },
            flags: r[15],
        })
    }

    /// The registry record (32 bytes).
    pub fn encode_record(&self) -> [u8; RECORD_SIZE] {
        let mut out = [0u8; RECORD_SIZE];
        out[0] = 1;
        out[1..1 + SESSION_SIZE].copy_from_slice(&self.encode_session());
        out
    }

    /// Decodes and validates a registry record with the monster builder rules and the limits of
    /// `t`. `Ok(None)` for an empty (all-zero) slot.
    pub fn decode_record(r: &[u8], t: &SkillTuning) -> Result<Option<MonsterSkill>, RecordError> {
        if r.len() < RECORD_SIZE {
            return Err(RecordError::Size);
        }
        let r = &r[..RECORD_SIZE];
        if r.iter().all(|b| *b == 0) {
            return Ok(None);
        }
        if r[0] != 1 || r[1 + SESSION_SIZE..].iter().any(|b| *b != 0) {
            return Err(RecordError::Layout);
        }
        let skill = Self::decode_session(&r[1..]).ok_or(RecordError::Layout)?;
        if skill.flags & !KIN_ONLY != 0 {
            return Err(RecordError::Layout);
        }
        validate_monster_skill(&skill.blocks, &skill.ext, t).map_err(RecordError::Skill)?;
        Ok(Some(skill))
    }
}

/// Why a registry record was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// Fewer than [`RECORD_SIZE`] bytes.
    Size,
    /// Not present (1), reserved bytes set, unknown flag or a byte that names no variant.
    Layout,
    /// The monster builder refused it.
    Skill(SkillError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heal() -> MonsterSkill {
        MonsterSkill {
            blocks: SkillBlocks {
                category: Category::Active,
                purpose: Purpose::Buff,
                source: Source::Magical,
                form: Form::Projectile,
                target: Target::Single,
                aspects: [Aspect::Blood, Aspect::Fire, Aspect::Fire],
                aspect_count: 1,
                effect: Effect::Heal,
                trigger: Trigger::None,
                aura_radius: 0,
            },
            ext: SkillExt {
                range: RangeTag::Ranged,
                ..SkillExt::MELEE
            },
            flags: KIN_ONLY,
        }
    }

    #[test]
    fn a_record_round_trips_and_is_validated() {
        let mut h = heal();
        let t = SkillTuning::DEFAULT;
        let r = h.encode_record();
        assert_eq!(MonsterSkill::decode_record(&r, &t), Ok(Some(h)));
        assert_eq!(MonsterSkill::decode_session(&h.encode_session()), Some(h));
        assert_eq!(MonsterSkill::decode_record(&[0; 32], &t), Ok(None));
        assert_eq!(
            MonsterSkill::decode_record(&r[..31], &t),
            Err(RecordError::Size)
        );
        let mut bad = r;
        bad[31] = 1;
        assert_eq!(
            MonsterSkill::decode_record(&bad, &t),
            Err(RecordError::Layout)
        );
        let mut bad = r;
        bad[16] = 2;
        assert_eq!(
            MonsterSkill::decode_record(&bad, &t),
            Err(RecordError::Layout)
        );
        let mut bad = r;
        bad[1] = 9;
        assert_eq!(
            MonsterSkill::decode_record(&bad, &t),
            Err(RecordError::Layout)
        );
        // An unused aspect slot set: the builder refuses it.
        let mut unused = heal();
        unused.blocks.aspects[2] = Aspect::Cold;
        assert_eq!(
            MonsterSkill::decode_record(&unused.encode_record(), &t),
            Err(RecordError::Skill(SkillError::UnusedAspectSet))
        );
        // A heal aura is refused for monsters too.
        h.blocks.category = Category::Reactive;
        h.blocks.trigger = Trigger::Hit;
        h.blocks.aura_radius = 2;
        assert_eq!(
            MonsterSkill::decode_record(&h.encode_record(), &t),
            Err(RecordError::Skill(SkillError::HealCategory))
        );
    }
}
