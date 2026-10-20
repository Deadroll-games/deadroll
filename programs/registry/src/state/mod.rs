//! Accounts owned by this program. One account = one file.
pub mod game_config;
pub mod pull;
pub mod records;
pub mod table;
pub mod view;

pub use game_config::*;
pub use records::TableKind;
pub use table::*;

/// Layout version written into every registry account.
pub const ACCOUNT_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    use anchor_lang::{Discriminator, Space};
    use deadroll_types::GameConfigData;

    use super::*;

    /// `map` hardcodes the `GameConfig` discriminator and reads the core at offset 8.
    #[test]
    fn game_config_format() {
        assert_eq!(
            GameConfig::DISCRIMINATOR,
            [0x2d, 0x92, 0x92, 0x21, 0xaa, 0x45, 0x60, 0x85]
        );
        // The core first, then the registry fields (agents/CONTRACTS.md 4.1).
        assert_eq!(
            GameConfig::INIT_SPACE,
            GameConfigData::SIZE + 32 * SPRITE_BOOKS_MAX
        );
        assert_eq!(LEGACY_CONFIG_SPACE, 8 + 420);
        assert_eq!(CONFIG_SPACE, 8 + 420 + 128);
    }

    #[test]
    fn table_is_header_plus_body() {
        assert_eq!(core::mem::size_of::<Table>(), 8 + TABLE_CAP);
    }
}
