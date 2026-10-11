use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{events::ProfileCreated, rules::ACCOUNT_VERSION, state::PlayerProfile};

#[derive(Accounts)]
pub struct CreateProfile<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = 8 + PlayerProfile::INIT_SPACE,
        seeds = [seeds::PROFILE, owner.key().as_ref()],
        bump,
    )]
    pub profile: Account<'info, PlayerProfile>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateProfile>) -> Result<()> {
    let owner = ctx.accounts.owner.key();
    ctx.accounts.profile.set_inner(PlayerProfile {
        version: ACCOUNT_VERSION,
        owner,
        character_count: 0,
        bump: ctx.bumps.profile,
    });
    emit!(ProfileCreated { owner });
    Ok(())
}
