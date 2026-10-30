use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::ChestOpenRequested,
    party::party_keys,
    rules::{Unit, BALANCE, EMPTY_UNIT},
    state::{ArenaMode, GameSession, MonsterState, Monsters, Participation, PendingAction},
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts, Session)]
pub struct OpenChest<'info> {
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
    /// CHECK: the participation's `Character`, passed on to the callback (its fixed list).
    #[account(address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `SkillSet` (address from the seeds), passed on to the callback.
    #[account(
        seeds = [seeds::SKILLS, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: the character's `Bag` (address from the seeds), passed on to the callback.
    #[account(
        seeds = [seeds::BAG, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub bag: UncheckedAccount<'info>,
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in `Monsters`.
    pub monsters: LazyAccount<'info, MonsterState>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = participation.player)]
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
    /// CHECK: the session's `MapLayout`, passed on to the callback (its fixed list).
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

fn chebyshev(a: Cell, b: Cell) -> u8 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

/// Patch 11 (decisions of patch 11, docs/PARTY_AND_TURNS.md 5.1): on their turn a player opens
/// visible chest `chest` from a cell next to it (Chebyshev 1) for `LootTuning.chest_open_ap`
/// action points, if no living monster stands within the map's `chest_radius` (set by the DM)
/// of the chest or of any living player of the party. An interaction, not the turn's one action. Each chest opens once a party; the
/// rewards are rolled with a VRF value of their own (`consume_randomness`, `crate::loot`).
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<OpenChest>, chest: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
    auth::check_bound(
        accounts.signer.key,
        &participation.player,
        &participation.core.session_key,
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let slot = participation.core.slot;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    // Arena update: chests are on the map.
    require!(session.mode == ArenaMode::Map, SessionError::InArena);
    require!(
        session.player_alive(slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );

    let point = *session
        .chest_list()
        .get(chest as usize)
        .ok_or(SessionError::InvalidChest)?;
    require!(
        session.chests_opened & (1 << chest) == 0,
        SessionError::ChestOpened
    );
    let at = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    let cell = point.cell();
    require!(chebyshev(at, cell) == 1, SessionError::ChestNotAdjacent);
    let monsters = Monsters::load(accounts.monsters.as_ref(), &session.key())?;
    let radius = session.chest_radius;
    // Arena update: on the map the living monsters are the packs' visible ones.
    for c in session.visible_monsters() {
        let near_player = (0..session.core.player_count).any(|p| {
            session.player_alive(p) && chebyshev(c, session.player_pos[p as usize]) <= radius
        });
        require!(
            chebyshev(c, cell) > radius && !near_player,
            SessionError::MonsterNearChest
        );
    }

    // A monster's debuff aura over the player counts for the action points, as in `act`.
    let field = crate::monster_cb::Field::load(session, &monsters)?;
    participation.refresh_ap(session.turn_seq, &field.view());
    participation.core.action_points = participation
        .core
        .action_points
        .checked_sub(BALANCE.loot.chest_open_ap)
        .ok_or(SessionError::NotEnoughActionPoints)?;
    // Any action resets every escape intent (docs/PARTY_AND_TURNS.md, 7.3).
    session.flee_mask = 0;

    session.pending = PendingAction {
        attacker: Unit::Player(slot).id(),
        target: EMPTY_UNIT,
        skill_slot: 0,
        participation: participation.key(),
    };
    session.pending_target = Pubkey::default();
    session.pending_chest = chest;
    session.core.pending_vrf = true;

    let session_key = session.key();
    let (party, party_len) = party_keys(session, slot);
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
            bag: accounts.bag.key(),
            target_participation: None,
            layout: session.layout,
            party,
            party_len,
        },
    )?;

    emit!(ChestOpenRequested {
        session: session_key,
        chest,
        slot,
        action: session.core.action_counter,
        action_points: participation.core.action_points,
    });
    Ok(())
}
