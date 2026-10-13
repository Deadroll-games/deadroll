use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::ActionRequested,
    rules::{in_range, monster_profile, Unit, ACT_AP, NATURAL_ATTACK},
    state::{GameSession, Monsters, Participation, PendingAction},
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts)]
pub struct MonsterAct<'info> {
    /// DM main wallet or the DM's session key; pays the VRF fee.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// The target player's participation.
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// CHECK: the target's `Character`; the callback marks it dead at 0 HP.
    #[account(address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
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

/// Monster `monster` attacks the player in `target` (a slot) with `action`
/// (`NATURAL_ATTACK` is the only P0 action; TODO(post-demo): skills from the spawn's pool).
pub fn handler(ctx: Context<MonsterAct>, monster: u8, action: u8, target: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    auth::check_actor(
        &accounts.signer,
        &session.core.dm,
        &session.dm_actor,
        accounts.session_token.as_ref(),
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(action == NATURAL_ATTACK, SessionError::InvalidAction);
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    require!(monster < monsters.count()?, SessionError::InvalidMonster);
    require!(
        session.current_unit() == Some(Unit::Monster(monster)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    let mut state = monsters.get(monster)?;
    require!(state.alive, SessionError::UnitDead);
    // One attack per turn, like players (PARTY_AND_TURNS 5.1).
    require!(!session.acted(), SessionError::AlreadyActed);

    let participation = &accounts.participation;
    require!(
        participation.core.slot == target
            && session.player_alive(target)
            && participation.core.hp > 0,
        SessionError::InvalidTarget
    );
    let profile =
        monster_profile(state.bestiary_id, state.level).ok_or(SessionError::InvalidMonster)?;
    let from = Cell {
        x: state.x,
        y: state.y,
    };
    let to = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    require!(in_range(profile.form, from, to), SessionError::OutOfRange);

    state.action_points = state
        .action_points
        .checked_sub(ACT_AP)
        .ok_or(SessionError::NotEnoughActionPoints)?;
    monsters.set(monster, &state)?;

    let attacker = Unit::Monster(monster).id();
    let target_unit = Unit::Player(target).id();
    session.pending = PendingAction {
        attacker,
        target: target_unit,
        skill_slot: action,
        participation: participation.key(),
    };
    session.core.pending_vrf = true;
    session.acted_seq = session.turn_seq;

    let session_key = session.key();
    vrf::request(
        RequestAccounts {
            payer: &accounts.signer,
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
            participation: participation.key(),
            character: accounts.character.key(),
            // Not read when a monster attacks; the list stays fixed.
            skill_set: deadroll_character::ID,
            target_participation: None,
        },
    )?;

    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker,
        target: target_unit,
        skill_slot: action,
        action_points: state.action_points,
    });
    Ok(())
}
