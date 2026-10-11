//! Deadroll combat math: dice, packets, modifiers, states, resonances. Integers only.
#![no_std]

pub mod rng;

pub use rng::{chance, d100, d20, RollSource, ScriptedRoll, SeededRoll, VrfRoll};
