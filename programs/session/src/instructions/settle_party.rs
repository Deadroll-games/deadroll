use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, PARTY_MAX};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::EscrowClaimed,
    loot, outcomes,
    settlement::{self, PlayerAccounts, SettleCaches},
    state::{Escrow, GameSession, Monsters},
};

#[derive(Accounts)]
pub struct SettleParty<'info> {
    /// DM main wallet only: this moves lamports and the rewards (docs/GAME_COMMISSIONS.md 7);
    /// the DM's session key is refused.
    #[account(mut)]
    pub dm: Signer<'info>,
    #[account(mut, seeds = [seeds::SESSION, session.core.map.as_ref()], bump = session.core.bump)]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load`: the kills still
    /// waiting for their rewards are paid from it.
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's map, written by the map CPI.
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
    /// CHECK: the character program (deaths, experience, unlock).
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

/// Patch 11 (replaces `claim_rewards` and `claim_escrow`): once the party ended and before the
/// DM's deadline, the DM settles every player at once (`crate::settlement`) and takes the escrow;
/// the map goes `Finished`.
///
/// `remaining_accounts`: a `(Character, SkillSet, Bag)` triple per pending death (finalized
/// first), then a writable `(Participation, RewardCache, Character)` triple per player, in slot
/// order.
pub fn handler<'info>(ctx: Context<'info, SettleParty<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let dm = accounts.dm.key();
    if dm != session.core.dm {
        require_keys_neq!(dm, session.dm_actor, SessionError::DmMainWalletOnly);
        return err!(SessionError::NotDm);
    }
    settlement::require_ended(session)?;
    let escrow = &mut accounts.escrow;
    require!(!escrow.claimed, SessionError::AlreadyClaimed);
    require!(
        Clock::get()?.unix_timestamp <= settlement::deadline(session),
        SessionError::SettleDeadlinePassed
    );
    escrow.claimed = true;

    let signer = SessionSigner {
        session_authority: &accounts.session_authority,
        config: &accounts.config,
        bump: ctx.bumps.session_authority,
    };
    let key = session.key();
    let remaining = ctx.remaining_accounts;
    // A party that ended in a callback may still have deaths to finalize; they come first.
    let deaths = outcomes::death_keys(session).1 * 3;
    require!(
        remaining.len() >= deaths,
        SessionError::InvalidDeathAccounts
    );
    outcomes::finalize_deaths(session, key, &signer, &remaining[..deaths])?;

    let players = &remaining[deaths..];
    let count = session.core.player_count as usize;
    require!(
        players.len() == count * 3,
        SessionError::InvalidSettleAccounts
    );
    let mut loaded: [Option<PlayerAccounts<'info>>; PARTY_MAX] = Default::default();
    for (slot, p) in loaded.iter_mut().enumerate().take(count) {
        *p = Some(settlement::load_player(
            session,
            &key,
            slot as u8,
            &players[slot * 3..slot * 3 + 3],
        )?);
    }
    // Kills over-time damage made after the last callback pay out here, from the last VRF value.
    let kills = session.unpaid_kills;
    if kills != 0 {
        let monsters = Monsters::load(&accounts.monsters, &key)?;
        let seed = session.core.last_seed;
        loot::pay_kills(
            session,
            key,
            &monsters,
            kills,
            seed,
            &mut SettleCaches {
                players: &mut loaded,
            },
        )?;
        session.unpaid_kills = 0;
    }
    for (slot, p) in loaded.iter_mut().enumerate().take(count) {
        let p = p.as_mut().ok_or(SessionError::InvalidSettleAccounts)?;
        settlement::settle_player(session, key, slot as u8, p, &signer)?;
    }
    session.settled_mask = session.full_mask();

    cpi_out::map::set_status(&signer, &accounts.map, MapStatus::Finished)?;
    // Lamports move last, after the CPIs: escrow is owned by this program, the DM is a signer.
    let amount = escrow.collected;
    escrow.sub_lamports(amount)?;
    accounts.dm.add_lamports(amount)?;
    emit!(EscrowClaimed {
        session: key,
        dm,
        amount,
    });
    Ok(())
}
