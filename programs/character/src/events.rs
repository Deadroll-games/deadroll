use anchor_lang::prelude::*;

use deadroll_types::{Aspect, Class, EquipSlot, ItemBase, Race, STATS_COUNT};

use crate::state::{MAP_SKILLS, RESOURCE_KINDS};

#[event]
pub struct ProfileCreated {
    pub owner: Pubkey,
}

#[event]
pub struct CharacterCreated {
    pub character: Pubkey,
    pub owner: Pubkey,
    pub index: u8,
    pub race: Race,
    pub class: Class,
    pub predisposition: Aspect,
    pub nft: Option<Pubkey>,
    pub skill_count: u8,
    /// Soulbound starter items given with the character.
    pub item_count: u8,
}

#[event]
pub struct CharacterLocked {
    pub character: Pubkey,
    pub session: Pubkey,
}

#[event]
pub struct RewardsGranted {
    pub character: Pubkey,
    pub xp: u32,
    pub resources: [u32; RESOURCE_KINDS],
    pub level: u16,
    pub total_xp: u32,
}

#[event]
pub struct CharacterDied {
    pub character: Pubkey,
    pub owner: Pubkey,
    pub session: Pubkey,
    pub nft: Option<Pubkey>,
    pub state_version: u32,
}

#[event]
pub struct CharacterUnlocked {
    pub character: Pubkey,
    pub session: Pubkey,
}

#[event]
pub struct TombstoneFrozen {
    pub character: Pubkey,
    pub asset: Pubkey,
}

#[event]
pub struct ItemEquipped {
    pub character: Pubkey,
    pub slot: EquipSlot,
    /// The soulbound record worn.
    pub item_index: u8,
    pub base: ItemBase,
    /// What the slot held before, if anything.
    pub replaced: Option<ItemBase>,
}

#[event]
pub struct ItemUnequipped {
    pub character: Pubkey,
    pub slot: EquipSlot,
    pub base: Option<ItemBase>,
}

#[event]
pub struct StatPointsSpent {
    pub character: Pubkey,
    /// Points added, by `Stat`.
    pub points: [u16; STATS_COUNT],
    /// The core stats after spending, by `Stat`.
    pub stats: [u16; STATS_COUNT],
    pub unspent_points: u8,
    /// The stored maximum HP after spending (Endurance raises it).
    pub hp_max: u32,
}

#[event]
pub struct SkillsSelected {
    pub character: Pubkey,
    pub session: Pubkey,
    /// `SkillSet` indexes: 4 active, 1 reactive, 1 passive (`deadroll_skillcheck::NO_SKILL` = empty).
    pub selection: [u8; MAP_SKILLS],
}
