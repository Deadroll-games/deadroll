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

/// Stuck party: `signer` takes part in the party: the player of `participation` (a participation
/// of this session, checked by its seeds) by main wallet or bound key, or without one the DM by
/// main wallet or bound key. Returns the player's slot, `None` for the DM.
pub fn participant(
    signer: &Pubkey,
    session: &crate::state::GameSession,
    participation: Option<&crate::state::Participation>,
) -> Result<Option<u8>> {
    let (authority, key, slot) = match participation {
        Some(p) => (&p.player, &p.core.session_key, Some(p.core.slot)),
        None => (&session.core.dm, &session.dm_actor, None),
    };
    check_bound(signer, authority, key).map_err(|_| error!(SessionError::NotParticipant))?;
    Ok(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_participant_is_a_player_of_the_party_or_its_dm() {
        use crate::state::test_support::session;
        use deadroll_types::Cell;
        let mut s = session(&[Cell::default()]);
        let (dm, dm_key) = (Pubkey::new_unique(), Pubkey::new_unique());
        s.core.dm = dm;
        s.dm_actor = dm_key;
        assert_eq!(participant(&dm, &s, None).unwrap(), None);
        assert_eq!(participant(&dm_key, &s, None).unwrap(), None);
        let stranger = Pubkey::new_unique();
        assert_eq!(
            participant(&stranger, &s, None).unwrap_err(),
            error!(SessionError::NotParticipant)
        );
    }

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
