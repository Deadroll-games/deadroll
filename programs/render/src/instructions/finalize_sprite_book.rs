use anchor_lang::prelude::*;

use crate::events::SpriteBookFinalized;
use crate::state::*;

/// Hashes the body, compares it with the expected SHA-256, checks that it parses as a sprite book
/// with this id (`crates/render/FORMAT.md` 3.3) and freezes the account.
#[derive(Accounts)]
pub struct FinalizeSpriteBook<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub sprite_book: AccountLoader<'info, SpriteBook>,
}

pub fn handler(ctx: Context<FinalizeSpriteBook>) -> Result<()> {
    let info = ctx.accounts.sprite_book.to_account_info();
    let mut header = read_header(&info)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    let book_id = header.id;
    let sha256 = {
        let data = info.try_borrow_data()?;
        finalize(&mut header, &data, |body| {
            deadroll_render::Book::parse(body).is_ok_and(|b| b.book_id == book_id)
        })?
    };
    write_header(&info, &header)?;
    emit!(SpriteBookFinalized {
        address: info.key(),
        book_id,
        sha256
    });
    Ok(())
}
