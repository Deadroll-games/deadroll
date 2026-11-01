//! What the builder rules need of a bestiary template (docs/BESTIARY.md, sections 2.1, 3.2, 4, 5;
//! MAP_DESCRIPTION.md 7.2). The caller builds it from the registry `Bestiary` record and its pool
//! in the `monster_skills` table (`deadroll_rules::monster`), so `check_map` holds no copy of
//! the bestiary.
use deadroll_skillcheck::{monster::POOL_MAX, Category};

use crate::{
    limits::{TierLimits, BOSSES},
    MapCheckError,
};

/// How a monster template enters a pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Swarm,
    Fast,
    Tank,
    /// At most one per pack.
    Loner,
    /// Its own pack; exactly one per map.
    Boss,
    /// A boss summon: only in the boss pack (docs/BESTIARY.md, section 6).
    Helper,
}

/// One template as the builder rules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnRules {
    pub role: Role,
    /// The lowest map tier it may stand on (BESTIARY 4).
    pub min_tier: u8,
    /// Its pool: `pool_len` skills from `pool_start` of the `monster_skills` table, each one's
    /// category (BESTIARY 3.2).
    pub pool_start: u8,
    pub pool_len: u8,
    pub pool: [Category; POOL_MAX],
    /// Skill slots by `Category` for a regular spawn, an elite one and a boss
    /// (`MonsterScaling::slots`).
    pub slots: [[u8; Category::COUNT]; 3],
    /// Patch B: cells per side in an arena (`crate::footprint`), 1 or 2.
    pub size: u8,
}

impl SpawnRules {
    /// A template with no skills.
    pub const fn plain(role: Role) -> Self {
        SpawnRules {
            role,
            min_tier: 1,
            pool_start: 0,
            pool_len: 0,
            pool: [Category::Active; POOL_MAX],
            slots: [[3, 0, 1], [4, 0, 1], [5, 1, 1]],
            size: 1,
        }
    }

    pub fn pool(&self) -> &[Category] {
        &self.pool[..(self.pool_len as usize).min(POOL_MAX)]
    }

    /// The slots of a spawn: a boss is always a boss, any other template may be elite.
    pub fn slots_for(&self, elite: bool) -> [u8; Category::COUNT] {
        match (self.role, elite) {
            (Role::Boss, _) => self.slots[2],
            (_, true) => self.slots[1],
            (_, false) => self.slots[0],
        }
    }
}

/// Patch C: what one revealed pack adds to the map's totals (`crate::arena::check_roster`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RosterCount {
    /// Monsters outside the boss pack (loners included).
    pub regular: u8,
    pub loners: u8,
    pub elites: u8,
    pub bosses: u8,
    pub helpers: u8,
}

/// Patch C: the map-wide limits over the packs revealed so far (docs/MAP_DESCRIPTION.md 10.2):
/// the program adds a pack at each reveal, the audit [`RosterTotals::finish`]es after the last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RosterTotals {
    pub monsters: u8,
    pub elites: u8,
    pub bosses: u8,
    /// Packs with regular monsters.
    pub packs: u8,
}

impl RosterTotals {
    pub fn add(&mut self, count: &RosterCount, limits: &TierLimits) -> Result<(), MapCheckError> {
        use MapCheckError as E;
        self.monsters = self.monsters.saturating_add(count.regular);
        self.elites = self.elites.saturating_add(count.elites);
        self.bosses = self.bosses.saturating_add(count.bosses);
        self.packs = self.packs.saturating_add((count.regular > 0) as u8);
        if self.monsters > limits.monsters {
            return Err(E::TooManyMonsters);
        }
        if self.elites > limits.elites {
            return Err(E::TooManyElites);
        }
        if self.packs > limits.packs {
            return Err(E::TooManyPacks);
        }
        if self.bosses > BOSSES {
            return Err(E::BossCount);
        }
        Ok(())
    }

    /// Every pack is in: exactly one boss.
    pub fn finish(&self) -> Result<(), MapCheckError> {
        if self.bosses != BOSSES {
            return Err(MapCheckError::BossCount);
        }
        Ok(())
    }
}
