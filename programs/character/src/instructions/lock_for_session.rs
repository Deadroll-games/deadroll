use anchor_lang::prelude::*;

use crate::{errors::CharacterError, events::CharacterLocked, state::Character, trust};

/// CPI from session at `join_session`. Accounts start with `[character, session_authority,
/// config]` like every session-only instruction here.
#[derive(Accounts)]
pub struct LockForSession<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    /// The player, signer of `join_session`: a session cannot lock someone else's character.
    #[account(address = character.core.owner @ CharacterError::NotOwner)]
    pub owner: Signer<'info>,
}

pub fn handler(ctx: Context<LockForSession>, session: Pubkey) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let c = &mut ctx.accounts.character.core;
    require!(c.alive, CharacterError::CharacterDead);
    require!(c.in_session.is_none(), CharacterError::AlreadyLocked);
    c.in_session = Some(session);
    emit!(CharacterLocked {
        character: ctx.accounts.character.key(),
        session
    });
    Ok(())
}
