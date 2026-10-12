use anchor_lang::prelude::*;

use crate::events::PaletteFinalized;
use crate::state::*;

/// Hashes the body, compares it with the expected SHA-256, checks that it parses as a palette of
/// this version (`crates/render/FORMAT.md` 3.2) and freezes the account.
#[derive(Accounts)]
pub struct FinalizePalette<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub palette: AccountLoader<'info, Palette>,
}

pub fn handler(ctx: Context<FinalizePalette>) -> Result<()> {
    let info = ctx.accounts.palette.to_account_info();
    let mut header = read_header(&info)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    let version = header.id;
    let sha256 = {
        let data = info.try_borrow_data()?;
        finalize(&mut header, &data, |body| {
            deadroll_render::Palette::parse(body).is_ok_and(|p| p.palette_version == version)
        })?
    };
    write_header(&info, &header)?;
    emit!(PaletteFinalized {
        address: info.key(),
        version,
        sha256
    });
    Ok(())
}
