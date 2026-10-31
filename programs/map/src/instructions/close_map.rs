use anchor_lang::{prelude::*, system_program};
use deadroll_types::{seeds, MapStatus, ProgramIdx};

use crate::{errors::MapError, events::MapClosed, state::Map, trust};

/// CPI target for `session` (CONTRACTS.md, section 5). Account order is part of the contract:
/// `[map (mut), layout (mut), dm (mut), session_authority (signer), config]`.
#[derive(Accounts)]
pub struct CloseMap<'info> {
    /// CHECK: owner here; discriminator, address and status checked by `check_closable`. Any map
    /// version (only the core is read).
    #[account(mut, owner = crate::ID)]
    pub map: UncheckedAccount<'info>,
    /// CHECK: the map's `MapLayout` (seeds), owned here; closed with the map.
    #[account(mut, owner = crate::ID, seeds = [seeds::LAYOUT, map.key().as_ref()], bump)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: receives the deposits; must be the map's DM (`check_closable`).
    #[account(mut)]
    pub dm: UncheckedAccount<'info>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig` in the handler.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`; owner and discriminator checked in `trust::check_caller`.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
}

/// The map in `data` (the account at `key`) is a map of this program, in a party, of `dm`.
pub fn check_closable(data: &[u8], key: &Pubkey, dm: &Pubkey) -> Result<()> {
    use anchor_lang::Discriminator;
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
        core.status == MapStatus::InSession,
        MapError::MapNotInSession
    );
    require_keys_eq!(core.dm, *dm, MapError::NotDm);
    Ok(())
}

/// Moves every lamport of `from` to `to` and gives the account back to the system program, empty.
fn close_into(from: &AccountInfo, to: &AccountInfo) -> Result<u64> {
    let lamports = from.lamports();
    **to.try_borrow_mut_lamports()? = to
        .lamports()
        .checked_add(lamports)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    **from.try_borrow_mut_lamports()? = 0;
    from.assign(&system_program::ID);
    from.resize(0)?;
    Ok(lamports)
}

/// Patch D (docs/MAP_DESCRIPTION.md 17.7): a map is played once. When its party has issued the
/// rewards (`settle_party`, or the last `claim_after_timeout`), the session closes the map and its
/// layout, and both deposits go back to the DM.
pub fn handler(ctx: Context<CloseMap>) -> Result<()> {
    let a = ctx.accounts;
    trust::check_caller(&a.config, a.session_authority.key, ProgramIdx::Session)?;
    let map = a.map.to_account_info();
    check_closable(&map.try_borrow_data()?, map.key, a.dm.key)?;
    let dm = a.dm.to_account_info();
    let lamports = close_into(&a.layout.to_account_info(), &dm)? + close_into(&map, &dm)?;
    emit!(MapClosed {
        map: map.key(),
        dm: dm.key(),
        lamports,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests as state_tests;
    use deadroll_types::Biome;

    fn code(e: Error) -> u32 {
        match e {
            Error::AnchorError(a) => a.error_code_number,
            Error::ProgramError(_) => 0,
        }
    }

    fn map(status: MapStatus) -> (Vec<u8>, Pubkey, Pubkey) {
        let mut m = state_tests::map(Biome::Caves);
        let (key, bump) = Pubkey::find_program_address(
            &[seeds::MAP, m.core.dm.as_ref(), &m.core.index.to_le_bytes()],
            &crate::ID,
        );
        m.core.bump = bump;
        m.core.status = status;
        (state_tests::stored(&m, 8 + Map::INIT_SPACE), key, m.core.dm)
    }

    #[test]
    fn only_a_map_in_a_party_closes_to_its_dm() {
        let (data, key, dm) = map(MapStatus::InSession);
        assert!(check_closable(&data, &key, &dm).is_ok());
        let off = |e: MapError| e as u32 + anchor_lang::error::ERROR_CODE_OFFSET;
        for status in [MapStatus::Draft, MapStatus::Finalized, MapStatus::Finished] {
            let (data, key, dm) = map(status);
            let err = check_closable(&data, &key, &dm).unwrap_err();
            assert_eq!(code(err), off(MapError::MapNotInSession), "{status:?}");
        }
        let err = check_closable(&data, &key, &Pubkey::new_unique()).unwrap_err();
        assert_eq!(code(err), off(MapError::NotDm));
        let err = check_closable(&data, &Pubkey::new_unique(), &dm).unwrap_err();
        assert_eq!(code(err), ErrorCode::ConstraintSeeds as u32);
        let mut other = data.clone();
        other[0] ^= 1;
        let err = check_closable(&other, &key, &dm).unwrap_err();
        assert_eq!(code(err), ErrorCode::AccountDiscriminatorMismatch as u32);
    }

    // Closing itself (lamports, owner, size) needs the runtime: tests/map.test.ts.
}
