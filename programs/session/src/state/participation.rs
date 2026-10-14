use anchor_lang::prelude::*;
use deadroll_character::state::SKILLS_MAX;
use deadroll_combat::{speed_ap, TimedMods, MODS_MAX};
use deadroll_rules::{items::Equipment, HeroStats};
use deadroll_types::{CharacterData, ParticipationData, EQUIP_SLOTS};

use super::mods::{self, TimedModData};

/// A character with an equipment snapshot (`Participation.equipment`), as the rules see it.
pub fn hero_of(c: &CharacterData, equipment: &[u8; EQUIP_SLOTS]) -> HeroStats {
    HeroStats::from(c).with_equipment(Equipment(*equipment))
}

/// `["participation", session, character]`: one character in one party.
#[account]
#[derive(InitSpace)]
pub struct Participation {
    /// Frozen contract, first after the discriminator.
    pub core: ParticipationData,
    /// Main wallet (character owner) that joined and paid.
    pub player: Pubkey,
    /// Action points granted at the start of each own turn (from speed).
    pub max_ap: u8,
    /// `GameSession.turn_seq` when `core.action_points` was last granted.
    pub ap_seq: u32,
    /// Lamports paid into the escrow, for refunds.
    pub contribution: u64,
    /// Per `SkillSet` slot: own turn starts until the skill is usable again, 0 = usable
    /// (`rules::cooldown_after_use`; `start_turn` counts them down).
    pub cooldowns: [u8; SKILLS_MAX],
    /// The character's maximum HP, copied at `join_session` (heals stop here).
    pub hp_max: u32,
    /// Buffs and debuffs on this player (`deadroll_combat::support`), one slot per kind;
    /// `start_turn` counts them down.
    pub mods: [TimedModData; MODS_MAX],
    /// What the character wears, `ItemBase` bytes by `EquipSlot` (`EMPTY_ITEM` = none), copied
    /// from its items `Loadout` at `join_session`. The items program locks the loadout while
    /// the character is in a session, so the copy stays exact.
    pub equipment: [u8; EQUIP_SLOTS],
}

impl Participation {
    /// Refills action points if this is the first action of a new own turn: speed, with Haste
    /// and Slow as an opposing pair.
    pub fn refresh_ap(&mut self, turn_seq: u32) {
        if self.ap_seq != turn_seq {
            self.ap_seq = turn_seq;
            self.core.action_points = speed_ap(self.max_ap, &self.combat_mods());
        }
    }

    /// The rules' view of this participant: the character with what it wears.
    pub fn hero(&self, c: &CharacterData) -> HeroStats {
        hero_of(c, &self.equipment)
    }

    pub fn combat_mods(&self) -> TimedMods {
        mods::to_combat(&self.mods)
    }

    pub fn set_combat_mods(&mut self, m: &TimedMods) {
        self.mods = mods::from_combat(m);
    }
}
