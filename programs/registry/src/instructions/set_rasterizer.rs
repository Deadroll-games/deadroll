use anchor_lang::prelude::*;
use deadroll_types::{seeds, GameConfigData, ProgramIdx};

use crate::{
    errors::RegistryError,
    events::RasterizerSet,
    state::{
        pull::{pull, KIND_RASTERIZER, RASTERIZER_DISCRIMINATOR},
        GameConfig,
    },
};

/// The registry pulls from render (`programs/render/README.md`).
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
    point_rasterizer(&mut ctx.accounts.config.core, &ctx.accounts.blob)
}

/// The pull shared with `set_rasterizer_from_render` (the render `finalize_rasterizer` CPI).
pub fn point_rasterizer(cfg: &mut GameConfigData, blob_info: &AccountInfo) -> Result<()> {
    let render = cfg.programs[ProgramIdx::Render as usize];
    let blob = pull(
        blob_info,
        &render,
        seeds::RASTERIZER,
        RASTERIZER_DISCRIMINATOR,
        KIND_RASTERIZER,
    )?;
    cfg.rasterizer_version = blob.id;
    cfg.rasterizer = blob_info.key();
    cfg.rasterizer_sha256 = blob.sha256;
    emit!(RasterizerSet {
        rasterizer: cfg.rasterizer,
        version: blob.id,
        sha256: blob.sha256,
    });
    Ok(())
}
