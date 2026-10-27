//! One instruction = one file (docs/ARCHITECTURE.md, 12).
pub mod add_building;
pub mod close_building;
pub mod close_quest;
pub mod init_city;
pub mod init_tavern_board;
pub mod post_quest;
pub mod set_city_objects;
pub mod update_building;
pub mod write_city;

// Globs: `#[program]` needs the `__client_accounts_*` modules generated next to each
// accounts struct. Every file also has a `handler`, called by path from `lib.rs`.
#[allow(ambiguous_glob_reexports)]
pub use {
    add_building::*, close_building::*, close_quest::*, init_city::*, init_tavern_board::*,
    post_quest::*, update_building::*, write_city::*,
};
