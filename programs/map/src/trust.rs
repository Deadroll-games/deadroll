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

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::{PROGRAMS_COUNT, URI_BASE_LEN};

    fn config(session: Pubkey) -> GameConfigData {
        let mut programs = [Pubkey::default(); PROGRAMS_COUNT];
        programs[ProgramIdx::Session as usize] = session;
        GameConfigData {
            version: 1,
            authority: Pubkey::new_unique(),
            programs,
            rasterizer_version: 0,
            rasterizer: Pubkey::default(),
            rasterizer_sha256: [0; 32],
            palette: Pubkey::default(),
            uri_base: [0; URI_BASE_LEN],
            bump: 255,
        }
    }

    /// Account bytes as the registry stores them: discriminator, core, lane fields.
    fn bytes(discriminator: [u8; 8], core: &GameConfigData) -> Vec<u8> {
        let mut data = discriminator.to_vec();
        core.serialize(&mut data).unwrap();
        data.extend_from_slice(&[0; 16]);
        data
    }

    fn check(owner: Pubkey, mut data: Vec<u8>, caller: Pubkey) -> Result<()> {
        let key = Pubkey::new_unique();
        let mut lamports = 1;
        let info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        check_caller(&info, &caller, ProgramIdx::Session)
    }

    fn authority_of(program: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[seeds::AUTHORITY], program).0
    }

    #[test]
    fn accepts_the_listed_programs_authority() {
        let session = Pubkey::new_unique();
        let data = bytes(GAME_CONFIG_DISCRIMINATOR, &config(session));
        assert_eq!(
            check(deadroll_registry::ID, data, authority_of(&session)),
            Ok(())
        );
    }

    #[test]
    fn rejects_other_callers() {
        let session = Pubkey::new_unique();
        let data = || bytes(GAME_CONFIG_DISCRIMINATOR, &config(session));
        let untrusted = Err(MapError::UntrustedCaller.into());
        // A wallet, the program id itself, another program's authority.
        assert_eq!(
            check(deadroll_registry::ID, data(), Pubkey::new_unique()),
            untrusted
        );
        assert_eq!(check(deadroll_registry::ID, data(), session), untrusted);
        let other = authority_of(&Pubkey::new_unique());
        assert_eq!(check(deadroll_registry::ID, data(), other), untrusted);
    }

    #[test]
    fn rejects_an_empty_slot() {
        let data = bytes(GAME_CONFIG_DISCRIMINATOR, &config(Pubkey::default()));
        let caller = authority_of(&Pubkey::default());
        let result = check(deadroll_registry::ID, data, caller);
        assert_eq!(result, Err(MapError::UntrustedCaller.into()));
    }

    #[test]
    fn rejects_a_config_from_another_owner() {
        let session = Pubkey::new_unique();
        let data = bytes(GAME_CONFIG_DISCRIMINATOR, &config(session));
        let result = check(Pubkey::new_unique(), data, authority_of(&session));
        assert_eq!(result, Err(MapError::InvalidConfig.into()));
    }

    #[test]
    fn rejects_bad_config_bytes() {
        let session = Pubkey::new_unique();
        let caller = authority_of(&session);
        let invalid = Err(MapError::InvalidConfig.into());
        let wrong_disc = bytes([0; 8], &config(session));
        assert_eq!(check(deadroll_registry::ID, wrong_disc, caller), invalid);
        let mut short = bytes(GAME_CONFIG_DISCRIMINATOR, &config(session));
        short.truncate(40);
        assert_eq!(check(deadroll_registry::ID, short, caller), invalid);
        assert_eq!(check(deadroll_registry::ID, vec![], caller), invalid);
    }

    #[test]
    fn discriminator_is_anchors() {
        let hash = solana_sha256_hasher::hash(b"account:GameConfig").to_bytes();
        assert_eq!(GAME_CONFIG_DISCRIMINATOR, hash[..8]);
    }
}
