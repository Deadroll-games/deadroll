//! Accounts owned by this program. One account = one file. Each wraps its frozen
//! `crates/types` core (CONTRACTS.md, section 4.1) and appends lane fields after it.
//! The struct names are part of the format: `programs/session` checks their discriminators.
pub mod map;
pub mod map_layout;

pub use map::*;
pub use map_layout::*;

/// Layout version written into every map account. 2: `Map.biome` appended (a version 1 map has
/// no biome byte and reads as `Biome::Caves`).
pub const ACCOUNT_VERSION: u8 = 2;
/// Patch 13: `MapLayout.core.version` of the `MapLayoutDataV2` format.
pub const LAYOUT_VERSION: u8 = 2;

#[cfg(test)]
pub(crate) mod tests {
    use anchor_lang::{
        prelude::Pubkey, AccountDeserialize, AccountSerialize, Discriminator, Space,
    };
    use deadroll_types::{Biome, MapData, MapLayoutDataV2, MapStatus};
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
        // tier, then patch 11: min_players, chest_radius, settle_timeout_secs (u32), then biome.
        assert_eq!(Map::INIT_SPACE, MapData::SIZE + 1 + 1 + 1 + 4 + 1);
    }

    pub(crate) fn map(biome: Biome) -> Map {
        Map {
            core: MapData {
                version: ACCOUNT_VERSION,
                dm: Pubkey::new_from_array([7; 32]),
                index: 3,
                width: 16,
                height: 16,
                status: MapStatus::Finalized,
                layout_hash: [1; 32],
                secrets_root: [2; 32],
                bump: 254,
            },
            tier: 1,
            min_players: 2,
            chest_radius: 3,
            settle_timeout_secs: 3600,
            biome: biome as u8,
        }
    }

    /// The account data of `map` in an account of `space` bytes (zeroed first, as `init`).
    pub(crate) fn stored(map: &Map, space: usize) -> Vec<u8> {
        let mut data = vec![0u8; space];
        let mut full = Vec::new();
        map.try_serialize(&mut full).unwrap();
        let len = space.min(full.len());
        data[..len].copy_from_slice(&full[..len]);
        data
    }

    #[test]
    fn biome_is_the_last_byte() {
        let data = stored(&map(Biome::Crypt), 8 + Map::INIT_SPACE);
        assert_eq!(data.last(), Some(&(Biome::Crypt as u8)));
        let back = Map::try_deserialize(&mut &data[..]).unwrap();
        assert_eq!(back.biome, Biome::Crypt as u8);
        assert_eq!(back.core, map(Biome::Crypt).core);
        let padded = Map::decode_padded(&data).unwrap();
        assert_eq!(padded.biome, Biome::Crypt as u8);
        assert_eq!(padded.settle_timeout_secs, 3600);
    }

    /// A map created before `biome` (one byte shorter) reads as Caves.
    #[test]
    fn map_without_biome_reads_as_caves() {
        let mut old = map(Biome::Forest);
        old.core.version = 1;
        let data = stored(&old, 8 + Map::V1_SPACE);
        assert!(Map::try_deserialize(&mut &data[..]).is_err());
        let back = Map::decode_padded(&data).unwrap();
        assert_eq!(back.core, old.core);
        assert_eq!(back.biome, Biome::Caves as u8);
        assert_eq!(back.settle_timeout_secs, 3600);
        // Shorter than a version 1 map, or no data at all: not a map.
        assert!(Map::decode_padded(&data[..data.len() - 1]).is_none());
        assert!(Map::decode_padded(&[]).is_none());
    }

    #[test]
    fn status_offset_is_the_core_status() {
        let mut m = map(Biome::Caves);
        for &status in MapStatus::ALL {
            m.core.status = status;
            assert_eq!(
                stored(&m, 8 + Map::INIT_SPACE)[Map::STATUS_OFFSET],
                status as u8
            );
        }
    }
}
