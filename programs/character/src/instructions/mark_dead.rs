use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::CharacterError,
    events::CharacterDied,
    state::{Bag, Character, SkillSet},
    trust,
};

/// CPI from session when HP reaches 0 (inside the VRF callback). Turns the character into a
/// tombstone (docs/CHARACTER.md, section 11): its skills burn (the names stay as a memorial), the
/// bag burns, the soulbound starter items vanish. No outgoing CPI here (docs/ARCHITECTURE.md,
/// 4.4): the NFT is frozen afterwards by the permissionless `freeze_tombstone`. The character
/// stays locked to its last session.
#[derive(Accounts)]
pub struct MarkDead<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::SKILLS, character.key().as_ref()],
        bump = skill_set.bump,
    )]
    pub skill_set: Box<Account<'info, SkillSet>>,
    #[account(
        mut,
        seeds = [seeds::BAG, character.key().as_ref()],
        bump = bag.bump,
    )]
    pub bag: Box<Account<'info, Bag>>,
}

pub fn handler(ctx: Context<MarkDead>) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let character = ctx.accounts.character.key();
    let ch = &mut ctx.accounts.character;
    let session = ch.core.in_session.ok_or(CharacterError::NotLocked)?;
    require!(ch.core.alive, CharacterError::CharacterDead);
    ch.core.alive = false;
    ch.core.state_version = ch
        .core
        .state_version
        .checked_add(1)
        .ok_or(CharacterError::MathOverflow)?;
    ch.gear.burn();
    ctx.accounts.skill_set.burn();
    ctx.accounts.bag.burn();
    emit!(CharacterDied {
        character,
        owner: ch.core.owner,
        session,
        nft: ch.core.nft,
        state_version: ch.core.state_version,
    });
    Ok(())
}
