use anchor_lang::prelude::*;
use deadroll_types::TavernBoardData;

/// `["tavern_board", building]`: the order board of a Tavern (docs/CITY_DESCRIPTION.md 3.3).
#[account]
#[derive(InitSpace)]
pub struct TavernBoard {
    pub core: TavernBoardData,
}
