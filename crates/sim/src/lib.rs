//! Deadroll balance simulator.
//!
//! Whole fights on the session program's rules: every rule call goes through `deadroll_rules`
//! and `deadroll_combat` (the code the program runs), only the orchestration lives here: the turn
//! loop, a simple AI, seeded dice instead of VRF, statistics and parallel batches (rayon).
//!
//! - Numbers: one [`deadroll_rules::BalanceConfig`] (JSON through [`config_io`]).
//! - Content: heroes, skills, monsters and encounters are JSON presets ([`presets`]); skills are
//!   checked with `deadroll_skillcheck` exactly like the in-game builder ([`setup`]).
//! - Enum lists come from each enum's `ALL` ([`named`]).
pub mod ai;
pub mod battle;
pub mod config_io;
pub mod named;
pub mod presets;
pub mod randgen;
pub mod run;
pub mod setup;
pub mod stats;

pub use battle::{fight, BattleStats, Outcome, Settings, SlotStats};
pub use deadroll_rules::BalanceConfig;
pub use presets::{Arena, EncounterPreset, EncounterSlot, HeroPreset, Kind, Presets, SkillPreset};
pub use run::{random_builds, run_batch, BuildScore, Progress, RandomBuilds};
pub use setup::{resolve_fight, resolve_party, Fight, Fighter, Issues};
pub use stats::{Histogram, Percentiles, Report, SlotReport};
