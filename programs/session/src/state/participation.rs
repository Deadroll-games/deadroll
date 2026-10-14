use anchor_lang::prelude::*;
use deadroll_character::state::{MAP_SKILLS, SKILLS_MAX};
use deadroll_combat::{speed_ap, TimedMods, MODS_MAX};
use deadroll_rules::{items::Equipment, HeroStats};
use deadroll_skillcheck::{ACTIVE_MAX, NO_SKILL};
use deadroll_types::{CharacterData, ParticipationData, EQUIP_SLOTS};

use super::mods::{self, TimedModData};

/// A character with its class trait tier and an equipment snapshot (`Participation.class_tier`,
/// `Participation.equipment`), as the rules see it.
pub fn hero_of(c: &CharacterData, class_tier: u8, equipment: &[u8; EQUIP_SLOTS]) -> HeroStats {
    HeroStats::new(c, class_tier).with_equipment(Equipment(*equipment))
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
    /// The character's maximum HP with its gear, copied at `set_ready(true)` (heals stop here).
    pub hp_max: u32,
    /// Buffs and debuffs on this player (`deadroll_combat::support`), one slot per kind;
    /// `start_turn` counts them down.
    pub mods: [TimedModData; MODS_MAX],
    /// What the character wears, `ItemBase` bytes by `EquipSlot` (`EMPTY_ITEM` = none), copied
    /// from `Character.gear` at `set_ready(true)`. The character program refuses gear changes
    /// while the character is locked into a session, so the copy stays exact.
    pub equipment: [u8; EQUIP_SLOTS],
    /// The skills taken onto this map, `SkillSet` indexes: slots `0..4` active, `4` reactive,
    /// `5` passive, `NO_SKILL` = empty (docs/SKILL_BUILDER.md, 3.4). Written at
    /// `set_ready(true)` after the character program checked it; cleared by `set_ready(false)`.
    pub skills: [u8; MAP_SKILLS],
    /// The character's class trait tier (`Character.class_tier`), copied at `set_ready(true)`
    /// with the gear; the locked character cannot change it during the party.
    pub class_tier: u8,
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
        hero_of(c, self.class_tier, &self.equipment)
    }

    /// Whether `SkillSet` slot `skill` is one of the active skills taken onto this map.
    pub fn active_selected(&self, skill: u8) -> bool {
        skill != NO_SKILL && self.skills[..ACTIVE_MAX].contains(&skill)
    }

    pub fn combat_mods(&self) -> TimedMods {
        mods::to_combat(&self.mods)
    }

    pub fn set_combat_mods(&mut self, m: &TimedMods) {
        self.mods = mods::from_combat(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_selected_active_skills() {
        let mut p = Participation {
            core: ParticipationData {
                version: 1,
                session: Pubkey::default(),
                character: Pubkey::default(),
                actor: Pubkey::default(),
                slot: 0,
                x: 0,
                y: 0,
                hp: 0,
                action_points: 0,
                states: Default::default(),
                ready: true,
                bump: 0,
            },
            player: Pubkey::default(),
            max_ap: 0,
            ap_seq: 0,
            contribution: 0,
            cooldowns: [0; SKILLS_MAX],
            hp_max: 0,
            mods: Default::default(),
            equipment: [0; EQUIP_SLOTS],
            // Actives 2, 0, 5 and an empty slot; reactive 1, passive 3.
            skills: [2, 0, NO_SKILL, 5, 1, 3],
            class_tier: 14,
        };
        for s in [0, 2, 5] {
            assert!(p.active_selected(s), "{s}");
        }
        // Reactive, passive, not taken, the empty marker.
        for s in [1, 3, 4, 6, NO_SKILL] {
            assert!(!p.active_selected(s), "{s}");
        }
        p.skills = [NO_SKILL; MAP_SKILLS];
        assert!(!p.active_selected(0));
    }
}
