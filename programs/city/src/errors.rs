use anchor_lang::prelude::*;

#[error_code]
pub enum CityError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
}
