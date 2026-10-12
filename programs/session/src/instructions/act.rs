use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    combat_input::{player_form, skill_use, SkillUse},
    errors::SessionError,
    events::ActionRequested,
    rules::{in_range, Unit, ACT_AP, BASE_ATTACK},
    state::{
        external::read_skill, GameSession, MonsterState, Monsters, Participation, PendingAction,
    },
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts)]
pub struct Act<'info> {
    /// Main wallet or the participation's session key; pays the VRF fee.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// CHECK: the participation's `Character`, decoded by the callback (passed on to it here).
    #[account(address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `SkillSet`, checked by `read_skill` (any account for `BASE_ATTACK`).
    pub skill_set: UncheckedAccount<'info>,
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in `Monsters`.
    pub monsters: LazyAccount<'info, MonsterState>,
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

/// `skill_slot`: a slot of the character's `SkillSet`, or `BASE_ATTACK` for the weapon.
/// `target`: a monster index.
pub fn handler(ctx: Context<Act>, skill_slot: u8, target: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
    auth::check_actor(
        &accounts.signer,
        &participation.player,
        &participation.core.actor,
        accounts.session_token.as_ref(),
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    let slot = participation.core.slot;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(
        session.player_alive(slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );
    // One attack or active skill per turn; movement spends what AP are left (PARTY_AND_TURNS 5.1).
    require!(!session.acted(), SessionError::AlreadyActed);

    let monsters = Monsters::load(accounts.monsters.as_ref(), &session.key())?;
    require!(target < monsters.count()?, SessionError::InvalidTarget);
    let monster = monsters.get(target)?;
    require!(monster.alive, SessionError::InvalidTarget);

    let skill = if skill_slot == BASE_ATTACK {
        None
    } else {
        let skill = read_skill(
            &accounts.skill_set,
            &participation.core.character,
            skill_slot,
        )?;
        require!(
            skill_use(&skill) == SkillUse::Ok,
            SessionError::InvalidSkill
        );
        Some(skill)
    };
    let from = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    let to = Cell {
        x: monster.x,
        y: monster.y,
    };
    require!(
        in_range(player_form(skill.as_ref()), from, to),
        SessionError::OutOfRange
    );

    participation.refresh_ap(session.turn_seq);
    participation.core.action_points = participation
        .core
        .action_points
        .checked_sub(ACT_AP)
        .ok_or(SessionError::NotEnoughActionPoints)?;
    // Attacking is another action: every escape intent resets (docs/PARTY_AND_TURNS.md, 7.3).
    session.flee_mask = 0;

    let attacker = Unit::Player(slot).id();
    let target_unit = Unit::Monster(target).id();
    session.pending = PendingAction {
        attacker,
        target: target_unit,
        skill_slot,
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
            skill_set: accounts.skill_set.key(),
        },
    )?;

    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker,
        target: target_unit,
        skill_slot,
        action_points: participation.core.action_points,
    });
    Ok(())
}
