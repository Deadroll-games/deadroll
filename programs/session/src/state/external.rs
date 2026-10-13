//! Read-only views of accounts owned by other game programs (docs/ARCHITECTURE.md: any program
//! can read any account, no CPI needed).
//!
//! The owner wraps the frozen `crates/types` core first after an Anchor default discriminator
//! (CONTRACTS.md, section 4.1), so only the core prefix is decoded here: lane fields appended
//! after it never break the reader.
use anchor_lang::prelude::*;
use anchor_lang::{Discriminator, Space};
use deadroll_character::state::{SkillRecord, SkillSet, SKILLS_MAX};
use deadroll_registry::state::{
    records::{BESTIARY_MAX, MONSTER_SIZE, TILE_SIZE},
    Table, TableKind, TABLE_CAP,
};
use deadroll_rules::items::Equipment;
use deadroll_types::{
    Cell, CharacterData, LoadoutData, MapData, Spawn, Tile, MAP_CELLS, MAP_MAX, SPAWNS_MAX,
};

use crate::errors::SessionError;

/// Registry `["bestiary"]` table (checked against its derivation in the tests).
pub const BESTIARY_TABLE: Pubkey = pubkey!("3uJyWiz7NeVekP4w1iZkKxKnJ5pMbxC6ciz7kxN3MZtM");
/// Registry `["tileset"]` table (checked against its derivation in the tests).
pub const TILESET_TABLE: Pubkey = pubkey!("GqzUadKRKmJNT2RQAhrTz31qdoMfTGmya3Cfz2vAhXHu");

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

/// What a character wears, from its `deadroll_items` `Loadout` (address checked by the caller).
/// A character without a loadout (created before items) wears nothing.
pub fn read_equipment(info: &AccountInfo) -> Result<Equipment> {
    if info.owner != &deadroll_items::ID {
        require!(info.data_is_empty(), SessionError::InvalidLoadout);
        return Ok(Equipment::EMPTY);
    }
    decode::<LoadoutData>(
        info,
        &deadroll_items::ID,
        deadroll_items::state::Loadout::DISCRIMINATOR
            .try_into()
            .map_err(|_| error!(SessionError::InvalidLoadout))?,
    )
    .map(|l| Equipment(l.equipment()))
    .ok_or(error!(SessionError::InvalidLoadout))
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

/// Byte offsets inside a registry `Table` account (programs/registry/README.md).
mod table_off {
    pub const KIND: usize = 8 + 1;
    pub const LEN: usize = 8 + 4;
    pub const BODY: usize = 8 + 8;
}

/// Record `index` of the registry table `kind` at `address`, copied out: owner, address,
/// discriminator and kind are checked, and the record must lie inside the written length.
fn table_record<const N: usize>(
    info: &AccountInfo,
    address: &Pubkey,
    kind: TableKind,
    index: usize,
) -> Result<Option<[u8; N]>> {
    require!(
        info.key == address && check_header(info, &deadroll_registry::ID, Table::DISCRIMINATOR),
        SessionError::InvalidRegistryTable
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= table_off::BODY + TABLE_CAP && data[table_off::KIND] == kind as u8,
        SessionError::InvalidRegistryTable
    );
    let mut len = [0u8; 4];
    len.copy_from_slice(&data[table_off::LEN..table_off::LEN + 4]);
    let end = (index + 1) * N;
    if end > u32::from_le_bytes(len) as usize || end > TABLE_CAP {
        return Ok(None);
    }
    let start = table_off::BODY + index * N;
    let mut record = [0u8; N];
    record.copy_from_slice(&data[start..start + N]);
    Ok(Some(record))
}

/// The bestiary fields the session uses (record layout: programs/registry/README.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BestiaryRecord {
    /// Action points per turn.
    pub speed: u8,
    pub initiative: u16,
    /// HP at level 0, and per level.
    pub hp: u32,
    pub hp_per_level: u32,
}

/// Bestiary entry `bestiary_id`; `UnknownMonster` if the slot is empty or not written yet.
pub fn read_bestiary(info: &AccountInfo, bestiary_id: u8) -> Result<BestiaryRecord> {
    require!(
        (bestiary_id as usize) < BESTIARY_MAX,
        SessionError::UnknownMonster
    );
    let r: [u8; MONSTER_SIZE] = table_record(
        info,
        &BESTIARY_TABLE,
        TableKind::Bestiary,
        bestiary_id as usize,
    )?
    .ok_or(SessionError::UnknownMonster)?;
    require!(r[0] == 1, SessionError::UnknownMonster);
    let u16_at = |at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]);
    Ok(BestiaryRecord {
        speed: r[5],
        initiative: u16_at(20),
        hp: u32_at(22),
        hp_per_level: u32_at(26),
    })
}

/// Step cost in action points of every tile, in `Tile` order; 0 = cannot be entered. Every
/// record must be written.
pub fn read_tile_costs(info: &AccountInfo) -> Result<[u8; Tile::COUNT]> {
    let mut costs = [0u8; Tile::COUNT];
    for (index, cost) in costs.iter_mut().enumerate() {
        let r: [u8; TILE_SIZE] = table_record(info, &TILESET_TABLE, TableKind::Tileset, index)?
            .ok_or(SessionError::InvalidRegistryTable)?;
        // [passable, move cost, ...]; `write_table` guarantees cost >= 1 when passable.
        *cost = if r[0] == 1 { r[1] } else { 0 };
    }
    Ok(costs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::TestAccount;
    use anchor_lang::AccountSerialize;
    use deadroll_types::seeds;

    /// A registry table account at `address` with `body` written (`len = body.len()`).
    fn table(address: Pubkey, kind: TableKind, body: &[u8]) -> TestAccount {
        let mut data = vec![0u8; table_off::BODY + TABLE_CAP];
        data[..8].copy_from_slice(Table::DISCRIMINATOR);
        data[8] = 1;
        data[table_off::KIND] = kind as u8;
        data[table_off::LEN..table_off::LEN + 4]
            .copy_from_slice(&(body.len() as u32).to_le_bytes());
        data[table_off::BODY..table_off::BODY + body.len()].copy_from_slice(body);
        let mut account = TestAccount::new(deadroll_registry::ID, data);
        account.key = address;
        account
    }

    fn monster_record(speed: u8, initiative: u16, hp: u32, per_level: u32) -> [u8; MONSTER_SIZE] {
        let mut r = [0u8; MONSTER_SIZE];
        r[0] = 1;
        r[5] = speed;
        r[20..22].copy_from_slice(&initiative.to_le_bytes());
        r[22..26].copy_from_slice(&hp.to_le_bytes());
        r[26..30].copy_from_slice(&per_level.to_le_bytes());
        r
    }

    #[test]
    fn table_addresses_match_their_derivation() {
        let pda = |seed: &[u8]| Pubkey::find_program_address(&[seed], &deadroll_registry::ID).0;
        assert_eq!(pda(seeds::BESTIARY), BESTIARY_TABLE);
        assert_eq!(pda(seeds::TILESET), TILESET_TABLE);
    }

    #[test]
    fn reads_bestiary_records() {
        let mut body = Vec::new();
        body.extend_from_slice(&monster_record(6, 7, 12, 3));
        body.extend_from_slice(&[0u8; MONSTER_SIZE]); // empty slot
        body.extend_from_slice(&monster_record(3, 2, 60, 15));
        let mut account = table(BESTIARY_TABLE, TableKind::Bestiary, &body);
        let info = account.info();
        assert_eq!(
            read_bestiary(&info, 0).unwrap(),
            BestiaryRecord {
                speed: 6,
                initiative: 7,
                hp: 12,
                hp_per_level: 3
            }
        );
        assert_eq!(read_bestiary(&info, 2).unwrap().hp, 60);
        let unknown = u32::from(SessionError::UnknownMonster);
        // Empty slot, past the written length, past the table.
        for id in [1, 3, 200] {
            assert_eq!(code(read_bestiary(&info, id).unwrap_err()), unknown, "{id}");
        }
    }

    #[test]
    fn reads_tile_costs() {
        let mut body = Vec::new();
        for (passable, cost) in [(1, 1), (0, 0), (1, 2), (1, 1), (1, 1), (1, 1), (1, 3)] {
            body.extend_from_slice(&[passable, cost, 0, 0, 0, 0, 0, 0]);
        }
        let mut account = table(TILESET_TABLE, TableKind::Tileset, &body);
        assert_eq!(
            read_tile_costs(&account.info()).unwrap(),
            [1, 0, 2, 1, 1, 1, 3]
        );
        let mut short = table(TILESET_TABLE, TableKind::Tileset, &body[..6 * TILE_SIZE]);
        assert_eq!(
            code(read_tile_costs(&short.info()).unwrap_err()),
            u32::from(SessionError::InvalidRegistryTable)
        );
    }

    #[test]
    fn rejects_foreign_tables() {
        let invalid = u32::from(SessionError::InvalidRegistryTable);
        let body = monster_record(6, 7, 12, 3);
        let mut moved = table(Pubkey::new_unique(), TableKind::Bestiary, &body);
        assert_eq!(code(read_bestiary(&moved.info(), 0).unwrap_err()), invalid);
        let mut foreign = table(BESTIARY_TABLE, TableKind::Bestiary, &body);
        foreign.owner = Pubkey::new_unique();
        assert_eq!(
            code(read_bestiary(&foreign.info(), 0).unwrap_err()),
            invalid
        );
        let mut wrong_kind = table(BESTIARY_TABLE, TableKind::Tileset, &body);
        assert_eq!(
            code(read_bestiary(&wrong_kind.info(), 0).unwrap_err()),
            invalid
        );
        let mut wrong_disc = table(BESTIARY_TABLE, TableKind::Bestiary, &body);
        wrong_disc.data[0] ^= 1;
        assert_eq!(
            code(read_bestiary(&wrong_disc.info(), 0).unwrap_err()),
            invalid
        );
        let mut short = table(BESTIARY_TABLE, TableKind::Bestiary, &body);
        short.data.truncate(100);
        assert_eq!(code(read_bestiary(&short.info(), 0).unwrap_err()), invalid);
    }

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
