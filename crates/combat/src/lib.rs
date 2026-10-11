//! Deadroll combat math: dice, packets, modifiers, states, resonances. Integers only.
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

pub use balance::Balance;
pub use hit::{
    preview_range, resolve_hit, resolve_hit_into, Attacker, Defender, HitInput, HitResult, Outcome,
    PacketResult, StateRoll,
};
pub use modifiers::{family_of, is_physical, Mod, ModFilter};
pub use packet::{build_packets, DamageRange, Packet, Packets, Skill};
pub use resonance::{
    resonance_id, Carrier, Resonance, ResonanceCondition, ResonanceEffect, Tint, TintRoll,
    RESONANCES,
};
pub use rng::{chance, d100, d20, RollSource, ScriptedRoll, SeededRoll, VrfRoll};
pub use states::{apply_chill, apply_state, native_state, tick_turn_start, Applied, States, Tick};
