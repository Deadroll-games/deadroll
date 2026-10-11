use anchor_lang::prelude::*;

use crate::state::BlobHeader;

/// Sprite book (crates/render/FORMAT.md 3.3).
/// PDA `[SPRITES, book_id: u16 LE]`. The body follows the header, see `state/blob.rs`.
#[account(zero_copy)]
pub struct SpriteBook {
    pub header: BlobHeader,
}
