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

/// Patch D: `map::close_map`: a played map closes when its party has issued the rewards
/// (`settle_party`, the last `claim_after_timeout`); the map's and the layout's deposits go to the
/// DM (docs/MAP_DESCRIPTION.md 17.7).
pub fn close_map<'info>(
    s: &SessionSigner<'_, 'info>,
    map: &AccountInfo<'info>,
    layout: &AccountInfo<'info>,
    dm: &AccountInfo<'info>,
) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::close_map(CpiContext::new_with_signer(
            deadroll_map::ID,
            accounts::CloseMap {
                map: map.clone(),
                layout: layout.clone(),
                dm: dm.clone(),
                session_authority: s.authority(),
                config: s.config(),
            },
            seeds,
        ))
    })
}
