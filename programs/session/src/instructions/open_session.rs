use anchor_lang::prelude::*;
use deadroll_combat::ResonanceTables;
use deadroll_registry::state::TableKind;
use deadroll_rules::{
    aura::{monster_passive_aura, Aura, AURAS_PER_PLAYER, PASSIVE_AURA},
    loot::settle_timeout_ok,
    monster::{select_skills, NO_SKILL, PASSIVE_SLOT},
    MonsterTemplate,
};
use deadroll_skillcheck::{monster::POOL_MAX, Category};
use deadroll_types::{
    seeds, ActiveState, GameSessionData, MapStatus, Monster, SessionStatus, Spawn, MAP_MAX,
    MONSTERS_MAX, MONSTER_SLOTS, NO_TABLE, PARTY_MAX, STATES_MAX, TURN_ORDER_MAX,
};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::SessionOpened,
    rules::{BALANCE, EMPTY_UNIT, MIN_PRICE_LAMPORTS},
    state::{
        external::{
            read_bestiary, read_map_full, read_monster_skill, read_reward_table, read_table_body,
            read_tile_costs, MapLayoutView, ASPECTS_TABLE, BESTIARY_TABLE, MONSTER_SKILLS_TABLE,
            RESONANCES_TABLE, REWARDS_TABLE, TILESET_TABLE, TRAITS_TABLE,
        },
        Escrow, GameSession, Monsters, ACCOUNT_VERSION, NO_CHEST, RES_ASPECTS_LEN, RES_PAIRS_LEN,
        RES_TRAITS_LEN,
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
    session.tile_cost = read_tile_costs(&accounts.tileset)?;
    // Patch 11: the map's rewards, fixed for the party.
    session.tier = full.tier;
    session.min_players = full.min_players;
    session.chest_radius = full.chest_radius;
    session.settle_timeout_secs = full.settle_timeout_secs;
    (session.chests, session.chest_count) = layout.chests()?;
    session.boss_table = layout.boss_table();
    session.pending_chest = NO_CHEST;

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
        &monsters,
    )?;
    monsters.clear_loot_tables()?;
    for i in 0..spawn_count {
        let spawn = layout.spawn(i as usize)?;
        let (record, template) = read_bestiary(&accounts.bestiary, spawn.bestiary_id)?;
        let hp = template.hp_at(spawn.level, spawn.flags, &BALANCE.monsters);
        session.monster_speed[i as usize] = template.speed;
        session.monster_initiative[i as usize] = template.initiative;
        monsters.set_record(i, &record)?;
        monsters.set_defense(i, &template.defense)?;
        copy_skills(&monsters, i, &spawn, &template, &accounts.monster_skills)?;
        copy_loot_table(&monsters, &accounts.rewards, template.loot_table)?;
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
    monsters: &Monsters,
) -> Result<()> {
    let aspects: [u8; RES_ASPECTS_LEN] =
        read_table_body(aspects, &ASPECTS_TABLE, TableKind::Aspects)?;
    let traits: [u8; RES_TRAITS_LEN] = read_table_body(traits, &TRAITS_TABLE, TableKind::Traits)?;
    let pairs: [u8; RES_PAIRS_LEN] =
        read_table_body(resonances, &RESONANCES_TABLE, TableKind::Resonances)?;
    let mut tables = Box::<ResonanceTables>::default();
    tables
        .decode_into(&aspects, &traits, &pairs)
        .map_err(|_| error!(SessionError::InvalidRegistryTable))?;
    monsters.set_tables(&aspects, &traits, &pairs)
}

/// The skills spawn `index` took from its template's pool (docs/BESTIARY.md 3.2, MAP_DESCRIPTION.md
/// 7.2): the mask's pool bits, at most the rank's slots (`finalize_map` checked the count; a map
/// finalized before patch 6 has none), copied once into the session table. The passive skill
/// starts its permanent aura here (docs/SKILL_BUILDER.md 3.3).
#[inline(never)]
fn copy_skills(
    monsters: &Monsters,
    index: u8,
    spawn: &Spawn,
    template: &MonsterTemplate,
    table: &AccountInfo,
) -> Result<()> {
    let len = (template.pool_len as usize).min(POOL_MAX);
    let mut pool = [None; POOL_MAX];
    let mut categories = [Category::Active; POOL_MAX];
    for (i, (slot, category)) in pool
        .iter_mut()
        .zip(categories.iter_mut())
        .take(len)
        .enumerate()
    {
        // Only the chosen bits are read: an unchosen pool skill costs nothing.
        if spawn.skills & (1 << i) == 0 {
            continue;
        }
        let skill = read_monster_skill(table, template.pool_start as usize + i)?;
        *category = skill.blocks.category;
        *slot = Some(skill);
    }
    // Unchosen pool entries keep a category that never takes a slot: their mask bit is clear.
    let chosen = select_skills(
        &categories[..len],
        spawn.skills,
        template.slots(spawn.flags, &BALANCE.monsters),
    );
    let mut slots = [NO_SKILL; MONSTER_SLOTS];
    for (slot, &p) in slots.iter_mut().zip(chosen.iter()) {
        if let Some(skill) = (p != NO_SKILL).then(|| pool[p as usize]).flatten() {
            *slot = monsters.add_skill(&skill)?;
        }
    }
    monsters.set_skill_slots(index, &slots)?;
    let mut auras = [Aura::default(); AURAS_PER_PLAYER];
    if let Some(skill) = monsters.skill(index, PASSIVE_SLOT as u8)? {
        let profile = template.profile(spawn.level, &BALANCE.monsters);
        if let Some(a) = monster_passive_aura(BALANCE, &profile, &skill) {
            auras[PASSIVE_AURA] = a;
        }
    }
    monsters.set_auras(index, &auras)
}
