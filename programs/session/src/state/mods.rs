//! Buffs and debuffs as account data: `deadroll_combat::support::TimedMod` with Anchor derives
//! (InitSpace, IDL). Field by field the same, so the Borsh bytes are the same (tested).
use anchor_lang::prelude::*;
use deadroll_combat::{ModKind, TimedMod, TimedMods, MODS_MAX};
use deadroll_types::Aspect;

/// `deadroll_combat::ModKind`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub enum ModKindData {
    #[default]
    Haste,
    Slow,
    Empowerment,
    Weakening,
    Protection,
    Vulnerability,
}

/// `deadroll_combat::TimedMod`: a buff or debuff on a unit, `turns == 0` = empty slot.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct TimedModData {
    pub kind: ModKindData,
    /// The skill's aspect (Vulnerability is to this aspect).
    pub aspect: Aspect,
    /// Own turn starts of the bearer until it ends.
    pub turns: u8,
    pub power_bp: u32,
    /// Turn-order id of the unit that applied it.
    pub source: u8,
}

/// A unit's buffs and debuffs, one slot per kind.
pub type ModsData = [TimedModData; MODS_MAX];

/// Borsh size of one [`TimedModData`].
pub const TIMED_MOD_SIZE: usize = 8;

impl From<ModKind> for ModKindData {
    fn from(k: ModKind) -> Self {
        match k {
            ModKind::Haste => Self::Haste,
            ModKind::Slow => Self::Slow,
            ModKind::Empowerment => Self::Empowerment,
            ModKind::Weakening => Self::Weakening,
            ModKind::Protection => Self::Protection,
            ModKind::Vulnerability => Self::Vulnerability,
        }
    }
}

impl From<ModKindData> for ModKind {
    fn from(k: ModKindData) -> Self {
        match k {
            ModKindData::Haste => Self::Haste,
            ModKindData::Slow => Self::Slow,
            ModKindData::Empowerment => Self::Empowerment,
            ModKindData::Weakening => Self::Weakening,
            ModKindData::Protection => Self::Protection,
            ModKindData::Vulnerability => Self::Vulnerability,
        }
    }
}

pub fn to_combat(mods: &ModsData) -> TimedMods {
    mods.map(|m| TimedMod {
        kind: m.kind.into(),
        aspect: m.aspect,
        turns: m.turns,
        power_bp: m.power_bp,
        source: m.source,
    })
}

pub fn from_combat(mods: &TimedMods) -> ModsData {
    mods.map(|m| TimedModData {
        kind: m.kind.into(),
        aspect: m.aspect,
        turns: m.turns,
        power_bp: m.power_bp,
        source: m.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_the_combat_bytes() {
        let mut mods = TimedMods::default();
        for (i, kind) in ModKind::ALL.into_iter().enumerate() {
            mods[i] = TimedMod {
                kind,
                aspect: Aspect::ALL[i % Aspect::COUNT],
                turns: i as u8 + 1,
                power_bp: 1_000 * i as u32 + 7,
                source: 4 + i as u8,
            };
        }
        let data = from_combat(&mods);
        assert_eq!(borsh::to_vec(&data).unwrap(), borsh::to_vec(&mods).unwrap());
        assert_eq!(to_combat(&data), mods);
        assert_eq!(TimedModData::INIT_SPACE, TIMED_MOD_SIZE);
        assert_eq!(
            borsh::to_vec(&TimedModData::default()).unwrap().len(),
            TIMED_MOD_SIZE
        );
    }
}
