//! One instruction = one file (docs/ARCHITECTURE.md, 12).
// Anchor needs the glob re-exports (generated `__client_accounts_*`); every module has its own `handler`.
#![allow(ambiguous_glob_reexports)]

pub mod apply_bag;
pub mod create_character;
pub mod create_profile;
pub mod equip;
pub mod freeze_tombstone;
pub mod grant_xp;
pub mod lock_for_session;
pub mod mark_dead;
pub mod set_skill_description;
pub mod spend_stat_points;
pub mod unequip;
pub mod unlock;

pub use apply_bag::*;
pub use create_character::*;
pub use create_profile::*;
pub use equip::*;
pub use freeze_tombstone::*;
pub use grant_xp::*;
pub use lock_for_session::*;
pub use mark_dead::*;
pub use set_skill_description::*;
pub use spend_stat_points::*;
pub use unequip::*;
pub use unlock::*;
