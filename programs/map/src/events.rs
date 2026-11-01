use anchor_lang::prelude::*;
use deadroll_types::MapStatus;

#[event]
pub struct MapCreated {
    pub map: Pubkey,
    pub dm: Pubkey,
    pub index: u32,
    pub width: u8,
    pub height: u8,
    pub tier: u8,
    /// Patch 11.
    pub min_players: u8,
    pub chest_radius: u8,
    pub settle_timeout_secs: u32,
    /// The art set (`deadroll_types::Biome`).
    pub biome: u8,
}

#[event]
pub struct LayoutWritten {
    pub map: Pubkey,
    pub offset: u16,
    pub len: u16,
}

#[event]
pub struct SpawnsWritten {
    pub map: Pubkey,
    pub spawn_count: u8,
}

#[event]
pub struct MapFinalized {
    pub map: Pubkey,
    pub dm: Pubkey,
    pub layout_hash: [u8; 32],
    pub secrets_root: [u8; 32],
}

#[event]
pub struct MapStatusChanged {
    pub map: Pubkey,
    pub from: MapStatus,
    pub to: MapStatus,
}

/// Patch 13 (replaces `ChestsWritten` of patch 11).
#[event]
pub struct PointsWritten {
    pub map: Pubkey,
    pub point_count: u8,
    pub boss_table: u8,
}

/// Patch C: a chunk of the DM's encrypted secrets was written (`write_secrets`).
#[event]
pub struct SecretsWritten {
    pub map: Pubkey,
    pub offset: u16,
    pub len: u16,
    /// The whole blob's length.
    pub total: u16,
    /// Each pack's threat (patch D: one per pack number of the map's class) and the declared
    /// counts (the public part).
    pub threat: Vec<u8>,
    pub declared: [u8; 4],
}

/// Patch D: the layout grew towards its full size (`grow_layout`).
#[event]
pub struct LayoutGrown {
    pub map: Pubkey,
    /// Its size now and its full size, in bytes.
    pub len: u32,
    pub full: u32,
}

/// Patch D: the reachability check went on (`check_step`, `finalize_map`).
#[event]
pub struct MapChecked {
    pub map: Pubkey,
    /// `deadroll_mapcheck::wide` phase and row reached.
    pub phase: u8,
    pub row: u16,
}

/// Patch D: a played map was closed and its deposits went back to its DM (`close_map`).
#[event]
pub struct MapClosed {
    pub map: Pubkey,
    pub dm: Pubkey,
    /// Lamports returned (the map and its layout).
    pub lamports: u64,
}

/// Arena update: a pack's record and arena were written (`write_arena`).
#[event]
pub struct ArenaWritten {
    pub map: Pubkey,
    pub pack: u8,
    pub width: u8,
    pub height: u8,
}
