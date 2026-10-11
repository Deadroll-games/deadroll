use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::errors::RenderError;
use crate::state::*;

/// Grows a rasterizer, palette or sprite book by up to 10 KB toward its full size and tops up
/// rent from the authority. Several `extend` instructions fit in one transaction.
#[derive(Accounts)]
pub struct Extend<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: owner, discriminator and header checked in `read_header`.
    #[account(mut)]
    pub target: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Extend>) -> Result<()> {
    let target = ctx.accounts.target.to_account_info();
    let header = read_header(&target)?;
    check_writable(&header, ctx.accounts.authority.key)?;
    let current = target.data_len();
    let full = header.full_size();
    require!(current < full, RenderError::AlreadyFullyAllocated);
    let new_len = full.min(current + GROW_STEP);

    let needed = Rent::get()?
        .minimum_balance(new_len)
        .saturating_sub(target.lamports());
    if needed > 0 {
        let accounts = Transfer {
            from: ctx.accounts.authority.to_account_info(),
            to: target.clone(),
        };
        transfer(
            CpiContext::new(ctx.accounts.system_program.key(), accounts),
            needed,
        )?;
    }
    target.resize(new_len)?;
    Ok(())
}
