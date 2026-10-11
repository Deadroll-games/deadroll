use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::config::game_authority;
use crate::errors::RenderError;
use crate::events::BlobInitialized;
use crate::state::*;

/// Creates the `SpriteBook` PDA. Only `GameConfig.authority` may call it.
#[derive(Accounts)]
#[instruction(book_id: u16, len: u32)]
pub struct InitSpriteBook<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: registry `["config"]` PDA; owner and address checked in `game_authority`.
    pub config: UncheckedAccount<'info>,
    #[account(
        init,
        payer = authority,
        space = initial_space(len),
        seeds = [seeds::SPRITES, &book_id.to_le_bytes()],
        bump,
    )]
    pub sprite_book: AccountLoader<'info, SpriteBook>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitSpriteBook>,
    book_id: u16,
    len: u32,
    sha256: [u8; 32],
) -> Result<()> {
    check_init_args(KIND_SPRITE_BOOK, len)?;
    let authority = ctx.accounts.authority.key();
    require_keys_eq!(
        game_authority(&ctx.accounts.config)?,
        authority,
        RenderError::Unauthorized
    );
    let mut account = ctx.accounts.sprite_book.load_init()?;
    let bump = ctx.bumps.sprite_book;
    init_header(
        &mut account.header,
        KIND_SPRITE_BOOK,
        book_id,
        len,
        sha256,
        authority,
        bump,
    );
    emit!(BlobInitialized {
        address: ctx.accounts.sprite_book.key(),
        kind: KIND_SPRITE_BOOK,
        id: book_id,
        len
    });
    Ok(())
}
