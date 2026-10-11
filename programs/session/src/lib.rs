//! Deadroll session: party, turns, combat, VRF callback, escrow.
use anchor_lang::prelude::*;

pub mod auth;
// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod movement;
pub mod rules;
pub mod state;

use instructions::*;
use rules::MAX_PATH;

declare_id!("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");

#[program]
pub mod deadroll_session {
    use super::*;

    /// DM opens a party on a finalized map: session, monsters and escrow; map goes `InSession`.
    pub fn open_session(
        ctx: Context<OpenSession>,
        price_lamports: u64,
        dm_actor: Pubkey,
    ) -> Result<()> {
        open_session::handler(ctx, price_lamports, dm_actor)
    }

    /// Player joins with a character, pays a share of the price into escrow, locks the character.
    pub fn join_session(ctx: Context<JoinSession>, contribution: u64, actor: Pubkey) -> Result<()> {
        join_session::handler(ctx, contribution, actor)
    }

    /// Player marks (or unmarks) ready in the lobby. Main wallet only.
    pub fn set_ready(ctx: Context<SetReady>, ready: bool) -> Result<()> {
        set_ready::handler(ctx, ready)
    }

    /// DM starts the party once everyone is ready and the price is collected.
    pub fn start_session(ctx: Context<StartSession>) -> Result<()> {
        start_session::handler(ctx)
    }

    /// Player moves along a path of grid steps on their turn.
    pub fn move_unit(ctx: Context<MoveUnit>, steps: [u8; MAX_PATH], len: u8) -> Result<()> {
        move_unit::handler(ctx, steps, len)
    }

    /// DM moves a monster along a path on that monster's turn.
    pub fn monster_move(
        ctx: Context<MonsterMove>,
        monster: u8,
        steps: [u8; MAX_PATH],
        len: u8,
    ) -> Result<()> {
        monster_move::handler(ctx, monster, steps, len)
    }

    /// Ends the current unit's turn and starts the next living unit's.
    pub fn end_turn(ctx: Context<EndTurn>) -> Result<()> {
        end_turn::handler(ctx)
    }
}
