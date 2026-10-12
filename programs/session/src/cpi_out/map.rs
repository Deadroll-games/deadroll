use anchor_lang::prelude::*;
use deadroll_map::{cpi, cpi::accounts};
use deadroll_types::MapStatus;

use super::{signed, SessionSigner};

/// `map::set_status(status)`: `InSession` at `open_session`, `Finished` at `claim_escrow`.
pub fn set_status<'info>(
    s: &SessionSigner<'_, 'info>,
    map: &AccountInfo<'info>,
    status: MapStatus,
) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::set_status(
            CpiContext::new_with_signer(
                deadroll_map::ID,
                accounts::SetStatus {
                    map: map.clone(),
                    session_authority: s.authority(),
                    config: s.config(),
                },
                seeds,
            ),
            status,
        )
    })
}
