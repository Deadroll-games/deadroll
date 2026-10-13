//! Deadroll game rules shared by `programs/session` and the balance simulator (`crates/sim`).
//!
//! `crates/combat` resolves one hit or one effect; this crate decides what goes into it and what
//! happens around it: hit inputs from a hero or a monster, action points, initiative, ranges, the
//! start of a turn and the resolution of one action. The program calls it with
//! [`BalanceConfig::DEFAULT`]; `tools/balance` calls the same code with a tuned config, so a
//! mechanic added here reaches both.
//!
//! - Every balance number lives in [`BalanceConfig`]. Placeholders, tune in `tools/balance`.
//! - Content (monster stats) is data: [`monster::MonsterBase`]. The program reads the P0 table
//!   in [`monster::p0`], the simulator reads JSON presets.
//! - Integers only, no allocation, `no_std` without features.
#![cfg_attr(not(feature = "std"), no_std)]

pub mod config;
pub mod input;
pub mod monster;
pub mod player;
pub mod resolve;
pub mod turn;
pub mod units;

pub use config::{BalanceConfig, MonsterScaling, PlayerTuning};
pub use input::{EffectAction, HeroStats, Side, SkillStats, SkillUse};
pub use monster::{MonsterBase, MonsterProfile};
pub use resolve::{AttackOutcome, RiderOutcome};
pub use turn::{start_of_turn, TurnStart};
pub use units::{Initiative, Unit, EMPTY_UNIT, MONSTER_UNIT_BASE};
