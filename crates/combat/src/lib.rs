//! Deadroll combat math: dice, packets, modifiers, states, resonances. Integers only.
#![no_std]

pub mod balance;
pub mod modifiers;
pub mod packet;
pub mod rng;
pub mod states;

pub use balance::Balance;
pub use modifiers::{family_of, is_physical, Mod, ModFilter};
pub use packet::{build_packets, DamageRange, Packet, Packets, Skill};
pub use rng::{chance, d100, d20, RollSource, ScriptedRoll, SeededRoll, VrfRoll};
pub use states::{apply_chill, apply_state, native_state, tick_turn_start, Applied, States, Tick};
