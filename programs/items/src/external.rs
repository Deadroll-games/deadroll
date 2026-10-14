//! Read-only views of accounts owned by other game programs (docs/ARCHITECTURE.md: any program
//! can read any account, no CPI needed). Ids are constants so this program does not link the
//! character and session crates; `crates/sim/tests/parity.rs` checks them against those crates.
use anchor_lang::prelude::*;
use deadroll_types::CharacterData;

use crate::errors::ItemsError;

/// `deadroll_character::ID`.
pub const CHARACTER_PROGRAM: Pubkey = pubkey!("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");
/// `deadroll_session::ID`: the target program of the game's session tokens.
pub const SESSION_PROGRAM: Pubkey = pubkey!("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");
/// `sha256("account:Character")[..8]`.
pub const CHARACTER_DISCRIMINATOR: [u8; 8] = [0x8c, 0x73, 0xa5, 0x24, 0xf1, 0x99, 0x66, 0x54];

/// A `deadroll_character` `Character` account: only the frozen `CharacterData` core is decoded.
pub fn read_character(info: &AccountInfo) -> Result<CharacterData> {
    require_keys_eq!(*info.owner, CHARACTER_PROGRAM, ItemsError::InvalidCharacter);
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 && data[..8] == CHARACTER_DISCRIMINATOR,
        ItemsError::InvalidCharacter
    );
    CharacterData::deserialize(&mut &data[8..]).map_err(|_| error!(ItemsError::InvalidCharacter))
}
