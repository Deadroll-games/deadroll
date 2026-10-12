use anchor_lang::{
    prelude::*,
    system_program::{
        allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
    },
};
use deadroll_types::{seeds, Monster, MonsterStateData, MONSTERS_MAX};

use crate::{errors::SessionError, state::ACCOUNT_VERSION};

/// `["monsters", session]`: every monster of the party, spawned from the map layout.
///
/// On-chain it is never Borsh-decoded as a whole: `[Monster; 24]` (~1.3 KB) decoded by
/// `Account<MonsterState>` overflows the 4 KB SBPF v3 stack frame. Contexts take it as
/// `LazyAccount` (owner + discriminator check, IDL entry) and instructions go through
/// [`Monsters`], which reads and writes one monster at a time in place. `cargo-build-sbf` still
/// warns about the generated `try_deserialize_unchecked`; nothing calls it.
#[account]
#[derive(InitSpace)]
pub struct MonsterState {
    /// Frozen contract, first after the discriminator.
    pub core: MonsterStateData,
    /// Per monster, per skill: own turn starts until usable, 0 = usable (like
    /// `Participation.cooldowns`). Counted down by `start_turn`; nothing sets them in P0: the
    /// natural attack has no cooldown. TODO(post-demo): monster skills from the spawn's pool.
    pub cooldowns: [[u8; MONSTER_SKILLS_MAX]; MONSTERS_MAX],
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
}

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
        assert_eq!(
            MONSTER_STATE_SPACE,
            8 + MonsterStateData::SIZE + MONSTERS_MAX * MONSTER_SKILLS_MAX
        );
        assert_eq!(off::COOLDOWNS, 8 + MonsterStateData::SIZE);
    }

    #[test]
    fn reads_and_writes_in_place_like_borsh() {
        let session = Pubkey::new_unique();
        let mut account = monsters_account(session, &[monster(5, 5), monster(6, 5)]);
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
    }
}
