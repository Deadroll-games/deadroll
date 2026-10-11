use anchor_lang::prelude::*;

#[error_code]
pub enum CharacterError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
}
