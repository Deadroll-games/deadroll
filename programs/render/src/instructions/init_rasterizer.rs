use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::config::game_authority;
use crate::errors::RenderError;
use crate::events::BlobInitialized;
use crate::state::*;

/// Creates the `RasterizerCode` PDA with the first 10 KB at most; `extend` grows it to `len`.
/// Only `GameConfig.authority` may call it.
#[derive(Accounts)]
#[instruction(version: u16, len: u32)]
pub struct InitRasterizer<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: registry `["config"]` PDA; owner and address checked in `game_authority`.
    pub config: UncheckedAccount<'info>,
    #[account(
        init,
        payer = authority,
        space = initial_space(len),
        seeds = [seeds::RASTERIZER, &version.to_le_bytes()],
        bump,
    )]
    pub rasterizer: AccountLoader<'info, RasterizerCode>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitRasterizer>,
    version: u16,
    len: u32,
    sha256: [u8; 32],
) -> Result<()> {
    check_init_args(KIND_RASTERIZER, len)?;
    let authority = ctx.accounts.authority.key();
    require_keys_eq!(
        game_authority(&ctx.accounts.config)?,
        authority,
        RenderError::Unauthorized
    );
    let mut account = ctx.accounts.rasterizer.load_init()?;
    let bump = ctx.bumps.rasterizer;
    init_header(
        &mut account.header,
        KIND_RASTERIZER,
        version,
        len,
        sha256,
        authority,
        bump,
    );
    emit!(BlobInitialized {
        address: ctx.accounts.rasterizer.key(),
        kind: KIND_RASTERIZER,
        id: version,
        len
    });
    Ok(())
}
