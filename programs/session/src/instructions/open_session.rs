use anchor_lang::prelude::*;
use deadroll_types::{
    seeds, ActiveState, GameSessionData, MapStatus, Monster, SessionStatus, MAP_MAX, MONSTERS_MAX,
    PARTY_MAX, STATES_MAX, TURN_ORDER_MAX,
};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::SessionOpened,
    rules::{monster_hp, EMPTY_UNIT, MIN_PRICE_LAMPORTS},
    state::{
        external::{read_map, MapLayoutView},
        Escrow, GameSession, Monsters, ACCOUNT_VERSION,
    },
};

#[derive(Accounts)]
pub struct OpenSession<'info> {
    #[account(mut)]
    pub dm: Signer<'info>,
    /// CHECK: `deadroll_map` `Map`, owner and dm checked in the handler; written by the map CPI.
    #[account(mut)]
    pub map: UncheckedAccount<'info>,
    /// CHECK: `deadroll_map` `MapLayout`, owner and `map` field checked in the handler.
    pub layout: UncheckedAccount<'info>,
    #[account(
        init,
        payer = dm,
        space = 8 + GameSession::INIT_SPACE,
        seeds = [seeds::SESSION, map.key().as_ref()],
        bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` PDA, created in the handler (see `state::Monsters`).
    #[account(mut, seeds = [seeds::MONSTERS, session.key().as_ref()], bump)]
    pub monsters: UncheckedAccount<'info>,
    #[account(
        init,
        payer = dm,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the map program.
    #[account(address = deadroll_map::ID)]
    pub map_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<OpenSession>, price_lamports: u64, dm_actor: Pubkey) -> Result<()> {
    let accounts = ctx.accounts;
    let map = read_map(&accounts.map)?;
    require_keys_eq!(map.dm, accounts.dm.key(), SessionError::NotDm);
    require!(
        map.status == MapStatus::Finalized,
        SessionError::MapNotFinalized
    );
    require!(
        map.width as usize <= MAP_MAX && map.height as usize <= MAP_MAX,
        SessionError::InvalidMap
    );
    require!(
        price_lamports >= MIN_PRICE_LAMPORTS,
        SessionError::PriceTooLow
    );

    let layout = MapLayoutView::load(&accounts.layout)?;
    require_keys_eq!(
        layout.map(),
        accounts.map.key(),
        SessionError::InvalidLayout
    );
    let spawn_count = layout.spawn_count();
    require!(
        spawn_count as usize <= MONSTERS_MAX,
        SessionError::InvalidLayout
    );

    let session_key = accounts.session.key();
    let session = &mut accounts.session;
    session.core = GameSessionData {
        version: ACCOUNT_VERSION,
        map: accounts.map.key(),
        dm: accounts.dm.key(),
        status: SessionStatus::Lobby,
        price_lamports,
        players: [Pubkey::default(); PARTY_MAX],
        player_count: 0,
        turn_order: [EMPTY_UNIT; TURN_ORDER_MAX],
        turn_index: 0,
        round: 0,
        action_counter: 0,
        pending_vrf: false,
        last_seed: [0; 32],
        bump: ctx.bumps.session,
    };
    session.layout = accounts.layout.key();
    session.width = map.width;
    session.height = map.height;
    session.entrance = layout.entrance();
    session.dm_actor = dm_actor;

    let monsters = Monsters::create(
        &accounts.monsters,
        &accounts.dm,
        &accounts.system_program,
        &session_key,
        ctx.bumps.monsters,
    )?;
    for i in 0..spawn_count {
        let spawn = layout.spawn(i as usize)?;
        let hp = monster_hp(spawn.bestiary_id, spawn.level);
        monsters.set(
            i,
            &Monster {
                bestiary_id: spawn.bestiary_id,
                level: spawn.level,
                x: spawn.x,
                y: spawn.y,
                hp,
                hp_max: hp,
                action_points: 0,
                alive: true,
                pack: spawn.pack,
                flags: spawn.flags,
                states: [ActiveState::default(); STATES_MAX],
            },
        )?;
    }
    monsters.set_count(spawn_count)?;
    drop(layout);

    let escrow = &mut accounts.escrow;
    escrow.version = ACCOUNT_VERSION;
    escrow.session = session_key;
    escrow.collected = 0;
    escrow.bump = ctx.bumps.escrow;
    escrow.claimed = false;

    cpi_out::map::set_status(
        &SessionSigner {
            session_authority: &accounts.session_authority,
            config: &accounts.config,
            bump: ctx.bumps.session_authority,
        },
        &accounts.map,
        MapStatus::InSession,
    )?;

    emit!(SessionOpened {
        session: session_key,
        map: accounts.map.key(),
        dm: accounts.dm.key(),
        price_lamports,
        monsters: spawn_count,
    });
    Ok(())
}
