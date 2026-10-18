//! Table kinds and their fixed record layouts. Little-endian, byte offsets in `README.md`.
//!
//! Pure functions, no account access: shared by `write_table`, the seed script spec and tests.

use anchor_lang::prelude::*;
use deadroll_combat::tables::{self, decode_aspect, decode_pair, decode_trait};
use deadroll_skillcheck::{monster, SkillTuning};
use deadroll_types::{seeds, Aspect, Form, Source, StateKind, Tile};

/// Which table an instruction works on. Discriminants are part of the format: append only.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TableKind {
    Aspects,
    Resonances,
    Bestiary,
    Tileset,
    Prefabs,
    /// Tint traits (docs/DAMAGE_TYPES_AND_MIXES.md 3.2), patch 4.
    Traits,
    /// Monster skills (docs/BESTIARY.md 3), patch 6: the records the bestiary pools point at.
    MonsterSkills,
}

impl TableKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Aspects,
            1 => Self::Resonances,
            2 => Self::Bestiary,
            3 => Self::Tileset,
            4 => Self::Prefabs,
            5 => Self::Traits,
            6 => Self::MonsterSkills,
            _ => return None,
        })
    }

    /// PDA seed prefix.
    pub fn seed(self) -> &'static [u8] {
        match self {
            Self::Aspects => seeds::ASPECTS,
            Self::Resonances => seeds::RESONANCES,
            Self::Bestiary => seeds::BESTIARY,
            Self::Tileset => seeds::TILESET,
            Self::Prefabs => seeds::PREFABS,
            Self::Traits => seeds::TRAITS,
            Self::MonsterSkills => seeds::MONSTER_SKILLS,
        }
    }

    /// Record size in bytes; 1 for the opaque prefab book.
    pub const fn record_size(self) -> usize {
        match self {
            Self::Aspects => ASPECT_SIZE,
            Self::Resonances => RESONANCE_SIZE,
            Self::Bestiary => MONSTER_SIZE,
            Self::Tileset => TILE_SIZE,
            Self::Prefabs => 1,
            Self::Traits => TRAIT_SIZE,
            Self::MonsterSkills => MONSTER_SKILL_SIZE,
        }
    }

    /// Most bytes the table may hold.
    pub const fn max_len(self) -> usize {
        match self {
            Self::Aspects => Aspect::COUNT * ASPECT_SIZE,
            Self::Resonances => Aspect::COUNT * Aspect::COUNT * RESONANCE_SIZE,
            Self::Bestiary => BESTIARY_MAX * MONSTER_SIZE,
            Self::Tileset => Tile::COUNT * TILE_SIZE,
            Self::Prefabs => crate::state::TABLE_CAP,
            Self::Traits => tables::TRAITS_LEN,
            Self::MonsterSkills => MONSTER_SKILLS_MAX * MONSTER_SKILL_SIZE,
        }
    }
}

pub const ASPECT_SIZE: usize = tables::ASPECT_RECORD;
pub const RESONANCE_SIZE: usize = tables::PAIR_RECORD;
pub const TRAIT_SIZE: usize = tables::TRAIT_RECORD;
pub const MONSTER_SIZE: usize = 48;
pub const TILE_SIZE: usize = 8;
/// Bestiary ids are record indices. P0: 0 wolf, 1 goblin, 2 goblin chieftain, 3 goblin warrior, 4 goblin shaman.
pub const BESTIARY_MAX: usize = 32;
/// Monster skill records (`deadroll_skillcheck::monster`): 128 x 32 B.
pub const MONSTER_SKILL_SIZE: usize = monster::RECORD_SIZE;
pub const MONSTER_SKILLS_MAX: usize = monster::RECORDS_MAX;

/// `Role` variants in `crates/mapcheck/src/roster.rs`: Swarm, Fast, Tank, Loner, Boss, Helper.
const ROLES: u8 = 6;
const BIOMES: u8 = 2;
/// "No predisposition" in a bestiary record.
pub const NO_ASPECT: u8 = 0xFF;

fn is<T: TryFrom<u8>>(v: u8) -> bool {
    T::try_from(v).is_ok()
}

fn u16_at(r: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([r[at], r[at + 1]])
}

/// Checks one record of a typed table. `index` is the record's position in the table.
pub fn validate_record(kind: TableKind, index: usize, r: &[u8]) -> bool {
    if r.len() != kind.record_size() {
        return false;
    }
    match kind {
        // The resonance tables decode with the code every reader uses (`deadroll_combat::tables`):
        // a record the registry accepts is a record the session, the simulator and the website
        // read the same way.
        TableKind::Aspects => decode_aspect(index, r).is_some(),
        TableKind::Resonances => decode_pair(r).is_some(),
        TableKind::Traits => decode_trait(r).is_some(),
        TableKind::Bestiary => validate_monster(r),
        // [passable, move_cost, blocks_los, flags, reserved x4]
        TableKind::Tileset => r[0] <= 1 && r[2] <= 1 && (r[0] == 0 || r[1] >= 1),
        TableKind::Prefabs => true,
        // The monster builder rules (docs/BESTIARY.md 3.1), the decoder every reader uses.
        TableKind::MonsterSkills => {
            monster::MonsterSkill::decode_record(r, &SkillTuning::DEFAULT).is_ok()
        }
    }
}

/// An all-zero record is an empty slot; otherwise `present == 1` and every field is checked.
fn validate_monster(r: &[u8]) -> bool {
    if r.iter().all(|&b| b == 0) {
        return true;
    }
    let (role, biome, size, min_tier, speed) = (r[1], r[2], r[3], r[4], r[5]);
    let predisposition_ok = r[9] == NO_ASPECT || is::<Aspect>(r[9]);
    let aspect_mask = (1u8 << Aspect::COUNT) - 1;
    let state_mask = ((1u16 << StateKind::COUNT) - 1) as u8;
    r[0] == 1
        && role < ROLES
        && biome < BIOMES
        && (size == 1 || size == 2)
        && (1..=14).contains(&min_tier)
        && speed >= 1
        && is::<Aspect>(r[6])
        && is::<Source>(r[7])
        && is::<Form>(r[8])
        && predisposition_ok
        && r[10] & !aspect_mask == 0
        && r[11] & !aspect_mask == 0
        && r[38] & !state_mask == 0
        // Patch 6: [39] pool start, [40] pool length (a run of the monster skills table), [41] kin.
        && r[40] as usize <= monster::POOL_MAX
        && r[39] as usize + r[40] as usize <= MONSTER_SKILLS_MAX
        && r[42..].iter().all(|&b| b == 0)
        && u16_at(r, 30) <= u16_at(r, 32)
}

/// Validates the records covered by a write (`offset` and the length are aligned, see
/// [`check_range`]).
pub fn validate_write(kind: TableKind, offset: usize, bytes: &[u8]) -> bool {
    let size = kind.record_size();
    bytes
        .chunks(size)
        .enumerate()
        .all(|(i, r)| validate_record(kind, offset / size + i, r))
}

/// Why a write is rejected before its records are looked at.
#[derive(Debug, PartialEq, Eq)]
pub enum RangeError {
    OutOfBounds,
    Misaligned,
}

/// Range and alignment rules of `write_table`. Returns the end offset.
pub fn check_range(
    kind: TableKind,
    offset: usize,
    len: usize,
) -> core::result::Result<usize, RangeError> {
    let end = offset.checked_add(len).ok_or(RangeError::OutOfBounds)?;
    if len == 0 || end > kind.max_len() {
        return Err(RangeError::OutOfBounds);
    }
    let size = kind.record_size();
    if !offset.is_multiple_of(size) || !len.is_multiple_of(size) {
        return Err(RangeError::Misaligned);
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wolf() -> [u8; MONSTER_SIZE] {
        let mut r = [0u8; MONSTER_SIZE];
        r[0] = 1; // present
        r[1] = 1; // Fast
        r[3] = 1; // size
        r[4] = 1; // min tier
        r[5] = 5; // speed
        r[9] = NO_ASPECT;
        r[30..32].copy_from_slice(&3u16.to_le_bytes());
        r[32..34].copy_from_slice(&6u16.to_le_bytes());
        r
    }

    /// Aspect `i`: elemental, native state Burning, trait `i`, condition Always.
    fn aspect(i: u8) -> [u8; ASPECT_SIZE] {
        [i, 0, 0, 0, i, 0, 0, 0]
    }

    #[test]
    fn sizes_fit_the_cap() {
        for k in [
            TableKind::Aspects,
            TableKind::Resonances,
            TableKind::Bestiary,
            TableKind::Tileset,
            TableKind::Prefabs,
            TableKind::Traits,
            TableKind::MonsterSkills,
        ] {
            assert!(k.max_len() <= crate::state::TABLE_CAP);
            assert_eq!(k.max_len() % k.record_size(), 0);
            assert_eq!(TableKind::from_u8(k as u8), Some(k));
        }
        assert_eq!(TableKind::from_u8(7), None);
        assert_eq!(TableKind::MonsterSkills.seed(), b"monster_skills");
        assert_eq!(TableKind::Traits.seed(), b"traits");
    }

    #[test]
    fn range_rules() {
        let k = TableKind::Bestiary;
        assert_eq!(check_range(k, 0, 48), Ok(48));
        assert_eq!(check_range(k, 48, 96), Ok(144));
        assert_eq!(check_range(k, 0, 0), Err(RangeError::OutOfBounds));
        assert_eq!(
            check_range(k, 0, k.max_len() + 48),
            Err(RangeError::OutOfBounds)
        );
        assert_eq!(check_range(k, usize::MAX, 2), Err(RangeError::OutOfBounds));
        assert_eq!(check_range(k, 1, 48), Err(RangeError::Misaligned));
        assert_eq!(check_range(k, 0, 47), Err(RangeError::Misaligned));
        assert_eq!(check_range(TableKind::Prefabs, 3, 5), Ok(8));
    }

    #[test]
    fn aspect_records_follow_their_index() {
        assert!(validate_write(TableKind::Aspects, 0, &aspect(0)));
        assert!(validate_write(TableKind::Aspects, 8, &aspect(1)));
        assert!(!validate_write(TableKind::Aspects, 0, &aspect(1)));
        let mut bad_family = aspect(0);
        bad_family[1] = 99;
        assert!(!validate_write(TableKind::Aspects, 0, &bad_family));
        assert!(!validate_write(TableKind::Aspects, 48, &aspect(6)));
        let bad = |at: usize, v: u8| {
            let mut r = aspect(0);
            r[at] = v;
            !validate_write(TableKind::Aspects, 0, &r)
        };
        assert!(bad(3, 7), "unknown native state");
        assert!(!bad(3, tables::NO_STATE), "no native state is allowed");
        assert!(bad(4, 6), "unknown trait");
        assert!(bad(5, 9), "unknown condition");
        assert!(bad(6, 1), "reserved byte");
    }

    #[test]
    fn resonance_records() {
        use tables::OpKind;
        let mut r = [0u8; RESONANCE_SIZE];
        assert!(validate_record(TableKind::Resonances, 0, &r));
        r[1] = tables::PAIR_OVERRIDE;
        r[2] = OpKind::Multiplier as u8;
        r[3] = tables::ANY_STATE;
        r[4..8].copy_from_slice(&10_000u32.to_le_bytes());
        r[8] = OpKind::Linked as u8 | tables::CRIT_ONLY;
        r[9] = StateKind::Chill as u8;
        assert!(validate_record(TableKind::Resonances, 0, &r));
        let bad = |at: usize, v: u8| {
            let mut x = r;
            x[at] = v;
            !validate_record(TableKind::Resonances, 0, &x)
        };
        assert!(bad(9, 200), "unknown state");
        assert!(bad(0, 9), "unknown condition");
        assert!(bad(1, 2), "unknown flag");
        assert!(bad(2, OpKind::COUNT), "unknown op");
        assert!(bad(14, 1), "reserved byte");
    }

    #[test]
    fn trait_records() {
        use tables::OpKind;
        let k = TableKind::Traits;
        assert!(validate_record(k, 0, &[0; TRAIT_SIZE]), "absent trait");
        let mut r = [0u8; TRAIT_SIZE];
        r[0] = 1;
        r[2] = OpKind::AddPacket as u8;
        r[3] = Aspect::Fire as u8;
        r[8] = OpKind::Jump as u8;
        r[9] = tables::NATIVE;
        assert!(validate_record(k, 0, &r));
        let bad = |at: usize, v: u8| {
            let mut x = r;
            x[at] = v;
            !validate_record(k, 0, &x)
        };
        assert!(bad(0, 2), "present flag");
        assert!(bad(3, 6), "unknown aspect");
        assert!(bad(14, 99), "unknown op");
        assert!(bad(31, 1), "reserved byte");
        assert_eq!(
            check_range(k, 0, TRAIT_SIZE * 7),
            Err(RangeError::OutOfBounds)
        );
    }

    #[test]
    fn tile_records() {
        let k = TableKind::Tileset;
        assert!(validate_record(k, 0, &[1, 1, 0, 0, 0, 0, 0, 0]));
        assert!(validate_record(k, 1, &[0, 0, 1, 0, 0, 0, 0, 0]));
        // Passable with zero cost would be a free move.
        assert!(!validate_record(k, 0, &[1, 0, 0, 0, 0, 0, 0, 0]));
        assert!(!validate_record(k, 0, &[2, 1, 0, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn monster_records() {
        let k = TableKind::Bestiary;
        assert!(validate_record(k, 0, &wolf()));
        assert!(validate_record(k, 7, &[0; MONSTER_SIZE]), "empty slot");
        let mutate = |f: fn(&mut [u8; MONSTER_SIZE])| {
            let mut r = wolf();
            f(&mut r);
            validate_record(k, 0, &r)
        };
        assert!(!mutate(|r| r[0] = 2));
        assert!(!mutate(|r| r[1] = ROLES));
        assert!(!mutate(|r| r[2] = BIOMES));
        assert!(!mutate(|r| r[3] = 3));
        assert!(!mutate(|r| r[4] = 0));
        assert!(!mutate(|r| r[4] = 15));
        assert!(!mutate(|r| r[5] = 0), "no speed");
        assert!(!mutate(|r| r[6] = 6), "unknown aspect");
        assert!(!mutate(|r| r[7] = 2));
        assert!(!mutate(|r| r[8] = 2));
        assert!(!mutate(|r| r[9] = 6));
        assert!(mutate(|r| r[9] = Aspect::Cold as u8));
        assert!(
            !mutate(|r| r[10] = 0b100_0000),
            "resist bit past the aspects"
        );
        assert!(!mutate(|r| r[11] = 0b100_0000));
        assert!(
            mutate(|r| r[38] = 1 << StateKind::Freeze as u8),
            "state immunity"
        );
        assert!(
            !mutate(|r| r[38] = 0b1000_0000),
            "state bit past the states"
        );
        assert!(mutate(|r| r[40] = 16), "a full pool");
        assert!(!mutate(|r| r[40] = 17), "pool past 16");
        assert!(
            !mutate(|r| {
                r[39] = 120;
                r[40] = 9
            }),
            "pool past the table"
        );
        assert!(mutate(|r| r[41] = 1), "kin");
        assert!(!mutate(|r| r[42] = 1), "reserved byte");
        assert!(!mutate(|r| r[30] = 9), "damage min above max");
    }

    #[test]
    fn monster_skill_records() {
        use deadroll_skillcheck::{monster::MonsterSkill, presets, Effect};
        let k = TableKind::MonsterSkills;
        let p = presets::SUPPORT[0];
        let mut skill = MonsterSkill {
            blocks: p.blocks,
            ext: p.ext,
            flags: monster::KIN_ONLY,
        };
        assert!(validate_record(k, 0, &skill.encode_record()));
        assert!(
            validate_record(k, 3, &[0; MONSTER_SKILL_SIZE]),
            "empty slot"
        );
        // A heal is a monster skill.
        skill.blocks.effect = Effect::Heal;
        assert!(validate_record(k, 0, &skill.encode_record()));
        let bad = |at: usize, v: u8| {
            let mut r = skill.encode_record();
            r[at] = v;
            !validate_record(k, 0, &r)
        };
        assert!(bad(0, 2), "present flag");
        assert!(bad(1, 9), "unknown category");
        assert!(bad(17, 2), "unknown flag");
        assert!(bad(31, 1), "reserved byte");
        assert!(bad(9, 3), "aspect count past the builder");
        assert_eq!(
            check_range(k, 0, MONSTER_SKILL_SIZE * 129),
            Err(RangeError::OutOfBounds)
        );
    }

    #[test]
    fn write_checks_every_record() {
        let mut two = wolf().to_vec();
        two.extend_from_slice(&wolf());
        assert!(validate_write(TableKind::Bestiary, 0, &two));
        let last = two.len() - MONSTER_SIZE;
        two[last + 5] = 0; // second record loses its speed
        assert!(!validate_write(TableKind::Bestiary, 0, &two));
        assert!(validate_write(TableKind::Prefabs, 0, &[1, 2, 3]));
    }
}
