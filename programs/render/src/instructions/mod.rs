//! One instruction = one file (docs/ARCHITECTURE.md, 12).
//! Each file has a `handler`; the glob re-exports are only for the `Accounts` structs.
#![allow(ambiguous_glob_reexports)]

pub mod extend;
pub mod finalize_palette;
pub mod finalize_rasterizer;
pub mod finalize_sprite_book;
pub mod init_palette;
pub mod init_rasterizer;
pub mod init_sprite_book;
pub mod write_chunk;

pub use extend::*;
pub use finalize_palette::*;
pub use finalize_rasterizer::*;
pub use finalize_sprite_book::*;
pub use init_palette::*;
pub use init_rasterizer::*;
pub use init_sprite_book::*;
pub use write_chunk::*;
