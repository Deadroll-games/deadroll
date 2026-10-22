//! Deadroll game rules shared by `programs/session` and the balance simulator (`crates/sim`).
//!
//! `crates/combat` resolves one hit or one effect; this crate decides what goes into it and what
//! happens around it: hit inputs from a hero or a monster, action points, initiative, ranges, the
//! start of a turn and the resolution of one action. The program calls it with
//! [`BalanceConfig::DEFAULT`]; `tools/balance` calls the same code with a tuned config, so a
//! mechanic added here reaches both.
//!
//! - Every balance number lives in [`BalanceConfig`]. Placeholders, tune in `tools/balance`.
//! - Item numbers and class starter kits are [`config::ItemTuning`]; [`items`] holds the equipment
//!   rules every program and the simulator share.
//! - Skills beyond the one action (patch 5): passive skills and auras ([`aura`]), reactive skills
//!   ([`reactive`]) and the line target ([`line`]).
//! - Content (monsters and their skills) is registry data: [`monster::MonsterTemplate`] decodes a
//!   bestiary record for the program, the simulator and the website; [`monster_ai`] is the simple
//!   AI the simulator and the website's auto mode play monsters with.
//! - Integers only, no allocation, `no_std` without features.
#![cfg_attr(not(feature = "std"), no_std)]

pub mod aura;
pub mod config;
pub mod input;
pub mod items;
pub mod line;
pub mod monster;
pub mod monster_ai;
pub mod player;
pub mod reactive;
#[cfg(feature = "serde")]
pub mod registry;
pub mod resolve;
pub mod spread;
pub mod turn;
pub mod units;

pub use config::{
    BalanceConfig, FeeTuning, ItemBaseStats, ItemTuning, MonsterScaling, PlayerTuning, LEVELS,
    TIERS,
};
pub use input::{EffectAction, HeroStats, Side, SkillStats, SkillUse};
pub use items::{EquipError, Equipment, ItemBonus};
pub use monster::{MonsterDefense, MonsterProfile, MonsterSkillStats, MonsterTemplate, Rank};
pub use player::SpendError;
pub use resolve::{AttackOutcome, RiderOutcome};
pub use turn::{start_of_turn, TurnStart};
pub use units::{Initiative, Unit, EMPTY_UNIT, MONSTER_UNIT_BASE};
