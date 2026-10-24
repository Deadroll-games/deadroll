use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::PartyLeft,
    settlement,
    state::{external::read_character, GameSession},
};

#[derive(Accounts)]
pub struct LeaveParty<'info> {
    /// The character's owner, main wallet.
    pub owner: Signer<'info>,
    #[account(seeds = [seeds::SESSION, session.core.map.as_ref()], bump = session.core.bump)]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: the `Character` locked to `session`, decoded with owner and discriminator checks;
    /// written by the unlock CPI.
    #[account(mut)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

/// Patch 11 (decisions of patch 11): the character is free once its party ends. Its owner
/// releases it from the ended party (`character.unlock`) to join another, change gear or press
/// "Done"; the rewards of the ended party wait in its `Participation` for the settlement, and do
/// not burn before the DM's deadline. A tombstone stays locked.
pub fn handler(ctx: Context<LeaveParty>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &accounts.session;
    settlement::require_ended(session)?;
    let character = read_character(&accounts.character)?;
    require_keys_eq!(
        character.owner,
        accounts.owner.key(),
        SessionError::NotCharacterOwner
    );
    require!(character.alive, SessionError::CharacterDead);
    require!(
        character.in_session == Some(session.key()),
        SessionError::NotLockedHere
    );
    cpi_out::character::unlock(
        &SessionSigner {
            session_authority: &accounts.session_authority,
            config: &accounts.config,
            bump: ctx.bumps.session_authority,
        },
        &accounts.character,
    )?;
    emit!(PartyLeft {
        session: session.key(),
        character: accounts.character.key(),
    });
    Ok(())
}
