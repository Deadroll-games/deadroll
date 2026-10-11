use anchor_lang::prelude::*;

use super::TrustedCall;

/// `sha256("global:lock_for_session")[..8]`.
pub const LOCK_FOR_SESSION: [u8; 8] = [0xec, 0x7e, 0x7f, 0x53, 0xf6, 0x0b, 0xa2, 0xc0];

/// `character::lock_for_session(session)`: accounts `[character (mut), session authority
/// (signer), config]`. STUB(L5), see the module docs.
pub fn lock_for_session(call: TrustedCall, session: Pubkey) -> Result<()> {
    let mut data = LOCK_FOR_SESSION.to_vec();
    data.extend_from_slice(session.as_ref());
    call.invoke(data)
}
