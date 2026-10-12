//! Who may act in a party (docs/GAME_COMMISSIONS.md, sections 4-5 and 7).
use anchor_lang::prelude::*;
use session_keys::SessionTokenV2;

use crate::errors::SessionError;

/// The one actor check every game instruction calls.
///
/// - `authority` is the main wallet: the participation's `player`, or the session's `dm`.
/// - `actor` is the session key bound to this party: `Participation.actor` or `GameSession.dm_actor`.
///
/// The main wallet always passes. A session key passes only together with a live MagicBlock
/// `SessionTokenV2` that the same main wallet issued for this program. The binding to this one
/// party is `actor`; `status == Active` (checked by the caller) makes it useless after the party.
pub fn check_actor<'info>(
    signer: &Signer<'info>,
    authority: &Pubkey,
    actor: &Pubkey,
    session_token: Option<&UncheckedAccount<'info>>,
) -> Result<()> {
    if signer.key() == *authority {
        return Ok(());
    }
    require!(
        *actor != Pubkey::default() && signer.key() == *actor,
        SessionError::Unauthorized
    );
    // FALLBACK(session-keys): if the lead switches to burner keys, return Ok(()) here instead
    // of requiring the token. Not enabled: the L0.4 spike kept Session Keys.
    let info = session_token.ok_or(SessionError::SessionTokenMissing)?;
    require_keys_eq!(
        *info.owner,
        session_keys::ID,
        SessionError::InvalidSessionToken
    );
    // Same checks as `SessionTokenV2::validate`, which needs an `Account<'info, _>` that cannot
    // be built from a borrowed `Context` field.
    let token = {
        let data = info.try_borrow_data()?;
        SessionTokenV2::try_deserialize(&mut &data[..])
            .map_err(|_| error!(SessionError::InvalidSessionToken))?
    };
    require_keys_eq!(
        token.authority,
        *authority,
        SessionError::InvalidSessionToken
    );
    require_keys_eq!(
        token.target_program,
        crate::ID,
        SessionError::InvalidSessionToken
    );
    require_keys_eq!(
        token.session_signer,
        signer.key(),
        SessionError::InvalidSessionToken
    );
    let (pda, _) = Pubkey::find_program_address(
        &[
            SessionTokenV2::SEED_PREFIX.as_bytes(),
            crate::ID.as_ref(),
            signer.key.as_ref(),
            authority.as_ref(),
        ],
        &session_keys::ID,
    );
    require_keys_eq!(pda, info.key(), SessionError::InvalidSessionToken);
    require!(!token.is_expired()?, SessionError::SessionTokenExpired);
    Ok(())
}
