use anchor_lang::prelude::*;

/// Append only: the codes are part of the client contract.
#[error_code]
pub enum CityError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
    #[msg("GameConfig account is not the registry config")]
    InvalidConfig,
    #[msg("City size is out of range")]
    SizeOutOfRange,
    #[msg("Cell or range is outside the city")]
    OutOfCity,
    #[msg("Too many decor objects")]
    TooManyObjects,
    #[msg("Program is not one of the game programs")]
    ProgramNotAllowed,
    #[msg("Building target does not match its kind")]
    BadTarget,
    #[msg("Building is closed")]
    BuildingClosed,
    #[msg("Building is not a Tavern")]
    NotATavern,
    #[msg("Board is not this building's board")]
    BadBoard,
    #[msg("Access condition of the building is not met")]
    NoAccess,
    #[msg("Map account is not a map of the map program")]
    InvalidMap,
    #[msg("Map is not in a party")]
    MapNotInSession,
    #[msg("Signer is not the DM")]
    NotTheDm,
    #[msg("Session account is not a party of the session program")]
    InvalidSession,
    #[msg("Party is not in the lobby")]
    SessionNotInLobby,
    #[msg("Party is not on this map")]
    SessionMapMismatch,
    #[msg("Party is still gathering players")]
    QuestStillOpen,
}
