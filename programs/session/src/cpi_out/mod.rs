//! Outgoing CPI into other game programs (CONTRACTS.md, section 5) through their generated `cpi`
//! modules, signed by the session `["authority"]` PDA. The callee checks that PDA against
//! `GameConfig.programs[Session]`.
pub mod character;
pub mod map;

use anchor_lang::prelude::*;

/// The accounts every trusted call carries after its target: our authority PDA and `GameConfig`.
pub struct SessionSigner<'a, 'info> {
    pub session_authority: &'a AccountInfo<'info>,
    pub config: &'a AccountInfo<'info>,
    pub bump: u8,
}

impl<'info> SessionSigner<'_, 'info> {
    fn authority(&self) -> AccountInfo<'info> {
        self.session_authority.clone()
    }

    fn config(&self) -> AccountInfo<'info> {
        self.config.clone()
    }
}

/// Runs `call` with the `["authority", bump]` signer seeds.
fn signed<R>(bump: u8, call: impl FnOnce(&[&[&[u8]]]) -> R) -> R {
    let bump = [bump];
    let seeds: &[&[u8]] = &[deadroll_types::seeds::AUTHORITY, &bump];
    call(&[seeds])
}
