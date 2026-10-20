use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::config::{game_authority, REGISTRY_ID};
use crate::cpi_out::registry;
use crate::errors::RenderError;
use crate::events::RasterizerFinalized;
use crate::state::*;

/// WASM module header: magic `\0asm` and binary version 1.
pub const WASM_MAGIC: &[u8] = b"\0asm\x01\0\0\0";

/// Hashes the body, compares it with the expected SHA-256 from init, checks the WASM header,
/// freezes the account and writes its version, address and hash to `GameConfig`
/// (`docs/STORAGE_AND_DELIVERY.md` 3.2) through `registry::set_rasterizer_from_render`.
#[derive(Accounts)]
pub struct FinalizeRasterizer<'info> {
    /// The blob authority, and the current `GameConfig.authority` (finalize repoints the game).
    pub authority: Signer<'info>,
    #[account(mut)]
    pub rasterizer: AccountLoader<'info, RasterizerCode>,
    /// CHECK: registry `["config"]` PDA; owner and address checked in `game_authority`, written by
    /// the registry.
    #[account(mut)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: this program's `["authority"]` PDA, the signer of the CPI into the registry.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub render_authority: UncheckedAccount<'info>,
    /// CHECK: the registry program, by address.
    #[account(address = REGISTRY_ID @ RenderError::WrongConfig)]
    pub registry_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<FinalizeRasterizer>) -> Result<()> {
    let info = ctx.accounts.rasterizer.to_account_info();
    let mut header = read_header(&info)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    require_keys_eq!(
        game_authority(&ctx.accounts.config)?,
        ctx.accounts.authority.key(),
        RenderError::Unauthorized
    );
    let sha256 = {
        let data = info.try_borrow_data()?;
        finalize(&mut header, &data, |body| body.starts_with(WASM_MAGIC))?
    };
    write_header(&info, &header)?;
    registry::set_rasterizer_from_render(
        &ctx.accounts.render_authority,
        ctx.bumps.render_authority,
        &ctx.accounts.config,
        &info,
    )?;
    emit!(RasterizerFinalized {
        address: info.key(),
        version: header.id,
        len: header.len,
        sha256
    });
    Ok(())
}
