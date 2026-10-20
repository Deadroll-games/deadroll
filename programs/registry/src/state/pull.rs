//! Reading render blobs (`RasterizerCode`, `Palette`, `SpriteBook`) for `set_rasterizer`,
//! `set_palette` and `set_sprite_books`.
//!
//! The registry pulls: no CPI from render (`programs/render/README.md`). Offsets of the blob
//! header in the account data are fixed there and mirrored here.

use anchor_lang::prelude::*;

use crate::errors::RegistryError;

/// `sha256("account:RasterizerCode")[..8]`.
pub const RASTERIZER_DISCRIMINATOR: [u8; 8] = [0x68, 0x2f, 0x12, 0x17, 0x9e, 0xd6, 0x7b, 0x2e];
/// `sha256("account:Palette")[..8]`.
pub const PALETTE_DISCRIMINATOR: [u8; 8] = [0x05, 0x3a, 0x00, 0xb2, 0xc1, 0xb5, 0x9c, 0x89];
/// `sha256("account:SpriteBook")[..8]`.
pub const SPRITE_BOOK_DISCRIMINATOR: [u8; 8] = [0xdd, 0xbb, 0xcf, 0xb6, 0x27, 0x5c, 0xbc, 0x05];

pub const KIND_RASTERIZER: u8 = 1;
pub const KIND_PALETTE: u8 = 2;
pub const KIND_SPRITE_BOOK: u8 = 3;
/// Blob header layout version this code understands.
const BLOB_VERSION: u8 = 1;

const OFF_VERSION: usize = 8;
const OFF_KIND: usize = 9;
const OFF_FINALIZED: usize = 10;
const OFF_ID: usize = 16;
const OFF_SHA256: usize = 84;
const BODY_OFFSET: usize = 116;

/// What the registry copies from a finalized blob.
#[derive(Debug, PartialEq, Eq)]
pub struct Blob {
    /// Rasterizer or palette version, or sprite book id (the PDA seed).
    pub id: u16,
    pub sha256: [u8; 32],
}

/// Header checks of a finalized blob of `kind`, then its id and hash.
pub fn read_finalized(data: &[u8], discriminator: [u8; 8], kind: u8) -> Result<Blob> {
    require!(
        data.len() >= BODY_OFFSET && data[..8] == discriminator,
        RegistryError::WrongBlobKind
    );
    require!(
        data[OFF_VERSION] == BLOB_VERSION && data[OFF_KIND] == kind,
        RegistryError::WrongBlobKind
    );
    require!(data[OFF_FINALIZED] == 1, RegistryError::BlobNotFinalized);
    let id = u16::from_le_bytes([data[OFF_ID], data[OFF_ID + 1]]);
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(&data[OFF_SHA256..OFF_SHA256 + 32]);
    Ok(Blob { id, sha256 })
}

/// Full pull: owner, header and PDA address, as `set_rasterizer` / `set_palette` need.
pub fn pull(
    info: &AccountInfo,
    render_program: &Pubkey,
    seed: &[u8],
    discriminator: [u8; 8],
    kind: u8,
) -> Result<Blob> {
    require_keys_eq!(*info.owner, *render_program, RegistryError::WrongBlobOwner);
    let data = info.try_borrow_data()?;
    let blob = read_finalized(&data, discriminator, kind)?;
    let (expected, _) =
        Pubkey::find_program_address(&[seed, &blob.id.to_le_bytes()], render_program);
    require_keys_eq!(info.key(), expected, RegistryError::WrongBlobAddress);
    Ok(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(disc: [u8; 8], kind: u8, finalized: u8, id: u16, sha: u8) -> Vec<u8> {
        let mut d = vec![0u8; BODY_OFFSET + 4];
        d[..8].copy_from_slice(&disc);
        d[OFF_VERSION] = 1;
        d[OFF_KIND] = kind;
        d[OFF_FINALIZED] = finalized;
        d[OFF_ID..OFF_ID + 2].copy_from_slice(&id.to_le_bytes());
        d[OFF_SHA256..OFF_SHA256 + 32].fill(sha);
        d
    }

    fn rasterizer(finalized: u8) -> Vec<u8> {
        blob(
            RASTERIZER_DISCRIMINATOR,
            KIND_RASTERIZER,
            finalized,
            7,
            0xAB,
        )
    }

    #[test]
    fn discriminators_are_anchors() {
        let h = |n: &[u8]| solana_sha256_hasher::hash(n).to_bytes();
        assert_eq!(RASTERIZER_DISCRIMINATOR, h(b"account:RasterizerCode")[..8]);
        assert_eq!(PALETTE_DISCRIMINATOR, h(b"account:Palette")[..8]);
        assert_eq!(SPRITE_BOOK_DISCRIMINATOR, h(b"account:SpriteBook")[..8]);
    }

    #[test]
    fn reads_a_finalized_blob() {
        let b = read_finalized(&rasterizer(1), RASTERIZER_DISCRIMINATOR, KIND_RASTERIZER);
        assert_eq!(
            b.unwrap(),
            Blob {
                id: 7,
                sha256: [0xAB; 32]
            }
        );
    }

    #[test]
    fn rejects_everything_else() {
        let read = |d: Vec<u8>| read_finalized(&d, RASTERIZER_DISCRIMINATOR, KIND_RASTERIZER);
        let not_final = read(rasterizer(0)).unwrap_err();
        assert_eq!(not_final, RegistryError::BlobNotFinalized.into());
        let kind: Error = RegistryError::WrongBlobKind.into();
        assert_eq!(read(vec![]).unwrap_err(), kind);
        assert_eq!(read(rasterizer(1)[..100].to_vec()).unwrap_err(), kind);
        let palette = blob(PALETTE_DISCRIMINATOR, KIND_PALETTE, 1, 1, 1);
        assert_eq!(read(palette).unwrap_err(), kind);
        let mut bad_version = rasterizer(1);
        bad_version[OFF_VERSION] = 2;
        assert_eq!(read(bad_version).unwrap_err(), kind);
        // Right discriminator, wrong kind byte.
        let wrong_kind = blob(RASTERIZER_DISCRIMINATOR, KIND_PALETTE, 1, 1, 1);
        assert_eq!(read(wrong_kind).unwrap_err(), kind);
    }

    fn run_pull(owner: Pubkey, key: Pubkey, data: &mut [u8], render: &Pubkey) -> Result<Blob> {
        let mut lamports = 1;
        let info = AccountInfo::new(&key, false, false, &mut lamports, data, &owner, false);
        pull(
            &info,
            render,
            deadroll_types::seeds::RASTERIZER,
            RASTERIZER_DISCRIMINATOR,
            KIND_RASTERIZER,
        )
    }

    #[test]
    fn pull_checks_owner_and_address() {
        let render = Pubkey::new_unique();
        let (pda, _) = Pubkey::find_program_address(&[b"rasterizer", &7u16.to_le_bytes()], &render);
        let ok = run_pull(render, pda, &mut rasterizer(1), &render);
        assert_eq!(ok.unwrap().id, 7);
        let other = Pubkey::new_unique();
        let wrong_owner = run_pull(other, pda, &mut rasterizer(1), &render);
        assert_eq!(
            wrong_owner.unwrap_err(),
            RegistryError::WrongBlobOwner.into()
        );
        // A copy of a finalized blob at another address (same owner) is not accepted.
        let wrong_key = run_pull(render, other, &mut rasterizer(1), &render);
        assert_eq!(
            wrong_key.unwrap_err(),
            RegistryError::WrongBlobAddress.into()
        );
    }
}
