use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::ItemsError,
    events::LoadoutCreated,
    external::read_character,
    state::{loadout, Loadout},
};

#[derive(Accounts)]
pub struct InitLoadout<'info> {
    /// The character owner; pays for the loadout.
    #[account(mut)]
    pub owner: Signer<'info>,
    /// CHECK: a `deadroll_character` `Character`, decoded and checked in `external::read_character`.
    pub character: UncheckedAccount<'info>,
    #[account(
        init,
        payer = owner,
        space = 8 + Loadout::INIT_SPACE,
        seeds = [seeds::LOADOUT, character.key().as_ref()],
        bump
    )]
    pub loadout: Box<Account<'info, Loadout>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitLoadout>) -> Result<()> {
    let c = read_character(&ctx.accounts.character)?;
    require_keys_eq!(c.owner, ctx.accounts.owner.key(), ItemsError::NotOwner);
    require!(c.alive, ItemsError::CharacterDead);

    let character = ctx.accounts.character.key();
    let core = loadout::starter(character, c.owner, c.class, ctx.bumps.loadout);
    let item_count = core.item_count;
    ctx.accounts.loadout.set_inner(Loadout { core });
    emit!(LoadoutCreated {
        character,
        owner: c.owner,
        class: c.class,
        item_count,
    });
    Ok(())
}
