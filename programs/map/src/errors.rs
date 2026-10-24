use anchor_lang::prelude::*;
use deadroll_mapcheck::MapCheckError;

#[error_code]
pub enum MapError {
    #[msg("Only the map's DM can do this")]
    NotDm,
    #[msg("Map is not a draft")]
    NotDraft,
    #[msg("Chunk goes past the end of the tile grid")]
    ChunkOutOfRange,
    #[msg("Account is not the registry GameConfig")]
    InvalidConfig,
    #[msg("Caller is not the trusted game program's authority PDA")]
    UntrustedCaller,
    #[msg("Map status cannot change this way")]
    InvalidStatusTransition,
    // Builder rules (deadroll_mapcheck::MapCheckError), same order.
    #[msg("Map side is outside 8..=32")]
    SizeOutOfRange,
    #[msg("Map tier has no limits")]
    TierOutOfRange,
    #[msg("Byte is not a known tile")]
    InvalidTile,
    #[msg("Map needs exactly one entrance tile")]
    EntranceCount,
    #[msg("Map needs exactly one exit tile")]
    ExitCount,
    #[msg("Entrance point is not on the entrance tile")]
    EntranceMismatch,
    #[msg("Exit point is not on the exit tile")]
    ExitMismatch,
    #[msg("Too many spawns")]
    TooManySpawns,
    #[msg("Spawn is outside the map")]
    SpawnOutOfBounds,
    #[msg("Spawn is on an impassable tile")]
    SpawnNotPassable,
    #[msg("Spawn shares a cell with another spawn, the entrance or the exit")]
    SpawnOverlap,
    #[msg("Unknown bestiary id")]
    UnknownMonster,
    #[msg("Monster level is outside the map tier")]
    LevelOutOfTier,
    #[msg("Map needs exactly one boss")]
    BossCount,
    #[msg("Boss helper outside the boss pack")]
    HelperOutsideBossPack,
    #[msg("Regular monster in the boss pack")]
    BossPackMixed,
    #[msg("Too many boss helpers")]
    TooManyHelpers,
    #[msg("Too many monsters for the map tier")]
    TooManyMonsters,
    #[msg("Too many packs for the map tier")]
    TooManyPacks,
    #[msg("Two loners in one pack")]
    TwoLoners,
    #[msg("Pack monsters stand too far apart")]
    PackTooSpread,
    #[msg("No path from the entrance to the exit")]
    ExitUnreachable,
    #[msg("A spawn cannot be reached from the entrance")]
    SpawnUnreachable,
    // Patch 6, in MapCheckError order, then the program's own.
    #[msg("Map tier is below the monster's minimum tier")]
    TierBelowTemplate,
    #[msg("A chosen skill is not in the monster's pool")]
    MonsterSkillUnknown,
    #[msg("Choose as many skills of each kind as the monster has slots (all if fewer)")]
    MonsterSkillCount,
    #[msg("A boss cannot be elite")]
    EliteBoss,
    #[msg("Too many elite monsters for the map tier")]
    TooManyElites,
    #[msg("Too many different monster skills for one party")]
    TooManySessionSkills,
    #[msg("Account is not the registry bestiary or monster skills table")]
    InvalidRegistryTable,
    // Patch 11, in MapCheckError order.
    #[msg("Minimum players of the map is outside 1..=4")]
    MinPlayersOutOfRange,
    #[msg("Too many visible chests for the map tier")]
    TooManyChests,
    #[msg("A map point is not a visible chest")]
    InvalidPoint,
    #[msg("Chest is outside the map")]
    ChestOutOfBounds,
    #[msg("Chest stands on a cell that cannot be entered")]
    ChestNotPassable,
    #[msg("Chest shares a cell with the entrance, the exit, a spawn or another chest")]
    ChestOverlap,
    #[msg("Unknown reward table")]
    UnknownTable,
    #[msg("Reward table is not allowed on this map tier")]
    TableNotAllowed,
    #[msg("The boss needs a reward table")]
    BossTableMissing,
    #[msg("The boss reward table needs at least two levels")]
    BossTableOneLevel,
    #[msg("Chests and boss cost more than the reward budget")]
    OverBudget,
    #[msg("No cell next to a chest can be reached from the entrance")]
    ChestUnreachable,
    #[msg("The chest radius is outside the limits (LootTuning.chest_radius_min..=max)")]
    ChestRadiusOutOfRange,
    #[msg("The time to settle is outside the limits (at most LootTuning.settle_timeout_max_secs)")]
    SettleTimeoutOutOfRange,
}

impl From<MapCheckError> for MapError {
    fn from(e: MapCheckError) -> Self {
        use MapCheckError as C;
        match e {
            C::SizeOutOfRange => MapError::SizeOutOfRange,
            C::TierOutOfRange => MapError::TierOutOfRange,
            C::InvalidTile => MapError::InvalidTile,
            C::EntranceCount => MapError::EntranceCount,
            C::ExitCount => MapError::ExitCount,
            C::EntranceMismatch => MapError::EntranceMismatch,
            C::ExitMismatch => MapError::ExitMismatch,
            C::TooManySpawns => MapError::TooManySpawns,
            C::SpawnOutOfBounds => MapError::SpawnOutOfBounds,
            C::SpawnNotPassable => MapError::SpawnNotPassable,
            C::SpawnOverlap => MapError::SpawnOverlap,
            C::UnknownMonster => MapError::UnknownMonster,
            C::LevelOutOfTier => MapError::LevelOutOfTier,
            C::BossCount => MapError::BossCount,
            C::HelperOutsideBossPack => MapError::HelperOutsideBossPack,
            C::BossPackMixed => MapError::BossPackMixed,
            C::TooManyHelpers => MapError::TooManyHelpers,
            C::TooManyMonsters => MapError::TooManyMonsters,
            C::TooManyPacks => MapError::TooManyPacks,
            C::TwoLoners => MapError::TwoLoners,
            C::PackTooSpread => MapError::PackTooSpread,
            C::ExitUnreachable => MapError::ExitUnreachable,
            C::SpawnUnreachable => MapError::SpawnUnreachable,
            C::TierBelowTemplate => MapError::TierBelowTemplate,
            C::MonsterSkillUnknown => MapError::MonsterSkillUnknown,
            C::MonsterSkillCount => MapError::MonsterSkillCount,
            C::EliteBoss => MapError::EliteBoss,
            C::TooManyElites => MapError::TooManyElites,
            C::TooManySessionSkills => MapError::TooManySessionSkills,
            C::MinPlayersOutOfRange => MapError::MinPlayersOutOfRange,
            C::TooManyChests => MapError::TooManyChests,
            C::InvalidPoint => MapError::InvalidPoint,
            C::ChestOutOfBounds => MapError::ChestOutOfBounds,
            C::ChestNotPassable => MapError::ChestNotPassable,
            C::ChestOverlap => MapError::ChestOverlap,
            C::UnknownTable => MapError::UnknownTable,
            C::TableNotAllowed => MapError::TableNotAllowed,
            C::BossTableMissing => MapError::BossTableMissing,
            C::BossTableOneLevel => MapError::BossTableOneLevel,
            C::OverBudget => MapError::OverBudget,
            C::ChestUnreachable => MapError::ChestUnreachable,
        }
    }
}
