use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    auth,
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::RewardsClaimed,
    outcomes,
    rules::{boss_rewards, REWARD_ROLL_BASE, REWARD_ROLL_STRIDE},
    state::{GameSession, Participation, TempStash, STASH_RESOURCES},
    vrf::SyscallRoll,
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    /// The character's owner (the participation's `player`) or its session key.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        seeds = [seeds::PARTICIPATION, session.key().as_ref(), character.key().as_ref()],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    #[account(
        mut,
        seeds = [seeds::STASH, session.key().as_ref(), character.key().as_ref()],
        bump = stash.bump,
    )]
    pub stash: Box<Account<'info, TempStash>>,
    /// CHECK: the participation's `Character` (bound by the seeds above); written by the CPI.
    #[account(mut)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `Bag`, checked by `character::grant_rewards` (seeds).
    #[account(mut)]
    pub bag: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

/// Owner (main wallet or session key) only, once per character after the party ended
/// (docs/PARTY_AND_TURNS.md, 7-9):
/// - victory, alive: the temporary cache (all accumulated XP + boss rewards) goes to the
///   character through `grant_rewards`, then `unlock`;
/// - fled, alive: the cache is abandoned, `unlock` only;
/// - dead: the cache burns; the tombstone stays locked to this session (`character::mark_dead`).
///
/// Patch 6: every pending death of the session is finalized first.
///
/// TODO(P1, L6.5): only after the DM's final audit.
pub fn handler<'info>(ctx: Context<'info, ClaimRewards<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let participation = &accounts.participation;
    let caller = accounts.signer.key();
    require!(
        caller == participation.player
            || (participation.core.actor != Pubkey::default()
                && caller == participation.core.actor),
        SessionError::NotRewardOwner
    );
    auth::check_actor(
        &accounts.signer,
        &participation.player,
        &participation.core.actor,
        accounts.session_token.as_ref(),
    )?;
    let session = &mut accounts.session;
    let status = session.core.status;
    require!(
        matches!(
            status,
            SessionStatus::Victory | SessionStatus::Fled | SessionStatus::Wiped
        ),
        SessionError::NotFinished
    );
    let slot = accounts.participation.core.slot;
    require!(
        session.settled_mask & (1 << slot) == 0,
        SessionError::AlreadySettled
    );
    session.settled_mask |= 1 << slot;

    let stash = &mut accounts.stash;
    let alive = session.player_alive(slot);
    let signer = SessionSigner {
        session_authority: &accounts.session_authority,
        config: &accounts.config,
        bump: ctx.bumps.session_authority,
    };
    // Patch 6: a party that ended in a callback may still have deaths to finalize; they come
    // first (`remaining_accounts`: a character, skill set and bag per queued slot).
    let key = session.key();
    outcomes::finalize_deaths(session, key, &signer, ctx.remaining_accounts)?;
    let (mut xp, mut resources) = (0, [0u32; STASH_RESOURCES]);
    if alive && status == SessionStatus::Victory {
        stash.xp = stash.xp.saturating_add(session.xp_pool);
        let mut rolls = SyscallRoll::at(
            session.victory_seed,
            REWARD_ROLL_BASE + slot as u32 * REWARD_ROLL_STRIDE,
        );
        for (have, add) in stash.resources.iter_mut().zip(boss_rewards(&mut rolls)) {
            *have = have.saturating_add(add);
        }
        (xp, resources) = (stash.xp, stash.resources);
        cpi_out::character::grant_rewards(
            &signer,
            &accounts.character,
            &accounts.bag,
            xp,
            resources,
        )?;
    }
    if alive {
        cpi_out::character::unlock(&signer, &accounts.character)?;
    }
    stash.xp = 0;
    stash.resources = [0; STASH_RESOURCES];

    emit!(RewardsClaimed {
        session: session.key(),
        character: accounts.character.key(),
        slot,
        xp,
        resources,
    });
    Ok(())
}
