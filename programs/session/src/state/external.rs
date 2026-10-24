//! Read-only views of accounts owned by other game programs (docs/ARCHITECTURE.md: any program
//! can read any account, no CPI needed).
//!
//! The owner wraps the frozen `crates/types` core first after an Anchor default discriminator
//! (CONTRACTS.md, section 4.1), so only the core prefix is decoded here: lane fields appended
//! after it never break the reader.
use anchor_lang::prelude::*;
use anchor_lang::{AccountDeserialize, Discriminator, Space};
use deadroll_character::state::{
    Character, SkillExtra, SkillExtras, SkillRecord, SkillSet, SKILLS_MAX, SKILL_DESCRIPTION_LEN,
};
use deadroll_registry::state::{
    records::{BESTIARY_MAX, MONSTER_SIZE, MONSTER_SKILLS_MAX, MONSTER_SKILL_SIZE, TILE_SIZE},
    Table, TableKind, TABLE_CAP,
};
use deadroll_rules::{items::Equipment, MonsterTemplate};
use deadroll_skillcheck::monster::MonsterSkill;
use deadroll_types::{
    Cell, CharacterData, MapData, MapPoint, PointKind, Spawn, TileKind, CHESTS_MAX, MAP_CELLS,
    MAP_MAX, POINTS_MAX, SPAWNS_MAX, TILE_KINDS_MAX,
};

use crate::{errors::SessionError, state::LOOT_RECORD_LEN};

/// Registry `["bestiary"]` table (checked against its derivation in the tests).
pub const BESTIARY_TABLE: Pubkey = pubkey!("3uJyWiz7NeVekP4w1iZkKxKnJ5pMbxC6ciz7kxN3MZtM");
/// Registry `["tileset"]` table (checked against its derivation in the tests).
pub const TILESET_TABLE: Pubkey = pubkey!("GqzUadKRKmJNT2RQAhrTz31qdoMfTGmya3Cfz2vAhXHu");
/// Registry `["aspects"]`, `["traits"]` and `["resonances"]` tables (patch 4).
pub const ASPECTS_TABLE: Pubkey = pubkey!("9MkMBjPca7x6iZV1rKaCT7mn3BmezhXF4DAvFeTpa7aJ");
pub const TRAITS_TABLE: Pubkey = pubkey!("3yqgW5wFnwbqWAoGvvA1cAmw9gQxqz8TKA4vHX62GxoC");
pub const RESONANCES_TABLE: Pubkey = pubkey!("5wvxCqYJgp3gckLyZeNz3md4g8qmRxd7nenhkqtBNXjG");
/// Registry `["monster_skills"]` table (patch 6).
pub const MONSTER_SKILLS_TABLE: Pubkey = pubkey!("NQNEuKDUZhACnTJ9sD3dW49BoiPtJPatp6bpY61r81p");
/// Registry `["rewards"]` table (patch 11).
pub const REWARDS_TABLE: Pubkey = pubkey!("FxXF2viQZ2zBSS4tEBFoMEVZWharoUA4PtQgimZCnytK");

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

/// A `deadroll_character` `Character` with what it wears (`Character.gear`) and its class trait
/// tier (`Character.class_tier`): the whole account, not only the frozen core. Read once per
/// party, at `set_ready(true)`.
pub fn read_character_gear(info: &AccountInfo) -> Result<(CharacterData, Equipment, u8)> {
    require!(
        info.owner == &deadroll_character::ID,
        SessionError::InvalidCharacter
    );
    let data = info.try_borrow_data()?;
    let character = Box::new(
        Character::try_deserialize(&mut &data[..])
            .map_err(|_| error!(SessionError::InvalidCharacter))?,
    );
    Ok((
        character.core.clone(),
        Equipment(character.gear.equipment()),
        character.class_tier,
    ))
}

/// A `deadroll_map` `Map` account.
pub fn read_map(info: &AccountInfo) -> Result<MapData> {
    decode(info, &deadroll_map::ID, &MAP_DISCRIMINATOR).ok_or(error!(SessionError::InvalidMap))
}

/// A `deadroll_map` `Map` with the fields after its core: the tier and (patch 11) the minimum
/// players, the chest radius and the time to settle.
pub fn read_map_full(info: &AccountInfo) -> Result<deadroll_map::state::Map> {
    decode(info, &deadroll_map::ID, &MAP_DISCRIMINATOR).ok_or(error!(SessionError::InvalidMap))
}

/// Byte offsets inside a `MapLayout` account (discriminator included), patch 13:
/// `MapLayoutDataV2`.
mod layout_off {
    use super::*;
    pub const VERSION: usize = 8;
    pub const MAP: usize = VERSION + 1;
    pub const CELLS: usize = MAP + 32;
    pub const SPAWNS: usize = CELLS + MAP_CELLS;
    pub const SPAWN_COUNT: usize = SPAWNS + SPAWNS_MAX * Spawn::SIZE;
    pub const POINTS: usize = SPAWN_COUNT + 1;
    pub const POINT_COUNT: usize = POINTS + POINTS_MAX * MapPoint::SIZE;
    pub const BOSS_TABLE: usize = POINT_COUNT + 1;
    /// After the bump.
    pub const END: usize = BOSS_TABLE + 2;
}

/// `MapLayout.core.version` of the `MapLayoutDataV2` format (`deadroll_map::state::LAYOUT_VERSION`).
pub const LAYOUT_VERSION: u8 = 2;

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
        require!(
            data.len() >= layout_off::END && data[layout_off::VERSION] == LAYOUT_VERSION,
            SessionError::InvalidLayout
        );
        Ok(Self { data })
    }

    pub fn map(&self) -> Pubkey {
        let mut key = [0u8; 32];
        key.copy_from_slice(&self.data[layout_off::MAP..layout_off::MAP + 32]);
        Pubkey::new_from_array(key)
    }

    /// Raw cell byte (tile type and variant, `deadroll_types::cell_kind`); callers check bounds
    /// against the map size first.
    pub fn tile(&self, x: u8, y: u8) -> Result<u8> {
        let index = y as usize * MAP_MAX + x as usize;
        require!(index < MAP_CELLS, SessionError::OutOfBounds);
        Ok(self.data[layout_off::CELLS + index])
    }

    /// Patch 13: the points in use (`finalize_map` checked them).
    fn point(&self, index: usize) -> Result<MapPoint> {
        let at = layout_off::POINTS + index * MapPoint::SIZE;
        MapPoint::deserialize(&mut &self.data[at..at + MapPoint::SIZE])
            .map_err(|_| error!(SessionError::InvalidLayout))
    }

    fn point_count(&self) -> Result<usize> {
        let count = self.data[layout_off::POINT_COUNT] as usize;
        require!(count <= POINTS_MAX, SessionError::InvalidLayout);
        Ok(count)
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

    /// Patch 11: the visible chests in point order (`chest_count` used; `finalize_map`
    /// checked them). Patch 13: the chest points of the layout.
    pub fn chests(&self) -> Result<([MapPoint; CHESTS_MAX], u8)> {
        let mut out = [MapPoint::default(); CHESTS_MAX];
        let mut count = 0;
        for i in 0..self.point_count()? {
            let p = self.point(i)?;
            if p.kind == PointKind::Chest {
                *out.get_mut(count).ok_or(SessionError::InvalidLayout)? = p;
                count += 1;
            }
        }
        Ok((out, count as u8))
    }

    /// Patch 11: the boss's reward table.
    pub fn boss_table(&self) -> u8 {
        self.data[layout_off::BOSS_TABLE]
    }

    /// Patch 13: the entrance point.
    pub fn entrance(&self) -> Result<Cell> {
        for i in 0..self.point_count()? {
            let p = self.point(i)?;
            if p.kind == PointKind::Entrance {
                return Ok(p.cell());
            }
        }
        err!(SessionError::InvalidLayout)
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

/// Byte offsets inside a `deadroll_character` `SkillExtras` account (discriminator included).
mod extras_off {
    pub const CHARACTER: usize = 8 + 2;
    pub const EXTRAS: usize = CHARACTER + 32;
}

/// The `SkillExtras` part of skill `slot` of `character` (patch 5), read in place (~3.3 KB).
/// Owner, discriminator and the `character` field are checked, as for [`read_skill`].
pub fn read_skill_extra(info: &AccountInfo, character: &Pubkey, slot: u8) -> Result<SkillExtra> {
    require!(
        check_header(info, &deadroll_character::ID, SkillExtras::DISCRIMINATOR),
        SessionError::InvalidSkillSet
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 + SkillExtras::SIZE
            && data[extras_off::CHARACTER..extras_off::CHARACTER + 32] == character.to_bytes(),
        SessionError::InvalidSkillSet
    );
    require!((slot as usize) < SKILLS_MAX, SessionError::InvalidSkill);
    let r = &data[extras_off::EXTRAS + slot as usize * SkillExtra::SIZE..];
    let mut description = [0u8; SKILL_DESCRIPTION_LEN];
    description.copy_from_slice(&r[4..4 + SKILL_DESCRIPTION_LEN]);
    Ok(SkillExtra {
        range: r[0],
        timing: r[1],
        hp_below_pct: r[2],
        ap_cost: r[3],
        description,
    })
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

/// The first `N` bytes of the registry table `kind` at `address` (a whole table body, every
/// record written), with the checks of [`table_record`].
pub fn read_table_body<const N: usize>(
    info: &AccountInfo,
    address: &Pubkey,
    kind: TableKind,
) -> Result<[u8; N]> {
    table_record::<N>(info, address, kind, 0)?.ok_or(error!(SessionError::InvalidRegistryTable))
}

/// Bestiary entry `bestiary_id` (record layout: programs/registry/README.md) and its template,
/// decoded by the shared `deadroll_rules::MonsterTemplate`; `UnknownMonster` if the slot is empty
/// or not written yet.
pub fn read_bestiary(
    info: &AccountInfo,
    bestiary_id: u8,
) -> Result<([u8; MONSTER_SIZE], MonsterTemplate)> {
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
    let t = MonsterTemplate::from_record(&r).ok_or(SessionError::UnknownMonster)?;
    Ok((r, t))
}

/// Monster skill `index` of the registry `monster_skills` table (validated by `write_table`);
/// `InvalidMonsterSkills` if it is empty or not written.
pub fn read_monster_skill(info: &AccountInfo, index: usize) -> Result<MonsterSkill> {
    require!(
        index < MONSTER_SKILLS_MAX,
        SessionError::InvalidMonsterSkills
    );
    let r: [u8; MONSTER_SKILL_SIZE] =
        table_record(info, &MONSTER_SKILLS_TABLE, TableKind::MonsterSkills, index)?
            .ok_or(SessionError::InvalidMonsterSkills)?;
    require!(r[0] == 1, SessionError::InvalidMonsterSkills);
    MonsterSkill::decode_session(&r[1..]).ok_or(error!(SessionError::InvalidMonsterSkills))
}

/// Patch 11: reward table `id` of the registry `rewards` table, checked by
/// `deadroll_rules::loot::LootTable::from_record`; `InvalidRewardTables` if it is empty, not
/// written or malformed.
pub fn read_reward_table(info: &AccountInfo, id: u8) -> Result<[u8; LOOT_RECORD_LEN]> {
    let r: [u8; LOOT_RECORD_LEN] =
        table_record(info, &REWARDS_TABLE, TableKind::Rewards, id as usize)?
            .ok_or(SessionError::InvalidRewardTables)?;
    require!(
        deadroll_rules::loot::LootTable::from_record(&r).is_some(),
        SessionError::InvalidRewardTables
    );
    Ok(r)
}

/// Step cost in action points of every tile type id (`TileKind`, the cell byte's bits 7-3);
/// 0 = cannot be entered. Patch 13: decoded by `deadroll_rules::tiles`, the same code
/// `finalize_map` checks the map with. Every starter tile must be written; ids past the written
/// records cost 0.
pub fn read_tile_costs(info: &AccountInfo) -> Result<[u8; TILE_KINDS_MAX]> {
    let mut body = [0u8; TILE_KINDS_MAX * TILE_SIZE];
    let mut len = 0;
    for index in 0..TILE_KINDS_MAX {
        let Some(r) = table_record::<TILE_SIZE>(info, &TILESET_TABLE, TableKind::Tileset, index)?
        else {
            break;
        };
        body[len..len + TILE_SIZE].copy_from_slice(&r);
        len += TILE_SIZE;
    }
    require!(
        len >= TileKind::COUNT * TILE_SIZE,
        SessionError::InvalidRegistryTable
    );
    Ok(deadroll_rules::tiles::tile_rules(&body[..len]).cost)
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
        r[3] = 1;
        r[4] = 1;
        r[5] = speed;
        r[9] = 0xFF;
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
        assert_eq!(pda(seeds::ASPECTS), ASPECTS_TABLE);
        assert_eq!(pda(seeds::TRAITS), TRAITS_TABLE);
        assert_eq!(pda(seeds::RESONANCES), RESONANCES_TABLE);
        assert_eq!(pda(seeds::MONSTER_SKILLS), MONSTER_SKILLS_TABLE);
        assert_eq!(pda(seeds::REWARDS), REWARDS_TABLE);
    }

    #[test]
    fn reads_bestiary_records() {
        let mut body = Vec::new();
        body.extend_from_slice(&monster_record(6, 7, 12, 3));
        body.extend_from_slice(&[0u8; MONSTER_SIZE]); // empty slot
        body.extend_from_slice(&monster_record(3, 2, 60, 15));
        let mut account = table(BESTIARY_TABLE, TableKind::Bestiary, &body);
        let info = account.info();
        let (r, t) = read_bestiary(&info, 0).unwrap();
        assert_eq!(r, monster_record(6, 7, 12, 3));
        assert_eq!(
            (t.speed, t.initiative, t.hp, t.hp_per_level, t.defense),
            (6, 7, 12, 3, deadroll_rules::MonsterDefense::NONE)
        );
        assert_eq!(read_bestiary(&info, 2).unwrap().1.hp, 60);
        let unknown = u32::from(SessionError::UnknownMonster);
        // Empty slot, past the written length, past the table.
        for id in [1, 3, 200] {
            assert_eq!(code(read_bestiary(&info, id).unwrap_err()), unknown, "{id}");
        }
        // A record the shared decoder refuses is unknown too.
        let mut bad = monster_record(6, 7, 12, 3);
        bad[3] = 3;
        let mut account = table(BESTIARY_TABLE, TableKind::Bestiary, &bad);
        assert_eq!(
            code(read_bestiary(&account.info(), 0).unwrap_err()),
            unknown
        );
    }

    #[test]
    fn reads_monster_skills() {
        use deadroll_skillcheck::presets;
        let p = presets::SUPPORT[0];
        let skill = MonsterSkill {
            blocks: p.blocks,
            ext: p.ext,
            flags: 1,
        };
        let mut body = skill.encode_record().to_vec();
        body.extend_from_slice(&[0u8; MONSTER_SKILL_SIZE]);
        let mut account = table(MONSTER_SKILLS_TABLE, TableKind::MonsterSkills, &body);
        let info = account.info();
        assert_eq!(read_monster_skill(&info, 0).unwrap(), skill);
        let missing = u32::from(SessionError::InvalidMonsterSkills);
        for i in [1, 2, MONSTER_SKILLS_MAX] {
            assert_eq!(
                code(read_monster_skill(&info, i).unwrap_err()),
                missing,
                "{i}"
            );
        }
        let mut moved = table(Pubkey::new_unique(), TableKind::MonsterSkills, &body);
        assert_eq!(
            code(read_monster_skill(&moved.info(), 0).unwrap_err()),
            u32::from(SessionError::InvalidRegistryTable)
        );
    }

    #[test]
    fn reads_tile_costs() {
        let tileset = deadroll_rules::registry::Registry::shipped()
            .unwrap()
            .tileset;
        let mut account = table(TILESET_TABLE, TableKind::Tileset, &tileset);
        let mut costs = [0; TILE_KINDS_MAX];
        costs[..TileKind::COUNT].copy_from_slice(&[0, 1, 1, 1, 0, 1, 2, 0, 1, 2, 0]);
        assert_eq!(read_tile_costs(&account.info()).unwrap(), costs);
        assert_eq!(costs, crate::state::test_support::TILE_COST);
        // A reserved id written later is read too.
        let mut more = tileset.clone();
        more.extend_from_slice(&[1, 3, 0, 0, 0, 0, 0, 0]);
        let mut account = table(TILESET_TABLE, TableKind::Tileset, &more);
        assert_eq!(
            read_tile_costs(&account.info()).unwrap()[TileKind::COUNT],
            3
        );
        let body = tileset;
        let mut short = table(
            TILESET_TABLE,
            TableKind::Tileset,
            &body[..(TileKind::COUNT - 1) * TILE_SIZE],
        );
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
