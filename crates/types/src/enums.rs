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
    /// One map cell. `MapLayout.tiles` stores these as raw `u8`.
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
