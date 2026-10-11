use anchor_lang::prelude::*;
use deadroll_skillcheck::{SkillBlocks, SKILL_ASPECTS_MAX};

/// Skills one character can own for now (docs/SKILL_BUILDER.md, section 11 allows 50 later).
pub const SKILLS_MAX: usize = 16;
/// Bytes of a skill name, zero-padded printable ASCII.
pub const SKILL_NAME_LEN: usize = 24;

/// One skill: blocks plus values computed once at creation (`deadroll_skillcheck::derive`).
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct SkillRecord {
    pub blocks: SkillBlocks,
    /// Share of the converted weapon base per aspect, bp; sums to `BP`.
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
    /// `deadroll_skillcheck::fusion_id` of the first two aspects, `0` = none.
    pub fusion: u8,
    /// In the character's turns, `0` = none.
    pub cooldown: u8,
    pub name: [u8; SKILL_NAME_LEN],
}

/// `["skills", character]`: every skill of a character; slot `i < count` is used.
#[account]
#[derive(InitSpace)]
pub struct SkillSet {
    pub version: u8,
    pub character: Pubkey,
    pub count: u8,
    pub skills: [SkillRecord; SKILLS_MAX],
    pub bump: u8,
}
