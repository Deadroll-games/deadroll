use anchor_lang::prelude::*;
use deadroll_types::{MapArenasData, MapLayoutDataV2};

/// `["layout", map]`. `finalize_map` hashes the account bytes after the discriminator into
/// `Map.layout_hash`.
/// Patch 13: the core is `MapLayoutDataV2` (cells with variants, points), version 2.
/// Arena update: version 3, the packs and their arenas after the core (`write_arena`). A layout
/// from before it is smaller: it can neither be written nor open a party.
#[account]
#[derive(InitSpace)]
pub struct MapLayout {
    pub core: MapLayoutDataV2,
    pub arenas: MapArenasData,
}
