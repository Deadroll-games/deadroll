use anchor_lang::prelude::*;
use deadroll_types::MapStatus;

use super::TrustedCall;

/// `sha256("global:set_status")[..8]`.
pub const SET_STATUS: [u8; 8] = [0xb5, 0xb8, 0xe0, 0xcb, 0xc1, 0x1d, 0xb1, 0xe0];

/// `map::set_status(status)`: accounts `[map (mut), session authority (signer), config]`.
/// STUB(L6), see the module docs.
pub fn set_status(call: TrustedCall, status: MapStatus) -> Result<()> {
    let mut data = SET_STATUS.to_vec();
    data.push(status as u8);
    call.invoke(data)
}
