//! The party accounts every VRF callback gets after its fixed list (patch 5): for each living
//! player other than the callback's `participation`, in slot order, its `Participation`
//! (writable) and its `Character`. Reactive skills, line targets and buffs on other allies need
//! them; most actions touch none, so each is decoded only when used and written back only when
//! it changed.
//!
//! The requester (`act`, `monster_act`, `monster_move`, `start_turn`) lists them from
//! `GameSession.participations` and `GameSession.core.players`; nothing can happen between the
//! request and the callback (the roll is pending), so the living players are the same.
use anchor_lang::prelude::*;
use anchor_lang::{AccountDeserialize, AccountSerialize};
use deadroll_rules::HeroStats;
use deadroll_types::{CharacterData, PARTY_MAX};

use crate::{
    errors::SessionError,
    state::{external::read_character, GameSession, Participation},
};

/// The `(participation, character)` keys of every living player but `except`, in slot order.
pub fn party_keys(session: &GameSession, except: u8) -> ([(Pubkey, Pubkey); PARTY_MAX], usize) {
    let mut out = [(Pubkey::default(), Pubkey::default()); PARTY_MAX];
    let mut n = 0;
    for slot in 0..session.core.player_count {
        if slot != except && session.player_alive(slot) {
            out[n] = (
                session.participations[slot as usize],
                session.core.players[slot as usize],
            );
            n += 1;
        }
    }
    (out, n)
}

/// The other players of a callback, by slot.
pub struct Party<'a, 'info> {
    main: u8,
    infos: [Option<(&'a AccountInfo<'info>, &'a AccountInfo<'info>)>; PARTY_MAX],
    loaded: [Option<Box<Participation>>; PARTY_MAX],
    characters: [Option<CharacterData>; PARTY_MAX],
    dirty: u8,
}

impl<'a, 'info> Party<'a, 'info> {
    /// Checks `remaining` against the session: exactly the pairs of [`party_keys`].
    pub fn load(
        remaining: &'a [AccountInfo<'info>],
        session: &GameSession,
        main: u8,
    ) -> Result<Self> {
        let (keys, n) = party_keys(session, main);
        require!(remaining.len() == n * 2, SessionError::InvalidPartyAccounts);
        let mut infos = [None; PARTY_MAX];
        let mut i = 0;
        for slot in 0..session.core.player_count {
            if slot == main || !session.player_alive(slot) {
                continue;
            }
            let (p, c) = (&remaining[i * 2], &remaining[i * 2 + 1]);
            require!(
                *p.key == keys[i].0 && *c.key == keys[i].1 && p.is_writable,
                SessionError::InvalidPartyAccounts
            );
            infos[slot as usize] = Some((p, c));
            i += 1;
        }
        Ok(Party {
            main,
            infos,
            loaded: core::array::from_fn(|_| None),
            characters: core::array::from_fn(|_| None),
            dirty: 0,
        })
    }

    fn ensure_loaded(&mut self, slot: u8) -> Result<()> {
        let s = slot as usize;
        let (info, _) = self
            .infos
            .get(s)
            .copied()
            .flatten()
            .ok_or(SessionError::InvalidPartyAccounts)?;
        if self.loaded[s].is_none() {
            let data = info.try_borrow_data()?;
            let p = Participation::try_deserialize(&mut &data[..])?;
            require!(p.core.slot == slot, SessionError::InvalidPartyAccounts);
            self.loaded[s] = Some(Box::new(p));
        }
        Ok(())
    }

    /// The callback's own `participation` slot.
    pub fn main(&self) -> u8 {
        self.main
    }

    /// Whether `slot` is one of these accounts (a living player other than the main one).
    pub fn has(&self, slot: u8) -> bool {
        self.infos.get(slot as usize).is_some_and(|i| i.is_some())
    }

    /// Player `slot`'s participation, decoded on first use; marks it to be written back.
    pub fn get(&mut self, slot: u8) -> Result<&mut Participation> {
        self.ensure_loaded(slot)?;
        self.dirty |= 1 << slot;
        Ok(self.loaded[slot as usize].as_mut().expect("loaded above"))
    }

    /// Player `slot`'s character with what it wears.
    pub fn hero(&mut self, slot: u8) -> Result<HeroStats> {
        let s = slot as usize;
        self.ensure_loaded(slot)?;
        if self.characters[s].is_none() {
            let (_, info) = self.infos[s].ok_or(SessionError::InvalidPartyAccounts)?;
            self.characters[s] = Some(read_character(info)?);
        }
        let c = self.characters[s].as_ref().expect("read above");
        Ok(self.loaded[s].as_ref().expect("loaded above").hero(c))
    }

    /// Takes player `slot`'s participation out (to work on it next to `&mut self`); give it back
    /// with [`Self::put`].
    pub fn take(&mut self, slot: u8) -> Result<Box<Participation>> {
        self.ensure_loaded(slot)?;
        self.dirty |= 1 << slot;
        Ok(self.loaded[slot as usize].take().expect("loaded above"))
    }

    pub fn put(&mut self, slot: u8, p: Box<Participation>) {
        self.loaded[slot as usize] = Some(p);
    }

    /// Writes back every participation that was handed out.
    pub fn store(&mut self) -> Result<()> {
        for s in 0..PARTY_MAX {
            if self.dirty & (1 << s) == 0 {
                continue;
            }
            if let (Some(p), Some((info, _))) = (&self.loaded[s], self.infos[s]) {
                let mut data = info.try_borrow_mut_data()?;
                p.try_serialize(&mut &mut data[..])?;
            }
        }
        Ok(())
    }
}
