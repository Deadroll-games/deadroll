//! Deadroll session: party, turns, combat, VRF callback, escrow.
use anchor_lang::prelude::*;

pub mod auth;
pub mod combat_input;
// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod hit_event;
pub mod instructions;
pub mod line;
pub mod loot;
pub mod monster_cb;
pub mod movement;
pub mod outcomes;
pub mod party;
pub mod reactions;
pub mod rules;
pub mod settlement;
pub mod state;
pub mod turns;
pub mod vrf;

use deadroll_character::state::MAP_SKILLS;
use deadroll_types::BagLayout;
use deadroll_types::Cell;
use instructions::*;

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

    /// Player joins with a character and pays a share of the price into escrow.
    pub fn join_session<'info>(
        ctx: Context<'info, JoinSession<'info>>,
        contribution: u64,
        actor: Pubkey,
    ) -> Result<()> {
        join_session::handler(ctx, contribution, actor)
    }

    /// Player marks (or unmarks) ready in the lobby, main wallet only. Ready takes `selection`
    /// onto the map (4 active, 1 reactive, 1 passive `SkillSet` indexes) and locks the character;
    /// unready unlocks it.
    pub fn set_ready(
        ctx: Context<SetReady>,
        ready: bool,
        selection: [u8; MAP_SKILLS],
    ) -> Result<()> {
        set_ready::handler(ctx, ready, selection)
    }

    /// DM starts the party once everyone is ready and the price is collected: requests the
    /// initiative roll; `consume_randomness` writes the turn order.
    pub fn start_session(ctx: Context<StartSession>) -> Result<()> {
        start_session::handler(ctx)
    }

    /// Starts the current unit's turn, before anything else on it (permissionless): over-time
    /// damage (same death and victory path as hits), Freeze / Stun skip the turn, state
    /// durations and cooldowns count down. A skip or a death ends the turn on its own.
    pub fn start_turn<'info>(ctx: Context<'info, StartTurn<'info>>) -> Result<()> {
        start_turn::handler(ctx)
    }

    /// Patch 13: the actor moves `unit` (a player slot, or a monster of the DM) along `path`, the
    /// cells in order (8 neighbors), on its turn; it spends action points, no rolls. Replaces
    /// `move_unit` and `monster_move`.
    pub fn move_path(ctx: Context<MovePath>, unit: u8, path: Vec<Cell>) -> Result<()> {
        move_path::handler(ctx, unit, path)
    }

    /// Ends the current unit's turn and starts the next living unit's.
    pub fn end_turn(ctx: Context<EndTurn>) -> Result<()> {
        end_turn::handler(ctx)
    }

    /// Player attacks with the weapon (`0xFF`) or one of the active skills taken onto the map:
    /// spends AP, requests VRF.
    pub fn act(ctx: Context<Act>, skill_slot: u8, target: u8) -> Result<()> {
        act::handler(ctx, skill_slot, target)
    }

    /// DM: the current monster attacks a player slot; same VRF path as `act`.
    pub fn monster_act(
        ctx: Context<MonsterAct>,
        monster: u8,
        action: u8,
        target: u8,
    ) -> Result<()> {
        monster_act::handler(ctx, monster, action, target)
    }

    /// VRF callback: resolves the pending attack through `deadroll_combat`, applies HP and
    /// states, death (`character.mark_dead`), boss victory and wipe.
    pub fn consume_randomness<'info>(
        ctx: Context<'info, ConsumeRandomness<'info>>,
        randomness: [u8; 32],
    ) -> Result<()> {
        consume_randomness::handler(ctx, randomness)
    }

    /// Player chooses "Escape" on their turn; the party flees when every living player did.
    pub fn flee(ctx: Context<Flee>) -> Result<()> {
        flee::handler(ctx)
    }

    /// Patch 11: a player opens a visible chest next to them on their turn (AP); the rewards are
    /// rolled in the VRF callback into every living player's cache.
    pub fn open_chest(ctx: Context<OpenChest>, chest: u8) -> Result<()> {
        open_chest::handler(ctx, chest)
    }

    /// Patch 11 (replaces `claim_rewards` and `claim_escrow`): the DM, main wallet only, settles
    /// every player once the party ended (caches become theirs, experience, unlock) and takes the
    /// escrow; the map goes `Finished`.
    pub fn settle_party<'info>(ctx: Context<'info, SettleParty<'info>>) -> Result<()> {
        settle_party::handler(ctx)
    }

    /// Patch 11: after the DM's deadline, a player settles their own character and takes their
    /// contribution back.
    pub fn claim_after_timeout<'info>(ctx: Context<'info, ClaimAfterTimeout<'info>>) -> Result<()> {
        claim_after_timeout::handler(ctx)
    }

    /// Patch 11: the owner releases a living character from its ended party (it may join another
    /// before the party is settled).
    pub fn leave_party(ctx: Context<LeaveParty>) -> Result<()> {
        leave_party::handler(ctx)
    }

    /// Patch 11, "Done" in the bag screen: the owner's bag layout from the settled cache (main
    /// wallet), then the cache closes.
    pub fn close_cache(ctx: Context<CloseCache>, session: Pubkey, layout: BagLayout) -> Result<()> {
        close_cache::handler(ctx, session, layout)
    }

    /// Patch 10: binds another temporary key to the party, main wallet only (the player's with
    /// its participation, the DM's without one); the old key stops passing.
    pub fn set_session_key(ctx: Context<SetSessionKey>, session_key: Pubkey) -> Result<()> {
        set_session_key::handler(ctx, session_key)
    }
}
