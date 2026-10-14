use anchor_lang::prelude::*;
use deadroll_rules::items::{check_equip, starter_loadout, EquipError};
use deadroll_types::{
    Class, EquipRef, EquipSlot, EquipSource, ItemBase, ItemRecord, LoadoutData, EQUIP_SLOTS,
    SOULBOUND_MAX,
};

use crate::{
    errors::ItemsError,
    rules::{ACCOUNT_VERSION, BALANCE},
};

/// `["loadout", character]`: soulbound items and equipment by slot. The core is
/// `deadroll_types::LoadoutData` first after the discriminator, so the session program reads it
/// as that type; fields of this lane are only appended after it.
#[account]
#[derive(InitSpace)]
pub struct Loadout {
    pub core: LoadoutData,
}

/// A new loadout: the class starter kit (ITEMS_AND_CRAFT 10), each item worn in its own slot.
pub fn starter(character: Pubkey, owner: Pubkey, class: Class, bump: u8) -> LoadoutData {
    let kit = starter_loadout(class, &BALANCE.items);
    let mut items = [ItemRecord::default(); SOULBOUND_MAX];
    for (record, (base, level)) in items.iter_mut().zip(&kit.items[..kit.count as usize]) {
        *record = ItemRecord {
            base: *base,
            level: *level,
        };
    }
    let mut equipped = [EquipRef::EMPTY; EQUIP_SLOTS];
    for (slot, worn) in equipped.iter_mut().zip(kit.worn) {
        if let Some(index) = worn {
            *slot = EquipRef::soulbound(index);
        }
    }
    LoadoutData {
        version: ACCOUNT_VERSION,
        character,
        owner,
        items,
        item_count: kit.count,
        equipped,
        bump,
    }
}

/// Wears record `item_index` in `slot` for a character of `level`; returns what the slot held.
/// An item is worn in one slot at a time (a ring moves to the other ring slot).
pub fn equip(
    l: &mut LoadoutData,
    level: u16,
    item_index: u8,
    slot: EquipSlot,
) -> Result<Option<ItemBase>> {
    require!(item_index < l.item_count, ItemsError::NoSuchItem);
    let item = l.items[item_index as usize];
    check_equip(level, item.base, item.level, slot, &BALANCE.items).map_err(|e| match e {
        EquipError::WrongSlot => error!(ItemsError::WrongSlot),
        EquipError::LevelTooLow => error!(ItemsError::LevelTooLow),
    })?;
    let replaced = ItemBase::try_from(l.equipment()[slot as usize]).ok();
    if let Some(old) = l.worn_slot(item_index) {
        l.equipped[old as usize] = EquipRef::EMPTY;
    }
    l.equipped[slot as usize] = EquipRef::soulbound(item_index);
    Ok(replaced)
}

/// Empties `slot`; returns what it held.
pub fn unequip(l: &mut LoadoutData, slot: EquipSlot) -> Result<Option<ItemBase>> {
    require!(
        l.equipped[slot as usize].source != EquipSource::Empty,
        ItemsError::SlotEmpty
    );
    let base = ItemBase::try_from(l.equipment()[slot as usize]).ok();
    l.equipped[slot as usize] = EquipRef::EMPTY;
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::EMPTY_ITEM;

    fn warrior() -> LoadoutData {
        starter(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Class::Warrior,
            254,
        )
    }

    fn code(r: Result<Option<ItemBase>>) -> u32 {
        match r.unwrap_err() {
            Error::AnchorError(e) => e.error_code_number,
            e => panic!("{e:?}"),
        }
    }

    fn items_code(e: ItemsError) -> u32 {
        anchor_lang::error::ERROR_CODE_OFFSET + e as u32
    }

    #[test]
    fn starter_kits_are_worn() {
        for (class, weapon, armor) in [
            (Class::Warrior, ItemBase::Sword, ItemBase::LeatherArmor),
            (Class::Mage, ItemBase::Staff, ItemBase::Robe),
            (Class::Support, ItemBase::Wand, ItemBase::LightLeatherArmor),
        ] {
            let l = starter(Pubkey::default(), Pubkey::default(), class, 1);
            assert_eq!((l.version, l.item_count, l.bump), (ACCOUNT_VERSION, 2, 1));
            assert_eq!(l.items[0].base, weapon);
            assert_eq!(l.items[1].base, armor);
            let eq = l.equipment();
            assert_eq!(eq[EquipSlot::MainHand as usize], weapon as u8);
            assert_eq!(eq[EquipSlot::Body as usize], armor as u8);
            assert_eq!(eq.iter().filter(|b| **b != EMPTY_ITEM).count(), 2);
        }
    }

    #[test]
    fn unequip_then_equip_again() {
        let mut l = warrior();
        assert_eq!(
            unequip(&mut l, EquipSlot::Body).unwrap(),
            Some(ItemBase::LeatherArmor)
        );
        assert_eq!(l.equipment()[EquipSlot::Body as usize], EMPTY_ITEM);
        assert_eq!(equip(&mut l, 1, 1, EquipSlot::Body).unwrap(), None);
        assert_eq!(
            l.equipment()[EquipSlot::Body as usize],
            ItemBase::LeatherArmor as u8
        );
        // Re-wearing what is worn replaces it with itself.
        assert_eq!(
            equip(&mut l, 1, 0, EquipSlot::MainHand).unwrap(),
            Some(ItemBase::Sword)
        );
        assert_eq!(l.worn_slot(0), Some(EquipSlot::MainHand));
    }

    #[test]
    fn every_rejection() {
        let mut l = warrior();
        assert_eq!(
            code(equip(&mut l, 1, 2, EquipSlot::MainHand)),
            items_code(ItemsError::NoSuchItem)
        );
        assert_eq!(
            code(equip(&mut l, 1, 0, EquipSlot::Body)),
            items_code(ItemsError::WrongSlot)
        );
        l.items[0].level = 10;
        assert_eq!(
            code(equip(&mut l, 1, 0, EquipSlot::MainHand)),
            items_code(ItemsError::LevelTooLow)
        );
        assert!(equip(&mut l, 6, 0, EquipSlot::MainHand).is_ok());
        assert_eq!(
            code(unequip(&mut l, EquipSlot::Helmet)),
            items_code(ItemsError::SlotEmpty)
        );
    }
}
