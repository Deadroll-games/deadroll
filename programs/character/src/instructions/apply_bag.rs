use anchor_lang::prelude::*;
use deadroll_rules::{
    loot::{check_bag_layout, BagError},
    BalanceConfig,
};
use deadroll_types::{seeds, BagLayout, CacheSlot, CACHE_SLOTS};

use crate::{
    errors::CharacterError,
    events::BagApplied,
    state::{Bag, Character},
    trust,
};

/// CPI from session `close_cache` (patch 11, "Done" in the bag screen): the owner's new bag
/// layout. The session passes the real cache it holds; the client never says what dropped.
/// Every `(kind, id)` of the new bag must come from the old bag and the cache
/// (`deadroll_rules::loot::check_bag_layout`); what is left over burns with the cache.
#[derive(Accounts)]
pub struct ApplyBag<'info> {
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::BAG, character.key().as_ref()],
        bump = bag.bump,
    )]
    pub bag: Box<Account<'info, Bag>>,
}

pub fn handler(
    ctx: Context<ApplyBag>,
    cache: [CacheSlot; CACHE_SLOTS],
    layout: BagLayout,
) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let c = &ctx.accounts.character.core;
    require!(c.alive, CharacterError::CharacterDead);
    // The bag is frozen while the character is in a party.
    require!(c.in_session.is_none(), CharacterError::AlreadyLocked);

    let bag = &mut ctx.accounts.bag;
    check_bag_layout(&bag.layout, &cache, &layout, &BalanceConfig::DEFAULT.loot).map_err(|e| {
        match e {
            BagError::DirtyEmptySlot => CharacterError::BagDirtySlot,
            BagError::WrongSection => CharacterError::BagWrongSection,
            BagError::StackTooBig => CharacterError::BagStackTooBig,
            BagError::MoreThanOwned => CharacterError::BagMoreThanOwned,
        }
    })?;
    bag.layout = layout;

    emit!(BagApplied {
        character: ctx.accounts.character.key(),
    });
    Ok(())
}
