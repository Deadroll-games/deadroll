use anchor_lang::prelude::*;

#[error_code]
pub enum RegistryError {
    #[msg("Caller is not the game authority")]
    Unauthorized,
    #[msg("A program id in the list is empty")]
    EmptyProgramId,
    #[msg("Slot 0 of the program list must be the registry itself")]
    WrongRegistryId,
    #[msg("Blob account is not owned by the render program")]
    WrongBlobOwner,
    #[msg("Blob account is not the render PDA for its id")]
    WrongBlobAddress,
    #[msg("Blob account has the wrong type or layout version")]
    WrongBlobKind,
    #[msg("Blob is not finalized yet")]
    BlobNotFinalized,
    #[msg("Write goes past the end of the table")]
    TableRangeOutOfBounds,
    #[msg("Write is not aligned to the record size")]
    TableMisaligned,
    #[msg("A record does not pass validation")]
    InvalidRecord,
    #[msg("Caller is not the authority PDA of the trusted render program")]
    UntrustedCaller,
    #[msg("Not the registry GameConfig account")]
    WrongConfigAccount,
    #[msg("GameConfig already has its current size")]
    ConfigAlreadyResized,
    #[msg("Give 1 to 4 sprite books")]
    SpriteBookCount,
    #[msg("The same sprite book is given twice")]
    DuplicateSpriteBook,
}
