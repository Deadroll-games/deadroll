use anchor_lang::prelude::*;
use deadroll_combat::rooted;
use deadroll_mapcheck::{
    footprint::Footprint,
    walk::{walk, walk_sized},
};
use deadroll_rules::reactive::{entered_next_to, Event};
use deadroll_types::{seeds, Cell, SessionStatus, ARENA_MONSTERS_MAX};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::{Moved, PackEngaged},
    monster_cb::{monster_reaction_from, request_monster_reactive_roll, Field, MonsterEvent},
    movement::{burn_on_path, walk_error, MapField},
    reactions::{fires_for, request_reactive_roll},
    rules::{Unit, MAX_PATH},
    state::{
        external::MapLayoutView, pending_kind, ArenaMode, GameSession, Monsters, Participation,
        PendingMonsterReactiveData, PendingReactiveData,
    },
    vrf::{self, RequestAccounts},
};

#[derive(Accounts, Session)]
pub struct MovePath<'info> {
    /// The actor: for a player its main wallet or its session key, for a monster the DM's main
    /// wallet or the DM's session key. Pays the VRF fee when the move triggers a reactive skill.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// The moving player's `Participation`; none when a monster moves.
    #[account(mut, constraint = participation.core.game_session == session.key() @ SessionError::ActorMismatch)]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place for a
    /// monster's move; a mutable `LazyAccount` would re-serialize the whole account on exit).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's `MapLayout`, address pinned at `open_session`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// MagicBlock `SessionTokenV2` of the actor's session key; none when the main wallet signs.
    #[session(signer = signer, authority = actor_authority())]
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

impl MovePath<'_> {
    /// Whose wallet acts: the player of the participation, else the DM (its monsters).
    pub fn actor_authority(&self) -> Pubkey {
        match &self.participation {
            Some(p) => p.player,
            None => self.session.core.dm,
        }
    }
}

/// Moves `unit` (`Unit::id`: a player slot, or `MONSTER_UNIT_BASE + monster`) along `path`, cells
/// in order, on its turn (docs/ARCHITECTURE.md 6: "actor; spends action points; no rolls"). Each
/// cell neighbors the one before (diagonals too, never cutting the corner of an impassable tile) and
/// costs its tile type's step cost; the checks are
/// `deadroll_mapcheck::walk`.
///
/// Coming next to an enemy whose `EnemyAdjacent` reactive skill answers (docs/SKILL_BUILDER.md 3.2,
/// docs/BESTIARY.md 3.2), the mover stops on that cell (the rest of the path is not walked, its
/// action points stay) and the move requests a VRF value for those reactions: the reactive skill
/// rolls, the move does not.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.actor_authority(),
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<MovePath>, unit: u8, path: Vec<Cell>) -> Result<()> {
    require!(
        !path.is_empty() && path.len() <= MAX_PATH,
        SessionError::InvalidPathLength
    );
    let mover = Unit::from_id(unit).ok_or(SessionError::ActorMismatch)?;
    let accounts = ctx.accounts;
    let session = &accounts.session;
    match (&accounts.participation, mover) {
        (Some(p), Unit::Player(slot)) => {
            require!(p.core.slot == slot, SessionError::ActorMismatch);
            auth::check_bound(accounts.signer.key, &p.player, &p.core.session_key)?;
        }
        (None, Unit::Monster(_)) => {
            auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
        }
        _ => return err!(SessionError::ActorMismatch),
    }
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let session_key = session.key();
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    if let Unit::Monster(index) = mover {
        require!(index < monsters.count()?, SessionError::InvalidMonster);
    }
    require!(
        session.current_unit() == Some(mover),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    session.require_in_play()?;
    let layout = MapLayoutView::load(&accounts.layout, session.ground())?;
    let request = RequestAccounts {
        payer: &accounts.signer,
        program_identity: &accounts.program_identity,
        oracle_queue: &accounts.oracle_queue,
        system_program: &accounts.system_program,
        slot_hashes: &accounts.slot_hashes,
    };
    let bump = ctx.bumps.program_identity;
    let monsters_key = accounts.monsters.key();
    let session = &mut accounts.session;

    match mover {
        Unit::Player(slot) => {
            let p = accounts
                .participation
                .as_mut()
                .ok_or(SessionError::ActorMismatch)?;
            require!(
                session.player_alive(slot) && p.core.hp > 0,
                SessionError::UnitDead
            );
            let field = Field::load(session, &monsters)?;
            p.refresh_ap(session.turn_seq, &field.view());
            require!(!rooted(&p.combat_mods()), SessionError::Rooted);
            let start = Cell {
                x: p.core.x,
                y: p.core.y,
            };
            // The first step that brings it next to a monster whose reactive answers. Arena
            // update: on the map, the first step next to a pack's visible monster engages the pack
            // (docs/PARTY_AND_TURNS.md 4.1.3); there are no live monsters there to react.
            let count = monsters.count()?;
            // Patch B: every monster's cells (a large one's whole square).
            let mut squares = [Footprint::cell(Cell::default()); ARENA_MONSTERS_MAX];
            for (i, f) in squares.iter_mut().enumerate().take(count as usize) {
                *f = monsters.footprint(i as u8)?;
            }
            let mut bearers = 0u32;
            let mut failed = None;
            let mut engaged = None;
            let walked = walk(
                &MapField::new(session, &layout, &monsters, mover)?,
                start,
                &path,
                p.core.action_points,
                |from, to| {
                    if session.ground().is_none() {
                        engaged = session.pack_next_to(to);
                        return engaged.is_some();
                    }
                    for i in 0..count {
                        let m = match monsters.get(i) {
                            Ok(m) => m,
                            Err(e) => {
                                failed = Some(e);
                                return true;
                            }
                        };
                        let square = squares.get(i as usize).copied();
                        let near = square.is_some_and(|f| entered_next_to(from, to, f));
                        if !m.alive || !near {
                            continue;
                        }
                        let e = MonsterEvent {
                            bearer: i,
                            event: Event::EnemyEntered,
                            source: Some(slot),
                        };
                        match monster_reaction_from(&monsters, &e, Some((slot, to))) {
                            Ok(Some(_)) => bearers |= 1 << i,
                            Ok(None) => {}
                            Err(e) => {
                                failed = Some(e);
                                return true;
                            }
                        }
                    }
                    bearers != 0
                },
            )
            .map_err(walk_error)?;
            if let Some(e) = failed {
                return Err(e);
            }
            burn_on_path(
                session,
                session_key,
                &monsters.burning_cells()?,
                mover,
                1,
                &path[..walked.steps],
                &mut p.core.states,
                0,
            );
            p.core.x = walked.end.x;
            p.core.y = walked.end.y;
            p.core.action_points = walked.ap_left;
            session.player_pos[slot as usize] = walked.end;
            // Moving is another action: every escape intent resets (docs/PARTY_AND_TURNS.md, 7.3).
            session.flee_mask = 0;
            emit!(Moved {
                session: session_key,
                unit,
                x: walked.end.x,
                y: walked.end.y,
                action_points: walked.ap_left,
            });
            if let Some(pack) = engaged {
                // The map waits: `enter_arena` takes the party into the pack's arena.
                session.mode = ArenaMode::Engaged;
                session.arena_pack = pack;
                session.engaged_by = slot;
                emit!(PackEngaged {
                    session: session_key,
                    pack,
                    slot,
                    x: walked.end.x,
                    y: walked.end.y,
                });
            }
            if bearers != 0 {
                request_monster_reactive_roll(
                    session,
                    session_key,
                    PendingMonsterReactiveData {
                        kind: pending_kind::ENEMY_ENTERED,
                        bearers,
                        source: unit,
                        ..Default::default()
                    },
                    slot,
                    request,
                    bump,
                    monsters_key,
                )?;
            }
        }
        Unit::Monster(index) => {
            let mut state = monsters.get(index)?;
            require!(state.alive, SessionError::UnitDead);
            require!(!rooted(&monsters.mods(index)?), SessionError::Rooted);
            let start = Cell {
                x: state.x,
                y: state.y,
            };
            // The first step that brings it next to a player whose reactive answers. Patch B: a
            // large monster walks with its whole square (its cell is the top-left one).
            let size = monsters.size(index)?;
            let mut bearers = 0u8;
            let walked = walk_sized(
                &MapField::new(session, &layout, &monsters, mover)?,
                start,
                size,
                &path,
                state.action_points,
                |from, to| {
                    let (from, to) = (Footprint::new(from, size), Footprint::new(to, size));
                    for b in 0..session.core.player_count {
                        let bearer = session.player_pos[b as usize];
                        if entered_next_to(from, to, bearer)
                            && fires_for(session, b, &Event::EnemyEntered, Some((index, to)))
                        {
                            bearers |= 1 << b;
                        }
                    }
                    bearers != 0
                },
            )
            .map_err(walk_error)?;
            let immune = monsters.defense(index)?.immune_states;
            burn_on_path(
                session,
                session_key,
                &monsters.burning_cells()?,
                mover,
                size,
                &path[..walked.steps],
                &mut state.states,
                immune,
            );
            state.x = walked.end.x;
            state.y = walked.end.y;
            state.action_points = walked.ap_left;
            monsters.set(index, &state)?;
            emit!(Moved {
                session: session_key,
                unit,
                x: walked.end.x,
                y: walked.end.y,
                action_points: walked.ap_left,
            });
            if bearers != 0 {
                request_reactive_roll(
                    session,
                    session_key,
                    PendingReactiveData {
                        kind: pending_kind::ENEMY_ENTERED,
                        bearers,
                        source: unit,
                        ..Default::default()
                    },
                    request,
                    bump,
                    monsters_key,
                )?;
            }
        }
    }
    Ok(())
}
