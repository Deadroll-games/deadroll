use anchor_lang::prelude::*;

use crate::state::BlobHeader;

/// Master palette (crates/render/FORMAT.md 3.2).
/// PDA `[PALETTE, version: u16 LE]`. The body follows the header, see `state/blob.rs`.
#[account(zero_copy)]
pub struct Palette {
    pub header: BlobHeader,
}
