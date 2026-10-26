use anchor_lang::prelude::*;
use deadroll_types::{seeds, BuildingKind, BuildingStatus, MapStatus, SessionStatus};

use crate::{
    errors::CityError,
    events::QuestPosted,
    reads,
    state::{check_requirement, Building, Quest, TavernBoard, ACCOUNT_VERSION},
};

#[derive(Accounts)]
pub struct PostQuest<'info> {
    /// The DM's main wallet (docs/CITY_DESCRIPTION.md 2).
    #[account(mut)]
    pub dm: Signer<'info>,
    /// CHECK: registry `GameConfig`, checked by `reads::config` (owners of the map and the party).
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    #[account(
        seeds = [seeds::BUILDING, &building.core.id.to_le_bytes()],
        bump = building.core.bump,
    )]
    pub building: Account<'info, Building>,
    #[account(
        mut,
        seeds = [seeds::TAVERN_BOARD, building.key().as_ref()],
        bump = board.core.bump,
    )]
    pub board: Account<'info, TavernBoard>,
    /// CHECK: the party's map, checked by `reads::map`.
    pub map: UncheckedAccount<'info>,
    /// CHECK: the party (session `["session", map]`), checked by `reads::session`.
    pub session: UncheckedAccount<'info>,
    #[account(
        init,
        payer = dm,
        space = 8 + Quest::INIT_SPACE,
        seeds = [seeds::QUEST, session.key().as_ref()],
        bump,
    )]
    pub quest: Account<'info, Quest>,
    pub system_program: Program<'info, System>,
}

/// The DM posts the party they opened (`session.open_session`, usually in the same transaction)
/// on a Tavern board (docs/PARTY_AND_TURNS.md 2, step 2). No CPI: the map and the party are read.
pub fn handler(ctx: Context<PostQuest>) -> Result<()> {
    let accounts = &ctx.accounts;
    let building = &accounts.building.core;
    require!(
        building.status == BuildingStatus::Active,
        CityError::BuildingClosed
    );
    require!(building.kind == BuildingKind::Tavern, CityError::NotATavern);
    require_keys_eq!(building.target, accounts.board.key(), CityError::BadBoard);
    check_requirement(building.requirement)?;

    let dm = accounts.dm.key();
    let config = reads::config(&accounts.config)?;
    let map = reads::map(&accounts.map, &config)?;
    require_keys_eq!(map.core.dm, dm, CityError::NotTheDm);
    require!(
        map.core.status == MapStatus::InSession,
        CityError::MapNotInSession
    );
    let session = reads::session(&accounts.session, &config)?;
    require_keys_eq!(
        session.map,
        accounts.map.key(),
        CityError::SessionMapMismatch
    );
    require_keys_eq!(session.dm, dm, CityError::NotTheDm);
    require!(
        session.status == SessionStatus::Lobby,
        CityError::SessionNotInLobby
    );

    let board = &mut ctx.accounts.board;
    board.core.posted = board.core.posted.saturating_add(1);
    board.core.open = board.core.open.saturating_add(1);
    let quest = &mut ctx.accounts.quest;
    quest.core.version = ACCOUNT_VERSION;
    quest.core.board = board.key();
    quest.core.session = ctx.accounts.session.key();
    quest.core.map = ctx.accounts.map.key();
    quest.core.dm = dm;
    quest.core.tier = map.tier;
    quest.core.biome = map.biome;
    quest.core.min_players = map.min_players;
    quest.core.price_lamports = session.price_lamports;
    quest.core.posted_at = Clock::get()?.unix_timestamp;
    quest.core.bump = ctx.bumps.quest;
    emit!(QuestPosted {
        quest: quest.key(),
        board: board.key(),
        session: quest.core.session,
        map: quest.core.map,
        dm,
        tier: map.tier,
        price_lamports: session.price_lamports,
    });
    Ok(())
}
