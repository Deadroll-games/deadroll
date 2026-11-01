use anchor_lang::{prelude::*, Discriminator};
use deadroll_types::{
    CellSecret, FoundSecret, SecretKind, Spawn, MONSTERS_CAP, SECRETS_CAP, SECRET_LEAVES_CAP,
};

use crate::errors::SessionError;

/// `["board", session]` (patch D, docs/MAP_DESCRIPTION.md 11, 17.6): what a party has uncovered
/// on its map, sized for the largest map class: a bit per leaf the DM revealed, the secrets found
/// and the revealed packs' monsters. Created with the session at `open_session`.
///
/// Kept out of `GameSession` so that the session stays small enough to decode on the 4 KB stack
/// frame (`Account<GameSession>` decodes on the stack before boxing). Never decoded whole: the
/// struct is its header (what the IDL shows) and the instructions go through [`BoardBytes`].
#[account]
#[derive(InitSpace)]
pub struct SessionBoard {
    pub version: u8,
    pub session: Pubkey,
    /// Entries of `found` and `spawns` in use.
    pub found_count: u8,
    pub spawn_count: u8,
}

/// Byte offsets inside a `SessionBoard` account (discriminator included).
pub mod board_off {
    use super::*;
    pub const VERSION: usize = 8;
    pub const SESSION: usize = VERSION + 1;
    pub const FOUND_COUNT: usize = SESSION + 32;
    pub const SPAWN_COUNT: usize = FOUND_COUNT + 1;
    /// A bit per leaf (`SECRET_LEAVES_CAP`).
    pub const REVEALED: usize = SPAWN_COUNT + 1;
    pub const FOUND: usize = REVEALED + REVEALED_BYTES;
    pub const SPAWNS: usize = FOUND + SECRETS_CAP * FoundSecret::SIZE;
    pub const END: usize = SPAWNS + MONSTERS_CAP * Spawn::SIZE;
}

/// Bytes of the revealed bitset.
pub const REVEALED_BYTES: usize = SECRET_LEAVES_CAP.div_ceil(8);
/// Account size, discriminator included.
pub const BOARD_SPACE: usize = board_off::END;

const _: () = assert!(8 + SessionBoard::INIT_SPACE == board_off::REVEALED);
const _: () = assert!(BOARD_SPACE <= 10_240);

/// In-place access to a `SessionBoard`.
pub struct BoardBytes<'a, 'info> {
    info: &'a AccountInfo<'info>,
}

fn read_found(b: &[u8]) -> FoundSecret {
    FoundSecret {
        leaf: u16::from_le_bytes([b[0], b[1]]),
        secret: CellSecret {
            kind: SecretKind::try_from(b[2]).unwrap_or_default(),
            a: b[3],
            b: b[4],
            c: b[5],
        },
    }
}

impl<'a, 'info> BoardBytes<'a, 'info> {
    /// The board of `session` (owner, discriminator, size, session field).
    pub fn load(info: &'a AccountInfo<'info>, session: &Pubkey) -> Result<Self> {
        require_keys_eq!(*info.owner, crate::ID, SessionError::InvalidBoard);
        let data = info.try_borrow_data()?;
        require!(
            data.len() >= BOARD_SPACE
                && data[..8] == *SessionBoard::DISCRIMINATOR
                && data[board_off::SESSION..board_off::SESSION + 32] == session.to_bytes(),
            SessionError::InvalidBoard
        );
        drop(data);
        Ok(Self { info })
    }

    pub fn is_revealed(&self, leaf: u16) -> Result<bool> {
        let data = self.info.try_borrow_data()?;
        Ok(data
            .get(board_off::REVEALED + leaf as usize / 8)
            .filter(|_| (leaf as usize / 8) < REVEALED_BYTES)
            .is_some_and(|b| b & (1 << (leaf % 8)) != 0))
    }

    pub fn set_revealed(&self, leaf: u16) -> Result<()> {
        if (leaf as usize / 8) < REVEALED_BYTES {
            self.info.try_borrow_mut_data()?[board_off::REVEALED + leaf as usize / 8] |=
                1 << (leaf % 8);
        }
        Ok(())
    }

    pub fn found_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[board_off::FOUND_COUNT])
    }

    pub fn found(&self, index: usize) -> Result<FoundSecret> {
        require!(index < SECRETS_CAP, SessionError::TooManySecrets);
        let at = board_off::FOUND + index * FoundSecret::SIZE;
        Ok(read_found(&self.info.try_borrow_data()?[at..]))
    }

    /// Appends a found secret; `false` when the list is full.
    pub fn push_found(&self, f: &FoundSecret) -> Result<bool> {
        let mut data = self.info.try_borrow_mut_data()?;
        let n = data[board_off::FOUND_COUNT] as usize;
        if n >= SECRETS_CAP {
            return Ok(false);
        }
        let at = board_off::FOUND + n * FoundSecret::SIZE;
        let [l0, l1] = f.leaf.to_le_bytes();
        let s = f.secret;
        data[at..at + FoundSecret::SIZE].copy_from_slice(&[l0, l1, s.kind as u8, s.a, s.b, s.c]);
        data[board_off::FOUND_COUNT] += 1;
        Ok(true)
    }

    /// A secret door found at leaf `leaf`: a door now.
    pub fn door_at(&self, leaf: u16) -> Result<bool> {
        let data = self.info.try_borrow_data()?;
        let n = (data[board_off::FOUND_COUNT] as usize).min(SECRETS_CAP);
        Ok((0..n).any(|i| {
            let f = read_found(&data[board_off::FOUND + i * FoundSecret::SIZE..]);
            f.leaf == leaf && f.secret.kind == SecretKind::Door
        }))
    }

    pub fn spawn_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[board_off::SPAWN_COUNT])
    }

    pub fn spawn(&self, index: usize) -> Result<Spawn> {
        require!(index < MONSTERS_CAP, SessionError::InvalidLayout);
        let at = board_off::SPAWNS + index * Spawn::SIZE;
        Ok(Spawn::read(&self.info.try_borrow_data()?[at..]))
    }

    /// Appends a revealed spawn; `false` when the list is full.
    pub fn push_spawn(&self, s: &Spawn) -> Result<bool> {
        let mut data = self.info.try_borrow_mut_data()?;
        let n = data[board_off::SPAWN_COUNT] as usize;
        if n >= MONSTERS_CAP {
            return Ok(false);
        }
        s.write(&mut data[board_off::SPAWNS + n * Spawn::SIZE..]);
        data[board_off::SPAWN_COUNT] += 1;
        Ok(true)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A board account's bytes for `session`, as `open_session` leaves it.
    pub(crate) fn board_data(session: &Pubkey) -> Vec<u8> {
        let mut data = vec![0u8; BOARD_SPACE];
        data[..8].copy_from_slice(SessionBoard::DISCRIMINATOR);
        data[board_off::VERSION] = crate::state::ACCOUNT_VERSION;
        data[board_off::SESSION..board_off::SESSION + 32].copy_from_slice(&session.to_bytes());
        data
    }

    #[test]
    fn reveals_finds_and_spawns_in_place() {
        let session = Pubkey::new_unique();
        let (key, owner, mut lamports) = (Pubkey::new_unique(), crate::ID, 1u64);
        let mut data = board_data(&session);
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let b = BoardBytes::load(&info, &session).unwrap();
        let last = SECRET_LEAVES_CAP as u16 - 1;
        assert!(!b.is_revealed(last).unwrap());
        b.set_revealed(last).unwrap();
        b.set_revealed(u16::MAX).unwrap();
        assert!(b.is_revealed(last).unwrap());
        assert!(!b.is_revealed(last - 1).unwrap());
        let door = FoundSecret {
            leaf: 900,
            secret: CellSecret::door(),
        };
        for i in 0..SECRETS_CAP {
            let f = FoundSecret {
                leaf: i as u16,
                ..door
            };
            assert!(b.push_found(if i == 7 { &door } else { &f }).unwrap());
        }
        assert!(!b.push_found(&door).unwrap(), "full");
        assert_eq!(b.found(7).unwrap(), door);
        assert!(b.door_at(900).unwrap());
        assert!(!b.door_at(901).unwrap());
        let spawn = Spawn {
            bestiary_id: 3,
            skills: 0x0102,
            pack: 39,
            ..Spawn::ZERO
        };
        for _ in 0..MONSTERS_CAP {
            assert!(b.push_spawn(&spawn).unwrap());
        }
        assert!(!b.push_spawn(&spawn).unwrap());
        assert_eq!(b.spawn(MONSTERS_CAP - 1).unwrap(), spawn);
        assert_eq!(b.spawn_count().unwrap() as usize, MONSTERS_CAP);
        assert_eq!(b.found_count().unwrap() as usize, SECRETS_CAP);
        assert!(BoardBytes::load(&info, &Pubkey::new_unique()).is_err());
    }
}
