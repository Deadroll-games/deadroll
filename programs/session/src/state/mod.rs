//! Session accounts. Each wraps its frozen `crates/types` core (CONTRACTS.md, section 4.1)
//! and appends lane fields after it.
pub mod escrow;
pub mod external;
pub mod game_session;
pub mod mods;
pub mod monster_state;
pub mod participation;
pub mod temp_stash;
#[cfg(test)]
pub(crate) mod test_support;

pub use escrow::*;
pub use game_session::*;
pub use mods::{ModKindData, ModsData, TimedModData};
pub use monster_state::*;
pub use participation::*;
pub use temp_stash::*;

/// Layout version written into every session account.
pub const ACCOUNT_VERSION: u8 = 1;
