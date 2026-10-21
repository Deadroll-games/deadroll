//! Deadroll combat math: dice, packets, modifiers, states, resonances, buffs and debuffs.
//! Integers only.
//!
//! Rules: docs/COMBAT_MATH.md and docs/DAMAGE_TYPES_AND_MIXES.md. Used by the `session`
//! program (every attack), `tools/sim` and the website (through `crates/combat/wasm`).
//!
//! - All dice come from one [`RollSource`]; production uses [`VrfRoll`] built from the VRF
//!   value of the action. The roll order of a hit is documented in [`hit`].
//! - Values: amounts `u32`; percentages and multipliers in basis points (`BP = 10_000`);
//!   packet damage scaled by `BP` until the single round-down at the end.
//! - Bonuses multiply, counterweights divide; nothing ever subtracts or goes negative.
//! - Every balance number lives in [`Balance`] (placeholder, tune in sim).
//! - No allocation, fixed-size arrays only, `no_std`.
#![no_std]

pub mod balance;
pub mod hit;
pub mod modifiers;
pub mod packet;
pub mod resonance;
pub mod rng;
pub mod states;
pub mod support;
pub mod tables;

pub use balance::Balance;
pub use hit::{
    preview_hit, preview_range, resolve_hit, resolve_hit_into, Attacker, Defender, HitInput,
    HitPreview, HitResult, Jump, JumpMode, OpOrigin, OpResult, Outcome, PacketResult, PreviewTint,
    StateRoll, JUMPS_MAX, RES_OPS_MAX, TARGET_MODS_MAX,
};
pub use modifiers::{family_of, is_physical, Mod, ModFilter};
pub use packet::{build_packets, DamageRange, Packet, Packets, Skill};
pub use resonance::{resonance_id, Carrier, Resonance, ResonanceCondition, Tint, TintRoll};
pub use rng::{chance, d100, d20, RollSource, ScriptedRoll, SeededRoll, VrfRoll};
pub use states::{
    apply_chill, apply_state, chill_slow_bp, form_bit, power_of, source_bit, tick_turn_start,
    Applied, States, Tick, TAGS,
};
pub use support::{
    apply_mod, apply_to_attacker, apply_to_defender, free_slot, mod_range, mod_turns,
    resistance_share, resolve_rider, resolve_support, speed_ap, tick_mods, AccuracyBuff,
    EffectRoll, ModKind, SupportBalance, SupportInput, TimedMod, TimedMods, MODS_MAX,
};
pub use tables::{Op, OpKind, ResonanceTables, TableError};
