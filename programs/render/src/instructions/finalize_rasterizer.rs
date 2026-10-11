use anchor_lang::prelude::*;

use crate::events::RasterizerFinalized;
use crate::state::*;

/// WASM module header: magic `\0asm` and binary version 1.
pub const WASM_MAGIC: &[u8] = b"\0asm\x01\0\0\0";

/// Hashes the body, compares it with the expected SHA-256 from init, checks the WASM header and
/// freezes the account. No CPI: `registry::set_rasterizer` reads the finalized account (L4.1).
#[derive(Accounts)]
pub struct FinalizeRasterizer<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub rasterizer: AccountLoader<'info, RasterizerCode>,
}

pub fn handler(ctx: Context<FinalizeRasterizer>) -> Result<()> {
    let info = ctx.accounts.rasterizer.to_account_info();
    let mut header = read_header(&info)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    let sha256 = {
        let data = info.try_borrow_data()?;
        finalize(&mut header, &data, |body| body.starts_with(WASM_MAGIC))?
    };
    write_header(&info, &header)?;
    emit!(RasterizerFinalized {
        address: info.key(),
        version: header.id,
        len: header.len,
        sha256
    });
    Ok(())
}
