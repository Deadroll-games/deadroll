use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    errors::SessionError,
    events::SessionKeyChanged,
    state::{GameSession, Participation},
};

#[derive(Accounts)]
pub struct SetSessionKey<'info> {
    /// Main wallet only, never a session key: the player's (with `participation`) or the DM's
    /// (without it).
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// The player's participation in this party; `None` sets the DM's key.
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
}

/// Binds a temporary key to this party: the player's first key at Ready, in the `set_ready(true)`
/// transaction (docs/PARTY_AND_TURNS.md 2.1), or another one (docs/GAME_COMMISSIONS.md, 8) when
/// the old key is lost (closed tab, another browser) or its token expired. The site creates the
/// key's `SessionTokenV2` and prepayment in the same transaction. From now on the old key fails
/// `auth::check_bound` even with a live token. `Pubkey::default()` unbinds (the main wallet
/// still acts). Lobby and Active only: after the party only the claims are left, and the main
/// wallet may sign them.
pub fn handler(ctx: Context<SetSessionKey>, session_key: Pubkey) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    require!(
        matches!(
            session.core.status,
            SessionStatus::Lobby | SessionStatus::Active
        ),
        SessionError::PartyEnded
    );
    let signer = accounts.signer.key();
    let character = match accounts.participation.as_mut() {
        Some(participation) => {
            require_keys_eq!(participation.player, signer, SessionError::NotPlayer);
            participation.core.session_key = session_key;
            Some(participation.core.character)
        }
        None => {
            require_keys_eq!(session.core.dm, signer, SessionError::NotDm);
            session.dm_actor = session_key;
            None
        }
    };
    emit!(SessionKeyChanged {
        session: session.key(),
        character,
        session_key,
    });
    Ok(())
}
