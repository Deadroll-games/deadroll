use anchor_lang::prelude::*;
use deadroll_types::MapLayoutDataV2;

/// `["layout", map]`. `finalize_map` hashes the core bytes into `Map.layout_hash`.
/// Patch 13: the core is `MapLayoutDataV2` (cells with variants, points), version 2.
#[account]
#[derive(InitSpace)]
pub struct MapLayout {
    pub core: MapLayoutDataV2,
}
