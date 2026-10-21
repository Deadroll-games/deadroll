//! Who may act in a party (docs/GAME_COMMISSIONS.md, sections 4-5 and 7).
//!
//! The MagicBlock token is checked by the `session-keys` crate: every game instruction derives
//! `Session` on its accounts (`session_token: Option<Account<'info, SessionTokenV2>>`, target
//! program = this one) and wraps its handler in `#[session_auth_or]`: the main wallet passes, a
//! session key passes with a live token the same main wallet issued to it. This module adds the
//! third condition of section 5: the key is valid only in its own party.
use anchor_lang::prelude::*;

use crate::errors::SessionError;

/// `signer` is the main wallet (`authority`: the participation's `player`, or the session's
/// `dm`) or the temporary key bound to this party (`Participation.session_key`,
/// `GameSession.dm_actor`). A key of another party, or a replaced one, fails here even with a
/// live token. `status == Active` (checked by the caller) makes the key useless after the party.
pub fn check_bound(signer: &Pubkey, authority: &Pubkey, session_key: &Pubkey) -> Result<()> {
    if signer == authority {
        return Ok(());
    }
    require!(
        *session_key != Pubkey::default() && signer == session_key,
        SessionError::Unauthorized
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_wallet_passes_without_a_key() {
        let wallet = Pubkey::new_unique();
        assert!(check_bound(&wallet, &wallet, &Pubkey::default()).is_ok());
    }

    #[test]
    fn bound_key_passes() {
        let (wallet, key) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert!(check_bound(&key, &wallet, &key).is_ok());
    }

    #[test]
    fn other_key_fails() {
        let (wallet, key, other) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        assert!(check_bound(&other, &wallet, &key).is_err());
    }

    #[test]
    fn no_bound_key_fails() {
        let (wallet, key) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert!(check_bound(&key, &wallet, &Pubkey::default()).is_err());
    }
}
