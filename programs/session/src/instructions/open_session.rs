use anchor_lang::prelude::*;
use deadroll_combat::ResonanceTables;
use deadroll_registry::state::TableKind;
use deadroll_rules::{loot::settle_timeout_ok, MonsterTemplate};
use deadroll_skillcheck::monster::POOL_MAX;
use deadroll_types::{
    seeds, GameSessionData, MapStatus, SessionStatus, Spawn, MAP_MAX, MONSTERS_MAX, NO_TABLE,
    PACKS_MAX, PARTY_MAX, TURN_ORDER_MAX,
};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::SessionOpened,
    rules::{BALANCE, EMPTY_UNIT, MIN_PRICE_LAMPORTS},
    state::{
        external::{
            read_bestiary, read_map_full, read_monster_skill, read_reward_table, read_tile_costs,
            with_table_body, MapLayoutView, ASPECTS_TABLE, BESTIARY_TABLE, MONSTER_SKILLS_TABLE,
            RESONANCES_TABLE, REWARDS_TABLE, TILESET_TABLE, TRAITS_TABLE, TRIPLES_TABLE,
        },
        ArenaMode, Escrow, GameSession, Monsters, ACCOUNT_VERSION, NO_CHEST, NO_PACK,
        RES_ASPECTS_LEN, RES_PAIRS_LEN, RES_TRAITS_LEN, RES_TRIPLES_LEN,
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
    /// CHECK: registry `["bestiary"]` table, checked by `read_bestiary`: every record a spawn names
    /// is copied into `MonsterState.records`.
    #[account(address = BESTIARY_TABLE @ SessionError::InvalidRegistryTable)]
    pub bestiary: UncheckedAccount<'info>,
    /// CHECK: registry `["tileset"]` table, checked by `read_tile_costs`.
    #[account(address = TILESET_TABLE @ SessionError::InvalidRegistryTable)]
    pub tileset: UncheckedAccount<'info>,
    /// CHECK: the map program.
    #[account(address = deadroll_map::ID)]
    pub map_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: registry `["aspects"]` table (patch 4), copied into `MonsterState`.
    #[account(address = ASPECTS_TABLE @ SessionError::InvalidRegistryTable)]
    pub aspects: UncheckedAccount<'info>,
    /// CHECK: registry `["traits"]` table, copied into `MonsterState`.
    #[account(address = TRAITS_TABLE @ SessionError::InvalidRegistryTable)]
    pub traits: UncheckedAccount<'info>,
    /// CHECK: registry `["resonances"]` table, copied into `MonsterState`.
    #[account(address = RESONANCES_TABLE @ SessionError::InvalidRegistryTable)]
    pub resonances: UncheckedAccount<'info>,
    /// CHECK: registry `["monster_skills"]` table (patch 6): the skills the spawns took from their
    /// pools, copied into `MonsterState`.
    #[account(address = MONSTER_SKILLS_TABLE @ SessionError::InvalidRegistryTable)]
    pub monster_skills: UncheckedAccount<'info>,
    /// CHECK: registry `["rewards"]` table (patch 11): the bodies', chests' and boss's reward
    /// tables, copied into `MonsterState`.
    #[account(address = REWARDS_TABLE @ SessionError::InvalidRegistryTable)]
    pub rewards: UncheckedAccount<'info>,
    /// CHECK: registry `["triples"]` table (triples pass), copied into `MonsterState`.
    #[account(address = TRIPLES_TABLE @ SessionError::InvalidRegistryTable)]
    pub triples: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<OpenSession>, price_lamports: u64, dm_actor: Pubkey) -> Result<()> {
    let accounts = ctx.accounts;
    let full = read_map_full(&accounts.map)?;
    let map = full.core;
    // The DM's time to settle: at least 10 minutes outside a localnet test build (the map
    // program checks the maximum).
    require!(
        settle_timeout_ok(
            full.settle_timeout_secs,
            &BALANCE.loot,
            cfg!(feature = "mock-vrf")
        ),
        SessionError::SettleTimeoutTooShort
    );
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

    // Arena update: a map finalized before it has no arenas (`MapWithoutArenas`).
    let layout = MapLayoutView::load(&accounts.layout, None)?;
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
    session.entrance = layout.entrance()?;
    session.dm_actor = dm_actor;
    session.tile_cost = read_tile_costs(&accounts.tileset)?;
    // Patch 11: the map's rewards, fixed for the party.
    session.tier = full.tier;
    session.min_players = full.min_players;
    session.chest_radius = full.chest_radius;
    session.settle_timeout_secs = full.settle_timeout_secs;
    (session.chests, session.chest_count) = layout.chests()?;
    session.boss_table = layout.boss_table();
    session.pending_chest = NO_CHEST;
    // Arena update: the packs on the map (their monsters come into `MonsterState` at
    // `enter_arena`).
    session.mode = ArenaMode::Map;
    session.arena_pack = NO_PACK;
    session.packs_alive = 0;
    for n in 0..PACKS_MAX as u8 {
        let p = layout.pack(n)?;
        session.packs[n as usize] = p;
        if p.is_used() {
            session.packs_alive |= 1 << n;
        }
    }

    let monsters = Monsters::create(
        &accounts.monsters,
        &accounts.dm,
        &accounts.system_program,
        &session_key,
        ctx.bumps.monsters,
    )?;
    copy_tables(
        &accounts.aspects,
        &accounts.traits,
        &accounts.resonances,
        &accounts.triples,
        &monsters,
    )?;
    monsters.clear_loot_tables()?;
    // Arena update: one record per template and the chosen skills, each once; the monsters
    // themselves are loaded pack by pack (`enter_arena`).
    for i in 0..spawn_count {
        let spawn = layout.spawn(i as usize)?;
        let (record, template) = read_bestiary(&accounts.bestiary, spawn.bestiary_id)?;
        monsters.add_template(spawn.bestiary_id, &record)?;
        copy_skills(&monsters, &spawn, &template, &accounts.monster_skills)?;
        copy_loot_table(&monsters, &accounts.rewards, template.loot_table)?;
    }
    monsters.set_count(0)?;
    for id in session
        .chest_list()
        .iter()
        .map(|c| c.data)
        .chain([session.boss_table])
    {
        copy_loot_table(&monsters, &accounts.rewards, id)?;
    }
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

/// Patch 11: copies registry reward table `id` into the party's `MonsterState` once
/// (`NO_TABLE`: nothing). `finalize_map` checked the chests' and the boss's; a body table must
/// decode too, or the kill could not be paid.
#[inline(never)]
fn copy_loot_table(monsters: &Monsters, rewards: &AccountInfo, id: u8) -> Result<()> {
    if id == NO_TABLE || monsters.loot_table(id)?.is_some() {
        return Ok(());
    }
    monsters.add_loot_table(id, &read_reward_table(rewards, id)?)
}

/// Copies the registry resonance tables into the session's `MonsterState` (fixed for the party,
/// read by every hit). They must decode: the callback would refuse them later otherwise.
#[inline(never)]
fn copy_tables(
    aspects: &AccountInfo,
    traits: &AccountInfo,
    resonances: &AccountInfo,
    triples: &AccountInfo,
    monsters: &Monsters,
) -> Result<()> {
    // The four bodies are borrowed from their accounts, never copied onto the 4 KB stack frame.
    let mut tables = Box::<ResonanceTables>::default();
    with_table_body(
        aspects,
        &ASPECTS_TABLE,
        TableKind::Aspects,
        RES_ASPECTS_LEN,
        |a| {
            with_table_body(
                traits,
                &TRAITS_TABLE,
                TableKind::Traits,
                RES_TRAITS_LEN,
                |t| {
                    with_table_body(
                        resonances,
                        &RESONANCES_TABLE,
                        TableKind::Resonances,
                        RES_PAIRS_LEN,
                        |p| {
                            with_table_body(
                                triples,
                                &TRIPLES_TABLE,
                                TableKind::Triples,
                                RES_TRIPLES_LEN,
                                |x| {
                                    tables
                                        .decode_into(a, t, p, x)
                                        .map_err(|_| error!(SessionError::InvalidRegistryTable))?;
                                    monsters.set_tables(a, t, p, x)
                                },
                            )
                        },
                    )
                },
            )
        },
    )
}

/// The skills spawn `spawn` took from its template's pool (docs/BESTIARY.md 3.2,
/// MAP_DESCRIPTION.md 7.2: the mask's pool bits, `finalize_map` checked the count), each copied
/// once into the session table with its registry index. Arena update: `enter_arena` gives the
/// spawn its slots and its passive aura from them (`crate::arena::load_monster`).
#[inline(never)]
fn copy_skills(
    monsters: &Monsters,
    spawn: &Spawn,
    template: &MonsterTemplate,
    table: &AccountInfo,
) -> Result<()> {
    let len = (template.pool_len as usize).min(POOL_MAX);
    for i in 0..len {
        // Only the chosen bits are read: an unchosen pool skill costs nothing.
        if spawn.skills & (1 << i) == 0 {
            continue;
        }
        let id = template.pool_start as usize + i;
        let skill = read_monster_skill(table, id)?;
        monsters.add_skill(&skill, id as u16)?;
    }
    Ok(())
}
