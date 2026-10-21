use anchor_lang::prelude::*;
use deadroll_types::{seeds, EquipSlot};
use session_keys::{session_auth_or, SessionTokenV2};

use crate::{
    auth::game_session_token, errors::CharacterError, events::ItemEquipped, state::Character,
};
// `#[session_auth_or]` names the error enum `SessionError` (its `InvalidToken`).
use crate::errors::CharacterError as SessionError;

#[derive(Accounts)]
pub struct Equip<'info> {
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

game_session_token!(Equip);

#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.character.core.owner,
    CharacterError::Unauthorized
)]
pub fn handler(ctx: Context<Equip>, item_index: u8, slot: EquipSlot) -> Result<()> {
    let character = &mut ctx.accounts.character;
    require!(character.core.alive, CharacterError::CharacterDead);
    // The gear is frozen with the bag while the character is in a party.
    require!(
        character.core.in_session.is_none(),
        CharacterError::AlreadyLocked
    );

    let level = character.core.level;
    let replaced = character.gear.equip(level, item_index, slot)?;
    // What the character wears is drawn on its NFT: a new image is a new state version
    // (docs/STORAGE_AND_DELIVERY.md 6.2).
    character.core.state_version = character
        .core
        .state_version
        .checked_add(1)
        .ok_or(CharacterError::MathOverflow)?;
    emit!(ItemEquipped {
        character: character.key(),
        slot,
        item_index,
        base: character.gear.items[item_index as usize].base,
        replaced,
    });
    Ok(())
}
