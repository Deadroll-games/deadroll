use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use deadroll_types::{seeds, MapStatus, ProgramIdx};

use crate::{errors::MapError, events::MapStatusChanged, state::Map, trust};

/// CPI target for `session` (CONTRACTS.md, section 5). Account order is part of the contract:
/// `[map (mut), session_authority (signer), config]`.
#[derive(Accounts)]
pub struct SetStatus<'info> {
    /// CHECK: owner here, discriminator and address in `write_status`.
    /// A map of this program, also one from before `Map.biome` (one byte shorter, which
    /// `Account<Map>` cannot read); only `core.status` is written.
    #[account(mut, owner = crate::ID)]
    pub map: UncheckedAccount<'info>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig` in the handler.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`; owner and discriminator checked in `trust::check_caller`.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
}

/// `Finalized -> InSession` when a party opens, `InSession -> Finished` when it ends.
pub fn transition_allowed(from: MapStatus, to: MapStatus) -> bool {
    matches!(
        (from, to),
        (MapStatus::Finalized, MapStatus::InSession) | (MapStatus::InSession, MapStatus::Finished)
    )
}

/// Moves the map in `data` (the account at `key`) to `to`; returns the status it had. The checks
/// `Account<Map>` with `seeds` would make, on the bytes, so that maps from before `biome` move too.
pub fn write_status(data: &mut [u8], key: &Pubkey, to: MapStatus) -> Result<MapStatus> {
    require!(
        data.starts_with(Map::DISCRIMINATOR),
        ErrorCode::AccountDiscriminatorMismatch
    );
    let map = Map::decode_padded(data).ok_or(error!(ErrorCode::AccountDidNotDeserialize))?;
    let core = &map.core;
    let address = Pubkey::create_program_address(
        &[
            seeds::MAP,
            core.dm.as_ref(),
            &core.index.to_le_bytes(),
            &[core.bump],
        ],
        &crate::ID,
    )
    .map_err(|_| error!(ErrorCode::ConstraintSeeds))?;
    require_keys_eq!(address, *key, ErrorCode::ConstraintSeeds);
    require!(
        transition_allowed(core.status, to),
        MapError::InvalidStatusTransition
    );
    data[Map::STATUS_OFFSET] = to as u8;
    Ok(core.status)
}

pub fn handler(ctx: Context<SetStatus>, status: MapStatus) -> Result<()> {
    trust::check_caller(
        &ctx.accounts.config,
        ctx.accounts.session_authority.key,
        ProgramIdx::Session,
    )?;
    let info = ctx.accounts.map.to_account_info();
    let from = write_status(&mut info.try_borrow_mut_data()?, info.key, status)?;

    emit!(MapStatusChanged {
        map: info.key(),
        from,
        to: status
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests as state_tests;
    use deadroll_types::Biome;

    #[test]
    fn only_the_party_lifecycle_moves_forward() {
        for &from in MapStatus::ALL {
            for &to in MapStatus::ALL {
                let expected = matches!(
                    (from, to),
                    (MapStatus::Finalized, MapStatus::InSession)
                        | (MapStatus::InSession, MapStatus::Finished)
                );
                assert_eq!(transition_allowed(from, to), expected, "{from:?} -> {to:?}");
            }
        }
    }

    fn old_map(status: MapStatus) -> (Vec<u8>, Pubkey) {
        let mut m = state_tests::map(Biome::Caves);
        let (key, bump) = Pubkey::find_program_address(
            &[seeds::MAP, m.core.dm.as_ref(), &m.core.index.to_le_bytes()],
            &crate::ID,
        );
        m.core.version = 1;
        m.core.bump = bump;
        m.core.status = status;
        (state_tests::stored(&m, 8 + Map::V1_SPACE), key)
    }

    /// A map from before `biome` enters a party and leaves it; nothing past the status changes.
    #[test]
    fn moves_a_map_without_biome() {
        let (mut data, key) = old_map(MapStatus::Finalized);
        let before = data.clone();
        assert_eq!(
            write_status(&mut data, &key, MapStatus::InSession).unwrap(),
            MapStatus::Finalized
        );
        assert_eq!(
            Map::decode_padded(&data).unwrap().core.status,
            MapStatus::InSession
        );
        assert_eq!(
            write_status(&mut data, &key, MapStatus::Finished).unwrap(),
            MapStatus::InSession
        );
        assert_eq!(data.len(), before.len());
        assert_eq!(
            data[Map::STATUS_OFFSET + 1..],
            before[Map::STATUS_OFFSET + 1..]
        );
    }

    #[test]
    fn moves_a_new_map() {
        let mut m = state_tests::map(Biome::Forest);
        let (key, bump) = Pubkey::find_program_address(
            &[seeds::MAP, m.core.dm.as_ref(), &m.core.index.to_le_bytes()],
            &crate::ID,
        );
        m.core.bump = bump;
        let mut data = state_tests::stored(&m, 8 + Map::INIT_SPACE);
        write_status(&mut data, &key, MapStatus::InSession).unwrap();
        let back = Map::decode_padded(&data).unwrap();
        assert_eq!(back.core.status, MapStatus::InSession);
        assert_eq!(back.biome, Biome::Forest as u8);
    }

    #[test]
    fn rejects_a_bad_transition_another_address_and_another_account() {
        let (mut data, key) = old_map(MapStatus::Finalized);
        let code = |e: Error| match e {
            Error::AnchorError(a) => a.error_code_number,
            Error::ProgramError(_) => 0,
        };
        let err = write_status(&mut data, &key, MapStatus::Finished).unwrap_err();
        assert_eq!(
            code(err),
            MapError::InvalidStatusTransition as u32 + anchor_lang::error::ERROR_CODE_OFFSET
        );
        let err = write_status(&mut data, &Pubkey::new_unique(), MapStatus::InSession).unwrap_err();
        assert_eq!(code(err), ErrorCode::ConstraintSeeds as u32);
        let mut short = data[..data.len() - 1].to_vec();
        let err = write_status(&mut short, &key, MapStatus::InSession).unwrap_err();
        assert_eq!(code(err), ErrorCode::AccountDidNotDeserialize as u32);
        data[0] ^= 1;
        let err = write_status(&mut data, &key, MapStatus::InSession).unwrap_err();
        assert_eq!(code(err), ErrorCode::AccountDiscriminatorMismatch as u32);
        // Nothing was written by a rejected call.
        data[0] ^= 1;
        assert_eq!(
            Map::decode_padded(&data).unwrap().core.status,
            MapStatus::Finalized
        );
    }
}
