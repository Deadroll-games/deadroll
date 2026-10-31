//! Shared enums (agents/CONTRACTS.md, section 3). Discriminants are part of the on-chain
//! format: never reorder, only append.

use crate::InvalidValue;

u8_enum!(
    /// Damage aspect (docs/DAMAGE_TYPES_AND_MIXES.md).
    Aspect { Fire, Cold, Lightning, Blood, Blunt, Piercing }
);
u8_enum!(Family {
    Elemental,
    Physical,
    Mental,
    Sacral
});
u8_enum!(Race { Human, ForestElf });
u8_enum!(Class {
    Warrior,
    Mage,
    Support
});
u8_enum!(Source { Magical, Combat });
u8_enum!(Form { Strike, Projectile });
u8_enum!(TimeTag { Instant, DoT });
u8_enum!(
    /// Range axis of a skill or a basic attack (docs/DAMAGE_TYPES_AND_MIXES.md, section 2).
    /// Added in patch 5.
    RangeTag { Melee, Ranged }
);
u8_enum!(
    /// Burning, Chill, Freeze, Bleeding, Shock, Stun, then Penetration (patch 3, append only).
    StateKind { Burning, Chill, Freeze, Bleeding, Shock, Stun, Penetration }
);
u8_enum!(
    /// One map cell of `MapLayoutData.tiles` (before patch 13). No longer written: a cell byte is
    /// a `TileKind` and a variant now (`cell_byte`, `MapLayoutDataV2.cells`).
    Tile { Floor, Wall, Water, Door, Entrance, Exit, Trap }
);
u8_enum!(SessionStatus {
    Lobby,
    Active,
    Victory,
    Wiped,
    Fled,
    Abandoned
});
u8_enum!(MapStatus {
    Draft,
    Finalized,
    InSession,
    Finished
});
u8_enum!(
    /// Index into `Character.stats` (docs/CHARACTER.md, section 2).
    Stat { Strength, Dexterity, Intelligence, Endurance }
);
u8_enum!(
    /// Index into `GameConfig.programs`, same order as agents/CONTRACTS.md, section 1.
    ProgramIdx { Registry, Render, Character, Items, Map, Session, City }
);

u8_enum!(
    /// Equipment slot (docs/ART_DESCRIPTION.md, section 6.3); rings take two slots.
    EquipSlot { MainHand, OffHand, Helmet, Body, Pants, Boots, Gloves, Necklace, Cloak, Ring1, Ring2 }
);
u8_enum!(
    /// Item base: the type of an item, its slot and base numbers (`deadroll_rules::ItemTuning`).
    /// P0 has the class starter bases (docs/ITEMS_AND_CRAFT.md, section 10); recipes append more.
    ItemBase { Sword, Staff, Wand, LeatherArmor, Robe, LightLeatherArmor }
);
u8_enum!(
    /// What an equipment slot holds: nothing, a soulbound record of the character, or an item NFT
    /// (crafting, later).
    EquipSource { Empty, Soulbound, Asset }
);

/// Predisposition tier, `1..=14`, 1 is best. Accounts and instruction args carry a raw `u8`;
/// `Tier::new` validates it. No borsh on purpose: deserializing must not skip the check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tier(u8);

impl Tier {
    pub const BEST: Tier = Tier(1);
    pub const WORST: Tier = Tier(14);

    pub const fn new(value: u8) -> Result<Tier, InvalidValue> {
        if value >= Self::BEST.0 && value <= Self::WORST.0 {
            Ok(Tier(value))
        } else {
            Err(InvalidValue(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Tier {
    type Error = InvalidValue;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Tier::new(value)
    }
}

impl From<Tier> for u8 {
    fn from(value: Tier) -> u8 {
        value.0
    }
}

// Patch 11: loot (docs/PARTY_AND_TURNS.md 8-9, docs/MAP_DESCRIPTION.md 7.1).
u8_enum!(
    /// A map point type (`MapPoint.kind`, docs/MAP_DESCRIPTION.md 7.1). `Entrance` and `Exit`
    /// appended in patch 13 (`MapLayoutDataV2.points`).
    PointKind { Chest, Entrance, Exit }
);
u8_enum!(
    /// What a reward is. All of them are counters by id in the bag (docs/ITEMS_AND_CRAFT.md 2);
    /// `Consumable` goes to the consumable slots, the rest to the resource slots.
    RewardKind { Resource, Recipe, Consumable, Blank, Reagent }
);
u8_enum!(
    /// Where a reward event comes from (docs/PARTY_AND_TURNS.md 9.3, docs/BESTIARY.md 9).
    /// `Stash` (hidden stashes) is reserved: secrets are not in the game yet.
    RewardSource { Body, Chest, Boss, Stash }
);
u8_enum!(
    /// One party's entry in the session `RewardCache`: `Empty` (free), `Open` while the party runs
    /// or waits to be settled (the rewards are in its `Participation`), `Claimed` once they are
    /// the player's (issued, or claimed after the timeout) and wait for "Done" in the bag screen.
    CacheStatus { Empty, Open, Claimed }
);

// Patch 13: tiles (docs/MAP_DESCRIPTION.md 4.1, 4.2).
u8_enum!(
    /// A tile type, bits 7-3 of a cell byte (`cell_kind`). Its properties (passable, step cost,
    /// blocks vision) are the registry `Tileset` record with the same id. A cell byte names up
    /// to `TILE_KINDS_MAX` types; the ones past `COUNT` are reserved.
    TileKind {
        Void,
        StoneFloor,
        WoodenFloor,
        Dirt,
        Wall,
        Door,
        ShallowWater,
        DeepWater,
        Stairs,
        Rubble,
        Chasm
    }
);

u8_enum!(
    /// The art set a map is drawn with, chosen by the DM at `create_map` (`Map.biome`). Picture
    /// only: it does not change any rule. A map from before the field is `Caves`.
    Biome { Caves, Forest, Crypt }
);

/// Patch 13: the tile type id of a cell byte (bits 7-3). A raw id, not a `TileKind`: whether
/// it exists is the `Tileset`'s call.
pub const fn cell_kind(byte: u8) -> u8 {
    byte >> 3
}

/// Patch 13: the variant of a cell byte (bits 2-0), chosen by the DM (docs/MAP_DESCRIPTION.md 4.3).
pub const fn cell_variant(byte: u8) -> u8 {
    byte & 0b111
}

/// Patch 13: a cell byte from a tile type id (`< TILE_KINDS_MAX`) and a variant (`< TILE_VARIANTS`).
pub const fn cell_byte(kind: u8, variant: u8) -> u8 {
    (kind << 3) | (variant & 0b111)
}

// City (docs/CITY_DESCRIPTION.md 3, 4).
u8_enum!(
    /// What a city building is (`Building.kind`). It picks the site's interface module
    /// (docs/CITY_DESCRIPTION.md 4).
    BuildingKind { Tavern, MastersGuild, Storage }
);
u8_enum!(
    /// Whether a building can be entered (`Building.status`). A `Closed` one is drawn with a
    /// boarded-up door (docs/CITY_DESCRIPTION.md 8).
    BuildingStatus { Active, Closed }
);
u8_enum!(
    /// The access condition of a building (`Building.requirement`, docs/CITY_DESCRIPTION.md 5.1).
    /// Only `None` for now; the other conditions are appended with their checks.
    Requirement { None }
);

// Arena update (docs/MAP_DESCRIPTION.md 17.2, PARTY_AND_TURNS.md 4.1.3).
u8_enum!(
    /// The direction a pack looks (`PackLayout.facing`), set by the DM. North is `y - 1`, east
    /// `x + 1`, clockwise. Coming from its back the party moves first in the arena.
    Facing { N, NE, E, SE, S, SW, W, NW }
);

// Patch C (docs/MAP_DESCRIPTION.md 8.1, 17.4).
u8_enum!(
    /// What a cell hides (`CellSecret.kind`). `Ambush` is the trap that starts a hidden pack's
    /// fight; `Door` turns a wall into a door once found.
    SecretKind { None, Trap, Ambush, Stash, Door }
);

impl SecretKind {
    /// Index into the declared counts (`None` has none).
    pub const fn counted(self) -> Option<usize> {
        match self {
            SecretKind::None => None,
            SecretKind::Trap => Some(0),
            SecretKind::Ambush => Some(1),
            SecretKind::Stash => Some(2),
            SecretKind::Door => Some(3),
        }
    }
}
