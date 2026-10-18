use anchor_lang::prelude::*;
use deadroll_skillcheck::SkillExt;
use deadroll_types::{RangeTag, TimeTag};

use super::SKILLS_MAX;

/// Bytes of a skill description, zero-padded printable ASCII (docs/SKILL_BUILDER.md, 2 and 12).
pub const SKILL_DESCRIPTION_LEN: usize = 200;

/// The patch 5 part of the skill at the same `SkillSet` index. Raw bytes (zero-copy): `range`
/// is a `RangeTag`, `timing` a `TimeTag`.
#[zero_copy]
#[derive(Debug, PartialEq, Eq)]
pub struct SkillExtra {
    pub range: u8,
    pub timing: u8,
    /// HP threshold of `HpBelow` / `AllyHpBelow`, percent; 0 for other skills.
    pub hp_below_pct: u8,
    /// `deadroll_skillcheck::Derived::ap_cost`: action points to use it, 0 for reactive and
    /// passive skills.
    pub ap_cost: u8,
    /// Written once by `set_skill_description`; all zero until then.
    pub description: [u8; SKILL_DESCRIPTION_LEN],
}

impl SkillExtra {
    /// Byte size, the same as the account layout (all fields are bytes).
    pub const SIZE: usize = 4 + SKILL_DESCRIPTION_LEN;

    pub fn new(ext: &SkillExt, ap_cost: u8) -> Self {
        SkillExtra {
            range: ext.range as u8,
            timing: ext.timing as u8,
            hp_below_pct: ext.hp_below_pct,
            ap_cost,
            description: [0; SKILL_DESCRIPTION_LEN],
        }
    }

    /// The blocks; `None` for bytes no variant has (never written by this program).
    pub fn ext(&self) -> Option<SkillExt> {
        Some(SkillExt {
            range: RangeTag::try_from(self.range).ok()?,
            timing: TimeTag::try_from(self.timing).ok()?,
            hp_below_pct: self.hp_below_pct,
        })
    }

    pub fn has_description(&self) -> bool {
        self.description.iter().any(|b| *b != 0)
    }
}

/// `["skill_extras", character]`: what patch 5 adds to every skill of a character, by `SkillSet`
/// index (a `SkillSet` record cannot grow in place). Zero-copy: over 3 KB.
#[account(zero_copy)]
#[derive(Debug)]
pub struct SkillExtras {
    pub version: u8,
    pub bump: u8,
    pub character: Pubkey,
    pub extras: [SkillExtra; SKILLS_MAX],
}

impl SkillExtras {
    /// Byte size after the discriminator (all fields are bytes, no padding).
    pub const SIZE: usize = 1 + 1 + 32 + SKILLS_MAX * SkillExtra::SIZE;
}

const _: () = assert!(core::mem::size_of::<SkillExtras>() == SkillExtras::SIZE);
const _: () = assert!(core::mem::size_of::<SkillExtra>() == SkillExtra::SIZE);
