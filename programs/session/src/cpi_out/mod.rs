//! Outgoing CPI into other game programs (CONTRACTS.md, section 5), signed by the session
//! `["authority"]` PDA.
//!
//! STUB(L5/L6): `deadroll_character` and `deadroll_map` have no instructions on `main` yet, so
//! there is no generated `cpi` module to call. These wrappers build the Anchor instruction by
//! hand (`sha256("global:<name>")[..8]` + Borsh args) with the account list below. When the
//! callee lands, swap the body for `<callee>::cpi::<name>` and keep the signature.
pub mod character;
pub mod map;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use deadroll_types::seeds;

/// Accounts every trusted callee needs: the target account, our authority PDA and `GameConfig`.
pub struct TrustedCall<'a, 'info> {
    pub program: &'a AccountInfo<'info>,
    pub target: &'a AccountInfo<'info>,
    pub session_authority: &'a AccountInfo<'info>,
    pub config: &'a AccountInfo<'info>,
    pub authority_bump: u8,
}

impl TrustedCall<'_, '_> {
    fn invoke(&self, data: Vec<u8>) -> Result<()> {
        let ix = Instruction {
            program_id: *self.program.key,
            accounts: vec![
                AccountMeta::new(*self.target.key, false),
                AccountMeta::new_readonly(*self.session_authority.key, true),
                AccountMeta::new_readonly(*self.config.key, false),
            ],
            data,
        };
        invoke_signed(
            &ix,
            &[
                self.target.clone(),
                self.session_authority.clone(),
                self.config.clone(),
                self.program.clone(),
            ],
            &[&[seeds::AUTHORITY, &[self.authority_bump]]],
        )
        .map_err(Into::into)
    }
}
