use anchor_lang::prelude::*;

use crate::{errors::CharacterError, events::CharacterUnlocked, state::Character, trust};

/// CPI from session when the party ends for this character. Works on a tombstone too (it just
/// cannot be locked again, `lock_for_session` needs a living character).
#[derive(Accounts)]
pub struct Unlock<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<Unlock>) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let character = ctx.accounts.character.key();
    let c = &mut ctx.accounts.character.core;
    let session = c.in_session.take().ok_or(CharacterError::NotLocked)?;
    emit!(CharacterUnlocked { character, session });
    Ok(())
}
