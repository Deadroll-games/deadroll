use anchor_lang::prelude::*;
use deadroll_character::state::{MAP_SKILLS, SKILLS_MAX};
use deadroll_combat::{chill_slow_bp, speed_ap, AccuracyBuff, TimedMods};
use deadroll_rules::loot::cache_add;
use deadroll_rules::{aura::AuraField, input::SkillStats, items::Equipment, HeroStats};
use deadroll_skillcheck::{ACTIVE_MAX, NO_SKILL};
use deadroll_types::Aspect;
use deadroll_types::{
    CacheSlot, Cell, CharacterData, ParticipationData, CACHE_BYTES, CACHE_SLOTS, EQUIP_SLOTS,
};

use super::mods::{self, ResistModData, TimedModData, STORED_MODS};
use super::MapSkillData;

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
    /// Buffs and debuffs on this player (`deadroll_combat::support`), one slot per kind (the
    /// Resistance is `resist_mod`); `start_turn` counts them down.
    pub mods: [TimedModData; STORED_MODS],
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
    /// Patch 4: a resonance's accuracy buff on this player (Pale Flame), `turns == 0` = none.
    pub accuracy: AccuracyBuffData,
    /// Patch 5: the skills of `skills`, slot by slot, as the rules see them (the character's
    /// `SkillSet` record with its `SkillExtras` part), copied at `set_ready(true)`.
    pub map_skills: [MapSkillData; MAP_SKILLS],
    /// Patch 9: the Resistance on this player with its aspect weights (`mods` has the others).
    pub resist_mod: ResistModData,
    // Patch 11: the temporary cache (docs/PARTY_AND_TURNS.md 8, 10.2), filled during the party.
    // Last in the account so a callback writes it in place for a participation it never decoded
    // ([`add_to_cache_raw`]).
    /// Experience of the kills, the player's share already applied.
    pub cache_xp: u32,
    /// The rewards, `CacheSlot`s as raw bytes (`CacheSlot::read_all`): never decoded field by
    /// field.
    pub cache: [u8; CACHE_BYTES],
}

/// Account size, discriminator included.
pub const PARTICIPATION_SPACE: usize = 8 + Participation::INIT_SPACE;
/// Offset of `cache_xp` in the account (discriminator included): the last two fields, and
/// nothing before them has a variable size.
pub const CACHE_XP_AT: usize = PARTICIPATION_SPACE - 4 - CACHE_BYTES;
pub const CACHE_AT: usize = CACHE_XP_AT + 4;

/// Adds experience and rewards to a cache. Rewards that do not fit are lost (the event shows
/// them); returns how many were lost.
pub fn add_to_cache(
    cache_xp: &mut u32,
    cache: &mut [u8; CACHE_BYTES],
    xp: u32,
    rewards: &[CacheSlot],
) -> u8 {
    *cache_xp = cache_xp.saturating_add(xp);
    if rewards.iter().all(|r| r.is_empty()) {
        return 0;
    }
    let mut slots = CacheSlot::read_all(cache);
    let lost = rewards
        .iter()
        .filter(|r| !cache_add(&mut slots, **r))
        .count() as u8;
    *cache = CacheSlot::write_all(&slots);
    lost
}

/// [`add_to_cache`] on the raw bytes of a `Participation` account (`data` from its discriminator).
pub fn add_to_cache_raw(data: &mut [u8], xp: u32, rewards: &[CacheSlot]) -> Result<u8> {
    require!(
        data.len() >= PARTICIPATION_SPACE,
        crate::errors::SessionError::InvalidPartyAccounts
    );
    let mut cache_xp = u32::from_le_bytes(data[CACHE_XP_AT..CACHE_AT].try_into().expect("4 bytes"));
    let mut cache: [u8; CACHE_BYTES] = data[CACHE_AT..CACHE_AT + CACHE_BYTES]
        .try_into()
        .expect("CACHE_BYTES");
    let lost = add_to_cache(&mut cache_xp, &mut cache, xp, rewards);
    data[CACHE_XP_AT..CACHE_AT].copy_from_slice(&cache_xp.to_le_bytes());
    data[CACHE_AT..CACHE_AT + CACHE_BYTES].copy_from_slice(&cache);
    Ok(lost)
}

/// `deadroll_combat::AccuracyBuff`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct AccuracyBuffData {
    /// Attacks with a packet of this aspect get the bonus.
    pub aspect: Aspect,
    /// Own turn starts until it ends.
    pub turns: u8,
    /// Added to the d20.
    pub bonus: u32,
}

impl From<AccuracyBuffData> for AccuracyBuff {
    fn from(b: AccuracyBuffData) -> Self {
        AccuracyBuff {
            aspect: b.aspect,
            turns: b.turns,
            bonus: b.bonus,
        }
    }
}

impl From<AccuracyBuff> for AccuracyBuffData {
    fn from(b: AccuracyBuff) -> Self {
        AccuracyBuffData {
            aspect: b.aspect,
            turns: b.turns,
            bonus: b.bonus,
        }
    }
}

impl Participation {
    /// Refills action points if this is the first action of a new own turn: speed, with Haste
    /// and Slow as an opposing pair, auras over the player included.
    pub fn refresh_ap(&mut self, turn_seq: u32, auras: &AuraField) {
        if self.ap_seq != turn_seq {
            self.ap_seq = turn_seq;
            let chill = chill_slow_bp(&self.core.states, &crate::rules::BALANCE.combat);
            let mods = self.mods_with_auras(auras, false);
            self.core.action_points = speed_ap(self.max_ap, &mods, chill);
        }
    }

    /// This player's buffs and debuffs with the auras over its cell (a view for one calculation).
    /// `basic_attack`: for the attacker of a basic attack (no passive Empowerment).
    pub fn mods_with_auras(&self, auras: &AuraField, basic_attack: bool) -> TimedMods {
        let at = Cell {
            x: self.core.x,
            y: self.core.y,
        };
        auras.hero_mods(&self.combat_mods(), self.core.slot, at, basic_attack)
    }

    /// The map skill in `SkillSet` slot `skill`, if it is one of the skills taken onto the map.
    pub fn map_skill(&self, skill: u8) -> Option<SkillStats> {
        if skill == NO_SKILL {
            return None;
        }
        let i = self.skills.iter().position(|s| *s == skill)?;
        Some(SkillStats::from(&self.map_skills[i]))
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
        mods::to_combat(&self.mods, &self.resist_mod)
    }

    pub fn set_combat_mods(&mut self, m: &TimedMods) {
        (self.mods, self.resist_mod) = mods::from_combat(m);
    }

    /// [`add_to_cache`] on this participation.
    pub fn add_to_cache(&mut self, xp: u32, rewards: &[CacheSlot]) -> u8 {
        add_to_cache(&mut self.cache_xp, &mut self.cache, xp, rewards)
    }

    /// The cache's rewards.
    pub fn cache_slots(&self) -> [CacheSlot; CACHE_SLOTS] {
        CacheSlot::read_all(&self.cache)
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
                game_session: Pubkey::default(),
                character: Pubkey::default(),
                session_key: Pubkey::default(),
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
            accuracy: Default::default(),
            map_skills: Default::default(),
            resist_mod: Default::default(),
            cache_xp: 0,
            cache: [0; CACHE_BYTES],
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

    #[test]
    fn the_cache_is_written_in_place_at_the_end() {
        use anchor_lang::AccountSerialize;
        use deadroll_types::RewardKind;
        let mut p: Participation = zeroed();
        p.cache_xp = 7;
        let ore = CacheSlot {
            kind: RewardKind::Resource,
            id: 3,
            qty: 2,
        };
        p.add_to_cache(0, &[ore]);
        let mut data = Vec::new();
        p.try_serialize(&mut data).unwrap();
        assert_eq!(
            data.len(),
            PARTICIPATION_SPACE,
            "fixed size: the offsets hold"
        );
        // The raw path sees what the struct wrote, and adds onto it.
        let recipe = CacheSlot {
            kind: RewardKind::Recipe,
            id: 1,
            qty: 1,
        };
        assert_eq!(add_to_cache_raw(&mut data, 5, &[ore, recipe]).unwrap(), 0);
        let back = Participation::try_deserialize(&mut &data[..]).unwrap();
        assert_eq!(back.cache_xp, 12);
        let slots = back.cache_slots();
        assert_eq!(slots[0], CacheSlot { qty: 4, ..ore });
        assert_eq!(slots[1], recipe);
        assert!(slots[2..].iter().all(|s| s.is_empty()));
    }

    #[test]
    fn a_full_cache_loses_the_rest() {
        use deadroll_types::RewardKind;
        let (mut xp, mut cache) = (0, [0u8; CACHE_BYTES]);
        let all: Vec<CacheSlot> = (0..CACHE_SLOTS as u16)
            .map(|id| CacheSlot {
                kind: RewardKind::Resource,
                id,
                qty: 1,
            })
            .collect();
        assert_eq!(add_to_cache(&mut xp, &mut cache, 0, &all), 0);
        let extra = CacheSlot {
            kind: RewardKind::Blank,
            id: 0,
            qty: 1,
        };
        assert_eq!(
            add_to_cache(&mut xp, &mut cache, 0, &[extra, all[0]]),
            1,
            "a new kind is lost, a known one merges"
        );
        assert_eq!(CacheSlot::read_all(&cache)[0].qty, 2);
    }

    /// A participation with every field zero (decoded from zero bytes).
    fn zeroed() -> Participation {
        use anchor_lang::Discriminator;
        let mut data = vec![0u8; PARTICIPATION_SPACE];
        data[..8].copy_from_slice(Participation::DISCRIMINATOR);
        Participation::try_deserialize(&mut &data[..]).unwrap()
    }
}
