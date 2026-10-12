use anchor_lang::prelude::*;

#[error_code]
pub enum SessionError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
    #[msg("Session key used without a session token")]
    SessionTokenMissing,
    #[msg("Session token is not valid for this signer, wallet or program")]
    InvalidSessionToken,
    #[msg("Session token has expired")]
    SessionTokenExpired,
    #[msg("Only the DM of this session can do this")]
    NotDm,
    #[msg("Only the player who joined with this character can do this")]
    NotPlayer,
    #[msg("Account is not a Deadroll map")]
    InvalidMap,
    #[msg("Account is not the layout of this map")]
    InvalidLayout,
    #[msg("Account is not a Deadroll character")]
    InvalidCharacter,
    #[msg("Map is not finalized")]
    MapNotFinalized,
    #[msg("Party price is below the minimum")]
    PriceTooLow,
    #[msg("Session is not in the lobby")]
    NotInLobby,
    #[msg("Session is not active")]
    NotActive,
    #[msg("Party is full")]
    PartyFull,
    #[msg("Character does not belong to the signer")]
    NotCharacterOwner,
    #[msg("Character is dead")]
    CharacterDead,
    #[msg("Character is already in a session")]
    CharacterBusy,
    #[msg("Party has no players")]
    NoPlayers,
    #[msg("Not every player is ready")]
    NotAllReady,
    #[msg("Party price is not collected yet")]
    PriceNotCollected,
    #[msg("Waiting for a dice roll")]
    RollPending,
    #[msg("It is not this unit's turn")]
    NotYourTurn,
    #[msg("Unit is dead")]
    UnitDead,
    #[msg("Account is not the monster state of this session")]
    InvalidMonsterState,
    #[msg("No such monster")]
    InvalidMonster,
    #[msg("Participation account is required on a player's turn")]
    ParticipationMissing,
    #[msg("Path is empty or too long")]
    InvalidPathLength,
    #[msg("Unknown step direction")]
    InvalidDirection,
    #[msg("Step leaves the map")]
    OutOfBounds,
    #[msg("Unknown tile in the layout")]
    InvalidTile,
    #[msg("Cell cannot be entered")]
    CellBlocked,
    #[msg("Cell is occupied")]
    CellOccupied,
    #[msg("Not enough action points")]
    NotEnoughActionPoints,
    #[msg("Arithmetic overflow")]
    Overflow,
}
