use anchor_lang::prelude::*;
use deadroll_types::{seeds, ProgramIdx};

use super::set_rasterizer::point_rasterizer;
use crate::{errors::RegistryError, state::GameConfig};

/// CPI from render `finalize_rasterizer` (`docs/STORAGE_AND_DELIVERY.md` 3.2): the finalize that
/// checked the hash also writes the hash and the address here.
#[derive(Accounts)]
pub struct SetRasterizerFromRender<'info> {
    /// The render program's `["authority"]` PDA; checked against `GameConfig.programs[Render]`.
    pub render_authority: Signer<'info>,
    #[account(mut, seeds = [seeds::CONFIG], bump = config.core.bump)]
    pub config: Account<'info, GameConfig>,
    /// CHECK: owner, discriminator, header and PDA address are checked in `state::pull`.
    pub blob: UncheckedAccount<'info>,
}

/// The signer render uses for its CPI into the registry.
pub fn check_render_caller(config: &GameConfig, caller: &Pubkey) -> Result<()> {
    let render = config.core.programs[ProgramIdx::Render as usize];
    let (expected, _) = Pubkey::find_program_address(&[seeds::AUTHORITY], &render);
    require_keys_eq!(*caller, expected, RegistryError::UntrustedCaller);
    Ok(())
}

pub fn handler(ctx: Context<SetRasterizerFromRender>) -> Result<()> {
    check_render_caller(&ctx.accounts.config, ctx.accounts.render_authority.key)?;
    point_rasterizer(&mut ctx.accounts.config.core, &ctx.accounts.blob)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SPRITE_BOOKS_MAX;
    use deadroll_types::{GameConfigData, PROGRAMS_COUNT};

    fn config(render: Pubkey) -> GameConfig {
        let mut programs = [Pubkey::default(); PROGRAMS_COUNT];
        for p in programs.iter_mut() {
            *p = Pubkey::new_unique();
        }
        programs[ProgramIdx::Render as usize] = render;
        GameConfig {
            core: GameConfigData {
                version: 1,
                authority: Pubkey::new_unique(),
                programs,
                rasterizer_version: 0,
                rasterizer: Pubkey::default(),
                rasterizer_sha256: [0; 32],
                palette: Pubkey::default(),
                uri_base: [0; 64],
                bump: 255,
            },
            sprite_books: [Pubkey::default(); SPRITE_BOOKS_MAX],
        }
    }

    #[test]
    fn accepts_only_the_render_authority_pda() {
        let render = Pubkey::new_unique();
        let cfg = config(render);
        let (pda, _) = Pubkey::find_program_address(&[seeds::AUTHORITY], &render);
        assert_eq!(check_render_caller(&cfg, &pda), Ok(()));
        let untrusted: Error = RegistryError::UntrustedCaller.into();
        // The game authority, another program's authority PDA, the render program id itself.
        assert_eq!(
            check_render_caller(&cfg, &cfg.core.authority).unwrap_err(),
            untrusted
        );
        let session = cfg.core.programs[ProgramIdx::Session as usize];
        let (other, _) = Pubkey::find_program_address(&[seeds::AUTHORITY], &session);
        assert_eq!(check_render_caller(&cfg, &other).unwrap_err(), untrusted);
        assert_eq!(check_render_caller(&cfg, &render).unwrap_err(), untrusted);
    }
}
