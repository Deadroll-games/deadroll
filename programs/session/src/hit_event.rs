//! `HitResolved` in the shape the website reads (app/src/chain/types.ts): `result` is the combat
//! WASM `HitResult` layout (crates/combat/wasm/README.md).
//!
//! The `*Data` types mirror `deadroll_combat`'s result types field by field, so their Borsh bytes
//! are the same. They exist only so the IDL (and the Codama client) describe the event. On-chain
//! the event is written straight from the boxed `HitResult` by [`emit_hit_resolved`]: building
//! the event struct would put another ~1 KB copy on the 4 KB stack frame.
use anchor_lang::{prelude::*, Discriminator};
use deadroll_combat::{
    packet::{PACKETS_MAX, SKILL_ASPECTS_MAX},
    resonance::TINT_ROLLS_MAX,
    EffectRoll, HitResult, Outcome,
};
use deadroll_rules::HeroStats;
use deadroll_types::{ActiveState, Aspect, ItemBase, StateKind, STATES_MAX};

use crate::events::HitResolved;

/// `deadroll_combat::Outcome`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitOutcome {
    Miss,
    Hit,
    Immune,
}

/// `deadroll_combat::Carrier`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrier {
    Race,
    Class,
    Item,
}

/// `deadroll_combat::Applied`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    None,
    Added,
    Refreshed,
    Froze,
}

/// `deadroll_combat::TintRoll`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TintRollData {
    pub base: Aspect,
    pub tint: Aspect,
    pub carrier: Carrier,
    pub tier: u8,
    pub chance_bp: u32,
    pub roll: u16,
    pub success: bool,
}

/// `deadroll_combat::Resonance`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResonanceData {
    pub base: Aspect,
    pub tint: Aspect,
    pub tier: u8,
    pub carrier: Carrier,
    /// `base * 6 + tint`.
    pub id: u8,
    pub active: bool,
    pub second_tint: Option<Aspect>,
}

/// `deadroll_combat::StateRoll`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRollData {
    pub kind: StateKind,
    pub rolled: bool,
    pub chance_bp: u32,
    pub roll: u16,
    pub success: bool,
    pub applied: Applied,
}

/// `deadroll_combat::PacketResult`. Damage values are scaled x10_000.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketResultData {
    pub aspect: Aspect,
    pub immune: bool,
    pub base_min: u64,
    pub base_max: u64,
    pub increased_bp: u32,
    pub more_bp: u32,
    pub weaken_bp: u32,
    pub min: u64,
    pub max: u64,
    pub rolled: u64,
    pub armor: u32,
    pub resist_bp: u32,
    pub vuln_bp: u32,
    pub after_defense: u64,
    pub state: StateRollData,
}

/// `deadroll_combat::HitResult`: every roll and per-packet step, in roll order. Arrays are full
/// size; `tint_roll_count` / `packet_count` say how many entries are used.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitResultData {
    pub outcome: HitOutcome,
    pub d20: u8,
    pub attack_total: u32,
    pub defense: u32,
    pub crit: bool,
    pub crit_roll: Option<u16>,
    pub tint_rolls: [TintRollData; TINT_ROLLS_MAX],
    pub tint_roll_count: u8,
    pub resonances: [Option<ResonanceData>; SKILL_ASPECTS_MAX],
    pub d100: u8,
    pub packets: [PacketResultData; PACKETS_MAX],
    pub packet_count: u8,
    pub resonance_states: [StateRollData; SKILL_ASPECTS_MAX],
    pub states: [ActiveState; STATES_MAX],
    pub damage: u32,
    pub rolls_used: u32,
}

/// What a player's worn items put into a hit (`deadroll_rules::items::ItemBonus`), so the
/// breakdown can show it. All zero / `None` for a monster.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemBonusData {
    /// The main-hand weapon whose damage is the base; `None` = bare hands (or a monster).
    pub weapon: Option<ItemBase>,
    pub armor: u32,
    pub defense: u32,
    /// Flat `d20` attack bonus (stat bonuses are already in the totals).
    pub attack: u32,
}

impl ItemBonusData {
    pub fn of(hero: &HeroStats) -> Self {
        let b = crate::rules::item_bonus(hero);
        Self {
            weapon: b.weapon,
            armor: b.armor,
            defense: b.defense,
            attack: b.attack,
        }
    }
}

/// The items of both sides of a hit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitItemsData {
    pub attacker: ItemBonusData,
    pub defender: ItemBonusData,
}

/// `deadroll_combat::EffectRoll`: the dice of a heal, buff, debuff or a damage skill's effect.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectRollData {
    /// `Hit`: the effect lands. `Miss`: the d20 or the effect chance failed.
    pub outcome: HitOutcome,
    /// The hit roll of a debuff, 0 = none (heals and buffs do not roll to land).
    pub d20: u8,
    pub attack_total: u32,
    pub defense: u32,
    pub crit: bool,
    pub crit_roll: Option<u16>,
    /// Effect chance of a damage skill's effect (0 for heals, buffs and debuffs).
    pub chance_bp: u32,
    /// `None` when not rolled (not a damage skill's effect, or certain on a crit).
    pub chance_roll: Option<u16>,
    pub d100: u8,
    /// HP for a heal, bp for a buff or debuff, 0 for Stun / Freeze or a miss.
    pub amount: u32,
    pub rolls_used: u32,
}

impl From<Outcome> for HitOutcome {
    fn from(o: Outcome) -> Self {
        match o {
            Outcome::Miss => Self::Miss,
            Outcome::Hit => Self::Hit,
            Outcome::Immune => Self::Immune,
        }
    }
}

impl From<deadroll_combat::Applied> for Applied {
    fn from(a: deadroll_combat::Applied) -> Self {
        match a {
            deadroll_combat::Applied::None => Self::None,
            deadroll_combat::Applied::Added => Self::Added,
            deadroll_combat::Applied::Refreshed => Self::Refreshed,
            deadroll_combat::Applied::Froze => Self::Froze,
        }
    }
}

impl From<&EffectRoll> for EffectRollData {
    fn from(r: &EffectRoll) -> Self {
        Self {
            outcome: r.outcome.into(),
            d20: r.d20,
            attack_total: r.attack_total,
            defense: r.defense,
            crit: r.crit,
            crit_roll: r.crit_roll,
            chance_bp: r.chance_bp,
            chance_roll: r.chance_roll,
            d100: r.d100,
            amount: r.amount,
            rolls_used: r.rolls_used,
        }
    }
}

/// Writes the [`HitResolved`] event: the same log line `emit!` would write (one
/// `Program data:` chunk of discriminator ‖ Borsh fields).
pub fn emit_hit_resolved(
    session: &Pubkey,
    attacker: u8,
    target: u8,
    seed: &[u8; 32],
    action_counter: u32,
    result: &HitResult,
    items: &HitItemsData,
) -> Result<()> {
    let data = hit_resolved_data(
        session,
        attacker,
        target,
        seed,
        action_counter,
        result,
        items,
    )?;
    anchor_lang::solana_program::log::sol_log_data(&[&data]);
    Ok(())
}

/// Discriminator and Borsh body of [`HitResolved`], field order as in the struct.
pub fn hit_resolved_data(
    session: &Pubkey,
    attacker: u8,
    target: u8,
    seed: &[u8; 32],
    action_counter: u32,
    result: &HitResult,
    items: &HitItemsData,
) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(8 + 32 + 2 + 32 + 4 + 1_024);
    data.extend_from_slice(HitResolved::DISCRIMINATOR);
    data.extend_from_slice(session.as_ref());
    data.push(attacker);
    data.push(target);
    data.extend_from_slice(seed);
    data.extend_from_slice(&action_counter.to_le_bytes());
    borsh::to_writer(&mut data, result)?;
    borsh::to_writer(&mut data, items)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::{resolve_hit_into, Balance, VrfRoll};

    use crate::combat_input::tests::preset_inputs;

    #[test]
    fn effect_roll_mirrors_the_combat_bytes() {
        use deadroll_combat::{resolve_rider, resolve_support, SupportInput};
        let input = SupportInput {
            hostile: true,
            attack_bonus: 3,
            defense: 12,
            crit_chance_bp: 2_000,
            min: 1_000,
            max: 3_000,
        };
        for seed_byte in 0..40u8 {
            let mut src = VrfRoll::new([seed_byte; 32]);
            for roll in [
                resolve_support(&input, &Balance::DEFAULT, &mut src),
                resolve_rider(3_000, seed_byte % 2 == 0, 0, 0, &Balance::DEFAULT, &mut src),
            ] {
                assert_eq!(
                    borsh::to_vec(&EffectRollData::from(&roll)).unwrap(),
                    borsh::to_vec(&roll).unwrap()
                );
            }
        }
        for a in [
            deadroll_combat::Applied::None,
            deadroll_combat::Applied::Added,
            deadroll_combat::Applied::Refreshed,
            deadroll_combat::Applied::Froze,
        ] {
            assert_eq!(Applied::from(a) as u8, a as u8);
        }
    }

    #[test]
    fn event_bytes_decode_as_hit_resolved() {
        let session = Pubkey::new_unique();
        let mut hits = 0;
        for input in preset_inputs() {
            for seed_byte in [0u8, 3, 20, 77, 0xC4, 0xFF] {
                let seed = [seed_byte; 32];
                let mut result = Box::<HitResult>::default();
                resolve_hit_into(
                    &input,
                    &Balance::DEFAULT,
                    &mut VrfRoll::new(seed),
                    &mut result,
                );
                let items = HitItemsData {
                    attacker: ItemBonusData {
                        weapon: Some(ItemBase::Sword),
                        armor: 0,
                        defense: 1,
                        attack: 2,
                    },
                    defender: ItemBonusData::default(),
                };
                let bytes = hit_resolved_data(&session, 1, 5, &seed, 42, &result, &items).unwrap();
                assert_eq!(&bytes[..8], HitResolved::DISCRIMINATOR);

                let mut rest = &bytes[8..];
                let event = HitResolved::deserialize(&mut rest).unwrap();
                assert!(rest.is_empty(), "trailing bytes");
                assert_eq!(event.session, session);
                assert_eq!((event.attacker, event.target), (1, 5));
                assert_eq!(event.seed, seed);
                assert_eq!(event.action_counter, 42);
                assert_eq!(event.items, items);
                assert_eq!(event.result.damage, result.damage);
                assert_eq!(event.result.d20, result.d20);
                assert_eq!(event.result.packet_count, result.packet_count);
                assert_eq!(event.result.outcome as u8, result.outcome as u8);
                // The mirror re-encodes to the combat crate's exact bytes.
                assert_eq!(
                    borsh::to_vec(&event.result).unwrap(),
                    borsh::to_vec(&*result).unwrap()
                );
                assert_eq!(anchor_lang::Event::data(&event), bytes);
                if result.outcome == deadroll_combat::Outcome::Hit {
                    hits += 1;
                }
            }
        }
        assert!(hits > 0, "fixtures never hit");
    }
}
