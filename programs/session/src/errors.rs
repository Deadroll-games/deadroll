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
    // Patch 6: monster skills (docs/BESTIARY.md 3).
    #[msg("Too many different monster skills for one party")]
    TooManyMonsterSkills,
    #[msg("This monster has no active skill in that slot")]
    MonsterSkillMissing,
    #[msg("A kin-only monster skill reaches allies of its own kin only")]
    NotSameKin,
    #[msg("Account is not the registry monster skills table, or a pool skill is missing")]
    InvalidMonsterSkills,
    #[msg("A death is waiting to be finalized: start_turn (or a claim) first")]
    DeathsPending,
    #[msg("The remaining accounts are not the character, skill set and bag of each pending death, in slot order")]
    InvalidDeathAccounts,
    // Patch 10: session keys through the `session-keys` macros (docs/GAME_COMMISSIONS.md 4-5).
    /// Named by `#[session_auth_or]`: the token is not this program's for this signer and
    /// wallet, or it expired.
    #[msg("Session token is not valid for this signer and wallet, or it has expired")]
    InvalidToken,
    #[msg("The party is over: its session key can no longer be set")]
    PartyEnded,
    #[msg("Joining binds no session key: the key and its prepayment come with Ready")]
    SessionKeyAtReady,
    // Patch 11: rewards (docs/PARTY_AND_TURNS.md 8-9, MAP_DESCRIPTION.md 7.1).
    #[msg("Too many different reward tables for one party")]
    TooManyLootTables,
    #[msg("Account is not the registry rewards table, or a table the map names is missing")]
    InvalidRewardTables,
    #[msg("Fewer players than the map's minimum")]
    NotEnoughPlayers,
    #[msg("No such chest on this map")]
    InvalidChest,
    #[msg("This chest is already open")]
    ChestOpened,
    #[msg("Stand next to the chest to open it")]
    ChestNotAdjacent,
    #[msg("A living monster is too close to the chest or to you")]
    MonsterNearChest,
    #[msg("A closed chest occupies this cell")]
    ChestInTheWay,
    #[msg("The DM's time to settle the party is over: each player claims their own")]
    SettleDeadlinePassed,
    #[msg("The DM can still settle the party: wait for the deadline")]
    SettleDeadlineNotPassed,
    #[msg("The remaining accounts are not the participation, reward cache and character of each player, in slot order")]
    InvalidSettleAccounts,
    #[msg("The DM settles with the main wallet, not a session key")]
    DmMainWalletOnly,
    #[msg("This cache is not settled yet")]
    CacheNotClaimed,
    #[msg("This cache belongs to another character or party")]
    InvalidRewardCache,
    #[msg("Only the character's owner, with the main wallet, closes its cache")]
    NotCacheOwner,
    #[msg("The map gives the DM less than the minimum time to settle")]
    SettleTimeoutTooShort,
    #[msg(
        "Too many parties wait for this character's rewards: claim one, or wait for its deadline"
    )]
    TooManyPendingParties,
    #[msg("The remaining accounts are not the GameSession of each open reward cache entry, in entry order")]
    InvalidCacheSessions,
    #[msg("The character is not locked in this party")]
    NotLockedHere,
}
