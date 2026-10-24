use anchor_lang::prelude::*;
use deadroll_types::{seeds, BagLayout, CacheStatus};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::CacheClosed,
    state::{external::read_character, RewardCache},
};

#[derive(Accounts)]
pub struct CloseCache<'info> {
    /// The character's owner, main wallet only: "Done" in the bag screen. Gets the rent back.
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        seeds = [seeds::CACHE, reward_cache.character.as_ref()],
        bump = reward_cache.bump,
    )]
    pub reward_cache: Box<Account<'info, RewardCache>>,
    /// CHECK: the cache's `Character`, decoded with owner and discriminator checks.
    #[account(address = reward_cache.character @ SessionError::InvalidRewardCache)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `Bag`, checked by `character::apply_bag` (seeds).
    #[account(mut)]
    pub bag: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

/// Patch 11, "Done" in the bag screen: the owner's final bag `layout` is applied against the
/// real cache and bag (`character::apply_bag`: every reward of the new bag comes from the old bag
/// and this cache, stacks and sections hold), what was not taken burns and the cache closes, its
/// rent back to the owner. A dead character's cache (always empty) just closes.
pub fn handler(ctx: Context<CloseCache>, layout: BagLayout) -> Result<()> {
    let accounts = ctx.accounts;
    let character = read_character(&accounts.character)?;
    require_keys_eq!(
        character.owner,
        accounts.owner.key(),
        SessionError::NotCacheOwner
    );
    let cache = &accounts.reward_cache;
    require!(
        cache.status == CacheStatus::Claimed,
        SessionError::CacheNotClaimed
    );
    let applied = character.alive;
    if applied {
        cpi_out::character::apply_bag(
            &SessionSigner {
                session_authority: &accounts.session_authority,
                config: &accounts.config,
                bump: ctx.bumps.session_authority,
            },
            &accounts.character,
            &accounts.bag,
            cache.slots,
            layout,
        )?;
    }
    emit!(CacheClosed {
        character: cache.character,
        session: cache.session,
        applied,
    });
    Ok(())
}
