use anchor_lang::prelude::*;
use anchor_lang::Discriminator;

use crate::errors::RenderError;
use crate::state::{Palette, RasterizerCode, SpriteBook};

/// Layout version of the blob header.
pub const BLOB_VERSION: u8 = 1;
/// Anchor discriminator bytes before the header.
pub const DISC_LEN: usize = 8;
/// Body offset from the start of the account data.
pub const BODY_OFFSET: usize = DISC_LEN + core::mem::size_of::<BlobHeader>();
/// Most an account may grow in one instruction (solana-account-info `MAX_PERMITTED_DATA_INCREASE`).
pub const GROW_STEP: usize = 10 * 1024;

/// Kinds, stored in `BlobHeader.kind`.
pub const KIND_RASTERIZER: u8 = 1;
pub const KIND_PALETTE: u8 = 2;
pub const KIND_SPRITE_BOOK: u8 = 3;

/// Body size limits per kind.
pub const MAX_RASTERIZER_LEN: u32 = 128 * 1024;
pub const MAX_PALETTE_LEN: u32 = 8 * 1024;
pub const MAX_SPRITE_BOOK_LEN: u32 = 64 * 1024;

/// Common header of the three blob accounts. `version` is first after the discriminator.
///
/// Byte offsets in the account data: version 8, kind 9, finalized 10, bump 11, len 12..16,
/// id 16..18, (pad 18..20), authority 20..52, expected_sha256 52..84, sha256 84..116, body 116.
#[zero_copy]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BlobHeader {
    pub version: u8,
    pub kind: u8,
    /// 1 after `finalize_*`; nothing can write afterwards.
    pub finalized: u8,
    pub bump: u8,
    /// Body length in bytes.
    pub len: u32,
    /// Rasterizer or palette version, or sprite book id (the PDA seed).
    pub id: u16,
    pub _pad: [u8; 2],
    /// `GameConfig.authority` at init; the only signer for extend, write and finalize.
    pub authority: Pubkey,
    pub expected_sha256: [u8; 32],
    /// SHA-256 of the body, set by `finalize_*`.
    pub sha256: [u8; 32],
}

impl BlobHeader {
    pub fn full_size(&self) -> usize {
        BODY_OFFSET + self.len as usize
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized != 0
    }
}

/// Max body length of a kind.
pub fn max_len(kind: u8) -> u32 {
    match kind {
        KIND_RASTERIZER => MAX_RASTERIZER_LEN,
        KIND_PALETTE => MAX_PALETTE_LEN,
        _ => MAX_SPRITE_BOOK_LEN,
    }
}

/// Size `init_*` allocates: everything if it fits in one step, else one step.
pub fn initial_space(len: u32) -> usize {
    (BODY_OFFSET + len as usize).min(GROW_STEP)
}

/// Checks the length and the hash argument of `init_*`.
pub fn check_init_args(kind: u8, len: u32) -> Result<()> {
    require!(len > 0 && len <= max_len(kind), RenderError::InvalidLength);
    Ok(())
}

/// Kind by discriminator, for instructions that accept any blob account.
fn kind_of(data: &[u8]) -> Option<u8> {
    let disc = data.get(..DISC_LEN)?;
    if disc == RasterizerCode::DISCRIMINATOR {
        Some(KIND_RASTERIZER)
    } else if disc == Palette::DISCRIMINATOR {
        Some(KIND_PALETTE)
    } else if disc == SpriteBook::DISCRIMINATOR {
        Some(KIND_SPRITE_BOOK)
    } else {
        None
    }
}

/// Reads the header of any blob account owned by this program.
pub fn read_header(info: &AccountInfo) -> Result<BlobHeader> {
    require_keys_eq!(*info.owner, crate::ID, RenderError::WrongAccount);
    let data = info.try_borrow_data()?;
    let kind = kind_of(&data).ok_or(RenderError::WrongAccount)?;
    let raw = data
        .get(DISC_LEN..BODY_OFFSET)
        .ok_or(RenderError::WrongAccount)?;
    let header: BlobHeader = bytemuck::pod_read_unaligned(raw);
    require!(
        header.version == BLOB_VERSION && header.kind == kind,
        RenderError::WrongAccount
    );
    Ok(header)
}

/// Common checks before changing a blob: the authority signs and it is not finalized.
pub fn check_writable(header: &BlobHeader, signer: &Pubkey) -> Result<()> {
    require_keys_eq!(header.authority, *signer, RenderError::Unauthorized);
    require!(!header.is_finalized(), RenderError::AlreadyFinalized);
    Ok(())
}

/// Fills a freshly created header.
pub fn init_header(
    h: &mut BlobHeader,
    kind: u8,
    id: u16,
    len: u32,
    sha256: [u8; 32],
    authority: Pubkey,
    bump: u8,
) {
    *h = BlobHeader {
        version: BLOB_VERSION,
        kind,
        finalized: 0,
        bump,
        len,
        id,
        _pad: [0; 2],
        authority,
        expected_sha256: sha256,
        sha256: [0; 32],
    };
}

/// Hashes the body and freezes the blob if it matches `expected_sha256`.
/// Returns the hash. `check_body` validates the content of the kind.
pub fn finalize(
    header: &mut BlobHeader,
    data: &[u8],
    check_body: impl FnOnce(&[u8]) -> bool,
) -> Result<[u8; 32]> {
    require!(
        data.len() == header.full_size(),
        RenderError::NotFullyAllocated
    );
    let body = &data[BODY_OFFSET..];
    let sha = solana_sha256_hasher::hashv(&[body]).to_bytes();
    require!(sha == header.expected_sha256, RenderError::HashMismatch);
    require!(check_body(body), RenderError::InvalidBody);
    header.sha256 = sha;
    header.finalized = 1;
    Ok(sha)
}

/// Writes the header of any blob account back (after `read_header` and changes).
pub fn write_header(info: &AccountInfo, header: &BlobHeader) -> Result<()> {
    let mut data = info.try_borrow_mut_data()?;
    data[DISC_LEN..BODY_OFFSET].copy_from_slice(bytemuck::bytes_of(header));
    Ok(())
}
