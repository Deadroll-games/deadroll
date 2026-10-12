use anchor_lang::prelude::*;
use deadroll_types::{seeds, ProgramIdx};

use crate::{
    errors::RegistryError,
    events::RasterizerSet,
    state::{
        pull::{pull, KIND_RASTERIZER, RASTERIZER_DISCRIMINATOR},
        GameConfig,
    },
};

/// The registry pulls from render: no CPI (`programs/render/README.md`).
#[derive(Accounts)]
pub struct PullBlob<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::CONFIG],
        bump = config.core.bump,
        constraint = config.core.authority == authority.key() @ RegistryError::Unauthorized,
    )]
    pub config: Account<'info, GameConfig>,
    /// CHECK: owner, discriminator, header and PDA address are checked in `state::pull`.
    pub blob: UncheckedAccount<'info>,
}

/// Points `GameConfig` at a finalized `RasterizerCode`: version, address, SHA-256.
pub fn handler(ctx: Context<PullBlob>) -> Result<()> {
    let cfg = &mut ctx.accounts.config.core;
    let render = cfg.programs[ProgramIdx::Render as usize];
    let blob = pull(
        &ctx.accounts.blob,
        &render,
        seeds::RASTERIZER,
        RASTERIZER_DISCRIMINATOR,
        KIND_RASTERIZER,
    )?;
    cfg.rasterizer_version = blob.id;
    cfg.rasterizer = ctx.accounts.blob.key();
    cfg.rasterizer_sha256 = blob.sha256;
    emit!(RasterizerSet {
        rasterizer: cfg.rasterizer,
        version: blob.id,
        sha256: blob.sha256,
    });
    Ok(())
}
