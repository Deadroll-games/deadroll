//! Deadroll character: player profiles, characters (with their gear), skills, bag.
use anchor_lang::prelude::*;

pub mod auth;
// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod rules;
pub mod state;
pub mod trust;

use deadroll_types::EquipSlot;
use instructions::*;
use state::{MAP_SKILLS, RESOURCE_KINDS};

declare_id!("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");

#[program]
pub mod deadroll_character {
    use super::*;

    /// One profile per wallet; it hands out character indexes.
    pub fn create_profile(ctx: Context<CreateProfile>) -> Result<()> {
        create_profile::handler(ctx)
    }

    /// Race, class, predisposition, stats, appearance, up to 6 starting skills and the class
    /// starter items (worn), plus the Core NFT (feature `nft`). Creates `Character`, `SkillSet`
    /// and `Bag`.
    pub fn create_character(
        ctx: Context<CreateCharacter>,
        args: CreateCharacterArgs,
    ) -> Result<()> {
        create_character::handler(ctx, args)
    }

    /// Owner or their session key: wears soulbound item `item_index` in `slot`. Not while the
    /// character is in a session.
    pub fn equip(ctx: Context<Equip>, item_index: u8, slot: EquipSlot) -> Result<()> {
        equip::handler(ctx, item_index, slot)
    }

    /// Owner or their session key: empties `slot`. Not while the character is in a session.
    pub fn unequip(ctx: Context<Unequip>, slot: EquipSlot) -> Result<()> {
        unequip::handler(ctx, slot)
    }

    /// Session only (CPI at `set_ready(true)`): checks the skills taken onto the map and locks
    /// a living character into `session`, which freezes its bag and gear.
    pub fn lock_for_session(
        ctx: Context<LockForSession>,
        session: Pubkey,
        selection: [u8; MAP_SKILLS],
    ) -> Result<()> {
        lock_for_session::handler(ctx, session, selection)
    }

    /// Session only: adds XP (level-ups) and resources to a locked, living character.
    pub fn grant_rewards(
        ctx: Context<GrantRewards>,
        xp: u32,
        resources: [u32; RESOURCE_KINDS],
    ) -> Result<()> {
        grant_rewards::handler(ctx, xp, resources)
    }

    /// Session only: the character dies and becomes a tombstone; its skills, bag and soulbound
    /// items burn. No outgoing CPI.
    pub fn mark_dead(ctx: Context<MarkDead>) -> Result<()> {
        mark_dead::handler(ctx)
    }

    /// Session only: releases the character from its session (`set_ready(false)`, or the end of
    /// the party).
    pub fn unlock(ctx: Context<Unlock>) -> Result<()> {
        unlock::handler(ctx)
    }

    /// Permissionless: freezes the Core NFT of a dead character.
    pub fn freeze_tombstone(ctx: Context<FreezeTombstone>) -> Result<()> {
        freeze_tombstone::handler(ctx)
    }
}
