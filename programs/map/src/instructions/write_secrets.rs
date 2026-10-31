use anchor_lang::prelude::*;
use deadroll_types::{PACKS_MAX, SECRET_TYPES};

use crate::{events::SecretsWritten, instructions::WriteLayout, state::LayoutBytes};

/// Patch C (docs/MAP_DESCRIPTION.md 8.3, 17.2, 17.5): writes a chunk of the map's encrypted
/// secrets from `offset` (`len`: the whole blob's length) and the public part: each pack's
/// threat and how many secrets of each type the map declares. The website encrypts the secrets
/// with a key from the DM's wallet signature, so the DM keeps no file; the program never reads
/// them. `finalize_map` checks the public part.
pub fn handler(
    ctx: Context<WriteLayout>,
    offset: u16,
    bytes: Vec<u8>,
    len: u16,
    threat: [u8; PACKS_MAX],
    declared: [u8; SECRET_TYPES],
) -> Result<()> {
    LayoutBytes::load(&ctx.accounts.layout)?.write_secrets(
        offset as usize,
        &bytes,
        len as usize,
        &threat,
        &declared,
    )?;
    emit!(SecretsWritten {
        map: ctx.accounts.map.key(),
        offset,
        len: bytes.len() as u16,
        total: len,
        threat,
        declared,
    });
    Ok(())
}
