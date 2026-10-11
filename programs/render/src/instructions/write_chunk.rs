use anchor_lang::prelude::*;

use crate::errors::RenderError;
use crate::state::*;

/// Writes `bytes` at `offset` of the body of a rasterizer, palette or sprite book
/// (the `write_sprites` of L2.5 is this instruction on a `SpriteBook`). Authority only,
/// before finalize, inside the body and inside the part already allocated by `extend`.
#[derive(Accounts)]
pub struct WriteChunk<'info> {
    pub authority: Signer<'info>,
    /// CHECK: owner, discriminator and header checked in `read_header`.
    #[account(mut)]
    pub target: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<WriteChunk>, offset: u32, bytes: Vec<u8>) -> Result<()> {
    let target = ctx.accounts.target.to_account_info();
    let header = read_header(&target)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    require!(!bytes.is_empty(), RenderError::InvalidLength);
    let start = offset as usize;
    let end = start
        .checked_add(bytes.len())
        .ok_or(RenderError::OutOfBounds)?;
    require!(end <= header.len as usize, RenderError::OutOfBounds);
    let mut data = target.try_borrow_mut_data()?;
    let dst = data
        .get_mut(BODY_OFFSET + start..BODY_OFFSET + end)
        .ok_or(RenderError::OutOfBounds)?;
    dst.copy_from_slice(&bytes);
    Ok(())
}
