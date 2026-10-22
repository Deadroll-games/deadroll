//! Who may change a character's gear (docs/GAME_COMMISSIONS.md, section 4: equipping moves no
//! value, so the session key may do it).
//!
//! The check is the `session-keys` crate's `#[session_auth_or]` on the handler. Its
//! `#[derive(Session)]` would make this program the token's target, but the game's tokens target
//! the session program (one token per party), so `game_session_token!` implements the same
//! `SessionV2` trait with that target.
use anchor_lang::prelude::*;

/// `deadroll_session::ID`: the target program of the game's session tokens. A constant, because
/// the session program depends on this one; `crates/sim/tests/parity.rs` checks it.
pub const SESSION_PROGRAM: Pubkey = pubkey!("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");

/// `session_keys::SessionV2` for an accounts struct with `signer`, `character` and
/// `session_token: Option<Account<'info, SessionTokenV2>>`: the token must be the session
/// program's, issued by the character's owner to `signer`.
macro_rules! game_session_token {
    ($accounts:ident) => {
        impl<'info> ::session_keys::SessionV2<'info> for $accounts<'info> {
            fn target_program(&self) -> Pubkey {
                $crate::auth::SESSION_PROGRAM
            }

            fn session_token(&self) -> Option<Account<'info, ::session_keys::SessionTokenV2>> {
                self.session_token.clone()
            }

            fn session_authority(&self) -> Pubkey {
                self.character.core.owner
            }

            fn session_signer(&self) -> Signer<'info> {
                self.signer.clone()
            }
        }
    };
}
pub(crate) use game_session_token;
