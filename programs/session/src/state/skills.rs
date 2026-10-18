//! Patch 5 skills as session data: the skills taken onto the map (copied at `set_ready(true)`
//! with their `SkillExtras` part), the reactive skill of every player, the auras of the party
//! (docs/SKILL_BUILDER.md, sections 3.2-3.5) and a roll waiting only for reactives.
use anchor_lang::prelude::*;
use deadroll_rules::{
    aura::{Aura, PartyAuras, PlayerAuras, AURAS_PER_PLAYER},
    input::SkillStats,
};
use deadroll_skillcheck::{SkillBlocks, SkillExt, NO_SKILL, SKILL_ASPECTS_MAX};
use deadroll_types::{Aspect, PARTY_MAX};

use super::mods::ModKindData;

/// A skill taken onto the map as the rules see it (`deadroll_rules::input::SkillStats`). The
/// character is locked for the party, so the copy stays exact.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct MapSkillData {
    pub blocks: SkillBlocks,
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
    /// In the character's turns, `0` = none.
    pub cooldown: u8,
    pub ext: SkillExt,
    /// Action points to use it (`0` for reactive and passive skills).
    pub ap_cost: u8,
}

impl From<&MapSkillData> for SkillStats {
    fn from(s: &MapSkillData) -> Self {
        SkillStats {
            blocks: s.blocks,
            shares_bp: s.shares_bp,
            power_bp: s.power_bp,
            cooldown: s.cooldown,
            ext: s.ext,
            ap_cost: s.ap_cost,
        }
    }
}

impl From<&SkillStats> for MapSkillData {
    fn from(s: &SkillStats) -> Self {
        MapSkillData {
            blocks: s.blocks,
            shares_bp: s.shares_bp,
            power_bp: s.power_bp,
            cooldown: s.cooldown,
            ext: s.ext,
            ap_cost: s.ap_cost,
        }
    }
}

/// A player's reactive skill on the map: kept in the session, so a trigger is checked without
/// loading the player's accounts. `skill_slot == NO_SKILL`: none.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReactiveData {
    /// Its `SkillSet` index.
    pub skill_slot: u8,
    pub skill: MapSkillData,
    /// Own turn starts until it can trigger again (`rules::cooldown_after_use`), counted down by
    /// `start_turn`.
    pub cooldown_left: u8,
}

impl Default for ReactiveData {
    fn default() -> Self {
        ReactiveData {
            skill_slot: NO_SKILL,
            skill: MapSkillData::default(),
            cooldown_left: 0,
        }
    }
}

impl ReactiveData {
    pub fn skill(&self) -> Option<SkillStats> {
        (self.skill_slot != NO_SKILL).then(|| SkillStats::from(&self.skill))
    }
}

/// `deadroll_rules::aura::Aura`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct AuraData {
    pub kind: ModKindData,
    pub aspect: Aspect,
    pub power_bp: u32,
    pub radius: u8,
    /// Own turn starts of the bearer until it ends, `255` = permanent (a passive), 0 = none.
    pub turns: u8,
    /// A debuff aura (on the enemies around).
    pub hostile: bool,
    /// From a passive skill.
    pub passive: bool,
    /// A passive skill without an aura: on its bearer only.
    pub self_only: bool,
}

impl From<AuraData> for Aura {
    fn from(a: AuraData) -> Self {
        Aura {
            kind: a.kind.into(),
            aspect: a.aspect,
            power_bp: a.power_bp,
            radius: a.radius,
            turns: a.turns,
            hostile: a.hostile,
            passive: a.passive,
            self_only: a.self_only,
            kin_only: false,
        }
    }
}

impl From<Aura> for AuraData {
    fn from(a: Aura) -> Self {
        AuraData {
            kind: a.kind.into(),
            aspect: a.aspect,
            power_bp: a.power_bp,
            radius: a.radius,
            turns: a.turns,
            hostile: a.hostile,
            passive: a.passive,
            self_only: a.self_only,
        }
    }
}

pub type PartyAurasData = [[AuraData; AURAS_PER_PLAYER]; PARTY_MAX];

pub fn auras_to_rules(a: &PartyAurasData) -> PartyAuras {
    a.map(|p| p.map(Aura::from))
}

pub fn player_auras(a: &[AuraData; AURAS_PER_PLAYER]) -> PlayerAuras {
    a.map(Aura::from)
}

/// What a reactive-only roll answers (`PendingReactiveData::kind`).
pub mod pending_kind {
    pub const NONE: u8 = 0;
    /// An enemy entered a cell next to the bearers (`monster_move`).
    pub const ENEMY_ENTERED: u8 = 1;
    /// Over-time damage at the start of `victim`'s turn dropped its HP (`start_turn`).
    pub const HP_DROP: u8 = 2;
}

/// A roll that only resolves monster reactives (patch 6), requested by `move_unit` (a player
/// came next to monsters) or `start_turn` (over-time damage dropped a monster's HP): what
/// happened, and to which monsters. `kind` as [`pending_kind`].
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct PendingMonsterReactiveData {
    pub kind: u8,
    /// Bit per monster index whose reactive answers.
    pub bearers: u32,
    /// Turn-order id of the unit that caused it (`EMPTY_UNIT` = none).
    pub source: u8,
    /// `HP_DROP`: the monster whose HP dropped, from `hp_before` to `hp_after` of `hp_max`.
    pub victim: u8,
    pub hp_before: u32,
    pub hp_after: u32,
    pub hp_max: u32,
}

/// A roll that only resolves reactives, requested by `monster_move` or `start_turn` (they have
/// no roll of their own): what happened, and to whom.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct PendingReactiveData {
    pub kind: u8,
    /// Bit per player slot whose reactive answers.
    pub bearers: u8,
    /// Turn-order id of the unit that caused it (`EMPTY_UNIT` = none).
    pub source: u8,
    /// `HP_DROP`: the player whose HP dropped, from `hp_before` to `hp_after` of `hp_max`.
    pub victim: u8,
    pub hp_before: u32,
    pub hp_after: u32,
    pub hp_max: u32,
}
