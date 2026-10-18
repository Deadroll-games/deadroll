use anchor_lang::prelude::*;
use deadroll_types::GameConfigData;

/// Sprite book slots in `GameConfig` (`docs/STORAGE_AND_DELIVERY.md` 2, "one or several").
pub const SPRITE_BOOKS_MAX: usize = 4;

/// `["config"]`. Singleton: trusted program ids, the rasterizer pointer, the NFT uri base.
/// The struct name is part of the format: other programs check `account:GameConfig`.
#[account]
#[derive(InitSpace)]
pub struct GameConfig {
    pub core: GameConfigData,
    /// Patch 7: the `SpriteBook`s the website draws from, in lookup order (`crates/render/FORMAT.md` 7).
    /// `Pubkey::default()` is an empty slot; set slots come first.
    pub sprite_books: [Pubkey; SPRITE_BOOKS_MAX],
}

/// Account size of a `GameConfig` created before `sprite_books` (`resize_config` grows it).
pub const LEGACY_CONFIG_SPACE: usize = 8 + GameConfigData::SIZE;

/// Account size of the current `GameConfig`.
pub const CONFIG_SPACE: usize = 8 + GameConfig::INIT_SPACE;
