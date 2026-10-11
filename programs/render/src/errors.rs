use anchor_lang::prelude::*;

#[error_code]
pub enum RenderError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
}
