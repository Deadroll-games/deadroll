use anchor_lang::{prelude::*, system_program, Discriminator};
use deadroll_types::{seeds, GameConfigData};

use crate::{
    errors::RegistryError,
    events::ConfigResized,
    state::{GameConfig, CONFIG_SPACE},
};

/// Authority grows a `GameConfig` created before the patch 7 field `sprite_books` to the current
/// size and tops up its rent. The new bytes are zero (empty slots). Run once after the program
/// upgrade: until then no instruction can load the account as a `GameConfig`.
#[derive(Accounts)]
pub struct ResizeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: the `["config"]` PDA. It is shorter than `GameConfig`, so the core is read by hand.
    #[account(mut, seeds = [seeds::CONFIG], bump)]
    pub config: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// The authority and the length of a registry `GameConfig` older than the current layout.
pub fn read_old_config(info: &AccountInfo) -> Result<(Pubkey, usize)> {
    require_keys_eq!(*info.owner, crate::ID, RegistryError::WrongConfigAccount);
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= 8 && data[..8] == *GameConfig::DISCRIMINATOR,
        RegistryError::WrongConfigAccount
    );
    let core = GameConfigData::deserialize(&mut &data[8..])
        .map_err(|_| RegistryError::WrongConfigAccount)?;
    require!(
        data.len() < CONFIG_SPACE,
        RegistryError::ConfigAlreadyResized
    );
    Ok((core.authority, data.len()))
}

pub fn handler(ctx: Context<ResizeConfig>) -> Result<()> {
    let info = ctx.accounts.config.to_account_info();
    let (authority, old) = read_old_config(&info)?;
    require_keys_eq!(
        authority,
        ctx.accounts.authority.key(),
        RegistryError::Unauthorized
    );
    let need = Rent::get()?
        .minimum_balance(CONFIG_SPACE)
        .saturating_sub(info.lamports());
    if need > 0 {
        system_program::transfer(
            CpiContext::new(
                system_program::ID,
                system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: info.clone(),
                },
            ),
            need,
        )?;
    }
    info.resize(CONFIG_SPACE)?;
    emit!(ConfigResized {
        old_len: old as u32,
        new_len: CONFIG_SPACE as u32,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LEGACY_CONFIG_SPACE;

    fn legacy(authority: Pubkey, len: usize) -> Vec<u8> {
        let mut d = vec![0u8; len];
        d[..8].copy_from_slice(GameConfig::DISCRIMINATOR);
        d[8] = 1;
        d[9..41].copy_from_slice(authority.as_ref());
        d
    }

    fn read(owner: Pubkey, data: &mut [u8]) -> Result<(Pubkey, usize)> {
        let key = Pubkey::new_unique();
        let mut lamports = 1;
        let info = AccountInfo::new(&key, false, true, &mut lamports, data, &owner, false);
        read_old_config(&info)
    }

    #[test]
    fn reads_a_legacy_config() {
        let a = Pubkey::new_unique();
        let mut d = legacy(a, LEGACY_CONFIG_SPACE);
        assert_eq!(read(crate::ID, &mut d).unwrap(), (a, LEGACY_CONFIG_SPACE));
    }

    #[test]
    fn rejects_resized_foreign_and_garbage() {
        let a = Pubkey::new_unique();
        let mut done = legacy(a, CONFIG_SPACE);
        assert_eq!(
            read(crate::ID, &mut done).unwrap_err(),
            RegistryError::ConfigAlreadyResized.into()
        );
        let wrong: Error = RegistryError::WrongConfigAccount.into();
        let mut d = legacy(a, LEGACY_CONFIG_SPACE);
        assert_eq!(read(Pubkey::new_unique(), &mut d).unwrap_err(), wrong);
        let mut short = legacy(a, 100);
        assert_eq!(read(crate::ID, &mut short).unwrap_err(), wrong);
        let mut disc = legacy(a, LEGACY_CONFIG_SPACE);
        disc[0] ^= 1;
        assert_eq!(read(crate::ID, &mut disc).unwrap_err(), wrong);
    }
}
