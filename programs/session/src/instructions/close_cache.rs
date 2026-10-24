use anchor_lang::prelude::*;
use deadroll_types::{seeds, BagLayout, CacheStatus};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::CacheClosed,
    state::{external::read_character, CacheEntry, RewardCache},
};

#[derive(Accounts)]
pub struct CloseCache<'info> {
    /// The character's owner, main wallet only: "Done" in the bag screen. Gets the rent back.
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
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

/// Patch 11, "Done" in the bag screen for party `session`: the owner's final bag `layout` is
/// applied against the real bag and that party's settled entry (`character::apply_bag`: every
/// reward of the new bag comes from the old bag and this cache, stacks and sections hold); what
/// was not taken burns and the entry empties. The character must not be locked in a party
/// (`leave_party` first when its last party ended). Once every entry is empty the account
/// closes, its rent back to the owner (a dead character's empty cache just closes).
pub fn handler(ctx: Context<CloseCache>, session: Pubkey, layout: BagLayout) -> Result<()> {
    let accounts = ctx.accounts;
    let character = read_character(&accounts.character)?;
    require_keys_eq!(
        character.owner,
        accounts.owner.key(),
        SessionError::NotCacheOwner
    );
    let cache = &mut accounts.reward_cache;
    let all_empty = cache.all_empty();
    let entry = cache.entry_mut(&session, CacheStatus::Claimed);
    require!(entry.is_some() || all_empty, SessionError::CacheNotClaimed);
    let slots = entry.map(|e| {
        let slots = e.slots;
        *e = CacheEntry::EMPTY;
        slots
    });
    let applied = character.alive && slots.is_some();
    if let (true, Some(slots)) = (applied, slots) {
        cpi_out::character::apply_bag(
            &SessionSigner {
                session_authority: &accounts.session_authority,
                config: &accounts.config,
                bump: ctx.bumps.session_authority,
            },
            &accounts.character,
            &accounts.bag,
            slots,
            layout,
        )?;
    }
    emit!(CacheClosed {
        character: accounts.character.key(),
        session,
        applied,
    });
    if accounts.reward_cache.all_empty() {
        accounts
            .reward_cache
            .close(accounts.owner.to_account_info())?;
    }
    Ok(())
}
