use anchor_lang::prelude::*;
use deadroll_types::GameConfigData;

/// `["config"]`. Singleton: trusted program ids, the rasterizer pointer, the NFT uri base.
/// The struct name is part of the format: other programs check `account:GameConfig`.
#[account]
#[derive(InitSpace)]
pub struct GameConfig {
    pub core: GameConfigData,
}
