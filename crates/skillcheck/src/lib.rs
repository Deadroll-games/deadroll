//! Deadroll skill checks (docs/SKILL_BUILDER.md).
//!
//! One crate for the builder on the website and the character program, so the limits cannot
//! be bypassed by going around the website (SKILL_BUILDER.md, section 1.4).
//!
//! - [`SkillBlocks`]: the mechanics of a skill, only blocks (category, purpose, tags, aspects,
//!   effect, target, trigger, aura). Name and description are text and live in the program.
//! - [`validate`]: the builder rules of SKILL_BUILDER.md, section 13, on the blocks.
//! - [`validate_skill`]: the same plus the patch 5 blocks ([`SkillExt`]: range, timing and the
//!   HP threshold of a reactive trigger).
//! - [`validate_starting`]: the class limits on the first six skills (section 10.1).
//! - [`check_loadout`]: at most 4 active, 1 reactive, 1 passive (section 3.4).
//! - [`check_map_selection`]: the skills taken onto a map, 4 + 1 + 1 (sections 3.4, 13).
//! - [`derive`]: shares, power, fusion, cooldown and AP cost, computed once at creation and
//!   stored.
//! - [`presets`]: starter sets per class for the demo.
//! - [`monster`]: monster skills (docs/BESTIARY.md 3): the registry record, [`validate_monster_skill`]
//!   (Heal allowed, no class limits) and [`check_monster_selection`] (a spawn's pool mask).
//!
//! Integers only, no allocation, `no_std` without the `std` feature. No Anchor: the block types
//! that programs store come from `deadroll_types::skill` (docs/ARCHITECTURE.md, section 5).
#![cfg_attr(not(feature = "std"), no_std)]

pub mod blocks;
pub mod derive;
pub mod monster;
pub mod presets;
pub mod rules;

pub use blocks::*;
pub use derive::{derive, derive_with, fusion_id, Derived, SkillTuning};
pub use rules::{
    check_loadout, check_map_selection, check_monster_selection, validate, validate_monster_skill,
    validate_skill, validate_skill_with, validate_starting, MapSelection, SkillError, ACTIVE_MAX,
    AURA_RADIUS_MAX, NO_SKILL, PASSIVE_MAX, REACTIVE_MAX, SELECTION_SLOTS, STARTING_MAX,
};
