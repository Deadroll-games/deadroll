use anchor_lang::prelude::*;

#[error_code]
pub enum RenderError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
    #[msg("Body length is zero or above the limit of this account kind")]
    InvalidLength,
    #[msg("The account is finalized and can no longer change")]
    AlreadyFinalized,
    #[msg("Write is outside the body or the allocated part of the account")]
    OutOfBounds,
    #[msg("The account is not extended to its full size yet")]
    NotFullyAllocated,
    #[msg("The account already has its full size")]
    AlreadyFullyAllocated,
    #[msg("SHA-256 of the body does not match the expected hash")]
    HashMismatch,
    #[msg("Body content is not valid for this account kind")]
    InvalidBody,
    #[msg("Not a rasterizer, palette or sprite book account of this program")]
    WrongAccount,
    #[msg("Not the registry GameConfig account")]
    WrongConfig,
}
