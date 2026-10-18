use anchor_lang::prelude::*;
use deadroll_rules::reactive::{entered_next_to, Event};
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::Moved,
    movement::{burn_on_path, step, walk_path, Path},
    reactions::{fires_for, request_reactive_roll},
    rules::{Unit, MAX_PATH},
    state::{external::MapLayoutView, pending_kind, GameSession, Monsters, PendingReactiveData},
    vrf::{self, RequestAccounts},
};

#[derive(Accounts)]
pub struct MonsterMove<'info> {
    /// DM main wallet or the DM's session key; pays the VRF fee when the move triggers a
    /// reactive skill.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place; a
    /// mutable `LazyAccount` would re-serialize the whole account on exit).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's `MapLayout`, address pinned at `open_session`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
    // Patch 5: a roll for the reactive skills an entering monster triggers.
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

/// Moves `monster` along `steps[..len]` on its turn. An enemy entering a cell next to a player
/// triggers that player's `EnemyAdjacent` reactive skill (docs/SKILL_BUILDER.md 3.2): the
/// monster stops on that cell (the rest of the path is not walked, its action points stay) and
/// the move requests a VRF value for the reactions.
pub fn handler(
    ctx: Context<MonsterMove>,
    monster: u8,
    steps: [u8; MAX_PATH],
    len: u8,
) -> Result<()> {
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
    let session_key = session.key();
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    require!(monster < monsters.count()?, SessionError::InvalidMonster);
    let unit = Unit::Monster(monster);
    require!(
        session.current_unit() == Some(unit),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    let mut state = monsters.get(monster)?;
    require!(state.alive, SessionError::UnitDead);

    let start = Cell {
        x: state.x,
        y: state.y,
    };
    // The first step that brings it next to a player whose reactive answers: it stops there.
    let mut taken = len.min(MAX_PATH as u8);
    let mut bearers = 0u8;
    let mut at = start;
    for (k, &dir) in steps[..taken as usize].iter().enumerate() {
        // An invalid step is `walk_path`'s error below.
        let Ok(next) = step(at, dir, session.width, session.height) else {
            break;
        };
        for b in 0..session.core.player_count {
            let bearer = session.player_pos[b as usize];
            if entered_next_to(at, next, bearer)
                && fires_for(session, b, &Event::EnemyEntered, Some((monster, next)))
            {
                bearers |= 1 << b;
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
        state.action_points,
    )?;
    let immune = monsters.defense(monster)?.immune_states;
    burn_on_path(
        session,
        session_key,
        &monsters.burning_cells()?,
        unit,
        start,
        &path,
        &mut state.states,
        immune,
    )?;

    state.x = end.x;
    state.y = end.y;
    state.action_points = action_points;
    monsters.set(monster, &state)?;

    emit!(Moved {
        session: session_key,
        unit: unit.id(),
        x: end.x,
        y: end.y,
        action_points,
    });

    if bearers != 0 {
        request_reactive_roll(
            session,
            session_key,
            PendingReactiveData {
                kind: pending_kind::ENEMY_ENTERED,
                bearers,
                source: unit.id(),
                ..Default::default()
            },
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
