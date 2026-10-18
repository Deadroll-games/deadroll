use anchor_lang::{
    prelude::*,
    system_program::{
        allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
    },
};
use deadroll_combat::{ResonanceTables, TimedMods, MODS_MAX};
use deadroll_rules::{
    spread::{BurningCell, BURNING_CELLS_MAX},
    MonsterDefense,
};
use deadroll_types::{seeds, Cell, Monster, MonsterStateData, MONSTERS_MAX};

use crate::{
    errors::SessionError,
    state::{
        mods::{self, ModsData, TimedModData, TIMED_MOD_SIZE},
        ACCOUNT_VERSION,
    },
};

/// `["monsters", session]`: every monster of the party, spawned from the map layout.
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
    /// Frozen contract, first after the discriminator.
    pub core: MonsterStateData,
    /// Per monster, per skill: own turn starts until usable, 0 = usable (like
    /// `Participation.cooldowns`). Counted down by `start_turn`; nothing sets them in P0: the
    /// natural attack has no cooldown. TODO(post-demo): monster skills from the spawn's pool.
    pub cooldowns: [[u8; MONSTER_SKILLS_MAX]; MONSTERS_MAX],
    /// Per monster: buffs and debuffs (`deadroll_combat::support`), like `Participation.mods`.
    pub mods: [[TimedModData; MODS_MAX]; MONSTERS_MAX],
    // Patch 4. Registry rules copied at `open_session`, fixed for the party
    // (docs/DAMAGE_TYPES_AND_MIXES.md 9 and 10.1), then the map state resonances make.
    /// Per monster: the defenses of its bestiary record.
    pub defense: [MonsterDefenseData; MONSTERS_MAX],
    /// The registry `aspects`, `traits` and `resonances` table bodies
    /// (`deadroll_combat::tables`).
    pub res_aspects: [u8; RES_ASPECTS_LEN],
    pub res_traits: [u8; RES_TRAITS_LEN],
    pub res_pairs: [u8; RES_PAIRS_LEN],
    /// Cells set on fire by a resonance (`deadroll_rules::spread`); `power == 0` is free.
    pub burning_cells: [BurningCellData; BURNING_CELLS_MAX],
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
const DEFENSE_SIZE: usize = 3;
const BURNING_CELL_SIZE: usize = 9;

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

/// Cooldown slots per monster.
pub const MONSTER_SKILLS_MAX: usize = 4;

/// `sha256("account:MonsterState")[..8]`, Anchor's discriminator for [`MonsterState`].
pub const MONSTER_STATE_DISCRIMINATOR: [u8; 8] = [0xae, 0xda, 0x67, 0xcc, 0xde, 0x2e, 0xe8, 0x18];

/// Account size, discriminator included.
pub const MONSTER_STATE_SPACE: usize = 8 + MonsterState::INIT_SPACE;

/// Byte offsets inside a `MonsterState` account (discriminator included).
mod off {
    use super::*;
    pub const VERSION: usize = 8;
    pub const SESSION: usize = VERSION + 1;
    pub const MONSTERS: usize = SESSION + 32;
    pub const COUNT: usize = MONSTERS + MONSTERS_MAX * Monster::SIZE;
    pub const BUMP: usize = COUNT + 1;
    pub const COOLDOWNS: usize = BUMP + 1;
    pub const MODS: usize = COOLDOWNS + MONSTERS_MAX * MONSTER_SKILLS_MAX;
    pub const DEFENSE: usize = MODS + MONSTERS_MAX * super::MODS_LEN;
    pub const RES_ASPECTS: usize = DEFENSE + MONSTERS_MAX * super::DEFENSE_SIZE;
    pub const RES_TRAITS: usize = RES_ASPECTS + super::RES_ASPECTS_LEN;
    pub const RES_PAIRS: usize = RES_TRAITS + super::RES_TRAITS_LEN;
    pub const BURNING: usize = RES_PAIRS + super::RES_PAIRS_LEN;
    #[cfg(test)]
    pub const END: usize = BURNING + BURNING_CELLS_MAX * super::BURNING_CELL_SIZE;
}

/// Bytes of one monster's `mods`.
const MODS_LEN: usize = MODS_MAX * TIMED_MOD_SIZE;

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
            data[off::VERSION] = ACCOUNT_VERSION;
            data[off::SESSION..off::SESSION + 32].copy_from_slice(session.as_ref());
            data[off::COUNT] = 0;
            data[off::BUMP] = bump;
        }
        Ok(Self { info })
    }

    pub fn count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[off::COUNT])
    }

    pub fn set_count(&self, count: u8) -> Result<()> {
        require!(count as usize <= MONSTERS_MAX, SessionError::InvalidMonster);
        self.info.try_borrow_mut_data()?[off::COUNT] = count;
        Ok(())
    }

    /// Monster `index` (any slot below `MONSTERS_MAX`; slots at or past `count` are empty).
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

    /// Cooldowns of monster `index`.
    pub fn cooldowns(&self, index: u8) -> Result<[u8; MONSTER_SKILLS_MAX]> {
        let start = Self::cooldown_slot(index)?;
        let data = self.info.try_borrow_data()?;
        let mut out = [0u8; MONSTER_SKILLS_MAX];
        out.copy_from_slice(&data[start..start + MONSTER_SKILLS_MAX]);
        Ok(out)
    }

    pub fn set_cooldowns(&self, index: u8, cooldowns: &[u8; MONSTER_SKILLS_MAX]) -> Result<()> {
        let start = Self::cooldown_slot(index)?;
        self.info.try_borrow_mut_data()?[start..start + MONSTER_SKILLS_MAX]
            .copy_from_slice(cooldowns);
        Ok(())
    }

    /// Buffs and debuffs of monster `index`.
    pub fn mods(&self, index: u8) -> Result<TimedMods> {
        let start = Self::mods_slot(index)?;
        let data = self.info.try_borrow_data()?;
        let m = ModsData::deserialize(&mut &data[start..start + MODS_LEN])
            .map_err(|_| error!(SessionError::InvalidMonsterState))?;
        Ok(mods::to_combat(&m))
    }

    pub fn set_mods(&self, index: u8, m: &TimedMods) -> Result<()> {
        let start = Self::mods_slot(index)?;
        let mut data = self.info.try_borrow_mut_data()?;
        mods::from_combat(m)
            .serialize(&mut &mut data[start..start + MODS_LEN])
            .map_err(|_| error!(SessionError::InvalidMonsterState))
    }

    /// The registry defenses of monster `index`.
    pub fn defense(&self, index: u8) -> Result<MonsterDefense> {
        let start = Self::defense_slot(index)?;
        let data = self.info.try_borrow_data()?;
        Ok(MonsterDefense {
            resist: data[start],
            immune_aspects: data[start + 1],
            immune_states: data[start + 2],
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

    /// The defenses of every monster slot, by index.
    pub fn defenses(&self) -> Result<[MonsterDefense; MONSTERS_MAX]> {
        let mut out = [MonsterDefense::NONE; MONSTERS_MAX];
        for (i, d) in out.iter_mut().enumerate() {
            *d = self.defense(i as u8)?;
        }
        Ok(out)
    }

    /// Copies the registry resonance table bodies.
    pub fn set_tables(
        &self,
        aspects: &[u8; RES_ASPECTS_LEN],
        traits: &[u8; RES_TRAITS_LEN],
        pairs: &[u8; RES_PAIRS_LEN],
    ) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        data[off::RES_ASPECTS..off::RES_TRAITS].copy_from_slice(aspects);
        data[off::RES_TRAITS..off::RES_PAIRS].copy_from_slice(traits);
        data[off::RES_PAIRS..off::BURNING].copy_from_slice(pairs);
        Ok(())
    }

    /// Decodes the copied resonance tables into `out` (kept in a `Box` by the caller).
    pub fn tables_into(&self, out: &mut ResonanceTables) -> Result<()> {
        let data = self.info.try_borrow_data()?;
        out.decode_into(
            &data[off::RES_ASPECTS..off::RES_TRAITS],
            &data[off::RES_TRAITS..off::RES_PAIRS],
            &data[off::RES_PAIRS..off::BURNING],
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

    fn defense_slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::DEFENSE + index as usize * DEFENSE_SIZE)
    }

    fn mods_slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::MODS + index as usize * MODS_LEN)
    }

    fn cooldown_slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < MONSTERS_MAX,
            SessionError::InvalidMonster
        );
        Ok(off::COOLDOWNS + index as usize * MONSTER_SKILLS_MAX)
    }

    fn slot(index: u8) -> Result<usize> {
        require!(
            (index as usize) < MONSTERS_MAX,
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
        assert_eq!(off::COOLDOWNS, 8 + MonsterStateData::SIZE);
        assert_eq!(off::END, MONSTER_STATE_SPACE);
        assert_eq!(RES_ASPECTS_LEN, deadroll_combat::tables::ASPECTS_LEN);
        assert_eq!(RES_TRAITS_LEN, deadroll_combat::tables::TRAITS_LEN);
        assert_eq!(RES_PAIRS_LEN, deadroll_combat::tables::PAIRS_LEN);
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
        slowed[0] = deadroll_combat::TimedMod {
            kind: deadroll_combat::ModKind::Slow,
            aspect: deadroll_types::Aspect::Cold,
            turns: 3,
            power_bp: 4_321,
            source: 1,
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
            assert_eq!(view.cooldowns(2).unwrap(), [0; MONSTER_SKILLS_MAX]);
            view.set_cooldowns(2, &[1, 0, 3, 0]).unwrap();
            view.set_cooldowns(MONSTERS_MAX as u8 - 1, &[9; 4]).unwrap();
            assert_eq!(view.mods(2).unwrap(), TimedMods::default());
            view.set_mods(2, &slowed).unwrap();
            view.set_mods(MONSTERS_MAX as u8 - 1, &slowed).unwrap();
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
        assert_eq!(state.cooldowns[2], [1, 0, 3, 0]);
        assert_eq!(state.cooldowns[MONSTERS_MAX - 1], [9; 4]);
        assert_eq!(state.cooldowns[0], [0; MONSTER_SKILLS_MAX]);
        assert_eq!(mods::to_combat(&state.mods[2]), slowed);
        assert_eq!(mods::to_combat(&state.mods[MONSTERS_MAX - 1]), slowed);
        assert_eq!(state.mods[1], ModsData::default());
        // The cooldowns of the last monster were not touched by its mods.
        assert_eq!(state.cooldowns[MONSTERS_MAX - 1], [9; 4]);
    }

    #[test]
    fn defenses_tables_and_burning_cells_in_place() {
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[monster(5, 5)]);
        let defense = MonsterDefense {
            resist: 0b10,
            immune_aspects: 0b1,
            immune_states: 0b100,
        };
        let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
        cells[3] = BurningCell {
            cell: Cell { x: 2, y: 9 },
            until_round: 7,
            power: 4,
            source: 1,
        };
        let tables = crate::state::test_support::tables();
        let (a, t, p) = tables.to_bytes();
        {
            let info = account.info();
            let view = Monsters::load(&info, &session).unwrap();
            view.set_defense(MONSTERS_MAX as u8 - 1, &defense).unwrap();
            assert!(view.set_defense(MONSTERS_MAX as u8, &defense).is_err());
            view.set_burning_cells(&cells).unwrap();
            view.set_tables(&a, &t, &p).unwrap();
            assert_eq!(view.defense(MONSTERS_MAX as u8 - 1).unwrap(), defense);
            assert_eq!(view.defense(0).unwrap(), MonsterDefense::NONE);
            assert_eq!(view.burning_cells().unwrap(), cells);
            let mut back = Box::<ResonanceTables>::default();
            view.tables_into(&mut back).unwrap();
            assert_eq!(*back, tables);
        }
        let state = MonsterState::try_deserialize(&mut &account.data[..]).unwrap();
        assert_eq!(state.defense[MONSTERS_MAX - 1].immune_states, 0b100);
        assert_eq!(state.burning_cells[3].power, 4);
        assert_eq!(state.res_pairs, p);
        // Garbled tables are refused.
        let mut account = monsters_account(session, &[]);
        let info = account.info();
        let view = Monsters::load(&info, &session).unwrap();
        let mut back = Box::<ResonanceTables>::default();
        assert!(view.tables_into(&mut back).is_err());
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
        assert!(view.get(MONSTERS_MAX as u8).is_err());
        assert!(view.set(MONSTERS_MAX as u8, &monster(0, 0)).is_err());
        assert!(view.set_count(MONSTERS_MAX as u8 + 1).is_err());
        assert!(view.cooldowns(MONSTERS_MAX as u8).is_err());
        assert!(view.set_cooldowns(MONSTERS_MAX as u8, &[0; 4]).is_err());
        assert!(view.mods(MONSTERS_MAX as u8).is_err());
        assert!(view
            .set_mods(MONSTERS_MAX as u8, &TimedMods::default())
            .is_err());
    }
}
