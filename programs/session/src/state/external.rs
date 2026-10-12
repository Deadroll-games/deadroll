//! Read-only views of accounts owned by other game programs (docs/ARCHITECTURE.md: any program
//! can read any account, no CPI needed).
//!
//! The owner wraps the frozen `crates/types` core first after an Anchor default discriminator
//! (CONTRACTS.md, section 4.1), so only the core prefix is decoded here: lane fields appended
//! after it never break the reader.
use anchor_lang::prelude::*;
use anchor_lang::{Discriminator, Space};
use deadroll_character::state::{SkillRecord, SkillSet, SKILLS_MAX};
use deadroll_types::{Cell, CharacterData, MapData, Spawn, Tile, MAP_CELLS, MAP_MAX, SPAWNS_MAX};

use crate::errors::SessionError;

/// `sha256("account:Character")[..8]`.
pub const CHARACTER_DISCRIMINATOR: [u8; 8] = [0x8c, 0x73, 0xa5, 0x24, 0xf1, 0x99, 0x66, 0x54];
/// `sha256("account:Map")[..8]`.
pub const MAP_DISCRIMINATOR: [u8; 8] = [0xb6, 0x1e, 0x8e, 0x97, 0x2a, 0xf1, 0xb4, 0xf4];
/// `sha256("account:MapLayout")[..8]`.
pub const MAP_LAYOUT_DISCRIMINATOR: [u8; 8] = [0x8e, 0x57, 0xdf, 0xf4, 0x3b, 0x7b, 0x3e, 0xca];

fn check_header(info: &AccountInfo, owner: &Pubkey, discriminator: &[u8]) -> bool {
    info.owner == owner
        && info
            .try_borrow_data()
            .is_ok_and(|data| data.len() >= 8 && &data[..8] == discriminator)
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

/// Byte offsets inside a `deadroll_character` `SkillSet` account (discriminator included).
mod skill_off {
    pub const CHARACTER: usize = 8 + 1;
    pub const COUNT: usize = CHARACTER + 32;
    pub const SKILLS: usize = COUNT + 1;
}

/// Skill `slot` of `character`, read in place from its `SkillSet` (16 records, ~0.9 KB).
///
/// Owner, discriminator and the `character` field are enough: only the character program
/// writes a `SkillSet`, and it writes `character` once at its PDA.
pub fn read_skill(info: &AccountInfo, character: &Pubkey, slot: u8) -> Result<SkillRecord> {
    require!(
        check_header(info, &deadroll_character::ID, SkillSet::DISCRIMINATOR),
        SessionError::InvalidSkillSet
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 + SkillSet::INIT_SPACE
            && data[skill_off::CHARACTER..skill_off::CHARACTER + 32] == character.to_bytes(),
        SessionError::InvalidSkillSet
    );
    require!(
        slot < data[skill_off::COUNT] && (slot as usize) < SKILLS_MAX,
        SessionError::InvalidSkill
    );
    let start = skill_off::SKILLS + slot as usize * SkillRecord::INIT_SPACE;
    SkillRecord::deserialize(&mut &data[start..start + SkillRecord::INIT_SPACE])
        .map_err(|_| error!(SessionError::InvalidSkillSet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::TestAccount;
    use anchor_lang::AccountSerialize;

    fn code(err: Error) -> u32 {
        match err {
            Error::AnchorError(e) => e.error_code_number,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn skill_set(character: Pubkey, count: u8) -> TestAccount {
        let mut skills = [SkillRecord::default(); SKILLS_MAX];
        for (i, s) in skills.iter_mut().enumerate() {
            s.power_bp = 10_000 + i as u32;
        }
        let set = SkillSet {
            version: 1,
            character,
            count,
            skills,
            bump: 250,
        };
        let mut data = Vec::new();
        set.try_serialize(&mut data).unwrap();
        TestAccount::new(deadroll_character::ID, data)
    }

    #[test]
    fn reads_one_record_in_place() {
        let character = Pubkey::new_unique();
        let mut account = skill_set(character, 3);
        let info = account.info();
        assert_eq!(read_skill(&info, &character, 0).unwrap().power_bp, 10_000);
        assert_eq!(read_skill(&info, &character, 2).unwrap().power_bp, 10_002);
        let err = read_skill(&info, &character, 3).unwrap_err();
        assert_eq!(code(err), u32::from(SessionError::InvalidSkill));
    }

    #[test]
    fn rejects_other_character_owner_or_discriminator() {
        let character = Pubkey::new_unique();
        let invalid = u32::from(SessionError::InvalidSkillSet);
        let mut other = skill_set(Pubkey::new_unique(), 3);
        assert_eq!(
            code(read_skill(&other.info(), &character, 0).unwrap_err()),
            invalid
        );
        let mut foreign = skill_set(character, 3);
        foreign.owner = Pubkey::new_unique();
        assert_eq!(
            code(read_skill(&foreign.info(), &character, 0).unwrap_err()),
            invalid
        );
        let mut wrong = skill_set(character, 3);
        wrong.data[0] ^= 1;
        assert_eq!(
            code(read_skill(&wrong.info(), &character, 0).unwrap_err()),
            invalid
        );
        let mut short = skill_set(character, 3);
        short.data.truncate(100);
        assert_eq!(
            code(read_skill(&short.info(), &character, 0).unwrap_err()),
            invalid
        );
    }
}
