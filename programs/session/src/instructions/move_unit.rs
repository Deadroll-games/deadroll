use anchor_lang::prelude::*;
use deadroll_rules::reactive::{entered_next_to, Event};
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::Moved,
    monster_cb::{monster_reaction_from, request_monster_reactive_roll, Field, MonsterEvent},
    movement::{burn_on_path, step, walk_path, Path},
    rules::{Unit, MAX_PATH},
    state::{
        external::MapLayoutView, pending_kind, GameSession, MonsterState, Monsters, Participation,
        PendingMonsterReactiveData,
    },
    vrf::{self, RequestAccounts},
};

#[derive(Accounts)]
pub struct MoveUnit<'info> {
    /// Main wallet or the participation's session key; pays the VRF fee when the move triggers a
    /// monster's reactive skill (patch 6).
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
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in `Monsters`.
    pub monsters: LazyAccount<'info, MonsterState>,
    /// CHECK: the session's `MapLayout`, address pinned at `open_session`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
    // Patch 6: a roll for the monster reactive skills an entering player triggers.
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

/// Moves the player along `steps[..len]` on its turn. Coming next to a monster whose
/// `EnemyAdjacent` reactive skill answers (docs/SKILL_BUILDER.md 3.2, docs/BESTIARY.md 3.2): the
/// player stops on that cell (the rest of the path is not walked, its action points stay) and the
/// move requests a VRF value for the monsters' reactions, like `monster_move` for the players'.
pub fn handler(ctx: Context<MoveUnit>, steps: [u8; MAX_PATH], len: u8) -> Result<()> {
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
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let slot = participation.core.slot;
    let unit = Unit::Player(slot);
    require!(
        session.current_unit() == Some(unit),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    require!(
        session.player_alive(slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );

    let session_key = session.key();
    let monsters = Monsters::load(accounts.monsters.as_ref(), &session_key)?;
    let field = Field::load(session, &monsters)?;
    participation.refresh_ap(session.turn_seq, &field.view());
    let start = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };

    // The first step that brings it next to a monster whose reactive answers: it stops there.
    let mut taken = len.min(MAX_PATH as u8);
    let mut bearers = 0u32;
    let count = monsters.count()?;
    let mut at = start;
    for (k, &dir) in steps[..taken as usize].iter().enumerate() {
        // An invalid step is `walk_path`'s error below.
        let Ok(next) = step(at, dir, session.width, session.height) else {
            break;
        };
        for i in 0..count {
            let m = monsters.get(i)?;
            if !m.alive || !entered_next_to(at, next, Cell { x: m.x, y: m.y }) {
                continue;
            }
            let e = MonsterEvent {
                bearer: i,
                event: Event::EnemyEntered,
                source: Some(slot),
            };
            if monster_reaction_from(&monsters, &e, Some((slot, next)))?.is_some() {
                bearers |= 1 << i;
            }
        }
        at = next;
        if bearers != 0 {
            taken = k as u8 + 1;
            break;
        }
    }

    let layout = MapLayoutView::load(&accounts.layout)?;
    let path = Path {
        steps: &steps,
        len: taken,
    };
    let (end, action_points) = walk_path(
        session,
        &layout,
        &monsters,
        unit,
        start,
        Path {
            steps: &steps,
            len: taken,
        },
        participation.core.action_points,
    )?;
    burn_on_path(
        session,
        session_key,
        &monsters.burning_cells()?,
        unit,
        start,
        &path,
        &mut participation.core.states,
        0,
    )?;

    participation.core.x = end.x;
    participation.core.y = end.y;
    participation.core.action_points = action_points;
    session.player_pos[slot as usize] = end;
    // Moving is another action: every escape intent resets (docs/PARTY_AND_TURNS.md, 7.3).
    session.flee_mask = 0;

    emit!(Moved {
        session: session_key,
        unit: unit.id(),
        x: end.x,
        y: end.y,
        action_points,
    });

    if bearers != 0 {
        request_monster_reactive_roll(
            session,
            session_key,
            PendingMonsterReactiveData {
                kind: pending_kind::ENEMY_ENTERED,
                bearers,
                source: unit.id(),
                ..Default::default()
            },
            slot,
            RequestAccounts {
                payer: &accounts.signer,
                program_identity: &accounts.program_identity,
                oracle_queue: &accounts.oracle_queue,
                system_program: &accounts.system_program,
                slot_hashes: &accounts.slot_hashes,
            },
            ctx.bumps.program_identity,
            accounts.monsters.key(),
        )?;
    }
    Ok(())
}
