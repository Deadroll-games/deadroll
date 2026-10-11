use anchor_lang::prelude::*;
use deadroll_types::{Aspect, Class, Race};

use crate::state::RESOURCE_KINDS;

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
