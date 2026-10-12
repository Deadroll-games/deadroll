use anchor_lang::prelude::*;
use deadroll_character::{cpi, cpi::accounts, state::RESOURCE_KINDS};

use super::{signed, SessionSigner};

/// `character::lock_for_session(session)` at `join_session`; `owner` (the player, signer of
/// `join_session`) must sign too, so a session cannot lock someone else's character.
pub fn lock_for_session<'info>(
    s: &SessionSigner<'_, 'info>,
    character: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    session: Pubkey,
) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::lock_for_session(
            CpiContext::new_with_signer(
                deadroll_character::ID,
                accounts::LockForSession {
                    character: character.clone(),
                    session_authority: s.authority(),
                    config: s.config(),
                    owner: owner.clone(),
                },
                seeds,
            ),
            session,
        )
    })
}

/// `character::mark_dead()` when a player's HP reaches 0 (inside the VRF callback). It only flips
/// state; the NFT freeze is the permissionless `character::freeze_tombstone`.
pub fn mark_dead<'info>(
    s: &SessionSigner<'_, 'info>,
    character: &AccountInfo<'info>,
) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::mark_dead(CpiContext::new_with_signer(
            deadroll_character::ID,
            accounts::MarkDead {
                character: character.clone(),
                session_authority: s.authority(),
                config: s.config(),
            },
            seeds,
        ))
    })
}

/// `character::grant_rewards(xp, resources)` on victory.
pub fn grant_rewards<'info>(
    s: &SessionSigner<'_, 'info>,
    character: &AccountInfo<'info>,
    bag: &AccountInfo<'info>,
    xp: u32,
    resources: [u32; RESOURCE_KINDS],
) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::grant_rewards(
            CpiContext::new_with_signer(
                deadroll_character::ID,
                accounts::GrantRewards {
                    character: character.clone(),
                    session_authority: s.authority(),
                    config: s.config(),
                    bag: bag.clone(),
                },
                seeds,
            ),
            xp,
            resources,
        )
    })
}

/// `character::unlock()` when the party ends for a living character.
pub fn unlock<'info>(s: &SessionSigner<'_, 'info>, character: &AccountInfo<'info>) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::unlock(CpiContext::new_with_signer(
            deadroll_character::ID,
            accounts::Unlock {
                character: character.clone(),
                session_authority: s.authority(),
                config: s.config(),
            },
            seeds,
        ))
    })
}
