use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, PARTY_MAX};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::ContributionRefunded,
    loot, outcomes,
    settlement::{self, PlayerAccounts, SettleCaches},
    state::{Escrow, GameSession, Monsters},
};

#[derive(Accounts)]
pub struct ClaimAfterTimeout<'info> {
    /// The player's main wallet (the participation's `player`): the refund goes here, so no
    /// session key.
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(mut, seeds = [seeds::SESSION, session.core.map.as_ref()], bump = session.core.bump)]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load`.
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's map, written by the map CPI when the last player claims.
    #[account(mut, address = session.core.map @ SessionError::InvalidMap)]
    pub map: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the map program.
    #[account(address = deadroll_map::ID)]
    pub map_program: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

/// Patch 11: the DM did not settle the party before the deadline (`ended_at +
/// settle_timeout_secs`). Each player settles their own character the same way
/// (`crate::settlement`) and takes their contribution back from the escrow, a dead character's
/// owner too (docs/PARTY_AND_TURNS.md 3.2: the DM did not finish the party). The DM can no longer
/// settle; the map goes `Finished` with the last claim.
///
/// `remaining_accounts`: the player's writable `(Participation, RewardCache, Character)`, then a
/// `(Character, SkillSet, Bag)` triple per pending death (finalized first).
pub fn handler<'info>(ctx: Context<'info, ClaimAfterTimeout<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    settlement::require_ended(session)?;
    let escrow = &mut accounts.escrow;
    // The DM settled in time: everything is done.
    require!(!escrow.claimed, SessionError::AlreadyClaimed);
    require!(
        Clock::get()?.unix_timestamp > settlement::deadline(session),
        SessionError::SettleDeadlineNotPassed
    );

    let key = session.key();
    let remaining = ctx.remaining_accounts;
    require!(remaining.len() >= 3, SessionError::InvalidSettleAccounts);
    let slot = session
        .participations
        .iter()
        .take(session.core.player_count as usize)
        .position(|p| p == remaining[0].key)
        .ok_or(SessionError::InvalidSettleAccounts)? as u8;
    let mut player = settlement::load_player(session, &key, slot, &remaining[..3])?;
    require_keys_eq!(
        player.participation.player,
        accounts.player.key(),
        SessionError::NotRewardOwner
    );
    require!(
        session.settled_mask & (1 << slot) == 0,
        SessionError::AlreadySettled
    );
    session.settled_mask |= 1 << slot;

    let signer = SessionSigner {
        session_authority: &accounts.session_authority,
        config: &accounts.config,
        bump: ctx.bumps.session_authority,
    };
    outcomes::finalize_deaths(session, key, &signer, &remaining[3..])?;
    // The kills still waiting pay this player's part; the others get theirs from the same seed.
    let kills = session.unpaid_kills;
    if kills != 0 {
        let monsters = Monsters::load(&accounts.monsters, &key)?;
        let seed = session.core.last_seed;
        let mut one: [Option<PlayerAccounts<'info>>; PARTY_MAX] = Default::default();
        one[slot as usize] = Some(player);
        loot::pay_kills(
            session,
            key,
            &monsters,
            kills,
            seed,
            &mut SettleCaches { players: &mut one },
        )?;
        player = one[slot as usize]
            .take()
            .ok_or(SessionError::InvalidSettleAccounts)?;
    }
    settlement::settle_player(session, key, slot, &mut player, &signer)?;

    if session.settled_mask == session.full_mask() {
        cpi_out::map::set_status(&signer, &accounts.map, MapStatus::Finished)?;
    }
    let amount = player.participation.contribution;
    escrow.sub_lamports(amount)?;
    accounts.player.add_lamports(amount)?;
    emit!(ContributionRefunded {
        session: key,
        player: accounts.player.key(),
        slot,
        amount,
    });
    Ok(())
}
