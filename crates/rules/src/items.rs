//! Equipment rules (docs/ITEMS_AND_CRAFT.md): what a worn item adds to a hero, where an item may
//! be worn, and the class starter kit. Item numbers live in [`ItemTuning`]; the items program,
//! the session program and the simulator all read them through this module.
use deadroll_combat::{Carrier, DamageRange, Tint};
use deadroll_types::{
    Aspect, Class, EquipSlot, Form, ItemBase, RangeTag, StateKind, EMPTY_ITEM, EQUIP_SLOTS,
    SOULBOUND_MAX, STATS_COUNT,
};

use crate::config::{ItemBaseStats, ItemTuning};

/// Items in one class starter kit.
pub const KIT_MAX: usize = SOULBOUND_MAX;

/// What a hero wears, as `ItemBase` bytes by `EquipSlot` (`EMPTY_ITEM` = nothing). The same bytes
/// as the session's `Participation.equipment` snapshot, so the program and the simulator feed the
/// rules the same value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Equipment(pub [u8; EQUIP_SLOTS]);

impl Equipment {
    pub const EMPTY: Equipment = Equipment([EMPTY_ITEM; EQUIP_SLOTS]);

    /// The base worn in `slot`; an unknown byte reads as nothing.
    pub fn get(&self, slot: EquipSlot) -> Option<ItemBase> {
        ItemBase::try_from(self.0[slot as usize]).ok()
    }

    pub fn set(&mut self, slot: EquipSlot, base: Option<ItemBase>) {
        self.0[slot as usize] = base.map_or(EMPTY_ITEM, |b| b as u8);
    }

    /// Every worn base with its slot.
    pub fn worn(&self) -> impl Iterator<Item = (EquipSlot, ItemBase)> + '_ {
        EquipSlot::ALL
            .iter()
            .filter_map(|&slot| self.get(slot).map(|base| (slot, base)))
    }

    pub fn is_empty(&self) -> bool {
        self.worn().next().is_none()
    }
}

impl Default for Equipment {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Why an item cannot go into a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipError {
    /// The base is not worn in that slot.
    WrongSlot,
    /// Character level below item level minus the allowed gap (ITEMS_AND_CRAFT 4.2).
    LevelTooLow,
}

/// Whether a base made for `home` may be worn in `slot`: its own slot, and either ring slot for
/// a ring.
pub fn slot_fits(home: EquipSlot, slot: EquipSlot) -> bool {
    let ring = |s: EquipSlot| matches!(s, EquipSlot::Ring1 | EquipSlot::Ring2);
    home == slot || (ring(home) && ring(slot))
}

/// Lowest character level that may wear an item of `item_level`.
pub fn level_required(item_level: u8, t: &ItemTuning) -> u16 {
    u16::from(item_level.saturating_sub(t.level_gap))
}

/// Can a character of `level` wear `base` of `item_level` in `slot`.
pub fn check_equip(
    level: u16,
    base: ItemBase,
    item_level: u8,
    slot: EquipSlot,
    t: &ItemTuning,
) -> Result<(), EquipError> {
    if !slot_fits(t.base(base).slot, slot) {
        return Err(EquipError::WrongSlot);
    }
    if level < level_required(item_level, t) {
        return Err(EquipError::LevelTooLow);
    }
    Ok(())
}

/// The class starter kit (ITEMS_AND_CRAFT 10) in record order.
pub fn starter_kit(class: Class, t: &ItemTuning) -> impl Iterator<Item = ItemBase> + '_ {
    t.starter_kits[class as usize].iter().flatten().copied()
}

/// A new loadout: the kit as soulbound records, each worn in its own slot. When two kit items
/// share a slot the first is worn and the other stays unworn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarterLoadout {
    /// `(base, item level)`, records `0..count`.
    pub items: [(ItemBase, u8); KIT_MAX],
    pub count: u8,
    /// Record index worn in each slot.
    pub worn: [Option<u8>; EQUIP_SLOTS],
}

impl StarterLoadout {
    pub fn equipment(&self) -> Equipment {
        let mut eq = Equipment::EMPTY;
        for (slot, worn) in EquipSlot::ALL.iter().zip(&self.worn) {
            if let Some(i) = worn {
                eq.set(*slot, Some(self.items[*i as usize].0));
            }
        }
        eq
    }
}

pub fn starter_loadout(class: Class, t: &ItemTuning) -> StarterLoadout {
    let mut out = StarterLoadout {
        items: [(ItemBase::default(), 0); KIT_MAX],
        count: 0,
        worn: [None; EQUIP_SLOTS],
    };
    for (i, base) in starter_kit(class, t).take(KIT_MAX).enumerate() {
        let stats = t.base(base);
        out.items[i] = (base, stats.level);
        out.count = i as u8 + 1;
        let slot = &mut out.worn[stats.slot as usize];
        if slot.is_none() {
            *slot = Some(i as u8);
        }
    }
    out
}

/// Everything worn items add to a hero. Flat values only, no negatives (COMBAT_MATH 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemBonus {
    /// The main-hand base when it is a weapon: its damage replaces the unarmed base.
    pub weapon: Option<ItemBase>,
    pub armor: u32,
    pub defense: u32,
    /// Added to the `d20` attack bonus.
    pub attack: u32,
    /// Added to the core stats, indexed by `Stat`.
    pub stats: [u16; STATS_COUNT],
    pub hp: u32,
    /// Crit chance besides a natural 20, percent.
    pub crit_pct: u32,
    /// Speed added before counterweights, and the summed speed counterweight (bp).
    pub speed: u16,
    pub speed_counter_bp: u32,
    pub initiative: u16,
    /// Patch 9. Resistance per aspect and state resistance, bp (summed counterweights).
    pub resist_bp: [u32; Aspect::COUNT],
    pub state_resist_bp: u32,
    /// Patch 9b. Resistance to each state on top of `state_resist_bp`, by `StateKind`.
    pub state_kind_resist_bp: [u32; StateKind::COUNT],
}

pub fn item_bonus(eq: &Equipment, t: &ItemTuning) -> ItemBonus {
    let mut out = ItemBonus {
        weapon: eq
            .get(EquipSlot::MainHand)
            .filter(|b| is_weapon(t.base(*b))),
        ..ItemBonus::default()
    };
    for (_, base) in eq.worn() {
        let b = t.base(base);
        out.armor = out.armor.saturating_add(b.armor);
        out.defense = out.defense.saturating_add(b.defense);
        out.attack = out.attack.saturating_add(b.attack);
        out.hp = out.hp.saturating_add(b.hp);
        out.crit_pct = out.crit_pct.saturating_add(b.crit_pct);
        out.speed = out.speed.saturating_add(b.speed);
        out.speed_counter_bp = out.speed_counter_bp.saturating_add(b.speed_counter_bp);
        out.initiative = out.initiative.saturating_add(b.initiative);
        out.state_resist_bp = out.state_resist_bp.saturating_add(b.state_resist_bp);
        for (sum, add) in out.stats.iter_mut().zip(b.stats) {
            *sum = sum.saturating_add(add);
        }
        for (sum, add) in out.resist_bp.iter_mut().zip(b.resist_bp) {
            *sum = sum.saturating_add(add);
        }
        for (sum, add) in out
            .state_kind_resist_bp
            .iter_mut()
            .zip(b.state_kind_resist_bp)
        {
            *sum = sum.saturating_add(add);
        }
    }
    out
}

/// A base with a damage range is a weapon.
pub fn is_weapon(b: &ItemBaseStats) -> bool {
    b.damage[1] > 0
}

/// The damage base of every attack and skill: the main-hand weapon, or bare hands
/// (SKILL_BUILDER 5.1; the weapon replaces the class base).
pub fn weapon_range(eq: &Equipment, t: &ItemTuning) -> DamageRange {
    match item_bonus(eq, t).weapon {
        Some(base) => {
            let b = t.base(base);
            DamageRange::new(b.aspect, b.damage[0], b.damage[1].max(b.damage[0]))
        }
        None => DamageRange::new(Aspect::Blunt, t.unarmed[0], t.unarmed[1].max(t.unarmed[0])),
    }
}

/// Range and form of the basic attack: the main-hand weapon's, or an unarmed punch (a melee
/// strike, docs/SKILL_BUILDER.md 3.1).
pub fn basic_attack(eq: &Equipment, t: &ItemTuning) -> (RangeTag, Form) {
    match item_bonus(eq, t).weapon {
        Some(base) => {
            let b = t.base(base);
            (b.basic_range, b.basic_form)
        }
        None => (RangeTag::Melee, Form::Strike),
    }
}

/// Item tint slots of an attacker (the race and class take the others).
pub const ITEM_TINTS: usize = 2;

/// Worn items that carry an aspect, as resonance tints (DAMAGE_TYPES_AND_MIXES.md 4 and 6.1,
/// SKILL_BUILDER.md 5.2): the main hand first, then the other slots in `EquipSlot` order.
/// `basic`: the attack is the weapon's basic attack, so the main-hand aspect is the base
/// itself and tints nothing.
pub fn item_tints(eq: &Equipment, t: &ItemTuning, basic: bool) -> [Tint; ITEM_TINTS] {
    let mut out = [Tint::default(); ITEM_TINTS];
    let carriers = eq
        .worn()
        .filter(|(slot, _)| !(basic && *slot == EquipSlot::MainHand))
        .map(|(_, base)| t.base(base))
        .filter(|b| b.aspect_tier > 0);
    for (slot, b) in out.iter_mut().zip(carriers) {
        *slot = Tint::new(b.aspect, b.aspect_tier, Carrier::Item);
    }
    out
}

/// Core stats with the worn items' stat bonuses.
pub fn effective_stats(
    stats: &[u16; STATS_COUNT],
    eq: &Equipment,
    t: &ItemTuning,
) -> [u16; STATS_COUNT] {
    let bonus = item_bonus(eq, t).stats;
    let mut out = *stats;
    for (s, add) in out.iter_mut().zip(bonus) {
        *s = s.saturating_add(add);
    }
    out
}
