use anchor_lang::prelude::*;
use deadroll_character::{
    cpi,
    cpi::accounts,
    state::{MAP_SKILLS, RESOURCE_KINDS},
};

use super::{signed, SessionSigner};

/// `character::lock_for_session(session, selection)` at `set_ready(true)`: the character program
/// checks the character and the skills taken onto the map against `skill_set`. `owner` (the
/// player, signer of `set_ready`) must sign too, so a session cannot lock someone else's
/// character.
pub fn lock_for_session<'info>(
    s: &SessionSigner<'_, 'info>,
    character: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    skill_set: &AccountInfo<'info>,
    session: Pubkey,
    selection: [u8; MAP_SKILLS],
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
                    skill_set: skill_set.clone(),
                },
                seeds,
            ),
            session,
            selection,
        )
    })
}

/// The accounts a death burns: the character (soulbound items), its `SkillSet` and its `Bag`.
pub struct DeathAccounts<'a, 'info> {
    pub character: &'a AccountInfo<'info>,
    pub skill_set: &'a AccountInfo<'info>,
    pub bag: &'a AccountInfo<'info>,
}

/// `character::mark_dead()`: a death made final (the tombstone; the skills, bag and soulbound
/// items burn), in `start_turn` or a claim (patch 6: never in the VRF callback). The NFT freeze is
/// the permissionless `character::freeze_tombstone`.
pub fn mark_dead<'info>(s: &SessionSigner<'_, 'info>, d: &DeathAccounts<'_, 'info>) -> Result<()> {
    signed(s.bump, |seeds| {
        cpi::mark_dead(CpiContext::new_with_signer(
            deadroll_character::ID,
            accounts::MarkDead {
                character: d.character.clone(),
                session_authority: s.authority(),
                config: s.config(),
                skill_set: d.skill_set.clone(),
                bag: d.bag.clone(),
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

/// `character::unlock()` when the party ends for a living character, or at `set_ready(false)`.
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
