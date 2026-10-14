use anchor_lang::prelude::*;

/// Append only: the codes are part of the client contract. Most were used by the removed
/// `Loadout` instructions (now `deadroll_character` equip / unequip); the codes stay reserved.
#[error_code]
pub enum ItemsError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
    #[msg("Only the character owner can do this")]
    NotOwner,
    #[msg("The character is in a session: equipment is locked until it ends")]
    CharacterLocked,
    #[msg("The character is dead")]
    CharacterDead,
    #[msg("Not a character account")]
    InvalidCharacter,
    #[msg("No such item in the loadout")]
    NoSuchItem,
    #[msg("This item is not worn in that slot")]
    WrongSlot,
    #[msg("Character level too low for this item")]
    LevelTooLow,
    #[msg("Nothing is worn in that slot")]
    SlotEmpty,
    #[msg("A session key needs its session token account")]
    SessionTokenMissing,
    #[msg("Session token does not match the signer, the owner or the game")]
    InvalidSessionToken,
    #[msg("Session token expired")]
    SessionTokenExpired,
}
