//! Deadroll character: player profiles, characters, skills, bag.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod rules;
pub mod state;
pub mod trust;

use instructions::*;
use state::RESOURCE_KINDS;

declare_id!("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");

#[program]
pub mod deadroll_character {
    use super::*;

    /// One profile per wallet; it hands out character indexes.
    pub fn create_profile(ctx: Context<CreateProfile>) -> Result<()> {
        create_profile::handler(ctx)
    }

    /// Race, class, predisposition, stats, appearance and up to 6 starting skills, plus the Core
    /// NFT (feature `nft`). Creates `Character`, `SkillSet` and `Bag`.
    pub fn create_character(
        ctx: Context<CreateCharacter>,
        args: CreateCharacterArgs,
    ) -> Result<()> {
        create_character::handler(ctx, args)
    }

    /// Session only (CPI at `join_session`): locks a living character into `session`.
    pub fn lock_for_session(ctx: Context<LockForSession>, session: Pubkey) -> Result<()> {
        lock_for_session::handler(ctx, session)
    }

    /// Session only: adds XP (level-ups) and resources to a locked, living character.
    pub fn grant_rewards(
        ctx: Context<GrantRewards>,
        xp: u32,
        resources: [u32; RESOURCE_KINDS],
    ) -> Result<()> {
        grant_rewards::handler(ctx, xp, resources)
    }

    /// Session only: the character dies and becomes a tombstone. No outgoing CPI.
    pub fn mark_dead(ctx: Context<MarkDead>) -> Result<()> {
        mark_dead::handler(ctx)
    }

    /// Session only: releases the character from its session.
    pub fn unlock(ctx: Context<Unlock>) -> Result<()> {
        unlock::handler(ctx)
    }

    /// Permissionless: freezes the Core NFT of a dead character.
    pub fn freeze_tombstone(ctx: Context<FreezeTombstone>) -> Result<()> {
        freeze_tombstone::handler(ctx)
    }
}
