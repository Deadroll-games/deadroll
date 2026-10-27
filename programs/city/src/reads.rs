//! Accounts of other programs read without CPI (docs/ARCHITECTURE.md, 4.2): owner, discriminator,
//! then the frozen `crates/types` core. The owners are the game programs of `GameConfig`.
use anchor_lang::{prelude::*, Discriminator};
use deadroll_map::state::Map;
use deadroll_types::{GameConfigData, GameSessionData, ProgramIdx};

use crate::errors::CityError;

/// `sha256("account:GameConfig")[..8]`.
pub const GAME_CONFIG_DISCRIMINATOR: [u8; 8] = [0x2d, 0x92, 0x92, 0x21, 0xaa, 0x45, 0x60, 0x85];
/// `sha256("account:GameSession")[..8]`.
pub const GAME_SESSION_DISCRIMINATOR: [u8; 8] = [0x96, 0x74, 0x14, 0xc5, 0xcd, 0x79, 0xdc, 0xf0];

/// The registry `GameConfig` core.
pub fn config(info: &AccountInfo) -> Result<GameConfigData> {
    require_keys_eq!(*info.owner, deadroll_registry::ID, CityError::InvalidConfig);
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 && data[..8] == GAME_CONFIG_DISCRIMINATOR,
        CityError::InvalidConfig
    );
    GameConfigData::deserialize(&mut &data[8..]).map_err(|_| error!(CityError::InvalidConfig))
}

/// The game programs a building may call (`GameConfig.programs`, empty slots left out).
pub fn allowlist(config: &GameConfigData) -> Vec<Pubkey> {
    config
        .programs
        .iter()
        .copied()
        .filter(|p| *p != Pubkey::default())
        .collect()
}

/// A map of the map program (a map from before `biome` reads as Caves).
pub fn map(info: &AccountInfo, config: &GameConfigData) -> Result<Map> {
    require_keys_eq!(
        *info.owner,
        config.programs[ProgramIdx::Map as usize],
        CityError::InvalidMap
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 && data[..8] == *Map::DISCRIMINATOR,
        CityError::InvalidMap
    );
    Map::decode_padded(&data).ok_or_else(|| error!(CityError::InvalidMap))
}

/// The core of a party of the session program.
pub fn session(info: &AccountInfo, config: &GameConfigData) -> Result<GameSessionData> {
    require_keys_eq!(
        *info.owner,
        config.programs[ProgramIdx::Session as usize],
        CityError::InvalidSession
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 + GameSessionData::SIZE && data[..8] == GAME_SESSION_DISCRIMINATOR,
        CityError::InvalidSession
    );
    GameSessionData::deserialize(&mut &data[8..]).map_err(|_| error!(CityError::InvalidSession))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminators_are_anchors() {
        let hash = |name: &[u8]| solana_sha256_hasher::hash(name).to_bytes();
        assert_eq!(GAME_CONFIG_DISCRIMINATOR, hash(b"account:GameConfig")[..8]);
        assert_eq!(
            GAME_SESSION_DISCRIMINATOR,
            hash(b"account:GameSession")[..8]
        );
    }
}
