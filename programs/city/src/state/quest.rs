use anchor_lang::prelude::*;
use deadroll_types::QuestData;

/// `["quest", session]`: a DM's order on a Tavern board (docs/PARTY_AND_TURNS.md 2, step 2).
#[account]
#[derive(InitSpace)]
pub struct Quest {
    pub core: QuestData,
}
