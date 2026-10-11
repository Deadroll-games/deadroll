//! Accounts owned by this program. One account = one file. Each wraps its frozen
//! `crates/types` core (CONTRACTS.md, section 4.1) and appends lane fields after it.
//! The struct names are part of the format: `programs/session` checks their discriminators.
pub mod map;
pub mod map_layout;

pub use map::*;
pub use map_layout::*;

/// Layout version written into every map account.
pub const ACCOUNT_VERSION: u8 = 1;
