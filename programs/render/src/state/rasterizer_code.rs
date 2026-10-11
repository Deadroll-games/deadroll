use anchor_lang::prelude::*;

use crate::state::BlobHeader;

/// WASM rasterizer binary, one account per rasterizer version.
/// PDA `[RASTERIZER, version: u16 LE]`. The body follows the header, see `state/blob.rs`.
#[account(zero_copy)]
pub struct RasterizerCode {
    pub header: BlobHeader,
}
