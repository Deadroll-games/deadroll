use anchor_lang::prelude::*;
use deadroll_rules::timeout::DmPhase;
use deadroll_types::{seeds, SessionStatus, PARTY_MAX};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::{ContributionRefunded, PartySettledWithoutEscrow},
    loot, outcomes,
    settlement::{self, PlayerAccounts, SettleCaches},
    state::{Escrow, GameSession, Monsters},
};

#[derive(Accounts)]
pub struct SettleUnconfirmed<'info> {
    /// DM main wallet only: this moves lamports and closes the map (docs/GAME_COMMISSIONS.md 7);
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
    /// CHECK: the session's `MapLayout` (address pinned at `open_session`), closed with the map
    /// by the map CPI.
    #[account(mut, address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

/// Stuck party (docs/PARTY_AND_TURNS.md 3.2): the DM came back after being marked missing, but not
/// every living player confirmed in time. The party ends `Abandoned` and the DM settles it before
/// the deadline: every living player gets their cache as on a peaceful exit (rewards and
/// experience), every player their contribution back, the DM only the map's deposits (the map
/// closes, docs/MAP_DESCRIPTION.md 17.7). Pending deaths are finalized first. After the deadline
/// each player claims their own instead (`claim_after_timeout`).
///
/// `remaining_accounts`: a `(Character, SkillSet, Bag)` triple per pending death, then a writable
/// `(Participation, RewardCache, Character, player wallet)` per player, in slot order.
pub fn handler<'info>(ctx: Context<'info, SettleUnconfirmed<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let dm = accounts.dm.key();
    if dm != session.core.dm {
        require_keys_neq!(dm, session.dm_actor, SessionError::DmMainWalletOnly);
        return err!(SessionError::NotDm);
    }
    let key = session.key();
    let now = Clock::get()?.unix_timestamp;
    check(session, now)?;
    settlement::end_if_dm_left(session, key, now)?;
    let escrow = &mut accounts.escrow;
    require!(!escrow.claimed, SessionError::AlreadyClaimed);
    escrow.claimed = true;

    let signer = SessionSigner {
        session_authority: &accounts.session_authority,
        config: &accounts.config,
        bump: ctx.bumps.session_authority,
    };
    let remaining = ctx.remaining_accounts;
    let deaths = outcomes::death_keys(session).1 * 3;
    require!(
        remaining.len() >= deaths,
        SessionError::InvalidDeathAccounts
    );
    outcomes::finalize_deaths(session, key, &signer, &remaining[..deaths])?;

    let players = &remaining[deaths..];
    let count = session.core.player_count as usize;
    require!(
        players.len() == count * 4,
        SessionError::InvalidSettleAccounts
    );
    let mut loaded: [Option<PlayerAccounts<'info>>; PARTY_MAX] = Default::default();
    for (slot, p) in loaded.iter_mut().enumerate().take(count) {
        let four = &players[slot * 4..slot * 4 + 4];
        let one = settlement::load_player(session, slot as u8, &four[..3])?;
        require!(
            four[3].is_writable && *four[3].key == one.participation.player,
            SessionError::InvalidSettleAccounts
        );
        *p = Some(one);
    }
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

    cpi_out::map::close_map(
        &signer,
        &accounts.map,
        &accounts.layout,
        &accounts.dm.to_account_info(),
    )?;
    // Lamports move last, after the CPIs: each contribution back to its player.
    let mut refunded = 0u64;
    for (slot, p) in loaded.iter().enumerate().take(count) {
        let p = p.as_ref().ok_or(SessionError::InvalidSettleAccounts)?;
        let amount = p.participation.contribution;
        escrow.sub_lamports(amount)?;
        players[slot * 4 + 3].add_lamports(amount)?;
        refunded = refunded.saturating_add(amount);
        emit!(ContributionRefunded {
            session: key,
            player: p.participation.player,
            slot: slot as u8,
            amount,
        });
    }
    emit!(PartySettledWithoutEscrow {
        session: key,
        dm,
        refunded,
    });
    Ok(())
}

/// The party is still on, its DM came back without every living player's confirmation, and the
/// DM's deadline after it has not passed at `now`.
pub fn check(session: &GameSession, now: i64) -> Result<()> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    let DmPhase::Unconfirmed { ended_at } = session.dm_phase(now) else {
        return err!(SessionError::WrongDmPhase);
    };
    require!(
        now <= ended_at.saturating_add(session.settle_timeout_secs as i64),
        SessionError::SettleDeadlinePassed
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::TIMEOUTS;
    use crate::state::test_support::session;
    use deadroll_types::Cell;

    const BACK: i64 = 1_200;

    fn unconfirmed() -> (GameSession, i64) {
        let mut s = session(&[Cell::default(), Cell { x: 1, y: 1 }]);
        s.settle_timeout_secs = 600;
        s.dm_missing_at = 1_000;
        s.dm_back_at = BACK;
        s.dm_confirm_mask = 0b01;
        (s, BACK + TIMEOUTS.dm_return_confirm_secs as i64)
    }

    #[test]
    fn the_dm_settles_after_the_window_until_the_deadline() {
        let (s, ended) = unconfirmed();
        check(&s, ended + 1).unwrap();
        check(&s, ended + 600).unwrap();
    }

    #[test]
    fn rejections() {
        let (s, ended) = unconfirmed();
        // Too early: the players may still confirm.
        assert_eq!(
            check(&s, ended).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        // Too late: the players claim their own.
        assert_eq!(
            check(&s, ended + 601).unwrap_err(),
            error!(SessionError::SettleDeadlinePassed)
        );
        // The DM never came back: the players claim, the DM settles nothing.
        let (mut s, _) = unconfirmed();
        s.dm_back_at = 0;
        assert_eq!(
            check(&s, 1_100).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        assert_eq!(
            check(&s, 5_000).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        // A party with its DM present, or one already over.
        let s = session(&[Cell::default()]);
        assert_eq!(
            check(&s, 5_000).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        let (mut s, ended) = unconfirmed();
        s.core.status = SessionStatus::Abandoned;
        assert_eq!(
            check(&s, ended + 1).unwrap_err(),
            error!(SessionError::NotActive)
        );
    }
}
