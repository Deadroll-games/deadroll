use anchor_lang::prelude::*;
use deadroll_mapcheck::tiles::TileRules;
use deadroll_rules::{
    arena::{entry_cells, footprint_cells, Approach, FirstSide, ARENA_TAKEN_MAX},
    exit::{first_side_guarded, retreat_buff},
    spread::{BurningCell, BURNING_CELLS_MAX},
};
use deadroll_types::{seeds, Cell, SessionStatus, ARENA_MONSTERS_MAX, NO_SPAWN, PARTY_MAX};

use crate::{
    arena::{living_participations, load_monster, store_participations},
    errors::SessionError,
    events::ArenaEntered,
    rules::{Unit, BALANCE},
    state::{external::MapLayoutView, spawn_bit, ArenaMode, GameSession, Monsters},
    turns,
};

#[derive(Accounts)]
pub struct EnterArena<'info> {
    /// Any participant: entering holds no choice (the engaging player's tab sends it with the move).
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's `MapLayout` (address pinned at `open_session`): the pack's spawns and
    /// its arena.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

/// Arena update (docs/PARTY_AND_TURNS.md 4.1.3): the party goes into the arena of the pack a
/// player stepped next to (`move_path` engaged it). The pack's monsters are loaded where the DM put
/// them; the living players take the free cells nearest the edge the engaging player came from;
/// HP, states, buffs and cooldowns stay as they are. From the pack's back the party starts the
/// timeline, otherwise the monsters do. Patch B: after a retreat from this pack (7.5) its killed
/// monsters stay dead, the rest come back with full HP, the pack moves first from any side and
/// starts its next fight with the retreat buff. The players' map cells, the map circle and the
/// round wait for `leave_arena`. `remaining_accounts`: each living player's `Participation`, in
/// slot order.
pub fn handler<'info>(ctx: Context<'info, EnterArena<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    session.require_no_reveals()?;
    require!(session.mode == ArenaMode::Engaged, SessionError::NotEngaged);
    let mut players = living_participations(ctx.remaining_accounts, session)?;
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    let layout = MapLayoutView::load(&accounts.layout, None)?;
    let pack = session.arena_pack;
    let record = *session
        .packs
        .get(pack as usize)
        .ok_or(SessionError::NotEngaged)?;
    // Patch C: the DM reveals the pack's composition first (`reveal_pack`).
    require!(
        session.packs_revealed & (1 << pack) != 0,
        SessionError::PackHidden
    );

    // The pack's monsters, in spawn order; every cell they stand on (patch B: a large one's whole
    // square) is taken.
    let mut taken = [Cell::default(); ARENA_TAKEN_MAX];
    let mut taken_count = 0usize;
    let mut spawns = [u8::MAX; ARENA_MONSTERS_MAX];
    let mut count = 0usize;
    for i in 0..session.spawn_count {
        let spawn = session.spawns[i as usize];
        // Patch B: a monster killed before a retreat stays dead.
        if spawn.pack != pack || session.dead_spawns & spawn_bit(i) != 0 {
            continue;
        }
        require!(count < ARENA_MONSTERS_MAX, SessionError::InvalidLayout);
        load_monster(session, &monsters, count as u8, i, &spawn)?;
        let square = monsters.footprint(count as u8)?;
        taken_count += footprint_cells(core::iter::once(square), &mut taken[taken_count..]);
        spawns[count] = i;
        count += 1;
    }
    monsters.set_count(count as u8)?;
    // Burning cells are cells of the ground they burn on.
    monsters.set_burning_cells(&[BurningCell::default(); BURNING_CELLS_MAX])?;

    let from = session.player_pos[session.engaged_by as usize];
    let pack_bit = 1u8 << pack;
    // Patch C: an ambush pack has no map cell; the party comes in from the side its facing
    // names and the monsters move first (docs/MAP_DESCRIPTION.md 17.4).
    let (approach, first) = if record.visible == NO_SPAWN {
        (Approach::of_facing(record.facing), FirstSide::Monsters)
    } else {
        let approach = Approach::between(record.cell(), from).ok_or(SessionError::NotEngaged)?;
        // Patch B: a pack the party retreated from is on guard and moves first from any side;
        // its next fight starts with the retreat buff (docs/PARTY_AND_TURNS.md 7.5).
        let first = first_side_guarded(
            record.facing,
            approach,
            session.packs_guarded & pack_bit != 0,
        );
        (approach, first)
    };
    if session.packs_buffed & pack_bit != 0 {
        for slot in 0..count as u8 {
            let mut mods = monsters.mods(slot)?;
            retreat_buff(&mut mods, Unit::Monster(slot).id(), &BALANCE.arena);
            monsters.set_mods(slot, &mods)?;
        }
        session.packs_buffed &= !pack_bit;
    }

    // The map waits.
    session.map_width = session.width;
    session.map_height = session.height;
    session.map_pos = session.player_pos;
    let len = (session.turn_order_len as usize).min(PARTY_MAX);
    session.map_order = [crate::rules::EMPTY_UNIT; PARTY_MAX];
    let order = session.core.turn_order;
    session.map_order[..len].copy_from_slice(&order[..len]);
    session.map_order_len = len as u8;
    session.map_turn_index = session.core.turn_index;
    session.map_round = session.core.round;

    // Every type the `Tileset` lacks has cost 0 in the session copy: it cannot be entered.
    let tiles = TileRules {
        cost: session.tile_cost,
        known: u32::MAX,
        blocks_vision: 0,
    };
    let living = (0..session.core.player_count)
        .filter(|&s| session.player_alive(s))
        .count();
    let mut cells = [Cell::default(); PARTY_MAX];
    let arenas = layout.arenas()?;
    let found = entry_cells(
        &arenas,
        pack,
        &tiles,
        approach,
        &taken[..taken_count],
        &mut cells[..living],
    );
    require!(found == living, SessionError::InvalidLayout);
    let mut k = 0;
    for slot in 0..session.core.player_count {
        if let Some(p) = players[slot as usize].as_mut() {
            p.core.x = cells[k].x;
            p.core.y = cells[k].y;
            session.player_pos[slot as usize] = cells[k];
            k += 1;
        }
    }
    session.width = record.width;
    session.height = record.height;
    session.mode = ArenaMode::Arena;
    drop(layout);

    emit!(ArenaEntered {
        session: session_key,
        pack,
        party_first: first == FirstSide::Party,
        width: record.width,
        height: record.height,
        players: session.player_pos,
        monsters: count as u8,
        spawns,
    });
    turns::enter_timeline(session, session_key, &monsters, first)?;
    store_participations(&players)
}
