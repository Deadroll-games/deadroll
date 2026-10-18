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
    #[msg("Target is not a living enemy")]
    InvalidTarget,
    #[msg("Target is out of range")]
    OutOfRange,
    #[msg("Skill cannot be used as an action")]
    InvalidSkill,
    #[msg("Account is not the skill set of this character")]
    InvalidSkillSet,
    #[msg("Unknown monster action")]
    InvalidAction,
    #[msg("No dice roll is pending")]
    NoRollPending,
    #[msg("Callback accounts do not match the pending action")]
    InvalidPendingAccounts,
    #[msg("Party has not ended")]
    NotFinished,
    #[msg("Rewards of this character are already settled")]
    AlreadySettled,
    #[msg("Escrow is already claimed")]
    AlreadyClaimed,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Account is not the expected registry table")]
    InvalidRegistryTable,
    #[msg("Monster is not in the registry bestiary")]
    UnknownMonster,
    #[msg("Only one attack or active skill per turn")]
    AlreadyActed,
    #[msg("Skill is on cooldown")]
    SkillOnCooldown,
    #[msg("The turn has not started: call start_turn first")]
    TurnNotStarted,
    #[msg("This turn has already started")]
    TurnAlreadyStarted,
    #[msg("Target is not a living ally")]
    InvalidAllyTarget,
    #[msg("A heal or buff on another ally needs that ally's participation")]
    AllyAccountMissing,
    #[msg("target_participation is only for a heal or buff on another ally")]
    UnexpectedAllyAccount,
    #[msg("Only the character's owner or their session key can claim its rewards")]
    NotRewardOwner,
    /// No longer raised (the items `Loadout` is gone); the code stays reserved.
    #[msg("Not the character's items loadout")]
    InvalidLoadout,
    #[msg("This skill was not taken onto the map (set_ready selection)")]
    SkillNotSelected,
    #[msg("Already in that ready state")]
    ReadyUnchanged,
    #[msg("A death needs the character's SkillSet and Bag")]
    DeathAccountsMissing,
    #[msg(
        "The callback needs every other living player's Participation and Character, in slot order"
    )]
    InvalidPartyAccounts,
    #[msg("A wall cuts the line before the target")]
    LineBlocked,
    #[msg("A reactive skill triggers here: the VRF accounts and the payer are required")]
    ReactiveAccountsMissing,
}
