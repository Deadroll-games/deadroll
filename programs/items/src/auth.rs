//! Who may change a loadout (docs/GAME_COMMISSIONS.md, section 5: equipping moves no value, so
//! the session key may do it).
use anchor_lang::prelude::*;
use session_keys::SessionTokenV2;

use crate::{errors::ItemsError, external::SESSION_PROGRAM};

/// The owner's main wallet always passes. Any other signer passes only together with a live
/// MagicBlock `SessionTokenV2` the owner issued to it for the game: the token players start once
/// for the session program (one popup per game, CLAUDE.md demo step 3).
pub fn check_owner_or_session_key<'info>(
    signer: &Signer<'info>,
    owner: &Pubkey,
    session_token: Option<&UncheckedAccount<'info>>,
) -> Result<()> {
    if signer.key() == *owner {
        return Ok(());
    }
    let info = session_token.ok_or(ItemsError::SessionTokenMissing)?;
    require_keys_eq!(
        *info.owner,
        session_keys::ID,
        ItemsError::InvalidSessionToken
    );
    // Same checks as `SessionTokenV2::validate`, which needs an `Account<'info, _>`.
    let token = {
        let data = info.try_borrow_data()?;
        SessionTokenV2::try_deserialize(&mut &data[..])
            .map_err(|_| error!(ItemsError::InvalidSessionToken))?
    };
    require_keys_eq!(token.authority, *owner, ItemsError::InvalidSessionToken);
    require_keys_eq!(
        token.target_program,
        SESSION_PROGRAM,
        ItemsError::InvalidSessionToken
    );
    require_keys_eq!(
        token.session_signer,
        signer.key(),
        ItemsError::InvalidSessionToken
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
    require_keys_eq!(pda, info.key(), ItemsError::InvalidSessionToken);
    require!(!token.is_expired()?, ItemsError::SessionTokenExpired);
    Ok(())
}
