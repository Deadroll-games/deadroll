//! Rewards (docs/PARTY_AND_TURNS.md 8-9, docs/BESTIARY.md 9, docs/ITEMS_AND_CRAFT.md 6.1).
//! Patch 11.
//!
//! - Reward tables are registry data (`Rewards`, one [`record`] per table id): a level range, a
//!   cost in budget points and, per level, the entries a reward is picked from.
//! - A reward event (a chest, the boss, a killed monster's body, later a hidden stash) rolls its
//!   level once for the whole party, then one reward for every living participant
//!   ([`reward_event`]). Every die comes from a [`RollSource`].
//! - Experience of a kill goes to every living participant in full ([`on_monster_killed`]).
//! - The temporary cache merges rewards by `(kind, id)` ([`cache_add`]); "Done" in the bag screen
//!   is accepted only if nothing appears from nowhere ([`check_bag_layout`]).

use deadroll_combat::RollSource;
use deadroll_mapcheck::{
    limits::tier_limits,
    loot::{LootRules, TableRules},
    roster::Role,
};
use deadroll_types::{BagLayout, BagSlot, CacheSlot, RewardKind, RewardSource, BP, PARTY_MAX};

use crate::config::{LootTuning, OVER_TIER_STEPS};

/// Registry `Rewards` record bytes (programs/registry README).
pub mod record {
    pub const SIZE: usize = 108;
    /// 1 = a table, 0 = an empty id (the whole record is zero).
    pub const PRESENT: usize = 0;
    pub const LEVEL_BASE: usize = 1;
    pub const LEVEL_MAX: usize = 2;
    /// `[3]` reserved (zero).
    pub const COST: usize = 4;
    /// `[6..8]` reserved (zero).
    pub const LEVELS_AT: usize = 8;
    /// Levels one table spans at most.
    pub const LEVELS: usize = 4;
    /// Entries of one level at most.
    pub const ENTRIES: usize = 6;
    /// `[count u8, ENTRIES x (kind u8, id u16 LE, qty u8)]`.
    pub const LEVEL_SIZE: usize = 1 + ENTRIES * ENTRY_SIZE;
    pub const ENTRY_SIZE: usize = 4;
    /// Tables in the registry account (`Table.body` holds 4096 bytes).
    pub const RECORDS_MAX: usize = 32;

    const _: () = assert!(LEVELS_AT + LEVELS * LEVEL_SIZE == SIZE);
    const _: () = assert!(SIZE * RECORDS_MAX <= 4096);
}

use record::{ENTRIES, LEVELS};

/// One reward a level can give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LootEntry {
    pub kind: RewardKind,
    pub id: u16,
    /// Units given, at least 1.
    pub qty: u8,
}

/// The rewards of one level: `entries[..count]`, each equally likely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LootLevel {
    pub count: u8,
    pub entries: [LootEntry; ENTRIES],
}

impl LootLevel {
    pub fn used(&self) -> &[LootEntry] {
        &self.entries[..(self.count as usize).min(ENTRIES)]
    }
}

/// A reward table (docs/PARTY_AND_TURNS.md 9.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LootTable {
    pub level_base: u8,
    pub level_max: u8,
    /// Budget points a DM spends to put it on a map.
    pub cost: u16,
    /// Level `level_base + i` at `levels[i]`.
    pub levels: [LootLevel; LEVELS],
}

impl LootTable {
    /// Decodes a present, well-formed record: levels `1 ..` with at most [`LEVELS`] of them, 1 to
    /// [`ENTRIES`] entries on each, every entry a known kind with `qty >= 1`, unused bytes zero.
    pub fn from_record(r: &[u8]) -> Option<LootTable> {
        use record::*;
        if r.len() != SIZE || r[PRESENT] != 1 || r[3] != 0 || r[6] != 0 || r[7] != 0 {
            return None;
        }
        let (level_base, level_max) = (r[LEVEL_BASE], r[LEVEL_MAX]);
        if level_base == 0 || level_max < level_base {
            return None;
        }
        let span = (level_max - level_base) as usize + 1;
        if span > LEVELS {
            return None;
        }
        let mut t = LootTable {
            level_base,
            level_max,
            cost: u16::from_le_bytes([r[COST], r[COST + 1]]),
            levels: [LootLevel::default(); LEVELS],
        };
        for (i, level) in t.levels.iter_mut().enumerate() {
            let at = LEVELS_AT + i * LEVEL_SIZE;
            let bytes = &r[at..at + LEVEL_SIZE];
            let count = bytes[0] as usize;
            let used = i < span;
            if (used && !(1..=ENTRIES).contains(&count)) || (!used && count != 0) {
                return None;
            }
            level.count = count as u8;
            for (j, entry) in level.entries.iter_mut().enumerate() {
                let e = &bytes[1 + j * ENTRY_SIZE..1 + (j + 1) * ENTRY_SIZE];
                if j >= count {
                    if e.iter().any(|b| *b != 0) {
                        return None;
                    }
                    continue;
                }
                *entry = LootEntry {
                    kind: RewardKind::try_from(e[0]).ok()?,
                    id: u16::from_le_bytes([e[1], e[2]]),
                    qty: e[3],
                };
                if entry.qty == 0 {
                    return None;
                }
            }
        }
        Some(t)
    }

    /// The record bytes (tools, tests; `from_record` reads them back).
    pub fn to_record(&self) -> [u8; record::SIZE] {
        use record::*;
        let mut r = [0u8; SIZE];
        r[PRESENT] = 1;
        r[LEVEL_BASE] = self.level_base;
        r[LEVEL_MAX] = self.level_max;
        r[COST..COST + 2].copy_from_slice(&self.cost.to_le_bytes());
        for (i, level) in self.levels.iter().enumerate() {
            let at = LEVELS_AT + i * LEVEL_SIZE;
            r[at] = level.count;
            for (j, e) in level.used().iter().enumerate() {
                let p = at + 1 + j * ENTRY_SIZE;
                r[p] = e.kind as u8;
                r[p + 1..p + 3].copy_from_slice(&e.id.to_le_bytes());
                r[p + 3] = e.qty;
            }
        }
        r
    }

    /// What the map builder checks (`deadroll_mapcheck::check_map`).
    pub const fn rules(&self) -> TableRules {
        TableRules {
            level_base: self.level_base,
            level_max: self.level_max,
            cost: self.cost,
        }
    }

    /// `level` moved into the table's range.
    pub fn clamp_level(&self, level: u8) -> u8 {
        level.clamp(self.level_base, self.level_max)
    }

    /// The rewards of `level`, if the table has it.
    pub fn level(&self, level: u8) -> Option<&LootLevel> {
        if !(self.level_base..=self.level_max).contains(&level) {
            return None;
        }
        self.levels.get((level - self.level_base) as usize)
    }
}

/// A registry record is either all zero (no table with this id) or a valid table.
pub fn validate_record(r: &[u8]) -> bool {
    r.len() == record::SIZE && (r.iter().all(|b| *b == 0) || LootTable::from_record(r).is_some())
}

/// The level of a reward event: the base level, one step higher with `level_step_bp`, and so on
/// up to the table's top (docs/PARTY_AND_TURNS.md 9.3: higher levels less often).
pub fn roll_level(t: &LootTable, step_bp: u16, src: &mut impl RollSource) -> u8 {
    let mut level = t.level_base;
    while level < t.level_max && src.next_u64() % (BP as u64) < u64::from(step_bp) {
        level += 1;
    }
    level
}

/// One reward of `level`: an entry picked with equal chance. `None` if the level is not in the
/// table (no die is rolled then).
pub fn roll_reward(t: &LootTable, level: u8, src: &mut impl RollSource) -> Option<CacheSlot> {
    let entries = t.level(level)?.used();
    if entries.is_empty() {
        return None;
    }
    let e = entries[(src.next_u64() % entries.len() as u64) as usize];
    Some(CacheSlot {
        kind: e.kind,
        id: e.id,
        qty: e.qty as u16,
    })
}

/// What one reward event gave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewardRoll {
    pub level: u8,
    /// By player slot: the reward, and the boss guarantee's extra one (docs/PARTY_AND_TURNS.md
    /// 9.4). Empty slots for players who get nothing.
    pub rewards: [[CacheSlot; 2]; PARTY_MAX],
}

/// One reward event (docs/PARTY_AND_TURNS.md 9.3). The level is rolled once for the party, except
/// for a body, whose level is the monster's (`body_level`, docs/BESTIARY.md 9.1). Then one reward
/// for every slot in `eligible` (bit per player slot: alive, and not too far above the tier), in
/// slot order. The boss adds one reward a level higher when its base level dropped.
pub fn reward_event(
    source: RewardSource,
    table: &LootTable,
    body_level: u8,
    eligible: u8,
    step_bp: u16,
    src: &mut impl RollSource,
) -> RewardRoll {
    let level = match source {
        RewardSource::Body => table.clamp_level(body_level),
        RewardSource::Chest | RewardSource::Boss | RewardSource::Stash => {
            roll_level(table, step_bp, src)
        }
    };
    let guarantee = source == RewardSource::Boss && level == table.level_base;
    let mut out = RewardRoll {
        level,
        ..RewardRoll::default()
    };
    for (slot, rewards) in out.rewards.iter_mut().enumerate() {
        if eligible & (1 << slot) == 0 {
            continue;
        }
        if let Some(r) = roll_reward(table, level, src) {
            rewards[0] = r;
        }
        if guarantee {
            if let Some(r) = roll_reward(table, level + 1, src) {
                rewards[1] = r;
            }
        }
    }
    out
}

/// Whether a killed monster of this role leaves body resources: summoned helpers do not
/// (docs/BESTIARY.md 9.1).
pub fn drops_body_loot(role: Role) -> bool {
    role != Role::Helper
}

/// The experience of one kill, by player slot: every living player (`alive`, bit per slot) gets
/// the monster's full experience times their share (`xp_bp`, [`reward_share`]); it is not split.
/// The one place to change when experience becomes "per pack" (docs/PARTY_AND_TURNS.md 4).
pub fn on_monster_killed(monster_xp: u32, alive: u8, xp_bp: &[u16; PARTY_MAX]) -> [u32; PARTY_MAX] {
    let mut out = [0; PARTY_MAX];
    for (slot, xp) in out.iter_mut().enumerate() {
        if alive & (1 << slot) != 0 {
            *xp = (monster_xp as u64 * xp_bp[slot] as u64 / BP as u64) as u32;
        }
    }
    out
}

/// A character's share of a party's rewards (docs/PARTY_AND_TURNS.md 9.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewardShare {
    /// Experience, basis points.
    pub xp_bp: u16,
    /// Whether rewards are rolled for it.
    pub rolls_loot: bool,
}

/// A character noticeably above the map tier gets less experience or none, and no rewards.
pub fn reward_share(char_level: u16, map_tier: u8, t: &LootTuning) -> RewardShare {
    let top = tier_limits(map_tier).map_or(u8::MAX, |l| l.level_max) as u16;
    let over = char_level.saturating_sub(top);
    let xp_bp = match over {
        0 => BP as u16,
        n => t.over_tier_xp_bp[(n as usize).min(OVER_TIER_STEPS) - 1],
    };
    RewardShare {
        xp_bp,
        rolls_loot: over < t.over_tier_no_loot as u16,
    }
}

/// Adds a reward to a cache: onto the slot of the same `(kind, id)`, else into the first empty
/// slot. `false` if the cache is full (the reward is lost; the event still shows it).
pub fn cache_add(slots: &mut [CacheSlot], add: CacheSlot) -> bool {
    if add.is_empty() {
        return true;
    }
    if let Some(s) = slots
        .iter_mut()
        .find(|s| !s.is_empty() && s.kind == add.kind && s.id == add.id)
    {
        s.qty = s.qty.saturating_add(add.qty);
        return true;
    }
    match slots.iter_mut().find(|s| s.is_empty()) {
        Some(s) => {
            *s = add;
            true
        }
        None => false,
    }
}

/// The chest and budget limits of a map tier for `deadroll_mapcheck::check_map` (default, i.e.
/// nothing allowed, for a tier without numbers: the check refuses the tier first).
pub fn map_rules(t: &LootTuning, tier: u8) -> LootRules {
    let Some(i) = (tier as usize)
        .checked_sub(1)
        .filter(|i| *i < t.chests.len())
    else {
        return LootRules::default();
    };
    LootRules {
        chests: t.chests[i],
        budget: t.budget[i],
        allowed: t.allowed_tables[i],
    }
}

/// Why a bag layout is refused. Codes are stable: append only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BagError {
    /// An empty slot (`qty == 0`) with a kind or id.
    DirtyEmptySlot,
    /// A consumable in a resource slot, or something else in a consumable slot.
    WrongSection,
    /// More units in one slot than a stack holds.
    StackTooBig,
    /// More of a `(kind, id)` than the bag and the cache held together.
    MoreThanOwned,
}

impl BagError {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// "Done" in the bag screen (decisions of patch 11): the new bag must come from the old bag and
/// the cache. For every `(kind, id)`: `bag after + cache after + thrown away = bag before +
/// cache before`; the cache closes with "Done", so its rest and what was thrown away burn, and the
/// check is `bag after <= bag before + cache`. Every slot holds one kind, at most a stack;
/// consumables only in the consumable slots, everything else only in the resource slots.
pub fn check_bag_layout(
    before: &BagLayout,
    cache: &[CacheSlot],
    after: &BagLayout,
    t: &LootTuning,
) -> Result<(), BagError> {
    let sections = [
        (&after.resources[..], false, t.resource_stack_max),
        (&after.consumables[..], true, t.consumable_stack_max),
    ];
    for (slots, consumable, stack) in sections {
        for s in slots {
            if s.is_empty() {
                if *s != BagSlot::EMPTY {
                    return Err(BagError::DirtyEmptySlot);
                }
                continue;
            }
            if (s.kind == RewardKind::Consumable) != consumable {
                return Err(BagError::WrongSection);
            }
            if s.qty > stack {
                return Err(BagError::StackTooBig);
            }
        }
    }
    let same =
        |s: &BagSlot, kind: RewardKind, id: u16| !s.is_empty() && s.kind == kind && s.id == id;
    for (i, s) in after.slots().enumerate() {
        if s.is_empty() || after.slots().take(i).any(|p| same(p, s.kind, s.id)) {
            continue;
        }
        let total = |l: &BagLayout| -> u32 {
            l.slots()
                .filter(|p| same(p, s.kind, s.id))
                .map(|p| p.qty as u32)
                .sum()
        };
        let cached: u32 = cache
            .iter()
            .filter(|c| !c.is_empty() && c.kind == s.kind && c.id == s.id)
            .map(|c| c.qty as u32)
            .sum();
        if total(after) > total(before) + cached {
            return Err(BagError::MoreThanOwned);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::ScriptedRoll;
    use deadroll_types::{BAG_CONSUMABLE_SLOTS, CACHE_SLOTS};

    const T: LootTuning = LootTuning::DEFAULT;

    fn entry(id: u16, qty: u8) -> LootEntry {
        LootEntry {
            kind: RewardKind::Resource,
            id,
            qty,
        }
    }

    /// Levels 1-3; level `n` has entries `n*10 ..`, `n` of them.
    fn table() -> LootTable {
        let mut t = LootTable {
            level_base: 1,
            level_max: 3,
            cost: 4,
            ..LootTable::default()
        };
        for (i, level) in t.levels.iter_mut().take(3).enumerate() {
            level.count = i as u8 + 1;
            for j in 0..=i {
                level.entries[j] = entry(((i + 1) * 10 + j) as u16, 2);
            }
        }
        t
    }

    fn slot(kind: RewardKind, id: u16, qty: u8) -> BagSlot {
        BagSlot { kind, id, qty }
    }

    fn cached(kind: RewardKind, id: u16, qty: u16) -> CacheSlot {
        CacheSlot { kind, id, qty }
    }

    #[test]
    fn record_round_trip_and_validation() {
        let t = table();
        let r = t.to_record();
        assert_eq!(LootTable::from_record(&r), Some(t));
        assert!(validate_record(&r));
        assert!(validate_record(&[0; record::SIZE]), "an empty id");
        assert_eq!(t.rules().cost, 4);
        let bad = |f: &dyn Fn(&mut [u8; record::SIZE])| {
            let mut b = r;
            f(&mut b);
            !validate_record(&b)
        };
        assert!(bad(&|b| b[record::PRESENT] = 2));
        assert!(bad(&|b| b[3] = 1), "reserved byte");
        assert!(bad(&|b| b[record::LEVEL_BASE] = 0));
        assert!(bad(&|b| b[record::LEVEL_MAX] = 0), "max below base");
        assert!(
            bad(&|b| b[record::LEVEL_MAX] = 5),
            "more than LEVELS levels"
        );
        assert!(
            bad(&|b| b[record::LEVELS_AT] = 0),
            "a used level without entries"
        );
        assert!(
            bad(&|b| b[record::LEVELS_AT + 3 * record::LEVEL_SIZE] = 1),
            "an unused level"
        );
        assert!(
            bad(&|b| b[record::LEVELS_AT + 1] = RewardKind::COUNT as u8),
            "kind"
        );
        assert!(bad(&|b| b[record::LEVELS_AT + 4] = 0), "qty 0");
        assert!(bad(&|b| b[record::LEVELS_AT + 5] = 1), "bytes past count");
        assert!(!validate_record(&r[..10]));
    }

    #[test]
    fn level_steps_up_with_its_chance() {
        let t = table();
        // 10_000 > step: stays at base.
        assert_eq!(roll_level(&t, 2_500, &mut ScriptedRoll::new([2_500])), 1);
        assert_eq!(
            roll_level(&t, 2_500, &mut ScriptedRoll::new([2_499, 9_999])),
            2
        );
        // Never above the top, and no die once there.
        let mut src = ScriptedRoll::new([0, 0, 0]);
        assert_eq!(roll_level(&t, 2_500, &mut src), 3);
        assert_eq!(src.index, 2, "no die once at the top");
        assert_eq!(roll_level(&t, 0, &mut ScriptedRoll::new([0])), 1);
    }

    #[test]
    fn reward_picks_an_entry_of_the_level() {
        let t = table();
        assert_eq!(
            roll_reward(&t, 3, &mut ScriptedRoll::new([4])),
            Some(cached(RewardKind::Resource, 31, 2))
        );
        assert_eq!(roll_reward(&t, 4, &mut ScriptedRoll::new([0])), None);
        assert_eq!(t.clamp_level(9), 3);
        assert_eq!(t.clamp_level(0), 1);
    }

    #[test]
    fn chest_rolls_level_once_then_one_reward_per_eligible_player() {
        let t = table();
        // Level die (stays at 1), then slots 0 and 2.
        let mut src = ScriptedRoll::new([9_999, 0, 0, 7]);
        let r = reward_event(RewardSource::Chest, &t, 0, 0b0101, 2_500, &mut src);
        assert_eq!(r.level, 1);
        assert_eq!(r.rewards[0][0], cached(RewardKind::Resource, 10, 2));
        assert!(r.rewards[1][0].is_empty());
        assert_eq!(r.rewards[2][0], cached(RewardKind::Resource, 10, 2));
        assert!(r.rewards[0][1].is_empty(), "no guarantee on a chest");
        assert_eq!(src.next_u64(), 7, "three dice used");
    }

    #[test]
    fn body_uses_the_monster_level_and_rolls_no_level() {
        let t = table();
        let r = reward_event(
            RewardSource::Body,
            &t,
            2,
            0b1,
            10_000,
            &mut ScriptedRoll::new([1]),
        );
        assert_eq!(r.level, 2);
        assert_eq!(r.rewards[0][0], cached(RewardKind::Resource, 21, 2));
        let high = reward_event(
            RewardSource::Body,
            &t,
            9,
            0b1,
            0,
            &mut ScriptedRoll::new([0]),
        );
        assert_eq!(high.level, 3, "clamped to the table");
        assert!(drops_body_loot(Role::Swarm) && !drops_body_loot(Role::Helper));
    }

    #[test]
    fn boss_guarantee_only_on_the_base_level() {
        let t = table();
        // Base level: the reward, then the guarantee from level 2.
        let r = reward_event(
            RewardSource::Boss,
            &t,
            0,
            0b1,
            2_500,
            &mut ScriptedRoll::new([9_999, 0, 1]),
        );
        assert_eq!(r.level, 1);
        assert_eq!(r.rewards[0][0], cached(RewardKind::Resource, 10, 2));
        assert_eq!(r.rewards[0][1], cached(RewardKind::Resource, 21, 2));
        // Level 2: no guarantee.
        let r = reward_event(
            RewardSource::Boss,
            &t,
            0,
            0b1,
            2_500,
            &mut ScriptedRoll::new([0, 9_999, 0]),
        );
        assert_eq!(r.level, 2);
        assert!(r.rewards[0][1].is_empty());
    }

    #[test]
    fn kill_xp_is_full_for_every_living_player() {
        let xp = on_monster_killed(40, 0b1011, &[10_000, 10_000, 10_000, 5_000]);
        assert_eq!(xp, [40, 40, 0, 20]);
    }

    #[test]
    fn share_drops_above_the_tier() {
        // Tier 1 is levels 1-2.
        assert_eq!(
            reward_share(2, 1, &T),
            RewardShare {
                xp_bp: 10_000,
                rolls_loot: true
            }
        );
        assert_eq!(
            reward_share(3, 1, &T),
            RewardShare {
                xp_bp: 5_000,
                rolls_loot: true
            }
        );
        assert_eq!(
            reward_share(9, 1, &T),
            RewardShare {
                xp_bp: 0,
                rolls_loot: false
            }
        );
        assert_eq!(reward_share(4, 2, &T).xp_bp, 10_000);
    }

    #[test]
    fn cache_merges_by_kind_and_id() {
        let mut c = [CacheSlot::EMPTY; 2];
        assert!(cache_add(&mut c, cached(RewardKind::Resource, 1, 2)));
        assert!(cache_add(&mut c, cached(RewardKind::Resource, 1, 3)));
        assert_eq!(c[0].qty, 5);
        assert!(
            cache_add(&mut c, cached(RewardKind::Recipe, 1, 1)),
            "another kind"
        );
        assert!(
            !cache_add(&mut c, cached(RewardKind::Resource, 2, 1)),
            "full"
        );
        assert!(cache_add(&mut c, CacheSlot::EMPTY), "nothing to add");
        assert_eq!(c[1], cached(RewardKind::Recipe, 1, 1));
    }

    #[test]
    fn map_rules_by_tier() {
        assert_eq!(map_rules(&T, 1).chests, 1);
        assert_eq!(map_rules(&T, 2).budget, T.budget[1]);
        assert_eq!(map_rules(&T, 0), LootRules::default());
        assert_eq!(map_rules(&T, 3), LootRules::default());
    }

    fn bag(resources: &[BagSlot], consumables: &[BagSlot]) -> BagLayout {
        let mut b = BagLayout::EMPTY;
        b.resources[..resources.len()].copy_from_slice(resources);
        b.consumables[..consumables.len()].copy_from_slice(consumables);
        b
    }

    const ORE: u16 = 1;
    const WOOD: u16 = 2;
    use RewardKind::{Consumable as C, Recipe as P, Resource as R};

    #[test]
    fn layout_from_bag_and_cache_is_accepted() {
        let before = bag(&[slot(R, ORE, 10), slot(R, WOOD, 3)], &[slot(C, 7, 1)]);
        let cache = [cached(R, ORE, 4), cached(P, 9, 1), cached(C, 7, 2)];
        // Split the ore, take the recipe and the consumables, move the wood to the end.
        let mut after = bag(
            &[slot(R, ORE, 7), slot(R, ORE, 7), slot(P, 9, 1)],
            &[slot(C, 7, 3)],
        );
        after.resources[39] = slot(R, WOOD, 3);
        assert_eq!(check_bag_layout(&before, &cache, &after, &T), Ok(()));
        // Taking nothing (everything burns) and throwing out are fine.
        assert_eq!(
            check_bag_layout(&before, &cache, &BagLayout::EMPTY, &T),
            Ok(())
        );
    }

    #[test]
    fn swapped_kind_is_refused() {
        // Got ore, sent wood.
        let before = BagLayout::EMPTY;
        let cache = [cached(R, ORE, 3)];
        let after = bag(&[slot(R, WOOD, 3)], &[]);
        assert_eq!(
            check_bag_layout(&before, &cache, &after, &T),
            Err(BagError::MoreThanOwned)
        );
        // The same id under another kind is another thing too.
        let after = bag(&[slot(P, ORE, 3)], &[]);
        assert_eq!(
            check_bag_layout(&before, &cache, &after, &T),
            Err(BagError::MoreThanOwned)
        );
    }

    #[test]
    fn more_than_owned_is_refused() {
        let before = bag(&[slot(R, ORE, 2)], &[]);
        let cache = [cached(R, ORE, 3)];
        let after = bag(&[slot(R, ORE, 5), slot(R, ORE, 1)], &[]);
        assert_eq!(
            check_bag_layout(&before, &cache, &after, &T),
            Err(BagError::MoreThanOwned)
        );
    }

    #[test]
    fn stack_of_eleven_is_refused() {
        let cache = [cached(R, ORE, 20)];
        let after = bag(&[slot(R, ORE, 11)], &[]);
        assert_eq!(
            check_bag_layout(&BagLayout::EMPTY, &cache, &after, &T),
            Err(BagError::StackTooBig)
        );
        let cache = [cached(C, 7, 20)];
        let after = bag(&[], &[slot(C, 7, T.consumable_stack_max + 1)]);
        assert_eq!(
            check_bag_layout(&BagLayout::EMPTY, &cache, &after, &T),
            Err(BagError::StackTooBig)
        );
    }

    #[test]
    fn sections_are_kept() {
        let cache = [cached(C, 7, 1), cached(R, ORE, 1)];
        let after = bag(&[slot(C, 7, 1)], &[]);
        assert_eq!(
            check_bag_layout(&BagLayout::EMPTY, &cache, &after, &T),
            Err(BagError::WrongSection)
        );
        let mut after = BagLayout::EMPTY;
        after.consumables[BAG_CONSUMABLE_SLOTS - 1] = slot(R, ORE, 1);
        assert_eq!(
            check_bag_layout(&BagLayout::EMPTY, &cache, &after, &T),
            Err(BagError::WrongSection)
        );
    }

    #[test]
    fn empty_slots_are_zero() {
        let after = bag(&[slot(R, ORE, 0)], &[]);
        assert_eq!(
            check_bag_layout(&BagLayout::EMPTY, &[], &after, &T),
            Err(BagError::DirtyEmptySlot)
        );
    }

    #[test]
    fn full_bag_and_cache() {
        // Worst case for the compute budget: every slot a different id.
        let mut before = BagLayout::EMPTY;
        for (i, s) in before.resources.iter_mut().enumerate() {
            *s = slot(R, i as u16, 10);
        }
        let mut cache = [CacheSlot::EMPTY; CACHE_SLOTS];
        for (i, c) in cache.iter_mut().enumerate() {
            *c = cached(R, 100 + i as u16, 1);
        }
        let mut after = before;
        after.resources[0] = slot(R, 100, 1);
        assert_eq!(check_bag_layout(&before, &cache, &after, &T), Ok(()));
    }
}
