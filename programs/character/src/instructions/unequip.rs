use anchor_lang::prelude::*;
use deadroll_types::{seeds, EquipSlot};

use crate::{
    auth::check_owner_or_session_key, errors::CharacterError, events::ItemUnequipped,
    state::Character,
};

#[derive(Accounts)]
pub struct Unequip<'info> {
    /// The owner, or their session key with `session_token`.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::CHARACTER, character.core.owner.as_ref(), &[character.core.index]],
        bump = character.core.bump,
    )]
    pub character: Box<Account<'info, Character>>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_owner_or_session_key`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

pub fn handler(ctx: Context<Unequip>, slot: EquipSlot) -> Result<()> {
    let character = &mut ctx.accounts.character;
    check_owner_or_session_key(
        &ctx.accounts.signer,
        &character.core.owner,
        ctx.accounts.session_token.as_ref(),
    )?;
    require!(character.core.alive, CharacterError::CharacterDead);
    require!(
        character.core.in_session.is_none(),
        CharacterError::AlreadyLocked
    );

    let base = character.gear.unequip(slot)?;
    emit!(ItemUnequipped {
        character: character.key(),
        slot,
        base,
    });
    Ok(())
}
