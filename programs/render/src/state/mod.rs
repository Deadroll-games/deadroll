//! Accounts owned by this program. One account = one file.
//!
//! `RasterizerCode`, `Palette` and `SpriteBook` share one layout: the 8-byte discriminator,
//! a fixed [`BlobHeader`] and then `header.len` body bytes (offsets in `programs/render/README.md`).
//! They are written in chunks and frozen by `finalize_*`.

pub mod blob;
pub mod palette;
pub mod rasterizer_code;
pub mod sprite_book;

pub use blob::*;
pub use palette::*;
pub use rasterizer_code::*;
pub use sprite_book::*;
