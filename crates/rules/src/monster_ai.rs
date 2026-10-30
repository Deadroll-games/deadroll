//! A simple monster AI: which action a monster takes this turn and on whom. Not a game rule: the
//! DM decides in the game. The balance simulator plays every monster with it and the website's
//! auto mode asks it through combat.wasm, so both play the same way.
//!
//! The order of preference:
//! 1. a heal on the most hurt ally below `MonsterScaling::ai_heal_below_pct` (itself included);
//! 2. a buff on an ally that lacks it (itself first);
//! 3. on the nearest living player (then the next ones): a debuff or hard control it lacks, then a
//!    damage skill whose effect it lacks, then the strongest damage skill, then the natural attack.
//!
//! Reach counts the action points left after the action as steps of one cell (no walls): the
//! caller walks there with its own path finding and may find the cell taken.
use deadroll_combat::{
    states::has,
    support::{find_mod, rooted, silenced},
    States, TimedMods,
};
use deadroll_skillcheck::{Category, Purpose};
use deadroll_types::{Cell, RangeTag, StateKind, MONSTER_SLOTS};

use crate::{
    config::BalanceConfig,
    input::{effect_action, EffectAction},
    monster::{kin_reaches, MonsterProfile, MonsterSkillStats},
    units::{form_range, reach},
    Footprint,
};

/// A unit as the AI sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiUnit {
    pub cell: Cell,
    pub hp: u32,
    pub hp_max: u32,
    pub alive: bool,
    pub states: States,
    /// Its own buffs and debuffs.
    pub mods: TimedMods,
    /// Monsters: `MonsterTemplate::kin`.
    pub kin: u8,
    /// Patch B: cells per side (`MonsterTemplate::size`; players and 0 count as 1).
    pub size: u8,
}

impl AiUnit {
    /// The cells it stands on.
    pub const fn footprint(&self) -> Footprint {
        Footprint::new(self.cell, self.size)
    }
}

/// The monster that acts.
#[derive(Clone, Copy, Debug)]
pub struct AiMonster<'a> {
    pub index: u8,
    pub ap: u8,
    pub profile: &'a MonsterProfile,
    /// By slot (`monster::ACTIVE_SLOT..`); only active skills are actions.
    pub skills: &'a [Option<MonsterSkillStats>; MONSTER_SLOTS],
    pub cooldowns: &'a [u8; MONSTER_SLOTS],
}

/// Who an action goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiTarget {
    Player(u8),
    Monster(u8),
}

/// The chosen action: `slot` is the skill slot, `None` the natural attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiChoice {
    pub slot: Option<u8>,
    pub target: AiTarget,
    pub range: RangeTag,
    /// Action points it costs.
    pub ap: u8,
}

fn ready<'a>(m: &AiMonster<'a>, slot: usize, silenced: bool) -> Option<&'a MonsterSkillStats> {
    let s = m.skills[slot].as_ref().filter(|_| !silenced)?;
    (s.stats.blocks.category == Category::Active
        && m.cooldowns[slot] == 0
        && s.stats.ap_cost <= m.ap)
        .then_some(s)
}

/// Whether a unit at `to` is within `range` after walking with the AP left by an action of `cost`
/// (`walk_ap`: the monster's AP, 0 when it is rooted).
fn reachable(
    cfg: &BalanceConfig,
    walk_ap: u8,
    from: Footprint,
    to: Footprint,
    range: RangeTag,
    cost: u8,
    ally: bool,
) -> bool {
    let d = from.distance(&to);
    if !ally && d == 0 {
        return false;
    }
    let walk = walk_ap.saturating_sub(cost);
    d <= reach(range, &cfg.players).saturating_add(walk) as u16
}

/// Whether `target` already has what `effect` gives (a modifier of its kind, or the control).
fn has_effect(effect: EffectAction, u: &AiUnit) -> bool {
    match effect {
        EffectAction::Mod(kind) => find_mod(&u.mods, kind).is_some(),
        EffectAction::Stun => has(&u.states, StateKind::Stun),
        EffectAction::Freeze => has(&u.states, StateKind::Freeze),
        EffectAction::Heal => false,
    }
}

/// Monster `m`'s action this turn; `None` when nothing reaches (walk toward the nearest player).
/// `heroes` by player slot, `monsters` by monster index (`m` included).
pub fn plan(
    cfg: &BalanceConfig,
    m: &AiMonster,
    heroes: &[AiUnit],
    monsters: &[AiUnit],
) -> Option<AiChoice> {
    let me = m.index as usize;
    let my = monsters.get(me)?;
    let from = my.footprint();
    // A rooted monster acts from where it stands; a silenced one uses no skill.
    let walk_ap = if rooted(&my.mods) { 0 } else { m.ap };
    let silenced = silenced(&my.mods);
    let ally_ok = |s: &MonsterSkillStats, i: usize| {
        let u = &monsters[i];
        u.alive && kin_reaches(s.kin_only, my.kin, u.kin, i == me)
    };

    // 1. A heal on the most hurt ally below the threshold.
    let pct = cfg.monsters.ai_heal_below_pct as u64;
    for slot in 0..MONSTER_SLOTS {
        let Some(s) = ready(m, slot, silenced) else {
            continue;
        };
        if effect_action(s.stats.blocks.effect) != Some(EffectAction::Heal) {
            continue;
        }
        let best = (0..monsters.len())
            .filter(|&i| ally_ok(s, i))
            .filter(|&i| (monsters[i].hp as u64) * 100 < monsters[i].hp_max as u64 * pct)
            .filter(|&i| {
                reachable(
                    cfg,
                    walk_ap,
                    from,
                    monsters[i].footprint(),
                    s.stats.ext.range,
                    s.stats.ap_cost,
                    true,
                )
            })
            .min_by_key(|&i| {
                let u = &monsters[i];
                ((u.hp as u64 * 10_000) / u.hp_max.max(1) as u64, i)
            });
        if let Some(i) = best {
            return Some(choice(slot, AiTarget::Monster(i as u8), s));
        }
    }

    // 2. A buff on an ally that lacks it, itself first.
    for slot in 0..MONSTER_SLOTS {
        let Some(s) = ready(m, slot, silenced) else {
            continue;
        };
        let b = &s.stats.blocks;
        if b.purpose != Purpose::Buff {
            continue;
        }
        let Some(effect @ EffectAction::Mod(_)) = effect_action(b.effect) else {
            continue;
        };
        let order = core::iter::once(me).chain((0..monsters.len()).filter(|&i| i != me));
        for i in order {
            if ally_ok(s, i)
                && !has_effect(effect, &monsters[i])
                && reachable(
                    cfg,
                    walk_ap,
                    from,
                    monsters[i].footprint(),
                    s.stats.ext.range,
                    s.stats.ap_cost,
                    true,
                )
            {
                return Some(choice(slot, AiTarget::Monster(i as u8), s));
            }
        }
    }

    // 3. The players, nearest first.
    let mut order = [0u8; deadroll_types::PARTY_MAX];
    let mut n = 0;
    for (i, h) in heroes.iter().enumerate().take(order.len()) {
        if h.alive {
            order[n] = i as u8;
            n += 1;
        }
    }
    let order = &mut order[..n];
    order.sort_by_key(|&i| {
        let h = &heroes[i as usize];
        (from.distance(&h.footprint()), h.hp, i)
    });
    for &slot_of in order.iter() {
        let h = &heroes[slot_of as usize];
        let target = AiTarget::Player(slot_of);
        let fits = |s: &MonsterSkillStats| {
            reachable(
                cfg,
                walk_ap,
                from,
                h.footprint(),
                s.stats.ext.range,
                s.stats.ap_cost,
                false,
            )
        };
        let lacks = |s: &MonsterSkillStats| {
            effect_action(s.stats.blocks.effect).is_some_and(|e| !has_effect(e, h))
        };
        let mut best: Option<(u8, usize)> = None;
        for slot in 0..MONSTER_SLOTS {
            let Some(s) = ready(m, slot, silenced) else {
                continue;
            };
            if s.stats.blocks.purpose == Purpose::Buff || !fits(s) {
                continue;
            }
            // Rank: debuff it lacks 3, damage with an effect it lacks 2, damage 1.
            let rank = match s.stats.blocks.purpose {
                Purpose::Debuff if lacks(s) => 3,
                Purpose::Debuff => continue,
                Purpose::Damage if lacks(s) => 2,
                Purpose::Damage => 1,
                Purpose::Buff => continue,
            };
            let better = match best {
                None => true,
                Some((r, b)) => {
                    rank > r
                        || (rank == r
                            && m.skills[slot].map_or(0, |x| x.stats.power_bp)
                                > m.skills[b].map_or(0, |x| x.stats.power_bp))
                }
            };
            if better {
                best = Some((rank, slot));
            }
        }
        if let Some((_, slot)) = best {
            let s = m.skills[slot].as_ref()?;
            return Some(choice(slot, target, s));
        }
        let range = form_range(m.profile.form);
        let cost = cfg.players.basic_attack_ap;
        if m.ap >= cost && reachable(cfg, walk_ap, from, h.footprint(), range, cost, false) {
            return Some(AiChoice {
                slot: None,
                target,
                range,
                ap: cost,
            });
        }
    }
    None
}

fn choice(slot: usize, target: AiTarget, s: &MonsterSkillStats) -> AiChoice {
    AiChoice {
        slot: Some(slot as u8),
        target,
        range: s.stats.ext.range,
        ap: s.stats.ap_cost,
    }
}
