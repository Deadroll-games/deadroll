//! Skill blocks (docs/SKILL_BUILDER.md, section 2). Mechanics come only from these.
//!
//! The types live in `deadroll_types::skill`: the programs store them in accounts, and only
//! `crates/types` may derive the Anchor traits (docs/ARCHITECTURE.md, section 5).

pub use deadroll_types::skill::*;
