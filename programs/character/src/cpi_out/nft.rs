//! Metaplex Core NFT of a character, behind the `nft` feature (FALLBACK(nft): without it the
//! character is PDA only and `nft = None`).
//!
//! Never deserialize a Core `Asset` on-chain: mpl-core's deserializers overflow the 4 KB stack
//! frame (agents/TASKS.md, dependency decision). Only builders and plugin updates here.
use anchor_lang::prelude::*;

/// mpl-core program id, also without the `nft` feature so account lists stay the same.
pub const MPL_CORE_ID: Pubkey = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");

/// `<uri_base>/char/<character>/v<state_version>.json` (docs/STORAGE_AND_DELIVERY.md, 6.2).
pub fn character_uri(uri_base: &[u8], character: &Pubkey, state_version: u32) -> String {
    let len = uri_base
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(uri_base.len());
    let base = core::str::from_utf8(&uri_base[..len]).unwrap_or("");
    format!(
        "{}/char/{}/v{}.json",
        base.trim_end_matches('/'),
        character,
        state_version
    )
}

/// Accounts for minting or freezing a character NFT.
pub struct NftAccounts<'a, 'info> {
    pub mpl_core: &'a AccountInfo<'info>,
    pub asset: &'a AccountInfo<'info>,
    pub payer: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

/// Creates the Core asset: owner = player, update authority = the character PDA, plus a
/// `PermanentFreezeDelegate` (authority-managed, so it survives transfers) for the tombstone.
#[cfg(feature = "nft")]
pub fn mint<'info>(
    a: NftAccounts<'_, 'info>,
    owner: &AccountInfo<'info>,
    character: &AccountInfo<'info>,
    name: String,
    uri: String,
) -> Result<()> {
    use mpl_core::{
        instructions::CreateV2CpiBuilder,
        types::{PermanentFreezeDelegate, Plugin, PluginAuthorityPair},
    };
    CreateV2CpiBuilder::new(a.mpl_core)
        .asset(a.asset)
        .payer(a.payer)
        .owner(Some(owner))
        .update_authority(Some(character))
        .system_program(a.system_program)
        .name(name)
        .uri(uri)
        .plugins(vec![PluginAuthorityPair {
            plugin: Plugin::PermanentFreezeDelegate(PermanentFreezeDelegate { frozen: false }),
            // Defaults to the update authority: the character PDA.
            authority: None,
        }])
        .invoke()
        .map_err(Into::into)
}

/// Sets the permanent freeze on the tombstone NFT, signed by the character PDA.
#[cfg(feature = "nft")]
pub fn freeze<'info>(
    a: NftAccounts<'_, 'info>,
    character: &AccountInfo<'info>,
    character_seeds: &[&[u8]],
) -> Result<()> {
    use mpl_core::{
        instructions::UpdatePluginV1CpiBuilder,
        types::{PermanentFreezeDelegate, Plugin},
    };
    UpdatePluginV1CpiBuilder::new(a.mpl_core)
        .asset(a.asset)
        .payer(a.payer)
        .authority(Some(character))
        .system_program(a.system_program)
        .plugin(Plugin::PermanentFreezeDelegate(PermanentFreezeDelegate {
            frozen: true,
        }))
        .invoke_signed(&[character_seeds])
        .map_err(Into::into)
}

#[cfg(all(test, feature = "nft"))]
mod tests {
    #[test]
    fn mpl_core_id_matches_the_crate() {
        assert_eq!(super::MPL_CORE_ID, mpl_core::ID);
    }

    #[test]
    fn uri() {
        let mut base = [0u8; 64];
        base[..19].copy_from_slice(b"https://r.example/x");
        let key = anchor_lang::prelude::Pubkey::new_from_array([1; 32]);
        assert_eq!(
            super::character_uri(&base, &key, 3),
            format!("https://r.example/x/char/{key}/v3.json")
        );
    }
}
