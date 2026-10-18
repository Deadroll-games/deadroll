//! One instruction = one file (docs/ARCHITECTURE.md, 12).
// Anchor needs the glob re-exports (generated `__client_accounts_*`); every module has its own `handler`.
#![allow(ambiguous_glob_reexports)]

pub mod create_character;
pub mod create_profile;
pub mod equip;
pub mod freeze_tombstone;
pub mod grant_rewards;
pub mod lock_for_session;
pub mod mark_dead;
pub mod unequip;
pub mod unlock;

pub use create_character::*;
pub use create_profile::*;
pub use equip::*;
pub use freeze_tombstone::*;
pub use grant_rewards::*;
pub use lock_for_session::*;
pub use mark_dead::*;
pub use unequip::*;
pub use unlock::*;
