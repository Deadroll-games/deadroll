//! Read-only access to the registry `GameConfig` (no CPI, `docs/ARCHITECTURE.md` 4.3).

use anchor_lang::prelude::*;
use deadroll_types::{seeds, GameConfigData};

use crate::errors::RenderError;

/// Registry program id (`agents/CONTRACTS.md` 1).
pub const REGISTRY_ID: Pubkey = pubkey!("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");

/// `GameConfig.authority`, after checking the account is the registry `["config"]` PDA.
/// The account is `discriminator ‖ GameConfigData ‖ registry fields` (`CONTRACTS.md` 4.1).
pub fn game_authority(config: &AccountInfo) -> Result<Pubkey> {
    require_keys_eq!(*config.owner, REGISTRY_ID, RenderError::WrongConfig);
    let (expected, _) = Pubkey::find_program_address(&[seeds::CONFIG], &REGISTRY_ID);
    require_keys_eq!(config.key(), expected, RenderError::WrongConfig);
    let data = config.try_borrow_data()?;
    let mut core = data.get(8..).ok_or(RenderError::WrongConfig)?;
    let cfg = GameConfigData::deserialize(&mut core).map_err(|_| RenderError::WrongConfig)?;
    Ok(cfg.authority)
}
