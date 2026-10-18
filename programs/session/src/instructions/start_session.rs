use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    errors::SessionError,
    events::ActionRequested,
    rules::EMPTY_UNIT,
    state::{Escrow, GameSession, MonsterState, Monsters, PendingAction},
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts)]
pub struct StartSession<'info> {
    /// DM main wallet; pays the VRF fee of the initiative roll.
    #[account(mut)]
    pub dm: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in
    /// `Monsters`; passed on to the callback.
    pub monsters: LazyAccount<'info, MonsterState>,
    #[account(seeds = [seeds::ESCROW, session.key().as_ref()], bump = escrow.bump)]
    pub escrow: Box<Account<'info, Escrow>>,
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

/// Checks the lobby, then asks the VRF for the initiative roll. The party is `Active` from here,
/// but nobody can act until `consume_randomness` writes the turn order (`turn_order_len == 0`
/// and `pending_vrf`).
pub fn handler(ctx: Context<StartSession>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require_keys_eq!(session.core.dm, accounts.dm.key(), SessionError::NotDm);
    require!(
        session.core.status == SessionStatus::Lobby,
        SessionError::NotInLobby
    );
    require!(session.core.player_count > 0, SessionError::NoPlayers);
    require!(
        session.ready_mask == session.full_mask(),
        SessionError::NotAllReady
    );
    require!(
        accounts.escrow.collected >= session.core.price_lamports,
        SessionError::PriceNotCollected
    );
    Monsters::load(accounts.monsters.as_ref(), &session_key)?;

    // The callback's account list is fixed (participation + character of the hit's player
    // side); the initiative roll passes player slot 0, which the callback does not modify.
    let character = session.core.players[0];
    let (participation, _) = Pubkey::find_program_address(
        &[
            seeds::PARTICIPATION,
            session_key.as_ref(),
            character.as_ref(),
        ],
        &crate::ID,
    );

    session.core.status = SessionStatus::Active;
    session.core.round = 1;
    session.core.turn_index = 0;
    session.turn_order_len = 0;
    session.turn_seq = 0;
    session.turn_started_at = Clock::get()?.unix_timestamp;
    session.pending = PendingAction::initiative(participation);
    session.core.pending_vrf = true;

    vrf::request(
        RequestAccounts {
            payer: &accounts.dm,
            program_identity: &accounts.program_identity,
            oracle_queue: &accounts.oracle_queue,
            system_program: &accounts.system_program,
            slot_hashes: &accounts.slot_hashes,
        },
        ctx.bumps.program_identity,
        vrf::caller_seed(&session_key, session.core.action_counter),
        &CallbackAccounts {
            session: session_key,
            monsters: accounts.monsters.key(),
            participation,
            character,
            // Not read for the initiative roll: no one dies of it.
            skill_set: character,
            bag: character,
            target_participation: None,
            layout: session.layout,
        },
    )?;

    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker: EMPTY_UNIT,
        target: EMPTY_UNIT,
        skill_slot: 0,
        action_points: 0,
    });
    Ok(())
}
