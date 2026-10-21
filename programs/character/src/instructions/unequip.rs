use anchor_lang::prelude::*;
use deadroll_types::{seeds, EquipSlot};
use session_keys::{session_auth_or, SessionTokenV2};

use crate::{
    auth::game_session_token, errors::CharacterError, events::ItemUnequipped, state::Character,
};
// `#[session_auth_or]` names the error enum `SessionError` (its `InvalidToken`).
use crate::errors::CharacterError as SessionError;

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
    /// MagicBlock `SessionTokenV2` of the session key; none when the owner signs.
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

game_session_token!(Unequip);

#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.character.core.owner,
    CharacterError::Unauthorized
)]
pub fn handler(ctx: Context<Unequip>, slot: EquipSlot) -> Result<()> {
    let character = &mut ctx.accounts.character;
    require!(character.core.alive, CharacterError::CharacterDead);
    require!(
        character.core.in_session.is_none(),
        CharacterError::AlreadyLocked
    );

    let base = character.gear.unequip(slot)?;
    // What the character wears is drawn on its NFT: a new image is a new state version
    // (docs/STORAGE_AND_DELIVERY.md 6.2).
    character.core.state_version = character
        .core
        .state_version
        .checked_add(1)
        .ok_or(CharacterError::MathOverflow)?;
    emit!(ItemUnequipped {
        character: character.key(),
        slot,
        base,
    });
    Ok(())
}
