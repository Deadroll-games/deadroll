use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::config::game_authority;
use crate::errors::RenderError;
use crate::events::BlobInitialized;
use crate::state::*;

/// Creates the `Palette` PDA. Only `GameConfig.authority` may call it.
#[derive(Accounts)]
#[instruction(version: u16, len: u32)]
pub struct InitPalette<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: registry `["config"]` PDA; owner and address checked in `game_authority`.
    pub config: UncheckedAccount<'info>,
    #[account(
        init,
        payer = authority,
        space = initial_space(len),
        seeds = [seeds::PALETTE, &version.to_le_bytes()],
        bump,
    )]
    pub palette: AccountLoader<'info, Palette>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitPalette>, version: u16, len: u32, sha256: [u8; 32]) -> Result<()> {
    check_init_args(KIND_PALETTE, len)?;
    let authority = ctx.accounts.authority.key();
    require_keys_eq!(
        game_authority(&ctx.accounts.config)?,
        authority,
        RenderError::Unauthorized
    );
    let mut account = ctx.accounts.palette.load_init()?;
    let bump = ctx.bumps.palette;
    init_header(
        &mut account.header,
        KIND_PALETTE,
        version,
        len,
        sha256,
        authority,
        bump,
    );
    emit!(BlobInitialized {
        address: ctx.accounts.palette.key(),
        kind: KIND_PALETTE,
        id: version,
        len
    });
    Ok(())
}
