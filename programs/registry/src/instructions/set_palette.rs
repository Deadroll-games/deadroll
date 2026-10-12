use anchor_lang::prelude::*;
use deadroll_types::{seeds, ProgramIdx};

use super::set_rasterizer::PullBlob;
use crate::{
    events::PaletteSet,
    state::pull::{pull, KIND_PALETTE, PALETTE_DISCRIMINATOR},
};

// TODO(post-demo): the same pull for `SpriteBook`; `GameConfigData` has no field for it yet.

/// Points `GameConfig` at a finalized `Palette` (address only: the config has no palette hash).
pub fn handler(ctx: Context<PullBlob>) -> Result<()> {
    let cfg = &mut ctx.accounts.config.core;
    let render = cfg.programs[ProgramIdx::Render as usize];
    pull(
        &ctx.accounts.blob,
        &render,
        seeds::PALETTE,
        PALETTE_DISCRIMINATOR,
        KIND_PALETTE,
    )?;
    cfg.palette = ctx.accounts.blob.key();
    emit!(PaletteSet {
        palette: cfg.palette
    });
    Ok(())
}
