use anchor_lang::prelude::*;
use deadroll_types::seeds;

#[cfg(feature = "nft")]
use crate::cpi_out::nft;
use crate::{
    cpi_out::nft::MPL_CORE_ID, errors::CharacterError, events::TombstoneFrozen, state::Character,
};

/// Permissionless: after `CharacterDied` anyone (the app does it automatically) sets the
/// permanent freeze on the dead character's Core NFT. Split from `mark_dead` so the death path
/// inside the VRF callback stays at depth 2 (docs/ARCHITECTURE.md, 4.4).
#[derive(Accounts)]
pub struct FreezeTombstone<'info> {
    #[account(
        mut,
        seeds = [seeds::CHARACTER, character.core.owner.as_ref(), &[character.core.index]],
        bump = character.core.bump,
    )]
    pub character: Box<Account<'info, Character>>,
    /// CHECK: the character's Core asset, key checked against `character.nft`. Never
    /// deserialized (mpl-core's deserializers overflow the stack frame).
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: mpl-core program, address checked.
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<FreezeTombstone>) -> Result<()> {
    let c = &ctx.accounts.character;
    require!(!c.core.alive, CharacterError::StillAlive);
    let nft = c.core.nft.ok_or(CharacterError::NoNft)?;
    require_keys_eq!(nft, ctx.accounts.asset.key(), CharacterError::AssetMismatch);
    require!(!c.nft_frozen, CharacterError::AlreadyFrozen);

    freeze(&ctx)?;

    ctx.accounts.character.nft_frozen = true;
    emit!(TombstoneFrozen {
        character: ctx.accounts.character.key(),
        asset: nft
    });
    Ok(())
}

#[cfg(feature = "nft")]
fn freeze(ctx: &Context<FreezeTombstone>) -> Result<()> {
    let a = &ctx.accounts;
    let core = &a.character.core;
    nft::freeze(
        nft::NftAccounts {
            mpl_core: &a.mpl_core_program.to_account_info(),
            asset: &a.asset.to_account_info(),
            payer: &a.payer.to_account_info(),
            system_program: &a.system_program.to_account_info(),
        },
        &a.character.to_account_info(),
        &[
            seeds::CHARACTER,
            core.owner.as_ref(),
            &[core.index],
            &[core.bump],
        ],
    )
}

/// FALLBACK(nft): no NFT support in this build.
#[cfg(not(feature = "nft"))]
fn freeze(_ctx: &Context<FreezeTombstone>) -> Result<()> {
    err!(CharacterError::NftDisabled)
}
