//! One instruction = one file (docs/ARCHITECTURE.md, 12).
// Anchor needs the glob re-exports (generated `__client_accounts_*`); every module has its own `handler`.
#![allow(ambiguous_glob_reexports)]
pub mod equip;
pub mod init_loadout;
pub mod unequip;

pub use equip::*;
pub use init_loadout::*;
pub use unequip::*;
