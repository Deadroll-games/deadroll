//! Accounts owned by this program. One account = one file. Each wraps its frozen
//! `crates/types` core (CONTRACTS.md, section 4.1) and appends lane fields after it.
//! The struct names are part of the format: `programs/session` checks their discriminators.
pub mod map;
pub mod map_layout;

pub use map::*;
pub use map_layout::*;

/// Layout version written into every map account.
pub const ACCOUNT_VERSION: u8 = 1;
/// Patch 13: `MapLayout.core.version` of the `MapLayoutDataV2` format.
pub const LAYOUT_VERSION: u8 = 2;

#[cfg(test)]
mod tests {
    use anchor_lang::{Discriminator, Space};
    use deadroll_types::{MapData, MapLayoutDataV2};
    use solana_sha256_hasher::hash;

    use super::*;

    /// `programs/session` checks these discriminators and reads the cores at fixed offsets.
    #[test]
    fn format_seen_by_session() {
        assert_eq!(Map::DISCRIMINATOR, &hash(b"account:Map").to_bytes()[..8]);
        assert_eq!(
            MapLayout::DISCRIMINATOR,
            &hash(b"account:MapLayout").to_bytes()[..8]
        );
        assert_eq!(MapLayout::INIT_SPACE, MapLayoutDataV2::SIZE);
        // tier, then patch 11: min_players, chest_radius, settle_timeout_secs (u32).
        assert_eq!(Map::INIT_SPACE, MapData::SIZE + 1 + 1 + 1 + 4);
    }
}
