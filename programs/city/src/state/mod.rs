//! Accounts owned by this program. One account = one file. Each wraps its frozen
//! `crates/types` core (CONTRACTS.md, section 4.1) and appends lane fields after it.
pub mod building;
pub mod city;
pub mod quest;
pub mod tavern_board;

pub use building::*;
pub use city::*;
pub use quest::*;
pub use tavern_board::*;

/// Layout version written into every account of this program.
pub const ACCOUNT_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    use anchor_lang::{Discriminator, Space};
    use deadroll_types::{BuildingData, CityData, QuestData, TavernBoardData};

    use super::*;

    /// The site reads these accounts by discriminator and at fixed sizes.
    #[test]
    fn layouts() {
        assert_eq!(City::INIT_SPACE, CityData::SIZE);
        assert_eq!(Building::INIT_SPACE, BuildingData::SIZE);
        assert_eq!(TavernBoard::INIT_SPACE, TavernBoardData::SIZE);
        assert_eq!(Quest::INIT_SPACE, QuestData::SIZE);
        let hash = |name: &str| solana_sha256_hasher::hash(format!("account:{name}").as_bytes());
        assert_eq!(City::DISCRIMINATOR, &hash("City").to_bytes()[..8]);
        assert_eq!(Quest::DISCRIMINATOR, &hash("Quest").to_bytes()[..8]);
    }
}
