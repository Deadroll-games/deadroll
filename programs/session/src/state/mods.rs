//! Buffs and debuffs as account data: `deadroll_combat::support::TimedMod` with Anchor derives
//! (InitSpace, IDL). Field by field the same, so the Borsh bytes are the same (tested).
//!
//! The accounts keep the six slots they had before patch 9 ([`STORED_MODS`], the kinds up to
//! Vulnerability, each in its own slot) and, appended at their end, the Resistance slot with its
//! aspect weights ([`ResistModData`]). The resonance kinds (Exposed, Root, and the triple kinds
//! from Blinded to Dulled) take a stored slot left free, in kind order (`from_combat`).
use anchor_lang::prelude::*;
#[cfg(test)]
use deadroll_combat::MODS_MAX;
use deadroll_combat::{ModKind, TimedMod, TimedMods};
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
    /// Patch 9.
    Resistance,
    /// Resonance pass (resonances only).
    Exposed,
    Root,
    /// Triples (resonances only).
    Blinded,
    OffGuard,
    Sundered,
    Dazed,
    Silenced,
    NoHeal,
    Dulled,
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

/// Stored slots of the kinds before patch 9 (`MODS_MAX - 1`; a literal for the IDL, a test pins
/// it): one per kind, by `ModKind`.
pub const STORED_MODS: usize = 6;

/// A unit's buffs and debuffs except Resistance, one slot per kind.
pub type ModsData = [TimedModData; STORED_MODS];

/// Patch 9: a unit's Resistance (`kind == Resistance`, `turns == 0` = none) and its aspect
/// weights (`deadroll_combat::TimedMod::weights_pct`). Borsh-identical to a `TimedMod`, so a hit
/// event mirrors its target modifiers with it too.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct ResistModData {
    pub data: TimedModData,
    pub weights_pct: [u8; Aspect::COUNT],
}

/// Borsh size of one [`ResistModData`].
pub const RESIST_MOD_SIZE: usize = TIMED_MOD_SIZE + Aspect::COUNT;

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
            ModKind::Resistance => Self::Resistance,
            ModKind::Exposed => Self::Exposed,
            ModKind::Root => Self::Root,
            ModKind::Blinded => Self::Blinded,
            ModKind::OffGuard => Self::OffGuard,
            ModKind::Sundered => Self::Sundered,
            ModKind::Dazed => Self::Dazed,
            ModKind::Silenced => Self::Silenced,
            ModKind::NoHeal => Self::NoHeal,
            ModKind::Dulled => Self::Dulled,
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
            ModKindData::Resistance => Self::Resistance,
            ModKindData::Exposed => Self::Exposed,
            ModKindData::Root => Self::Root,
            ModKindData::Blinded => Self::Blinded,
            ModKindData::OffGuard => Self::OffGuard,
            ModKindData::Sundered => Self::Sundered,
            ModKindData::Dazed => Self::Dazed,
            ModKindData::Silenced => Self::Silenced,
            ModKindData::NoHeal => Self::NoHeal,
            ModKindData::Dulled => Self::Dulled,
        }
    }
}

fn mod_of(m: &TimedModData, weights_pct: [u8; Aspect::COUNT]) -> TimedMod {
    TimedMod {
        kind: m.kind.into(),
        aspect: m.aspect,
        turns: m.turns,
        power_bp: m.power_bp,
        source: m.source,
        weights_pct,
    }
}

/// One stored slot of `m` (its weights dropped).
pub fn mod_data(m: &TimedMod) -> TimedModData {
    data_of(m)
}

fn data_of(m: &TimedMod) -> TimedModData {
    TimedModData {
        kind: m.kind.into(),
        aspect: m.aspect,
        turns: m.turns,
        power_bp: m.power_bp,
        source: m.source,
    }
}

/// The stored slots and the Resistance slot as one `TimedMods`, each kind in its own slot.
pub fn to_combat(mods: &ModsData, resist: &ResistModData) -> TimedMods {
    let mut out = TimedMods::default();
    for m in mods.iter().filter(|m| m.turns > 0) {
        let kind = ModKind::from(m.kind);
        out[kind as usize] = mod_of(m, [0; Aspect::COUNT]);
    }
    out[ModKind::Resistance as usize] = mod_of(&resist.data, resist.weights_pct);
    out
}

/// `mods` as the stored slots and the Resistance slot. The kinds before patch 9 keep their own
/// slot; the resonance kinds (Exposed, Root, Blinded to Dulled) take the stored slots left free,
/// in kind order, and are dropped when none is (the account size stays).
pub fn from_combat(mods: &TimedMods) -> (ModsData, ResistModData) {
    let mut out = ModsData::default();
    let mut resist = ResistModData::default();
    let mut late = [None; ModKind::COUNT - STORED_MODS - 1];
    for m in mods.iter().filter(|m| !m.is_empty()) {
        match m.kind {
            ModKind::Resistance => {
                resist = ResistModData {
                    data: data_of(m),
                    weights_pct: m.weights_pct,
                }
            }
            ModKind::Haste
            | ModKind::Slow
            | ModKind::Empowerment
            | ModKind::Weakening
            | ModKind::Protection
            | ModKind::Vulnerability => out[m.kind as usize] = data_of(m),
            // The kinds after Resistance, in kind order.
            late_kind => late[late_kind as usize - STORED_MODS - 1] = Some(data_of(m)),
        }
    }
    for m in late.into_iter().flatten() {
        if let Some(slot) = out.iter_mut().find(|s| s.turns == 0) {
            *slot = m;
        }
    }
    (out, resist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_the_combat_bytes() {
        let mut mods = TimedMods::default();
        for (i, kind) in ModKind::ALL[..=ModKind::Resistance as usize]
            .iter()
            .copied()
            .enumerate()
        {
            mods[i] = TimedMod {
                kind,
                aspect: Aspect::ALL[i % Aspect::COUNT],
                turns: i as u8 + 1,
                power_bp: 1_000 * i as u32 + 7,
                source: 4 + i as u8,
                weights_pct: [0; Aspect::COUNT],
            };
        }
        mods[ModKind::Resistance as usize].weights_pct = [65, 0, 35, 0, 0, 65];
        let (data, resist) = from_combat(&mods);
        // A stored slot is the combat modifier without its (Resistance-only) weights.
        let one = borsh::to_vec(&mods[0]).unwrap();
        assert_eq!(borsh::to_vec(&data[0]).unwrap(), one[..TIMED_MOD_SIZE]);
        assert_eq!(to_combat(&data, &resist), mods);
        assert_eq!(resist.weights_pct, [65, 0, 35, 0, 0, 65]);
        assert_eq!(STORED_MODS, ModKind::Resistance as usize);
        assert_eq!(MODS_MAX, ModKind::COUNT);
        assert_eq!(TimedModData::INIT_SPACE, TIMED_MOD_SIZE);
        assert_eq!(ResistModData::INIT_SPACE, RESIST_MOD_SIZE);
        assert_eq!(
            borsh::to_vec(&TimedModData::default()).unwrap().len(),
            TIMED_MOD_SIZE
        );
    }

    #[test]
    fn every_kind_goes_back_into_its_own_slot() {
        // Written before patch 9: Slow in the first slot.
        let mut legacy = TimedMods::default();
        legacy[0] = TimedMod {
            kind: ModKind::Slow,
            turns: 2,
            power_bp: 500,
            ..TimedMod::default()
        };
        legacy[1] = TimedMod {
            kind: ModKind::Resistance,
            turns: 1,
            power_bp: 900,
            weights_pct: [100, 0, 0, 0, 0, 0],
            ..TimedMod::default()
        };
        let (data, resist) = from_combat(&legacy);
        let back = to_combat(&data, &resist);
        assert!(back[0].is_empty());
        assert_eq!(back[ModKind::Slow as usize].power_bp, 500);
        assert_eq!(back[ModKind::Resistance as usize].power_bp, 900);
        assert_eq!(back[ModKind::Resistance as usize].weights_pct[0], 100);
    }

    #[test]
    fn the_triple_kinds_take_the_free_stored_slots_too() {
        let m = |kind, power_bp| TimedMod {
            kind,
            turns: 2,
            power_bp,
            ..TimedMod::default()
        };
        let mut mods = TimedMods::default();
        mods[ModKind::Haste as usize] = m(ModKind::Haste, 500);
        mods[ModKind::Dazed as usize] = m(ModKind::Dazed, 10_000);
        mods[ModKind::Silenced as usize] = m(ModKind::Silenced, 10_000);
        mods[ModKind::Dulled as usize] = m(ModKind::Dulled, 7_000);
        let (data, resist) = from_combat(&mods);
        assert_eq!(data[0].kind, ModKindData::Haste);
        assert_eq!(data[1].kind, ModKindData::Dazed);
        assert_eq!(data[2].kind, ModKindData::Silenced);
        assert_eq!(data[3].kind, ModKindData::Dulled);
        assert_eq!(to_combat(&data, &resist), mods);
        // Every kind survives the data enum and back.
        for kind in ModKind::ALL {
            assert_eq!(ModKind::from(ModKindData::from(kind)), kind);
        }
        // Nine late kinds, six slots: the first six in kind order are kept.
        let mut many = TimedMods::default();
        for kind in &ModKind::ALL[ModKind::Exposed as usize..] {
            many[*kind as usize] = m(*kind, 100);
        }
        let (data, resist) = from_combat(&many);
        let back = to_combat(&data, &resist);
        assert!(!back[ModKind::Exposed as usize].is_empty());
        assert!(!back[ModKind::Dazed as usize].is_empty());
        assert!(back[ModKind::Silenced as usize].is_empty());
        assert!(back[ModKind::Dulled as usize].is_empty());
    }

    #[test]
    fn exposed_and_root_take_the_free_stored_slots() {
        let m = |kind, power_bp| TimedMod {
            kind,
            turns: 2,
            power_bp,
            ..TimedMod::default()
        };
        let mut mods = TimedMods::default();
        mods[ModKind::Slow as usize] = m(ModKind::Slow, 500);
        mods[ModKind::Exposed as usize] = m(ModKind::Exposed, 2_000);
        mods[ModKind::Root as usize] = m(ModKind::Root, 10_000);
        let (data, resist) = from_combat(&mods);
        assert_eq!(data[0].kind, ModKindData::Exposed);
        assert_eq!(data[2].kind, ModKindData::Root);
        assert_eq!(to_combat(&data, &resist), mods);
        // Every stored slot taken: they are dropped.
        let mut full = TimedMods::default();
        for kind in &ModKind::ALL[..STORED_MODS] {
            full[*kind as usize] = m(*kind, 100);
        }
        full[ModKind::Root as usize] = m(ModKind::Root, 10_000);
        let (data, resist) = from_combat(&full);
        assert!(to_combat(&data, &resist)[ModKind::Root as usize].is_empty());
    }
}
