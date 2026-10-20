use anchor_lang::prelude::*;
use deadroll_types::{seeds, GameConfigData, ProgramIdx, PROGRAMS_COUNT, URI_BASE_LEN};

use crate::{
    errors::RegistryError,
    events::ConfigInitialized,
    state::{GameConfig, ACCOUNT_VERSION, SPRITE_BOOKS_MAX},
};

#[derive(Accounts)]
pub struct InitConfig<'info> {
    /// Becomes `GameConfig.authority`.
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + GameConfig::INIT_SPACE,
        seeds = [seeds::CONFIG],
        bump,
    )]
    pub config: Account<'info, GameConfig>,
    pub system_program: Program<'info, System>,
}

/// No slot may be empty and the `Registry` slot is this program.
pub fn check_programs(programs: &[Pubkey; PROGRAMS_COUNT]) -> Result<()> {
    require!(
        programs.iter().all(|p| *p != Pubkey::default()),
        RegistryError::EmptyProgramId
    );
    require_keys_eq!(
        programs[ProgramIdx::Registry as usize],
        crate::ID,
        RegistryError::WrongRegistryId
    );
    Ok(())
}

pub fn handler(
    ctx: Context<InitConfig>,
    programs: [Pubkey; PROGRAMS_COUNT],
    uri_base: [u8; URI_BASE_LEN],
) -> Result<()> {
    check_programs(&programs)?;
    ctx.accounts.config.core = GameConfigData {
        version: ACCOUNT_VERSION,
        authority: ctx.accounts.authority.key(),
        programs,
        rasterizer_version: 0,
        rasterizer: Pubkey::default(),
        rasterizer_sha256: [0; 32],
        palette: Pubkey::default(),
        uri_base,
        bump: ctx.bumps.config,
    };
    ctx.accounts.config.sprite_books = [Pubkey::default(); SPRITE_BOOKS_MAX];
    emit!(ConfigInitialized {
        authority: ctx.accounts.authority.key()
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> [Pubkey; PROGRAMS_COUNT] {
        let mut p = [Pubkey::default(); PROGRAMS_COUNT];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = Pubkey::new_from_array([i as u8 + 1; 32]);
        }
        p[ProgramIdx::Registry as usize] = crate::ID;
        p
    }

    #[test]
    fn accepts_a_full_list() {
        assert_eq!(check_programs(&list()), Ok(()));
    }

    #[test]
    fn rejects_an_empty_slot() {
        let mut p = list();
        p[ProgramIdx::City as usize] = Pubkey::default();
        assert_eq!(
            check_programs(&p),
            Err(RegistryError::EmptyProgramId.into())
        );
    }

    #[test]
    fn rejects_a_foreign_registry_slot() {
        let mut p = list();
        p[ProgramIdx::Registry as usize] = Pubkey::new_unique();
        assert_eq!(
            check_programs(&p),
            Err(RegistryError::WrongRegistryId.into())
        );
    }
}
