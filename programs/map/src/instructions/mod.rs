//! One instruction = one file (docs/ARCHITECTURE.md, 12).
pub mod check_step;
pub mod close_map;
pub mod create_map;
pub mod finalize_map;
pub mod grow_layout;
pub mod set_status;
pub mod write_arena;
pub mod write_layout;
pub mod write_points;
pub mod write_secrets;
pub mod write_spawns;

// Globs: `#[program]` needs the `__client_accounts_*` modules generated next to each
// accounts struct. Every file also has a `handler`, called by path from `lib.rs`.
#[allow(ambiguous_glob_reexports)]
pub use {
    check_step::*, close_map::*, create_map::*, finalize_map::*, grow_layout::*, set_status::*,
    write_layout::*,
};
