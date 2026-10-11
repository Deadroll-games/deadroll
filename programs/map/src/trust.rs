//! Trust between game programs (docs/ARCHITECTURE.md, 4.3): a sensitive instruction accepts
//! only the `["authority"]` PDA of a program listed in the registry `GameConfig`.
//!
//! `GameConfig` is read from its bytes (any program can read any account), so the map program
//! needs no registry types: owner, address (checked by the accounts struct) and discriminator,
//! then the frozen `GameConfigData` core.
use anchor_lang::prelude::*;
use deadroll_types::{seeds, GameConfigData, ProgramIdx};

use crate::errors::MapError;

/// `sha256("account:GameConfig")[..8]`.
pub const GAME_CONFIG_DISCRIMINATOR: [u8; 8] = [0x2d, 0x92, 0x92, 0x21, 0xaa, 0x45, 0x60, 0x85];

/// `caller` must be the authority PDA of `GameConfig.programs[program]`.
pub fn check_caller(config: &AccountInfo, caller: &Pubkey, program: ProgramIdx) -> Result<()> {
    require_keys_eq!(
        *config.owner,
        deadroll_registry::ID,
        MapError::InvalidConfig
    );
    let data = config.try_borrow_data()?;
    require!(
        data.len() >= 8 && data[..8] == GAME_CONFIG_DISCRIMINATOR,
        MapError::InvalidConfig
    );
    let core = GameConfigData::deserialize(&mut &data[8..])
        .map_err(|_| error!(MapError::InvalidConfig))?;
    let trusted = core.programs[program as usize];
    require_keys_neq!(trusted, Pubkey::default(), MapError::UntrustedCaller);
    let (authority, _) = Pubkey::find_program_address(&[seeds::AUTHORITY], &trusted);
    require_keys_eq!(*caller, authority, MapError::UntrustedCaller);
    Ok(())
}
