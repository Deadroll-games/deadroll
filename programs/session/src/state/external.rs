//! Read-only views of accounts owned by other game programs (docs/ARCHITECTURE.md: any program
//! can read any account, no CPI needed).
//!
//! The owner wraps the frozen `crates/types` core first after an Anchor default discriminator
//! (CONTRACTS.md, section 4.1), so only the core prefix is decoded here: lane fields appended
//! after it never break the reader.
use anchor_lang::prelude::*;
use deadroll_types::{Cell, CharacterData, MapData, Spawn, Tile, MAP_CELLS, MAP_MAX, SPAWNS_MAX};

use crate::errors::SessionError;

/// `sha256("account:Character")[..8]`.
pub const CHARACTER_DISCRIMINATOR: [u8; 8] = [0x8c, 0x73, 0xa5, 0x24, 0xf1, 0x99, 0x66, 0x54];
/// `sha256("account:Map")[..8]`.
pub const MAP_DISCRIMINATOR: [u8; 8] = [0xb6, 0x1e, 0x8e, 0x97, 0x2a, 0xf1, 0xb4, 0xf4];
/// `sha256("account:MapLayout")[..8]`.
pub const MAP_LAYOUT_DISCRIMINATOR: [u8; 8] = [0x8e, 0x57, 0xdf, 0xf4, 0x3b, 0x7b, 0x3e, 0xca];

fn check_header(info: &AccountInfo, owner: &Pubkey, discriminator: &[u8; 8]) -> bool {
    info.owner == owner
        && info
            .try_borrow_data()
            .is_ok_and(|data| data.len() >= 8 && data[..8] == discriminator[..])
}

fn decode<T: AnchorDeserialize>(
    info: &AccountInfo,
    owner: &Pubkey,
    discriminator: &[u8; 8],
) -> Option<T> {
    if !check_header(info, owner, discriminator) {
        return None;
    }
    let data = info.try_borrow_data().ok()?;
    T::deserialize(&mut &data[8..]).ok()
}

/// A `deadroll_character` `Character` account.
pub fn read_character(info: &AccountInfo) -> Result<CharacterData> {
    decode(info, &deadroll_character::ID, &CHARACTER_DISCRIMINATOR)
        .ok_or(error!(SessionError::InvalidCharacter))
}

/// A `deadroll_map` `Map` account.
pub fn read_map(info: &AccountInfo) -> Result<MapData> {
    decode(info, &deadroll_map::ID, &MAP_DISCRIMINATOR).ok_or(error!(SessionError::InvalidMap))
}

/// Byte offsets inside a `MapLayout` account (discriminator included).
mod layout_off {
    use super::*;
    pub const MAP: usize = 8 + 1;
    pub const TILES: usize = MAP + 32;
    pub const SPAWNS: usize = TILES + MAP_CELLS;
    pub const SPAWN_COUNT: usize = SPAWNS + SPAWNS_MAX * Spawn::SIZE;
    pub const ENTRANCE: usize = SPAWN_COUNT + 1;
    pub const END: usize = ENTRANCE + 2 * Cell::SIZE + 1;
}

/// A `deadroll_map` `MapLayout` account, read in place (the account is ~1.3 KB, too big to copy
/// onto the stack on every move).
pub struct MapLayoutView<'a> {
    data: core::cell::Ref<'a, &'a mut [u8]>,
}

impl<'a> MapLayoutView<'a> {
    pub fn load<'info>(info: &'a AccountInfo<'info>) -> Result<Self> {
        require!(
            check_header(info, &deadroll_map::ID, &MAP_LAYOUT_DISCRIMINATOR),
            SessionError::InvalidLayout
        );
        let data = info.try_borrow_data()?;
        require!(data.len() >= layout_off::END, SessionError::InvalidLayout);
        Ok(Self { data })
    }

    pub fn map(&self) -> Pubkey {
        let mut key = [0u8; 32];
        key.copy_from_slice(&self.data[layout_off::MAP..layout_off::MAP + 32]);
        Pubkey::new_from_array(key)
    }

    /// Raw tile byte; callers check bounds against the map size first.
    pub fn tile(&self, x: u8, y: u8) -> Result<Tile> {
        let index = y as usize * MAP_MAX + x as usize;
        require!(index < MAP_CELLS, SessionError::OutOfBounds);
        Tile::try_from(self.data[layout_off::TILES + index])
            .map_err(|_| error!(SessionError::InvalidTile))
    }

    pub fn spawn_count(&self) -> u8 {
        self.data[layout_off::SPAWN_COUNT]
    }

    pub fn spawn(&self, index: usize) -> Result<Spawn> {
        require!(index < SPAWNS_MAX, SessionError::InvalidLayout);
        let start = layout_off::SPAWNS + index * Spawn::SIZE;
        Spawn::deserialize(&mut &self.data[start..start + Spawn::SIZE])
            .map_err(|_| error!(SessionError::InvalidLayout))
    }

    pub fn entrance(&self) -> Cell {
        Cell {
            x: self.data[layout_off::ENTRANCE],
            y: self.data[layout_off::ENTRANCE + 1],
        }
    }
}
