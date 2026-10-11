use anchor_lang::prelude::*;
use deadroll_types::MapLayoutData;

/// `["layout", map]`. `finalize_map` hashes the core bytes into `Map.layout_hash`.
#[account]
#[derive(InitSpace)]
pub struct MapLayout {
    pub core: MapLayoutData,
}
