//! One instruction = one file (docs/ARCHITECTURE.md, 12).
pub mod create_map;
pub mod finalize_map;
pub mod set_status;
pub mod write_layout;
pub mod write_spawns;

// Globs: `#[program]` needs the `__client_accounts_*` modules generated next to each
// accounts struct. Every file also has a `handler`, called by path from `lib.rs`.
#[allow(ambiguous_glob_reexports)]
pub use {create_map::*, finalize_map::*, set_status::*, write_layout::*};
