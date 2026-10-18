//! Who may change a character's gear (docs/GAME_COMMISSIONS.md, section 5: equipping moves no
//! value, so the session key may do it).
use anchor_lang::prelude::*;
use session_keys::SessionTokenV2;

use crate::errors::CharacterError;

/// `deadroll_session::ID`: the target program of the game's session tokens. A constant, because
/// the session program depends on this one; `crates/sim/tests/parity.rs` checks it.
pub const SESSION_PROGRAM: Pubkey = pubkey!("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");

/// The owner's main wallet always passes. Any other signer passes only together with a live
/// MagicBlock `SessionTokenV2` the owner issued to it for the game: the token players start once
/// for the session program (one popup per game).
pub fn check_owner_or_session_key<'info>(
    signer: &Signer<'info>,
    owner: &Pubkey,
    session_token: Option<&UncheckedAccount<'info>>,
) -> Result<()> {
    if signer.key() == *owner {
        return Ok(());
    }
    let info = session_token.ok_or(CharacterError::SessionTokenMissing)?;
    require_keys_eq!(
        *info.owner,
        session_keys::ID,
        CharacterError::InvalidSessionToken
    );
    // Same checks as `SessionTokenV2::validate`, which needs an `Account<'info, _>`.
    let token = {
        let data = info.try_borrow_data()?;
        SessionTokenV2::try_deserialize(&mut &data[..])
            .map_err(|_| error!(CharacterError::InvalidSessionToken))?
    };
    require_keys_eq!(token.authority, *owner, CharacterError::InvalidSessionToken);
    require_keys_eq!(
        token.target_program,
        SESSION_PROGRAM,
        CharacterError::InvalidSessionToken
    );
    require_keys_eq!(
        token.session_signer,
        signer.key(),
        CharacterError::InvalidSessionToken
    );
    let (pda, _) = Pubkey::find_program_address(
        &[
            SessionTokenV2::SEED_PREFIX.as_bytes(),
            SESSION_PROGRAM.as_ref(),
            signer.key.as_ref(),
            owner.as_ref(),
        ],
        &session_keys::ID,
    );
    require_keys_eq!(pda, info.key(), CharacterError::InvalidSessionToken);
    require!(!token.is_expired()?, CharacterError::SessionTokenExpired);
    Ok(())
}
