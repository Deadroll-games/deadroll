use anchor_lang::{
    prelude::*,
    system_program::{
        allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
    },
};
use deadroll_combat::{ResonanceTables, TimedMods};
use deadroll_rules::{
    aura::{Aura, MonsterAuraField, MonsterAuras, MonsterBearer, AURAS_PER_PLAYER},
    loot::LootTable,
    monster::{record, NO_SKILL},
    spread::{BurningCell, BURNING_CELLS_MAX},
    MonsterDefense, MonsterProfile, MonsterSkillStats, MonsterTemplate,
};
use deadroll_skillcheck::monster::{MonsterSkill, SESSION_SIZE};
use deadroll_types::{
    seeds, Aspect, Cell, Monster, MonsterStateData, ARENA_MONSTERS_MAX, MONSTERS_MAX,
    MONSTER_SLOTS, NO_TABLE, SESSION_SKILLS_MAX, SESSION_TEMPLATES_MAX,
};

use crate::{
    errors::SessionError,
    rules::BALANCE,
    state::{
        mods::{
            self, ModsData, ResistModData, TimedModData, RESIST_MOD_SIZE, STORED_MODS,
            TIMED_MOD_SIZE,
        },
        AuraData,
    },
};

/// `["monsters", session]`: the monsters of the arena the party fights in, and the registry copies
/// the party fixed at `open_session`. Arena update: the live slots `0..ARENA_MONSTERS_MAX` hold the
/// current pack only, loaded by `enter_arena` (on the map there are none); the copies are one
/// bestiary record per template, the chosen skills, the reward and resonance tables, so the size
/// no longer grows with the monsters of the map. Version `MONSTER_STATE_VERSION`; an account from
/// before the arena update (version 1, every monster of the map) is refused.
///
/// On-chain it is never Borsh-decoded as a whole: `[Monster; 24]` (~1.3 KB) decoded by
/// `Account<MonsterState>` overflows the 4 KB SBPF v3 stack frame. Contexts take it as
/// `LazyAccount` (owner + discriminator check, IDL entry) and instructions go through
/// [`Monsters`], which reads and writes one monster at a time in place. `cargo-build-sbf` still
/// warns about the generated Borsh decoders (`try_deserialize_unchecked`, `deserialize_reader`)
/// and the `LazyAccount` loaders (`__load_common`); nothing calls them.
#[account]
#[derive(InitSpace)]
pub struct MonsterState {
    /// Frozen contract, first after the discriminator. Arena update: `monsters[..count]` are the
    /// arena's pack (`count <= ARENA_MONSTERS_MAX`); the entries past `ARENA_MONSTERS_MAX` are not
    /// used.
    pub core: MonsterStateData,
    /// Per arena slot: buffs and debuffs (`deadroll_combat::support`), like `Participation.mods`
    /// (the Resistance is in `resist_mods`).
    pub mods: [[TimedModData; STORED_MODS]; ARENA_MONSTERS_MAX],
    // Patch 4. Registry rules copied at `open_session`, fixed for the party
    // (docs/DAMAGE_TYPES_AND_MIXES.md 9 and 10.1), then the map state resonances make.
    /// Per arena slot: the defenses of its bestiary record.
    pub defense: [MonsterDefenseData; ARENA_MONSTERS_MAX],
    /// The registry `aspects`, `traits` and `resonances` table bodies
    /// (`deadroll_combat::tables`).
    pub res_aspects: [u8; RES_ASPECTS_LEN],
    pub res_traits: [u8; RES_TRAITS_LEN],
    pub res_pairs: [u8; RES_PAIRS_LEN],
    /// Cells set on fire by a resonance (`deadroll_rules::spread`); `power == 0` is free.
    pub burning_cells: [BurningCellData; BURNING_CELLS_MAX],
    // Patch 6. The registry bestiary records and monster skills of the spawns, copied at
    // `open_session` (docs/BESTIARY.md 3, 12), then what the skills do in the fight.
    /// Per arena slot: its bestiary record (`deadroll_rules::monster::record`), from `templates`.
    pub records: [[u8; MONSTER_RECORD_LEN]; ARENA_MONSTERS_MAX],
    /// Every skill the spawns took, each once (the session copy of
    /// `deadroll_skillcheck::monster`); the first `skill_count` are used.
    pub skill_table: [[u8; SKILL_COPY_LEN]; SESSION_SKILLS_MAX],
    pub skill_count: u8,
    /// Per arena slot, per skill slot (`0..5` active, `5` reactive, `6` passive): an index into
    /// `skill_table`, `NO_SKILL` (0xFF) = empty.
    pub skills: [[u8; MONSTER_SLOTS]; ARENA_MONSTERS_MAX],
    /// Per arena slot, per skill slot: own turn starts until usable (the reactive's too), counted
    /// down by `start_turn`.
    pub skill_cooldowns: [[u8; MONSTER_SLOTS]; ARENA_MONSTERS_MAX],
    /// Per arena slot: its passive skill's aura (from `enter_arena`), then its reactive skill's.
    pub auras: [[MonsterAuraData; AURAS_PER_PLAYER]; ARENA_MONSTERS_MAX],
    // Patch 9: resistances (docs/DAMAGE_TYPES_AND_MIXES.md 9).
    /// Per arena slot: its Resistance with the aspect weights, like `Participation.resist_mod`.
    pub resist_mods: [ResistModData; ARENA_MONSTERS_MAX],
    /// Per arena slot, per aura slot: a Resistance aura's aspect weights.
    pub aura_weights: [[[u8; Aspect::COUNT]; AURAS_PER_PLAYER]; ARENA_MONSTERS_MAX],
    // Patch 11: the registry reward tables of the party (the spawns' bodies, the chests, the
    // boss), copied at `open_session` and read in place by the reward events.
    /// Registry id of each copied table, `NO_TABLE` = free.
    pub loot_ids: [u8; LOOT_TABLES_MAX],
    /// The copied records (`deadroll_rules::loot::record`), by `loot_ids` position.
    pub loot_tables: [[u8; LOOT_RECORD_LEN]; LOOT_TABLES_MAX],
    // Triples pass: the registry `triples` table body, copied at `open_session` like the other
    // resonance tables.
    pub res_triples: [u8; RES_TRIPLES_LEN],
    // Arena update.
    /// Per arena slot: the spawn of the map layout it was loaded from.
    pub spawn_of: [u8; ARENA_MONSTERS_MAX],
    /// The bestiary records of the party's templates, each once, copied at `open_session`;
    /// `template_ids` gives their bestiary ids, `NO_TEMPLATE` = free.
    pub template_ids: [u8; SESSION_TEMPLATES_MAX],
    pub templates: [[u8; MONSTER_RECORD_LEN]; SESSION_TEMPLATES_MAX],
    /// The registry `monster_skills` index of each `skill_table` entry: `enter_arena` finds a
    /// spawn's chosen skills there.
    pub skill_ids: [u16; SESSION_SKILLS_MAX],
}

/// `MonsterState.core.version` since the arena update.
pub const MONSTER_STATE_VERSION: u8 = 2;
/// `MonsterState.template_ids` of a free slot.
pub const NO_TEMPLATE: u8 = 0xFF;

/// Distinct reward tables one party copies (bodies, chests and the boss together).
pub const LOOT_TABLES_MAX: usize = 12;
/// `deadroll_rules::loot::record::SIZE` (a literal for the IDL; a test pins it).
pub const LOOT_RECORD_LEN: usize = 108;

/// `deadroll_rules::aura::Aura` of a monster: an aura plus its kin-only flag.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct MonsterAuraData {
    pub aura: AuraData,
    pub kin_only: bool,
}

impl From<MonsterAuraData> for Aura {
    fn from(a: MonsterAuraData) -> Self {
        Aura {
            kin_only: a.kin_only,
            ..Aura::from(a.aura)
        }
    }
}

impl From<Aura> for MonsterAuraData {
    fn from(a: Aura) -> Self {
        MonsterAuraData {
            aura: a.into(),
            kin_only: a.kin_only,
        }
    }
}

/// The living monsters that bear an aura (`deadroll_rules::aura::MonsterAuraField`), kept in a
/// `Box` by the caller.
#[derive(Clone, Copy, Debug)]
pub struct MonsterBearers {
    pub list: [MonsterBearer; MONSTERS_MAX],
    pub len: usize,
}

impl Default for MonsterBearers {
    fn default() -> Self {
        MonsterBearers {
            list: [MonsterBearer::default(); MONSTERS_MAX],
            len: 0,
        }
    }
}

impl MonsterBearers {
    pub fn field(&self) -> MonsterAuraField<'_> {
        MonsterAuraField {
            bearers: &self.list[..self.len],
        }
    }
}

/// `deadroll_rules::MonsterDefense`: bits by `Aspect` / `StateKind`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct MonsterDefenseData {
    pub resist: u8,
    pub immune_aspects: u8,
    pub immune_states: u8,
}

/// `deadroll_rules::spread::BurningCell`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct BurningCellData {
    pub x: u8,
    pub y: u8,
    /// Burns through this round.
    pub until_round: u16,
    /// Burning per turn on whoever steps in.
    pub power: u32,
    /// Turn-order id of the unit that set it on fire.
    pub source: u8,
}

/// `deadroll_combat::tables::ASPECTS_LEN`, `TRAITS_LEN`, `PAIRS_LEN` (literals for the IDL;
/// a test pins them).
pub const RES_ASPECTS_LEN: usize = 48;
pub const RES_TRAITS_LEN: usize = 192;
pub const RES_PAIRS_LEN: usize = 576;
/// `deadroll_combat::tables::TRIPLES_LEN` (120 records of 16 bytes).
pub const RES_TRIPLES_LEN: usize = 1920;
/// `deadroll_rules::monster::record::SIZE` and `deadroll_skillcheck::monster::SESSION_SIZE`
/// (literals for the IDL; a test pins them).
pub const MONSTER_RECORD_LEN: usize = 48;
pub const SKILL_COPY_LEN: usize = 16;
const DEFENSE_SIZE: usize = 3;
const BURNING_CELL_SIZE: usize = 9;
const MONSTER_AURA_SIZE: usize = 12;

impl From<BurningCellData> for BurningCell {
    fn from(c: BurningCellData) -> Self {
        BurningCell {
            cell: Cell { x: c.x, y: c.y },
            until_round: c.until_round,
            power: c.power,
            source: c.source,
        }
    }
}

impl From<BurningCell> for BurningCellData {
    fn from(c: BurningCell) -> Self {
        BurningCellData {
            x: c.cell.x,
            y: c.cell.y,
            until_round: c.until_round,
            power: c.power,
            source: c.source,
        }
    }
}

/// `sha256("account:MonsterState")[..8]`, Anchor's discriminator for [`MonsterState`].
pub const MONSTER_STATE_DISCRIMINATOR: [u8; 8] = [0xae, 0xda, 0x67, 0xcc, 0xde, 0x2e, 0xe8, 0x18];

/// Account size, discriminator included.
pub const MONSTER_STATE_SPACE: usize = 8 + MonsterState::INIT_SPACE;
// A CPI allocates at most 10 KiB, and `open_session` creates the account in one call. Since the
// arena update the size does not depend on the map's monsters (a test pins it).
const _: () = assert!(MONSTER_STATE_SPACE <= 10_240);

/// What the arena update made of the account (10_144 bytes before it, for every map monster).
#[cfg(test)]
const MONSTER_STATE_SPACE_PINNED: usize = 8_100;

/// Byte offsets inside a `MonsterState` account (discriminator included).
mod off {
    use super::*;
    pub const VERSION: usize = 8;
    pub const SESSION: usize = VERSION + 1;
    pub const MONSTERS: usize = SESSION + 32;
    pub const COUNT: usize = MONSTERS + MONSTERS_MAX * Monster::SIZE;
    pub const BUMP: usize = COUNT + 1;
    pub const MODS: usize = BUMP + 1;
    pub const DEFENSE: usize = MODS + ARENA_MONSTERS_MAX * super::MODS_LEN;
    pub const RES_ASPECTS: usize = DEFENSE + ARENA_MONSTERS_MAX * super::DEFENSE_SIZE;
    pub const RES_TRAITS: usize = RES_ASPECTS + super::RES_ASPECTS_LEN;
    pub const RES_PAIRS: usize = RES_TRAITS + super::RES_TRAITS_LEN;
    pub const BURNING: usize = RES_PAIRS + super::RES_PAIRS_LEN;
    pub const RECORDS: usize = BURNING + BURNING_CELLS_MAX * super::BURNING_CELL_SIZE;
    pub const SKILL_TABLE: usize = RECORDS + ARENA_MONSTERS_MAX * super::MONSTER_RECORD_LEN;
    pub const SKILL_COUNT: usize = SKILL_TABLE + SESSION_SKILLS_MAX * super::SKILL_COPY_LEN;
    pub const SKILLS: usize = SKILL_COUNT + 1;
    pub const SKILL_COOLDOWNS: usize = SKILLS + ARENA_MONSTERS_MAX * MONSTER_SLOTS;
    pub const AURAS: usize = SKILL_COOLDOWNS + ARENA_MONSTERS_MAX * MONSTER_SLOTS;
    pub const RESIST_MODS: usize =
        AURAS + ARENA_MONSTERS_MAX * AURAS_PER_PLAYER * super::MONSTER_AURA_SIZE;
    pub const AURA_WEIGHTS: usize = RESIST_MODS + ARENA_MONSTERS_MAX * super::RESIST_MOD_SIZE;
    pub const LOOT_IDS: usize =
        AURA_WEIGHTS + ARENA_MONSTERS_MAX * AURAS_PER_PLAYER * Aspect::COUNT;
    pub const LOOT_TABLES: usize = LOOT_IDS + super::LOOT_TABLES_MAX;
    pub const RES_TRIPLES: usize = LOOT_TABLES + super::LOOT_TABLES_MAX * super::LOOT_RECORD_LEN;
    pub const SPAWN_OF: usize = RES_TRIPLES + super::RES_TRIPLES_LEN;
    pub const TEMPLATE_IDS: usize = SPAWN_OF + ARENA_MONSTERS_MAX;
    pub const TEMPLATES: usize = TEMPLATE_IDS + SESSION_TEMPLATES_MAX;
    pub const SKILL_IDS: usize = TEMPLATES + SESSION_TEMPLATES_MAX * super::MONSTER_RECORD_LEN;
    #[cfg(test)]
    pub const END: usize = SKILL_IDS + SESSION_SKILLS_MAX * 2;
}

/// Bytes of one monster's `mods`.
const MODS_LEN: usize = STORED_MODS * TIMED_MOD_SIZE;

/// In-place access to a `MonsterState` account.
pub struct Monsters<'a, 'info> {
    info: &'a AccountInfo<'info>,
}

impl<'a, 'info> Monsters<'a, 'info> {
    /// The monsters of `session`: owned by this program, `MonsterState` discriminator and
    /// `session` field. Only `open_session` writes that combination, at the PDA.
    pub fn load(info: &'a AccountInfo<'info>, session: &Pubkey) -> Result<Self> {
        require_keys_eq!(*info.owner, crate::ID, SessionError::InvalidMonsterState);
        {
            let data = info.try_borrow_data()?;
            require!(
                data.len() >= MONSTER_STATE_SPACE
                    && data[..8] == MONSTER_STATE_DISCRIMINATOR
                    && data[off::VERSION] == MONSTER_STATE_VERSION
                    && data[off::SESSION..off::SESSION + 32] == session.to_bytes(),
                SessionError::InvalidMonsterState
            );
        }
        Ok(Self { info })
    }

    /// Creates the PDA `["monsters", session]` and writes its header with no monsters.
    /// Works like Anchor's `init`, including an address that was pre-funded by someone else.
    pub fn create(
        info: &'a AccountInfo<'info>,
        payer: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        session: &Pubkey,
        bump: u8,
    ) -> Result<Self> {
        let signer: &[&[&[u8]]] = &[&[seeds::MONSTERS, session.as_ref(), &[bump]]];
        let rent = Rent::get()?.minimum_balance(MONSTER_STATE_SPACE);
        let program = system_program.key();
        if info.lamports() == 0 {
            create_account(
                CpiContext::new_with_signer(
                    program,
                    CreateAccount {
                        from: payer.clone(),
                        to: info.clone(),
                    },
                    signer,
                ),
                rent,
                MONSTER_STATE_SPACE as u64,
                &crate::ID,
            )?;
        } else {
            let missing = rent.saturating_sub(info.lamports());
            if missing > 0 {
                transfer(
                    CpiContext::new(
                        program,
                        Transfer {
                            from: payer.clone(),
                            to: info.clone(),
                        },
                    ),
                    missing,
                )?;
            }
            allocate(
                CpiContext::new_with_signer(
                    program,
                    Allocate {
                        account_to_allocate: info.clone(),
                    },
                    signer,
                ),
                MONSTER_STATE_SPACE as u64,
            )?;
            assign(
                CpiContext::new_with_signer(
                    program,
                    Assign {
                        account_to_assign: info.clone(),
                    },
                    signer,
                ),
                &crate::ID,
            )?;
        }
        {
            let mut data = info.try_borrow_mut_data()?;
            data[..8].copy_from_slice(&MONSTER_STATE_DISCRIMINATOR);
            data[off::VERSION] = MONSTER_STATE_VERSION;
            data[off::SESSION..off::SESSION + 32].copy_from_slice(session.as_ref());
            data[off::COUNT] = 0;
            data[off::BUMP] = bump;
            data[off::TEMPLATE_IDS..off::TEMPLATES].fill(NO_TEMPLATE);
        }
        Ok(Self { info })
    }

    pub fn count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[off::COUNT])
    }

    pub fn set_count(&self, count: u8) -> Result<()> {
        require!(
            count as usize <= ARENA_MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        self.info.try_borrow_mut_data()?[off::COUNT] = count;
        Ok(())
    }

    /// Monster `index` (any arena slot below `ARENA_MONSTERS_MAX`; slots at or past `count` are
    /// not in the arena).
    pub fn get(&self, index: u8) -> Result<Monster> {
        let start = Self::slot(index)?;
        let data = self.info.try_borrow_data()?;
        Monster::deserialize(&mut &data[start..start + Monster::SIZE])
            .map_err(|_| error!(SessionError::InvalidMonsterState))
    }

    pub fn set(&self, index: u8, monster: &Monster) -> Result<()> {
        let start = Self::slot(index)?;
        let mut data = self.info.try_borrow_mut_data()?;
        monster
            .serialize(&mut &mut data[start..start + Monster::SIZE])
            .map_err(|_| error!(SessionError::InvalidMonsterState))
    }

    /// Buffs and debuffs of monster `index` (its Resistance included).
    pub fn mods(&self, index: u8) -> Result<TimedMods> {
        let start = Self::mods_slot(index)?;
        let r = off::RESIST_MODS + index as usize * RESIST_MOD_SIZE;
        let data = self.info.try_borrow_data()?;
        let m = ModsData::deserialize(&mut &data[start..start + MODS_LEN])
            .map_err(|_| error!(SessionError::InvalidMonsterState))?;
        let resist = ResistModData::deserialize(&mut &data[r..r + RESIST_MOD_SIZE])
            .map_err(|_| error!(SessionError::InvalidMonsterState))?;
        Ok(mods::to_combat(&m, &resist))
    }

    pub fn set_mods(&self, index: u8, m: &TimedMods) -> Result<()> {
        let start = Self::mods_slot(index)?;
        let r = off::RESIST_MODS + index as usize * RESIST_MOD_SIZE;
        let mut data = self.info.try_borrow_mut_data()?;
        let (stored, resist) = mods::from_combat(m);
        stored
            .serialize(&mut &mut data[start..start + MODS_LEN])
            .map_err(|_| error!(SessionError::InvalidMonsterState))?;
        resist
            .serialize(&mut &mut data[r..r + RESIST_MOD_SIZE])
            .map_err(|_| error!(SessionError::InvalidMonsterState))
    }

    /// The registry defenses of monster `index` (the tags from its copied record, patch 9).
    pub fn defense(&self, index: u8) -> Result<MonsterDefense> {
        let start = Self::defense_slot(index)?;
        let rec = off::RECORDS + index as usize * MONSTER_RECORD_LEN;
        let data = self.info.try_borrow_data()?;
        Ok(MonsterDefense {
            resist: data[start],
            immune_aspects: data[start + 1],
            immune_states: data[start + 2],
            resist_tags: data[rec + record::RESIST_TAGS],
            immune_tags: data[rec + record::IMMUNE_TAGS],
        })
    }

    pub fn set_defense(&self, index: u8, d: &MonsterDefense) -> Result<()> {
        let start = Self::defense_slot(index)?;
        let mut data = self.info.try_borrow_mut_data()?;
        data[start..start + DEFENSE_SIZE].copy_from_slice(&[
            d.resist,
            d.immune_aspects,
            d.immune_states,
        ]);
        Ok(())
    }

    /// The defenses of every arena slot, by index (`MONSTERS_MAX` long for the rules; the slots
    /// past `ARENA_MONSTERS_MAX` have none).
    pub fn defenses(&self) -> Result<[MonsterDefense; MONSTERS_MAX]> {
        let mut out = [MonsterDefense::NONE; MONSTERS_MAX];
        for (i, d) in out.iter_mut().enumerate().take(ARENA_MONSTERS_MAX) {
            *d = self.defense(i as u8)?;
        }
        Ok(out)
    }

    /// Copies the registry resonance table bodies.
    pub fn set_tables(
        &self,
        aspects: &[u8],
        traits: &[u8],
        pairs: &[u8],
        triples: &[u8],
    ) -> Result<()> {
        require!(
            aspects.len() == RES_ASPECTS_LEN
                && traits.len() == RES_TRAITS_LEN
                && pairs.len() == RES_PAIRS_LEN
                && triples.len() == RES_TRIPLES_LEN,
            SessionError::InvalidRegistryTable
        );
        let mut data = self.info.try_borrow_mut_data()?;
        data[off::RES_ASPECTS..off::RES_TRAITS].copy_from_slice(aspects);
        data[off::RES_TRAITS..off::RES_PAIRS].copy_from_slice(traits);
        data[off::RES_PAIRS..off::BURNING].copy_from_slice(pairs);
        data[off::RES_TRIPLES..off::RES_TRIPLES + RES_TRIPLES_LEN].copy_from_slice(triples);
        Ok(())
    }

    /// Patch 11: marks every reward table slot free (`open_session`, before the copies).
    pub fn clear_loot_tables(&self) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        data[off::LOOT_IDS..off::LOOT_TABLES].fill(NO_TABLE);
        Ok(())
    }

    /// Copies registry reward table `id` once (`record` from the registry); a second copy of
    /// the same id is a no-op. `TooManyLootTables` when every slot is taken.
    pub fn add_loot_table(&self, id: u8, record: &[u8; LOOT_RECORD_LEN]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let ids = &data[off::LOOT_IDS..off::LOOT_TABLES];
        if ids.contains(&id) {
            return Ok(());
        }
        let free = ids
            .iter()
            .position(|i| *i == NO_TABLE)
            .ok_or(SessionError::TooManyLootTables)?;
        data[off::LOOT_IDS + free] = id;
        let at = off::LOOT_TABLES + free * LOOT_RECORD_LEN;
        data[at..at + LOOT_RECORD_LEN].copy_from_slice(record);
        Ok(())
    }

    /// The copied reward table with registry id `id`, `None` if the party has none.
    pub fn loot_table(&self, id: u8) -> Result<Option<LootTable>> {
        if id == NO_TABLE {
            return Ok(None);
        }
        let data = self.info.try_borrow_data()?;
        let Some(i) = data[off::LOOT_IDS..off::LOOT_TABLES]
            .iter()
            .position(|x| *x == id)
        else {
            return Ok(None);
        };
        let at = off::LOOT_TABLES + i * LOOT_RECORD_LEN;
        Ok(LootTable::from_record(&data[at..at + LOOT_RECORD_LEN]))
    }

    /// Decodes the copied resonance tables into `out` (kept in a `Box` by the caller).
    pub fn tables_into(&self, out: &mut ResonanceTables) -> Result<()> {
        let data = self.info.try_borrow_data()?;
        out.decode_into(
            &data[off::RES_ASPECTS..off::RES_TRAITS],
            &data[off::RES_TRAITS..off::RES_PAIRS],
            &data[off::RES_PAIRS..off::BURNING],
            &data[off::RES_TRIPLES..off::RES_TRIPLES + RES_TRIPLES_LEN],
        )
        .map_err(|_| error!(SessionError::InvalidRegistryTable))
    }

    pub fn burning_cells(&self) -> Result<[BurningCell; BURNING_CELLS_MAX]> {
        let data = self.info.try_borrow_data()?;
        let mut out = [BurningCell::default(); BURNING_CELLS_MAX];
        for (i, c) in out.iter_mut().enumerate() {
            let at = off::BURNING + i * BURNING_CELL_SIZE;
            let d = BurningCellData::deserialize(&mut &data[at..at + BURNING_CELL_SIZE])
                .map_err(|_| error!(SessionError::InvalidMonsterState))?;
            *c = d.into();
        }
        Ok(out)
    }

    pub fn set_burning_cells(&self, cells: &[BurningCell; BURNING_CELLS_MAX]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        for (i, c) in cells.iter().enumerate() {
            let at = off::BURNING + i * BURNING_CELL_SIZE;
            BurningCellData::from(*c)
                .serialize(&mut &mut data[at..at + BURNING_CELL_SIZE])
                .map_err(|_| error!(SessionError::InvalidMonsterState))?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------------
    // Patch 6: the copied bestiary records, the skills, their cooldowns, the monsters' auras.

    /// The bestiary record of monster `index`.
    pub fn record(&self, index: u8) -> Result<[u8; MONSTER_RECORD_LEN]> {
        let at = off::RECORDS + Self::monster(index)? * MONSTER_RECORD_LEN;
        let data = self.info.try_borrow_data()?;
        let mut out = [0u8; MONSTER_RECORD_LEN];
        out.copy_from_slice(&data[at..at + MONSTER_RECORD_LEN]);
        Ok(out)
    }

    pub fn set_record(&self, index: u8, r: &[u8; MONSTER_RECORD_LEN]) -> Result<()> {
        let at = off::RECORDS + Self::monster(index)? * MONSTER_RECORD_LEN;
        self.info.try_borrow_mut_data()?[at..at + MONSTER_RECORD_LEN].copy_from_slice(r);
        Ok(())
    }

    /// The bestiary template of monster `index` (docs/BESTIARY.md 12).
    pub fn template(&self, index: u8) -> Result<MonsterTemplate> {
        MonsterTemplate::from_record(&self.record(index)?)
            .ok_or(error!(SessionError::InvalidMonster))
    }

    /// Monster `index`'s numbers at `level`: its template with its registry defenses.
    pub fn profile(&self, index: u8, level: u8) -> Result<MonsterProfile> {
        Ok(self.template(index)?.profile(level, &BALANCE.monsters))
    }

    /// Copies `skill`, registry `monster_skills` record `id`, into the session table (once per
    /// id) and returns its index there.
    pub fn add_skill(&self, skill: &MonsterSkill, id: u16) -> Result<u8> {
        if let Some(i) = self.skill_index_of(id)? {
            return Ok(i);
        }
        let copy = skill.encode_session();
        let mut data = self.info.try_borrow_mut_data()?;
        let count = data[off::SKILL_COUNT] as usize;
        require!(
            count < SESSION_SKILLS_MAX,
            SessionError::TooManyMonsterSkills
        );
        let at = off::SKILL_TABLE + count * SKILL_COPY_LEN;
        data[at..at + SKILL_COPY_LEN].copy_from_slice(&copy);
        let at = off::SKILL_IDS + count * 2;
        data[at..at + 2].copy_from_slice(&id.to_le_bytes());
        data[off::SKILL_COUNT] = count as u8 + 1;
        Ok(count as u8)
    }

    /// Arena update: the session table index of registry skill `id`, if the party copied it.
    pub fn skill_index_of(&self, id: u16) -> Result<Option<u8>> {
        let data = self.info.try_borrow_data()?;
        let count = (data[off::SKILL_COUNT] as usize).min(SESSION_SKILLS_MAX);
        Ok((0..count)
            .find(|i| {
                let at = off::SKILL_IDS + i * 2;
                u16::from_le_bytes([data[at], data[at + 1]]) == id
            })
            .map(|i| i as u8))
    }

    /// The session copy of table entry `index`.
    pub fn table_skill(&self, index: u8) -> Result<MonsterSkill> {
        let data = self.info.try_borrow_data()?;
        require!(
            index < data[off::SKILL_COUNT],
            SessionError::InvalidMonsterState
        );
        let at = off::SKILL_TABLE + index as usize * SKILL_COPY_LEN;
        MonsterSkill::decode_session(&data[at..at + SESSION_SIZE])
            .ok_or(error!(SessionError::InvalidMonsterState))
    }

    /// Arena update: copies the bestiary record of template `id` once (`open_session`).
    /// `TooManyTemplates` when every slot is taken.
    pub fn add_template(&self, id: u8, r: &[u8; MONSTER_RECORD_LEN]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let ids = &data[off::TEMPLATE_IDS..off::TEMPLATES];
        if ids.contains(&id) {
            return Ok(());
        }
        let free = ids
            .iter()
            .position(|i| *i == NO_TEMPLATE)
            .ok_or(SessionError::TooManyTemplates)?;
        data[off::TEMPLATE_IDS + free] = id;
        let at = off::TEMPLATES + free * MONSTER_RECORD_LEN;
        data[at..at + MONSTER_RECORD_LEN].copy_from_slice(r);
        Ok(())
    }

    /// The copied bestiary record of template `id`.
    pub fn template_record(&self, id: u8) -> Result<[u8; MONSTER_RECORD_LEN]> {
        let data = self.info.try_borrow_data()?;
        let i = data[off::TEMPLATE_IDS..off::TEMPLATES]
            .iter()
            .position(|x| *x == id && id != NO_TEMPLATE)
            .ok_or(SessionError::InvalidMonster)?;
        let at = off::TEMPLATES + i * MONSTER_RECORD_LEN;
        let mut out = [0u8; MONSTER_RECORD_LEN];
        out.copy_from_slice(&data[at..at + MONSTER_RECORD_LEN]);
        Ok(out)
    }

    /// Arena update: the layout spawn arena slot `index` was loaded from.
    pub fn spawn_of(&self, index: u8) -> Result<u8> {
        let at = off::SPAWN_OF + Self::monster(index)?;
        Ok(self.info.try_borrow_data()?[at])
    }

    pub fn set_spawn_of(&self, index: u8, spawn: u8) -> Result<()> {
        let at = off::SPAWN_OF + Self::monster(index)?;
        self.info.try_borrow_mut_data()?[at] = spawn;
        Ok(())
    }

    /// Skill slots of monster `index`: indices into the session table, `NO_SKILL` = empty.
    pub fn skill_slots(&self, index: u8) -> Result<[u8; MONSTER_SLOTS]> {
        let at = off::SKILLS + Self::monster(index)? * MONSTER_SLOTS;
        let data = self.info.try_borrow_data()?;
        let mut out = [0u8; MONSTER_SLOTS];
        out.copy_from_slice(&data[at..at + MONSTER_SLOTS]);
        Ok(out)
    }

    pub fn set_skill_slots(&self, index: u8, slots: &[u8; MONSTER_SLOTS]) -> Result<()> {
        let at = off::SKILLS + Self::monster(index)? * MONSTER_SLOTS;
        self.info.try_borrow_mut_data()?[at..at + MONSTER_SLOTS].copy_from_slice(slots);
        Ok(())
    }

    /// The skill in `slot` of monster `index`, as the rules see it; `None` for an empty slot.
    pub fn skill(&self, index: u8, slot: u8) -> Result<Option<MonsterSkillStats>> {
        require!((slot as usize) < MONSTER_SLOTS, SessionError::InvalidAction);
        let t = self.skill_slots(index)?[slot as usize];
        if t == NO_SKILL {
            return Ok(None);
        }
        let skill = self.table_skill(t)?;
        Ok(Some(MonsterSkillStats::new(&skill, &BALANCE.skills)))
    }

    /// Cooldowns of every slot of monster `index`.
    pub fn skill_cooldowns(&self, index: u8) -> Result<[u8; MONSTER_SLOTS]> {
        let at = off::SKILL_COOLDOWNS + Self::monster(index)? * MONSTER_SLOTS;
        let data = self.info.try_borrow_data()?;
        let mut out = [0u8; MONSTER_SLOTS];
        out.copy_from_slice(&data[at..at + MONSTER_SLOTS]);
        Ok(out)
    }

    pub fn set_skill_cooldowns(&self, index: u8, c: &[u8; MONSTER_SLOTS]) -> Result<()> {
        let at = off::SKILL_COOLDOWNS + Self::monster(index)? * MONSTER_SLOTS;
        self.info.try_borrow_mut_data()?[at..at + MONSTER_SLOTS].copy_from_slice(c);
        Ok(())
    }

    /// The auras of monster `index`: its passive skill's, then its reactive skill's.
    pub fn auras(&self, index: u8) -> Result<MonsterAuras> {
        let at = off::AURAS + Self::monster(index)? * AURAS_PER_PLAYER * MONSTER_AURA_SIZE;
        let data = self.info.try_borrow_data()?;
        let w = off::AURA_WEIGHTS + index as usize * AURAS_PER_PLAYER * Aspect::COUNT;
        let mut out = [Aura::default(); AURAS_PER_PLAYER];
        for (i, a) in out.iter_mut().enumerate() {
            let r = at + i * MONSTER_AURA_SIZE;
            *a = MonsterAuraData::deserialize(&mut &data[r..r + MONSTER_AURA_SIZE])
                .map_err(|_| error!(SessionError::InvalidMonsterState))?
                .into();
            let k = w + i * Aspect::COUNT;
            a.weights_pct.copy_from_slice(&data[k..k + Aspect::COUNT]);
        }
        Ok(out)
    }

    pub fn set_auras(&self, index: u8, auras: &MonsterAuras) -> Result<()> {
        let at = off::AURAS + Self::monster(index)? * AURAS_PER_PLAYER * MONSTER_AURA_SIZE;
        let w = off::AURA_WEIGHTS + index as usize * AURAS_PER_PLAYER * Aspect::COUNT;
        let mut data = self.info.try_borrow_mut_data()?;
        for (i, a) in auras.iter().enumerate() {
            let r = at + i * MONSTER_AURA_SIZE;
            MonsterAuraData::from(*a)
                .serialize(&mut &mut data[r..r + MONSTER_AURA_SIZE])
                .map_err(|_| error!(SessionError::InvalidMonsterState))?;
            let k = w + i * Aspect::COUNT;
            data[k..k + Aspect::COUNT].copy_from_slice(&a.weights_pct);
        }
        Ok(())
    }

    /// The living monsters that bear an aura, for `deadroll_rules::aura::AuraField`. Reads only
    /// the aura bytes of the others.
    pub fn bearers(&self) -> Result<Box<MonsterBearers>> {
        let mut out = Box::<MonsterBearers>::default();
        let count = self.count()?;
        for i in 0..count {
            let any = {
                let data = self.info.try_borrow_data()?;
                let at = off::AURAS + i as usize * AURAS_PER_PLAYER * MONSTER_AURA_SIZE;
                // `AuraData.turns` (offset 7 of each aura) is 0 for an empty slot.
                (0..AURAS_PER_PLAYER).any(|k| data[at + k * MONSTER_AURA_SIZE + 7] != 0)
            };
            if !any {
                continue;
            }
            let m = self.get(i)?;
            if !m.alive {
                continue;
            }
            out.list[out.len] = MonsterBearer {
                index: i,
                cell: Cell { x: m.x, y: m.y },
                kin: self.record(i)?[record::KIN],
                auras: self.auras(i)?,
            };
            out.len += 1;
        }
        Ok(out)
    }

    fn monster(index: u8) -> Result<usize> {
        require!(
            (index as usize) < ARENA_MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(index as usize)
    }

    fn defense_slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < ARENA_MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::DEFENSE + index as usize * DEFENSE_SIZE)
    }

    fn mods_slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < ARENA_MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::MODS + index as usize * MODS_LEN)
    }

    fn slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < ARENA_MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::MONSTERS + index as usize * Monster::SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{monster, monsters_account, TestAccount};
    use anchor_lang::{AccountDeserialize, Discriminator};

    fn code(err: Error) -> u32 {
        match err {
            Error::AnchorError(e) => e.error_code_number,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn discriminator_and_size_are_anchors() {
        assert_eq!(MONSTER_STATE_DISCRIMINATOR, *MonsterState::DISCRIMINATOR);
        assert_eq!(off::MODS, 8 + MonsterStateData::SIZE);
        assert_eq!(off::END, MONSTER_STATE_SPACE);
        assert_eq!(RES_ASPECTS_LEN, deadroll_combat::tables::ASPECTS_LEN);
        assert_eq!(RES_TRAITS_LEN, deadroll_combat::tables::TRAITS_LEN);
        assert_eq!(RES_PAIRS_LEN, deadroll_combat::tables::PAIRS_LEN);
        assert_eq!(RES_TRIPLES_LEN, deadroll_combat::tables::TRIPLES_LEN);
        // Arena update: the pack's slots only, no longer the whole map's monsters.
        assert_eq!(MONSTER_STATE_SPACE, MONSTER_STATE_SPACE_PINNED);
        assert_eq!(MONSTER_RECORD_LEN, record::SIZE);
        assert_eq!(LOOT_RECORD_LEN, deadroll_rules::loot::record::SIZE);
        assert_eq!(SKILL_COPY_LEN, SESSION_SIZE);
        assert_eq!(
            borsh::to_vec(&MonsterAuraData::default()).unwrap().len(),
            MONSTER_AURA_SIZE
        );
        assert_eq!(
            borsh::to_vec(&MonsterDefenseData::default()).unwrap().len(),
            DEFENSE_SIZE
        );
        assert_eq!(
            borsh::to_vec(&BurningCellData::default()).unwrap().len(),
            BURNING_CELL_SIZE
        );
    }

    #[test]
    fn reads_and_writes_in_place_like_borsh() {
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[monster(5, 5), monster(6, 5)]);
        let mut slowed = TimedMods::default();
        slowed[deadroll_combat::ModKind::Slow as usize] = deadroll_combat::TimedMod {
            kind: deadroll_combat::ModKind::Slow,
            aspect: deadroll_types::Aspect::Cold,
            turns: 3,
            power_bp: 4_321,
            source: 1,
            weights_pct: [0; deadroll_types::Aspect::COUNT],
        };
        // Patch 9: the Resistance goes to the appended slot with its weights.
        slowed[deadroll_combat::ModKind::Resistance as usize] = deadroll_combat::TimedMod {
            kind: deadroll_combat::ModKind::Resistance,
            aspect: deadroll_types::Aspect::Piercing,
            turns: 2,
            power_bp: 3_000,
            source: 2,
            weights_pct: [0, 0, 0, 65, 35, 65],
        };
        {
            let info = account.info();
            let view = Monsters::load(&info, &session).unwrap();
            assert_eq!(view.count().unwrap(), 2);
            assert_eq!(view.get(1).unwrap(), monster(6, 5));
            let mut m = view.get(0).unwrap();
            m.x = 1;
            m.hp = 3;
            m.alive = false;
            view.set(0, &m).unwrap();
            view.set(2, &monster(7, 7)).unwrap();
            view.set_count(3).unwrap();
            assert_eq!(view.skill_cooldowns(2).unwrap(), [0; MONSTER_SLOTS]);
            view.set_skill_cooldowns(2, &[1, 0, 3, 0, 0, 2, 0]).unwrap();
            view.set_skill_cooldowns(ARENA_MONSTERS_MAX as u8 - 1, &[9; MONSTER_SLOTS])
                .unwrap();
            assert_eq!(view.mods(2).unwrap(), TimedMods::default());
            view.set_mods(2, &slowed).unwrap();
            view.set_mods(ARENA_MONSTERS_MAX as u8 - 1, &slowed)
                .unwrap();
            assert_eq!(view.mods(2).unwrap(), slowed);
        }
        let state = MonsterState::try_deserialize(&mut &account.data[..]).unwrap();
        assert_eq!(state.core.count, 3);
        assert_eq!(state.core.session, session);
        assert_eq!(
            (state.core.monsters[0].x, state.core.monsters[0].hp),
            (1, 3)
        );
        assert!(!state.core.monsters[0].alive);
        assert_eq!(state.core.monsters[1], monster(6, 5));
        assert_eq!(state.core.monsters[2], monster(7, 7));
        assert_eq!(state.core.bump, 254);
        assert_eq!(state.skill_cooldowns[2], [1, 0, 3, 0, 0, 2, 0]);
        assert_eq!(
            state.skill_cooldowns[ARENA_MONSTERS_MAX - 1],
            [9; MONSTER_SLOTS]
        );
        assert_eq!(state.skill_cooldowns[0], [0; MONSTER_SLOTS]);
        assert_eq!(
            mods::to_combat(&state.mods[2], &state.resist_mods[2]),
            slowed
        );
        assert_eq!(
            mods::to_combat(
                &state.mods[ARENA_MONSTERS_MAX - 1],
                &state.resist_mods[ARENA_MONSTERS_MAX - 1]
            ),
            slowed
        );
        assert_eq!(state.mods[1], ModsData::default());
        assert_eq!(state.resist_mods[2].weights_pct, [0, 0, 0, 65, 35, 65]);
        assert_eq!(state.resist_mods[1], ResistModData::default());
        // The cooldowns of the last monster were not touched by its mods.
        assert_eq!(
            state.skill_cooldowns[ARENA_MONSTERS_MAX - 1],
            [9; MONSTER_SLOTS]
        );
    }

    #[test]
    fn defenses_tables_and_burning_cells_in_place() {
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[monster(5, 5)]);
        let defense = MonsterDefense {
            resist: 0b10,
            immune_aspects: 0b1,
            immune_states: 0b100,
            resist_tags: 0,
            immune_tags: 0,
        };
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        cells[3] = BurningCell {
            cell: Cell { x: 2, y: 9 },
            until_round: 7,
            power: 4,
            source: 1,
        };
        let tables = crate::state::test_support::tables();
        let (a, t, p, x) = tables.to_bytes();
        {
            let info = account.info();
            let view = Monsters::load(&info, &session).unwrap();
            view.set_defense(ARENA_MONSTERS_MAX as u8 - 1, &defense)
                .unwrap();
            assert!(view
                .set_defense(ARENA_MONSTERS_MAX as u8, &defense)
                .is_err());
            view.set_burning_cells(&cells).unwrap();
            view.set_tables(&a, &t, &p, &x).unwrap();
            assert_eq!(view.defense(ARENA_MONSTERS_MAX as u8 - 1).unwrap(), defense);
            assert_eq!(view.defense(0).unwrap(), MonsterDefense::NONE);
            assert_eq!(view.burning_cells().unwrap(), cells);
            let mut back = Box::<ResonanceTables>::default();
            view.tables_into(&mut back).unwrap();
            assert_eq!(*back, tables);
        }
        let state = MonsterState::try_deserialize(&mut &account.data[..]).unwrap();
        assert_eq!(state.defense[ARENA_MONSTERS_MAX - 1].immune_states, 0b100);
        assert_eq!(state.burning_cells[3].power, 4);
        assert_eq!(state.res_pairs, p);
        assert_eq!(state.res_triples, x);
        // Garbled tables are refused.
        let mut account = monsters_account(session, &[]);
        let info = account.info();
        let view = Monsters::load(&info, &session).unwrap();
        let mut back = Box::<ResonanceTables>::default();
        assert!(view.tables_into(&mut back).is_err());
    }

    #[test]
    fn records_skills_and_auras_in_place() {
        use crate::state::test_support::{ids, registry};
        use deadroll_rules::{
            aura::{PASSIVE_AURA, REACTIVE_AURA},
            monster::{PASSIVE_SLOT, REACTIVE_SLOT},
        };
        let reg = registry();
        let session = Pubkey::new_unique();
        let mut goblin = monster(5, 5);
        goblin.bestiary_id = ids::GOBLIN;
        let mut chief = monster(6, 5);
        chief.bestiary_id = ids::GOBLIN_CHIEFTAIN;
        let mut wolf = monster(2, 2);
        wolf.bestiary_id = ids::WOLF;
        let mut account = monsters_account(session, &[goblin, chief, wolf]);
        let info = account.info();
        let view = Monsters::load(&info, &session).unwrap();
        // Records: the shipped ones (test support copies them like `open_session`).
        assert_eq!(
            view.record(1).unwrap(),
            reg.record(ids::GOBLIN_CHIEFTAIN).unwrap()
        );
        assert!(view.profile(1, 1).unwrap().boss);
        assert_eq!(
            view.template(0).unwrap(),
            reg.template(ids::GOBLIN).unwrap()
        );
        // Skills: copied once each, by slot.
        let pool = reg.pool(&reg.template(ids::GOBLIN_CHIEFTAIN).unwrap());
        let a = view.add_skill(&pool[0].unwrap(), 40).unwrap();
        let b = view.add_skill(&pool[1].unwrap(), 41).unwrap();
        assert_eq!((a, b), (0, 1));
        assert_eq!(
            view.add_skill(&pool[0].unwrap(), 40).unwrap(),
            0,
            "deduplicated"
        );
        let mut slots = [NO_SKILL; MONSTER_SLOTS];
        slots[0] = a;
        slots[PASSIVE_SLOT] = b;
        view.set_skill_slots(1, &slots).unwrap();
        let s0 = view.skill(1, 0).unwrap().unwrap();
        assert_eq!(s0.stats.blocks, pool[0].unwrap().blocks);
        assert_eq!(view.skill(1, 1).unwrap(), None);
        assert_eq!(view.skill(0, 0).unwrap(), None);
        // Auras: a bearer list of the living monsters with one.
        assert_eq!(view.bearers().unwrap().len, 0);
        let mut auras = [Aura::default(); AURAS_PER_PLAYER];
        auras[REACTIVE_AURA] = Aura {
            power_bp: 2_000,
            radius: 2,
            turns: 3,
            kin_only: true,
            weights_pct: [100, 0, 0, 0, 0, 55],
            ..Aura::default()
        };
        view.set_auras(1, &auras).unwrap();
        assert_eq!(view.auras(1).unwrap(), auras);
        assert_eq!(view.auras(0).unwrap()[REACTIVE_AURA].weights_pct, [0; 6]);
        let bearers = view.bearers().unwrap();
        assert_eq!(bearers.len, 1);
        assert_eq!(bearers.list[0].index, 1);
        assert_eq!(bearers.list[0].cell, Cell { x: 6, y: 5 });
        assert_eq!(
            bearers.list[0].kin,
            reg.template(ids::GOBLIN_CHIEFTAIN).unwrap().kin
        );
        assert!(bearers.list[0].auras[PASSIVE_AURA].is_empty());
        // A full table is refused.
        let mut other = pool[0].unwrap();
        for i in 2..SESSION_SKILLS_MAX as u8 {
            other.ext.hp_below_pct = i;
            view.add_skill(&other, 40 + i as u16).unwrap();
        }
        other.ext.hp_below_pct = 200;
        assert_eq!(
            code(view.add_skill(&other, 200).unwrap_err()),
            u32::from(SessionError::TooManyMonsterSkills)
        );
        let _ = REACTIVE_SLOT;
    }

    #[test]
    fn rejects_other_session() {
        let mut account = monsters_account(Pubkey::new_unique(), &[]);
        let info = account.info();
        let err = Monsters::load(&info, &Pubkey::new_unique()).err().unwrap();
        assert_eq!(code(err), u32::from(SessionError::InvalidMonsterState));
    }

    #[test]
    fn rejects_other_owner_discriminator_or_size() {
        let session = Pubkey::new_unique();
        let mut foreign = monsters_account(session, &[]);
        foreign.owner = Pubkey::new_unique();
        assert!(Monsters::load(&foreign.info(), &session).is_err());

        let mut wrong = monsters_account(session, &[]);
        wrong.data[0] ^= 1;
        assert!(Monsters::load(&wrong.info(), &session).is_err());

        let mut short = TestAccount::new(crate::ID, vec![0; 64]);
        assert!(Monsters::load(&short.info(), &session).is_err());
    }

    #[test]
    fn rejects_out_of_range_slots() {
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[]);
        let info = account.info();
        let view = Monsters::load(&info, &session).unwrap();
        assert!(view.get(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view.set(ARENA_MONSTERS_MAX as u8, &monster(0, 0)).is_err());
        assert!(view.set_count(ARENA_MONSTERS_MAX as u8 + 1).is_err());
        assert!(view.skill_cooldowns(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view
            .set_skill_cooldowns(ARENA_MONSTERS_MAX as u8, &[0; MONSTER_SLOTS])
            .is_err());
        assert!(view.record(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view.skill_slots(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view.auras(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view.skill(0, MONSTER_SLOTS as u8).is_err());
        assert!(view.mods(ARENA_MONSTERS_MAX as u8).is_err());
        assert!(view
            .set_mods(ARENA_MONSTERS_MAX as u8, &TimedMods::default())
            .is_err());
    }

    #[test]
    fn templates_skill_ids_and_spawns_in_place() {
        use crate::state::test_support::{ids, registry};
        let reg = registry();
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[]);
        // A fresh party copies its own templates: start from none.
        account.data[off::TEMPLATE_IDS..off::TEMPLATES].fill(NO_TEMPLATE);
        let info = account.info();
        let view = Monsters::load(&info, &session).unwrap();
        assert!(view.template_record(ids::WOLF).is_err());
        let wolf = reg.record(ids::WOLF).unwrap();
        view.add_template(ids::WOLF, &wolf).unwrap();
        view.add_template(ids::WOLF, &[0; MONSTER_RECORD_LEN])
            .unwrap();
        assert_eq!(
            view.template_record(ids::WOLF).unwrap(),
            wolf,
            "copied once"
        );
        for id in 1..SESSION_TEMPLATES_MAX as u8 {
            view.add_template(100 + id, &wolf).unwrap();
        }
        assert_eq!(
            code(view.add_template(200, &wolf).unwrap_err()),
            u32::from(SessionError::TooManyTemplates)
        );
        assert!(view.template_record(NO_TEMPLATE).is_err());
        // Skills by their registry index.
        let pool = reg.pool(&reg.template(ids::GOBLIN_CHIEFTAIN).unwrap());
        assert_eq!(view.skill_index_of(7).unwrap(), None);
        assert_eq!(view.add_skill(&pool[0].unwrap(), 7).unwrap(), 0);
        assert_eq!(view.add_skill(&pool[0].unwrap(), 9).unwrap(), 1, "by id");
        assert_eq!(view.skill_index_of(9).unwrap(), Some(1));
        assert_eq!(view.table_skill(1).unwrap(), pool[0].unwrap());
        assert!(view.table_skill(2).is_err());
        // The spawn of each arena slot.
        view.set_spawn_of(ARENA_MONSTERS_MAX as u8 - 1, 17).unwrap();
        assert_eq!(view.spawn_of(ARENA_MONSTERS_MAX as u8 - 1).unwrap(), 17);
        assert!(view.set_spawn_of(ARENA_MONSTERS_MAX as u8, 1).is_err());
    }

    #[test]
    fn an_account_from_before_the_arena_update_is_refused() {
        let session = Pubkey::new_unique();
        let mut old = monsters_account(session, &[]);
        old.data[off::VERSION] = 1;
        assert_eq!(
            code(Monsters::load(&old.info(), &session).err().unwrap()),
            u32::from(SessionError::InvalidMonsterState)
        );
    }
}
