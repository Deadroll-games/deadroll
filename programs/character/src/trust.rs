//! Trust between game programs (docs/ARCHITECTURE.md, 4.3): sensitive instructions accept only
//! the `["authority"]` PDA of a game program listed in `GameConfig.programs`.
//!
//! `GameConfig` is read in place from the registry account, no CPI and no registry types: the
//! frozen `GameConfigData` core sits right after the 8-byte discriminator (CONTRACTS.md, 4.1).
use anchor_lang::prelude::*;
use deadroll_types::{seeds, GameConfigData, ProgramIdx};

use crate::errors::CharacterError;

/// `deadroll_registry::ID`, the owner of `GameConfig`. A constant, not a dependency on the registry
/// crate (whose SBF build clashes with this program's mpl-core and session-keys graph);
/// `crates/sim/tests/parity.rs` checks it.
pub const REGISTRY_PROGRAM: Pubkey = pubkey!("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");

/// Decodes `GameConfig` and checks it is the registry's `["config"]` PDA (owner, then address
/// with the stored bump: one hash instead of a bump search).
pub fn read_config(info: &AccountInfo) -> Result<GameConfigData> {
    require_keys_eq!(
        *info.owner,
        REGISTRY_PROGRAM,
        CharacterError::InvalidConfig
    );
    let config = {
        let data = info.try_borrow_data()?;
        require!(
            data.len() >= 8 + GameConfigData::SIZE,
            CharacterError::InvalidConfig
        );
        GameConfigData::deserialize(&mut &data[8..])
            .map_err(|_| error!(CharacterError::InvalidConfig))?
    };
    let address =
        Pubkey::create_program_address(&[seeds::CONFIG, &[config.bump]], &REGISTRY_PROGRAM)
            .map_err(|_| error!(CharacterError::InvalidConfig))?;
    require_keys_eq!(*info.key, address, CharacterError::InvalidConfig);
    Ok(config)
}

/// `caller` must be the `["authority"]` PDA of `config.programs[program]`.
pub fn check_caller(config: &GameConfigData, caller: &Pubkey, program: ProgramIdx) -> Result<()> {
    let program_id = config.programs[program as usize];
    require!(
        program_id != Pubkey::default(),
        CharacterError::UntrustedCaller
    );
    let (authority, _) = Pubkey::find_program_address(&[seeds::AUTHORITY], &program_id);
    require_keys_eq!(*caller, authority, CharacterError::UntrustedCaller);
    Ok(())
}

/// The check every session-only instruction runs first.
pub fn check_session_caller(config: &AccountInfo, caller: &Pubkey) -> Result<()> {
    let config = read_config(config)?;
    check_caller(&config, caller, ProgramIdx::Session)
}
