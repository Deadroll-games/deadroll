use anchor_lang::prelude::*;

use crate::{errors::CharacterError, events::CharacterDied, state::Character, trust};

/// CPI from session when HP reaches 0 (inside the VRF callback). Turns the character into a
/// tombstone. No outgoing CPI here (docs/ARCHITECTURE.md, 4.4): the NFT is frozen afterwards
/// by the permissionless `freeze_tombstone`. The character stays locked to its last session.
#[derive(Accounts)]
pub struct MarkDead<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<MarkDead>) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let character = ctx.accounts.character.key();
    let c = &mut ctx.accounts.character.core;
    let session = c.in_session.ok_or(CharacterError::NotLocked)?;
    require!(c.alive, CharacterError::CharacterDead);
    c.alive = false;
    c.state_version = c
        .state_version
        .checked_add(1)
        .ok_or(CharacterError::MathOverflow)?;
    emit!(CharacterDied {
        character,
        owner: c.owner,
        session,
        nft: c.nft,
        state_version: c.state_version,
    });
    Ok(())
}
