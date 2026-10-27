//! PDA seed prefixes (agents/CONTRACTS.md, section 2). Numbers in seeds are little-endian.

/// `["config"]` in registry.
pub const CONFIG: &[u8] = b"config";
/// `["aspects"]` in registry.
pub const ASPECTS: &[u8] = b"aspects";
/// `["resonances"]` in registry.
pub const RESONANCES: &[u8] = b"resonances";
/// `["bestiary"]` in registry.
pub const BESTIARY: &[u8] = b"bestiary";
/// `["tileset"]` in registry.
pub const TILESET: &[u8] = b"tileset";
/// `["prefabs"]` in registry.
pub const PREFABS: &[u8] = b"prefabs";
/// `["traits"]` in registry (patch 4).
pub const TRAITS: &[u8] = b"traits";
/// `["monster_skills"]` in registry (patch 6): the skills of the bestiary pools.
pub const MONSTER_SKILLS: &[u8] = b"monster_skills";
/// `["authority"]` in every program: the signer for CPI into other game programs.
pub const AUTHORITY: &[u8] = b"authority";
/// `["rasterizer", version: u16]` in render.
pub const RASTERIZER: &[u8] = b"rasterizer";
/// `["palette", version: u16]` in render.
pub const PALETTE: &[u8] = b"palette";
/// `["sprites", book_id: u16]` in render.
pub const SPRITES: &[u8] = b"sprites";
/// `["profile", owner]` in character.
pub const PROFILE: &[u8] = b"profile";
/// `["character", owner, index: u8]` in character.
pub const CHARACTER: &[u8] = b"character";
/// `["skills", character]` in character.
pub const SKILLS: &[u8] = b"skills";
/// `["bag", character]` in character.
pub const BAG: &[u8] = b"bag";
/// `["skill_extras", character]` in character (patch 5): range, timing, HP threshold, AP cost
/// and description of every skill, by `SkillSet` index.
pub const SKILL_EXTRAS: &[u8] = b"skill_extras";
/// `["item", asset]` in items.
pub const ITEM: &[u8] = b"item";
/// `["loadout", character]` in items. Not used since patch 1 (gear lives in the character).
pub const LOADOUT: &[u8] = b"loadout";
/// `["map", dm, map_index: u32]` in map.
pub const MAP: &[u8] = b"map";
/// `["layout", map]` in map.
pub const LAYOUT: &[u8] = b"layout";
/// `["session", map]` in session.
pub const SESSION: &[u8] = b"session";
/// `["participation", session, character]` in session.
pub const PARTICIPATION: &[u8] = b"participation";
/// `["monsters", session]` in session.
pub const MONSTERS: &[u8] = b"monsters";
/// `["escrow", session]` in session.
pub const ESCROW: &[u8] = b"escrow";
/// `["stash", session, character]` in session.
pub const STASH: &[u8] = b"stash";
/// `["city"]` in city.
pub const CITY: &[u8] = b"city";
/// `["rewards"]` in registry (patch 11): reward tables (docs/PARTY_AND_TURNS.md 9.2).
pub const REWARDS: &[u8] = b"rewards";
/// `["cache", character]` in session (patch 11): the temporary cache once the party is over,
/// until the player lays it out into the bag.
pub const CACHE: &[u8] = b"cache";
/// `["building", id: u16]` in city.
pub const BUILDING: &[u8] = b"building";
/// `["tavern_board", building]` in city: the order board of a Tavern (`Building.target`).
pub const TAVERN_BOARD: &[u8] = b"tavern_board";
/// `["quest", session]` in city: one order on a Tavern board, for one party.
pub const QUEST: &[u8] = b"quest";
