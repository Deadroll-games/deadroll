use anchor_lang::prelude::*;
use deadroll_rules::timeout::vrf_retry_due;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::VrfRetried,
    rules::TIMEOUTS,
    state::{GameSession, Participation},
    vrf::{self, RequestAccounts},
};

#[derive(Accounts, Session)]
pub struct RetryVrf<'info> {
    /// Any participant: a player's main wallet or session key (with its `participation`), or the
    /// DM's (without one). Pays the VRF fee.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: this session's `MonsterState` (address from the seeds), passed on to the callback.
    #[account(seeds = [seeds::MONSTERS, session.key().as_ref()], bump)]
    pub monsters: UncheckedAccount<'info>,
    /// The signer's participation in this party; none when the DM signs.
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// MagicBlock `SessionTokenV2` of the session key; none when a main wallet signs.
    #[session(signer = signer, authority = authority())]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
    /// CHECK: our `["identity"]` PDA, signs the VRF request.
    #[account(seeds = [vrf::PROGRAM_IDENTITY_SEED], bump)]
    pub program_identity: UncheckedAccount<'info>,
    /// CHECK: VRF oracle queue, address checked.
    #[account(mut, address = vrf::oracle_queue())]
    pub oracle_queue: UncheckedAccount<'info>,
    /// CHECK: MagicBlock VRF program, address checked (not `Program<_>`: localnet has none).
    #[account(address = vrf::vrf_program_id())]
    pub vrf_program: UncheckedAccount<'info>,
    /// CHECK: SlotHashes sysvar, address checked.
    #[account(address = vrf::slot_hashes())]
    pub slot_hashes: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl RetryVrf<'_> {
    /// The main wallet behind the signer: the participation's player, else the DM.
    fn authority(&self) -> Pubkey {
        match &self.participation {
            Some(p) => p.player,
            None => self.session.core.dm,
        }
    }
}

/// Stuck party: the VRF oracle did not answer the pending roll for `vrf_retry_slots`; any
/// participant requests a new value for the same action, with the same callback accounts
/// (`vrf::pending_callback`). The request gets the next number: the older request's callback is
/// refused from now on (`StaleRandomness`), so the action resolves once, from the new value.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.authority(),
    SessionError::NotParticipant
)]
pub fn handler(ctx: Context<RetryVrf>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    auth::participant(
        accounts.signer.key,
        session,
        accounts.participation.as_deref().map(|p| &**p),
    )?;
    check(session, Clock::get()?.slot)?;
    let key = session.key();
    let callback = vrf::pending_callback(session, key, accounts.monsters.key())?;
    let ticket = vrf::next_request(session, &key)?;
    vrf::request(
        RequestAccounts {
            payer: &accounts.signer,
            program_identity: &accounts.program_identity,
            oracle_queue: &accounts.oracle_queue,
            system_program: &accounts.system_program,
            slot_hashes: &accounts.slot_hashes,
        },
        ctx.bumps.program_identity,
        ticket,
        &callback,
    )?;
    emit!(VrfRetried {
        session: key,
        action: session.core.action_counter,
        request: ticket.number,
    });
    Ok(())
}

/// A roll is pending and its request waited `vrf_retry_slots` at `slot`. Never while the party is
/// frozen (no roll can be pending then: the mark needs none).
pub fn check(session: &GameSession, slot: u64) -> Result<()> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(session.core.pending_vrf, SessionError::NoRollPending);
    require!(session.dm_missing_at == 0, SessionError::PartyFrozen);
    require!(
        vrf_retry_due(session.vrf_requested_slot, slot, TIMEOUTS),
        SessionError::TooEarly
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{test_support::session, PendingAction};
    use crate::vrf::pending_callback;
    use deadroll_types::Cell;

    const SENT: u64 = 5_000;

    fn pending() -> (GameSession, u64) {
        let mut s = session(&[Cell::default(), Cell { x: 1, y: 1 }]);
        s.core.pending_vrf = true;
        s.vrf_requested_slot = SENT;
        s.vrf_request = 3;
        (s, SENT + TIMEOUTS.vrf_retry_slots as u64)
    }

    #[test]
    fn a_silent_oracle_can_be_asked_again() {
        let (s, due) = pending();
        check(&s, due).unwrap();
    }

    #[test]
    fn rejections() {
        let (s, due) = pending();
        assert_eq!(
            check(&s, due - 1).unwrap_err(),
            error!(SessionError::TooEarly)
        );
        let (mut s, due) = pending();
        s.core.pending_vrf = false;
        assert_eq!(
            check(&s, due).unwrap_err(),
            error!(SessionError::NoRollPending)
        );
        let (mut s, due) = pending();
        s.dm_missing_at = 1;
        assert_eq!(
            check(&s, due).unwrap_err(),
            error!(SessionError::PartyFrozen)
        );
        let (mut s, due) = pending();
        s.core.status = SessionStatus::Wiped;
        assert_eq!(check(&s, due).unwrap_err(), error!(SessionError::NotActive));
    }

    /// The retry sends the callback accounts the first request sent: the pending player's, its
    /// character's `SkillSet` and `Bag`, the other living players; the initiative roll's own form.
    #[test]
    fn the_retry_rebuilds_the_first_requests_callback() {
        let (mut s, _) = pending();
        let key = Pubkey::new_unique();
        let monsters = Pubkey::new_unique();
        for slot in 0..2 {
            s.participations[slot] = Pubkey::new_unique();
            s.core.players[slot] = Pubkey::new_unique();
        }
        s.pending = PendingAction {
            attacker: 0,
            target: 30,
            skill_slot: 1,
            participation: s.participations[1],
        };
        let c = pending_callback(&s, key, monsters).unwrap();
        let character = s.core.players[1];
        let pda = |seed: &[u8]| {
            Pubkey::find_program_address(&[seed, character.as_ref()], &deadroll_character::ID).0
        };
        assert_eq!(
            (c.session, c.monsters, c.participation, c.character),
            (key, monsters, s.participations[1], character)
        );
        assert_eq!((c.skill_set, c.bag), (pda(seeds::SKILLS), pda(seeds::BAG)));
        assert_eq!((c.target_participation, c.layout), (None, s.layout));
        let party = crate::party::party_keys(&s, 1);
        assert_eq!((c.party, c.party_len), party);
        assert_eq!(c.party_len, 1);

        s.pending = PendingAction::initiative(s.participations[0]);
        let c = pending_callback(&s, key, monsters).unwrap();
        assert_eq!(
            (c.character, c.skill_set, c.bag, c.party_len),
            (s.core.players[0], s.core.players[0], s.core.players[0], 0)
        );

        s.pending.participation = Pubkey::new_unique();
        assert!(pending_callback(&s, key, monsters).is_err());
    }
}
