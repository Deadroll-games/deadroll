//! One instruction = one file (docs/ARCHITECTURE.md, 12).
pub mod init_config;
pub mod init_table;
pub mod resize_config;
pub mod set_palette;
pub mod set_programs;
pub mod set_rasterizer;
pub mod set_rasterizer_from_render;
pub mod set_sprite_books;
pub mod set_uri_base;
pub mod write_table;

// Globs: `#[program]` needs the `__client_accounts_*` modules generated next to each
// accounts struct. Every file also has a `handler`, called by path from `lib.rs`.
#[allow(ambiguous_glob_reexports)]
pub use {
    init_config::*, init_table::*, resize_config::*, set_programs::*, set_rasterizer::*,
    set_rasterizer_from_render::*, write_table::*,
};
