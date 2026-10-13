use anchor_lang::prelude::*;
use deadroll_types::{seeds, EquipSlot};

use crate::{
    auth::check_owner_or_session_key,
    errors::ItemsError,
    events::ItemUnequipped,
    external::read_character,
    state::{loadout, Loadout},
};

#[derive(Accounts)]
pub struct Unequip<'info> {
    /// The owner, or their session key with `session_token`.
    pub signer: Signer<'info>,
    /// CHECK: a `deadroll_character` `Character`, decoded and checked in `external::read_character`.
    pub character: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::LOADOUT, character.key().as_ref()],
        bump = loadout.core.bump
    )]
    pub loadout: Box<Account<'info, Loadout>>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_owner_or_session_key`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

pub fn handler(ctx: Context<Unequip>, slot: EquipSlot) -> Result<()> {
    let c = read_character(&ctx.accounts.character)?;
    check_owner_or_session_key(
        &ctx.accounts.signer,
        &c.owner,
        ctx.accounts.session_token.as_ref(),
    )?;
    require!(c.alive, ItemsError::CharacterDead);
    require!(c.in_session.is_none(), ItemsError::CharacterLocked);

    let base = loadout::unequip(&mut ctx.accounts.loadout.core, slot)?;
    emit!(ItemUnequipped {
        character: ctx.accounts.character.key(),
        slot,
        base,
    });
    Ok(())
}
