use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    errors::CityError,
    events::QuestClosed,
    reads,
    state::{Quest, TavernBoard},
};

#[derive(Accounts)]
pub struct CloseQuest<'info> {
    /// The quest's DM; gets the rent back.
    #[account(mut)]
    pub dm: Signer<'info>,
    /// CHECK: registry `GameConfig`, checked by `reads::config` (owner of the party).
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::TAVERN_BOARD, board.core.building.as_ref()],
        bump = board.core.bump,
    )]
    pub board: Account<'info, TavernBoard>,
    #[account(
        mut,
        close = dm,
        seeds = [seeds::QUEST, quest.core.session.as_ref()],
        bump = quest.core.bump,
        constraint = quest.core.dm == dm.key() @ CityError::NotTheDm,
        constraint = quest.core.board == board.key() @ CityError::BadBoard,
    )]
    pub quest: Account<'info, Quest>,
    /// CHECK: the quest's party, read by `reads::session` unless it no longer exists.
    #[account(address = quest.core.session @ CityError::InvalidSession)]
    pub session: UncheckedAccount<'info>,
}

/// The DM takes a quest off the board once the party left the lobby, or while nobody joined.
pub fn handler(ctx: Context<CloseQuest>) -> Result<()> {
    let session = &ctx.accounts.session;
    let gone = session.lamports() == 0 && session.owner == &System::id();
    if !gone {
        let config = reads::config(&ctx.accounts.config)?;
        let party = reads::session(session, &config)?;
        require!(
            party.status != SessionStatus::Lobby || party.player_count == 0,
            CityError::QuestStillOpen
        );
    }
    let board = &mut ctx.accounts.board;
    board.core.open = board.core.open.saturating_sub(1);
    let quest = &ctx.accounts.quest;
    emit!(QuestClosed {
        quest: quest.key(),
        session: quest.core.session,
        dm: quest.core.dm,
    });
    Ok(())
}
