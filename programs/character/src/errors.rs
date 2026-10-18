use anchor_lang::prelude::*;

#[error_code]
pub enum CharacterError {
    #[msg("Caller is not authorized for this instruction")]
    Unauthorized,
    #[msg("Name must be printable ASCII, not empty, zero-padded")]
    InvalidName,
    #[msg("Predisposition not allowed for this race")]
    PredispositionNotAllowed,
    #[msg("Stats must start at the base and spend exactly the starting points")]
    InvalidStats,
    #[msg("Too many starting skills")]
    TooManySkills,
    #[msg("Skill rejected by the builder rules")]
    InvalidSkill,
    #[msg("Profile has no free character index")]
    TooManyCharacters,
    #[msg("Not the registry GameConfig account")]
    InvalidConfig,
    #[msg("Caller is not the trusted game program")]
    UntrustedCaller,
    #[msg("Signer does not own this character")]
    NotOwner,
    #[msg("Character is dead")]
    CharacterDead,
    #[msg("Character is already locked in a session")]
    AlreadyLocked,
    #[msg("Character is not locked in a session")]
    NotLocked,
    #[msg("Character is still alive")]
    StillAlive,
    #[msg("Character has no NFT")]
    NoNft,
    #[msg("Asset is not this character's NFT")]
    AssetMismatch,
    #[msg("Tombstone NFT is already frozen")]
    AlreadyFrozen,
    #[msg("NFT accounts missing: asset and mpl-core program are required")]
    NftAccountsMissing,
    #[msg("This build has no NFT support (FALLBACK nft)")]
    NftDisabled,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Map skill selection rejected: 4 active, 1 reactive, 1 passive of the character's own")]
    InvalidSkillSelection,
    #[msg("No such soulbound item")]
    NoSuchItem,
    #[msg("This item is not worn in that slot")]
    WrongSlot,
    #[msg("Character level too low for this item")]
    LevelTooLow,
    #[msg("Nothing is worn in that slot")]
    SlotEmpty,
    #[msg("A session key needs its session token account")]
    SessionTokenMissing,
    #[msg("Session token does not match the signer, the owner or the game")]
    InvalidSessionToken,
    #[msg("Session token expired")]
    SessionTokenExpired,
}
