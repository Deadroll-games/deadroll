use anchor_lang::prelude::*;
use deadroll_rules::items::{check_equip, starter_loadout, EquipError};
use deadroll_types::{
    Class, EquipRef, EquipSlot, EquipSource, ItemBase, ItemRecord, EMPTY_ITEM, EQUIP_SLOTS,
    SOULBOUND_MAX,
};

use crate::{errors::CharacterError, rules::BALANCE};

/// What a character carries and wears (docs/CHARACTER.md, section 10; ITEMS_AND_CRAFT.md,
/// sections 2 and 10): the class starter items as soulbound records, which never leave the
/// character and take no bag slot, and the equipment by `EquipSlot`. `EquipSource::Asset` is
/// kept for item NFTs, so the layout does not change when they arrive.
///
/// Pure account logic: `crates/sim/tests/parity.rs` runs it against the simulator.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct Gear {
    /// Records `0..item_count` are used.
    pub items: [ItemRecord; SOULBOUND_MAX],
    pub item_count: u8,
    /// Indexed by `EquipSlot`.
    pub equipped: [EquipRef; EQUIP_SLOTS],
}

impl Gear {
    /// A new character's gear: the class starter kit, each item worn in its own slot.
    pub fn starter(class: Class) -> Gear {
        let kit = starter_loadout(class, &BALANCE.items);
        let mut gear = Gear {
            items: [ItemRecord::default(); SOULBOUND_MAX],
            item_count: kit.count,
            equipped: [EquipRef::EMPTY; EQUIP_SLOTS],
        };
        for (record, (base, level)) in gear.items.iter_mut().zip(&kit.items[..kit.count as usize]) {
            *record = ItemRecord {
                base: *base,
                level: *level,
            };
        }
        for (slot, worn) in gear.equipped.iter_mut().zip(kit.worn) {
            if let Some(index) = worn {
                *slot = EquipRef::soulbound(index);
            }
        }
        gear
    }

    /// The `ItemBase` byte worn in each slot (`EMPTY_ITEM` for none): what combat reads. Item
    /// NFTs (`EquipSource::Asset`) are not resolved yet and read as empty.
    pub fn equipment(&self) -> [u8; EQUIP_SLOTS] {
        let mut out = [EMPTY_ITEM; EQUIP_SLOTS];
        for (slot, r) in out.iter_mut().zip(&self.equipped) {
            if r.source == EquipSource::Soulbound && r.index < self.item_count {
                if let Some(item) = self.items.get(r.index as usize) {
                    *slot = item.base as u8;
                }
            }
        }
        out
    }

    /// The slot record `index` is worn in, if any.
    pub fn worn_slot(&self, index: u8) -> Option<EquipSlot> {
        self.equipped
            .iter()
            .position(|r| r.source == EquipSource::Soulbound && r.index == index)
            .and_then(|i| EquipSlot::ALL.get(i).copied())
    }

    /// Wears record `item_index` in `slot` for a character of `level`; returns what the slot
    /// held. An item is worn in one slot at a time (a ring moves to the other ring slot).
    pub fn equip(
        &mut self,
        level: u16,
        item_index: u8,
        slot: EquipSlot,
    ) -> Result<Option<ItemBase>> {
        require!(item_index < self.item_count, CharacterError::NoSuchItem);
        let item = self.items[item_index as usize];
        check_equip(level, item.base, item.level, slot, &BALANCE.items).map_err(|e| match e {
            EquipError::WrongSlot => error!(CharacterError::WrongSlot),
            EquipError::LevelTooLow => error!(CharacterError::LevelTooLow),
        })?;
        let replaced = ItemBase::try_from(self.equipment()[slot as usize]).ok();
        if let Some(old) = self.worn_slot(item_index) {
            self.equipped[old as usize] = EquipRef::EMPTY;
        }
        self.equipped[slot as usize] = EquipRef::soulbound(item_index);
        Ok(replaced)
    }

    /// Empties `slot`; returns what it held.
    pub fn unequip(&mut self, slot: EquipSlot) -> Result<Option<ItemBase>> {
        require!(
            self.equipped[slot as usize].source != EquipSource::Empty,
            CharacterError::SlotEmpty
        );
        let base = ItemBase::try_from(self.equipment()[slot as usize]).ok();
        self.equipped[slot as usize] = EquipRef::EMPTY;
        Ok(base)
    }

    /// Death: the soulbound items vanish with the character (ITEMS_AND_CRAFT.md, section 10).
    pub fn burn(&mut self) {
        // `EquipRef::default()` is `EquipRef::EMPTY`.
        *self = Gear::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(r: Result<Option<ItemBase>>) -> u32 {
        match r.unwrap_err() {
            Error::AnchorError(e) => e.error_code_number,
            e => panic!("{e:?}"),
        }
    }

    fn character_code(e: CharacterError) -> u32 {
        anchor_lang::error::ERROR_CODE_OFFSET + e as u32
    }

    #[test]
    fn starter_kits_are_worn() {
        for (class, weapon, armor) in [
            (Class::Warrior, ItemBase::Sword, ItemBase::LeatherArmor),
            (Class::Mage, ItemBase::Staff, ItemBase::Robe),
            (Class::Support, ItemBase::Wand, ItemBase::LightLeatherArmor),
        ] {
            let g = Gear::starter(class);
            assert_eq!(g.item_count, 2);
            assert_eq!(g.items[0].base, weapon);
            assert_eq!(g.items[1].base, armor);
            let eq = g.equipment();
            assert_eq!(eq[EquipSlot::MainHand as usize], weapon as u8);
            assert_eq!(eq[EquipSlot::Body as usize], armor as u8);
            assert_eq!(eq.iter().filter(|b| **b != EMPTY_ITEM).count(), 2);
            assert_eq!(
                eq,
                starter_loadout(class, &BALANCE.items).equipment().0,
                "same kit as the rules give the simulator"
            );
        }
    }

    #[test]
    fn unequip_then_equip_again() {
        let mut g = Gear::starter(Class::Warrior);
        assert_eq!(
            g.unequip(EquipSlot::Body).unwrap(),
            Some(ItemBase::LeatherArmor)
        );
        assert_eq!(g.equipment()[EquipSlot::Body as usize], EMPTY_ITEM);
        assert_eq!(g.equip(1, 1, EquipSlot::Body).unwrap(), None);
        assert_eq!(
            g.equipment()[EquipSlot::Body as usize],
            ItemBase::LeatherArmor as u8
        );
        // Re-wearing what is worn replaces it with itself.
        assert_eq!(
            g.equip(1, 0, EquipSlot::MainHand).unwrap(),
            Some(ItemBase::Sword)
        );
        assert_eq!(g.worn_slot(0), Some(EquipSlot::MainHand));
    }

    #[test]
    fn every_rejection() {
        let mut g = Gear::starter(Class::Warrior);
        assert_eq!(
            code(g.equip(1, 2, EquipSlot::MainHand)),
            character_code(CharacterError::NoSuchItem)
        );
        assert_eq!(
            code(g.equip(1, 0, EquipSlot::Body)),
            character_code(CharacterError::WrongSlot)
        );
        g.items[0].level = 10;
        assert_eq!(
            code(g.equip(1, 0, EquipSlot::MainHand)),
            character_code(CharacterError::LevelTooLow)
        );
        assert!(g.equip(6, 0, EquipSlot::MainHand).is_ok());
        assert_eq!(
            code(g.unequip(EquipSlot::Helmet)),
            character_code(CharacterError::SlotEmpty)
        );
    }

    #[test]
    fn burn_leaves_nothing() {
        let mut g = Gear::starter(Class::Mage);
        g.burn();
        assert_eq!(g.item_count, 0);
        assert_eq!(g.equipment(), [EMPTY_ITEM; EQUIP_SLOTS]);
        assert!(g.equipped.iter().all(|r| *r == EquipRef::EMPTY));
    }
}
