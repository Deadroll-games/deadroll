//! Many fights in parallel (rayon). Fight `i` always gets the same seed, so a batch gives the same
//! report on any number of threads.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use deadroll_combat::{RollSource, SeededRoll};
use deadroll_rules::BalanceConfig;
use deadroll_types::{Class, PARTY_MAX};
use rayon::prelude::*;

use crate::{
    battle::{fight, BattleStats, Outcome, Settings},
    presets::{HeroPreset, Presets},
    randgen::random_hero,
    setup::{fighter, Fight, Fighter, Issues},
    stats::Report,
};

/// Shared with the UI thread: fights done, fights planned, a cancel flag.
#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.done.load(Ordering::Relaxed) as f32 / total as f32
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn tick(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}

/// Seed of fight `i` of a batch seeded with `base`.
pub fn fight_seed(base: u64, i: u64) -> u64 {
    SeededRoll::new(base ^ i.wrapping_mul(0xA24B_AED4_963E_E407)).next_u64()
}

/// `fights` fights of `party` against `encounter`. `None` when cancelled.
pub fn run_batch(
    cfg: &BalanceConfig,
    party: &[Fighter],
    encounter: &Fight,
    settings: Settings,
    fights: u32,
    seed: u64,
    progress: &Progress,
) -> Option<Report> {
    progress.done.store(0, Ordering::Relaxed);
    progress.total.store(fights as u64, Ordering::Relaxed);
    let results: Vec<Option<BattleStats>> = (0..fights as u64)
        .into_par_iter()
        .map(|i| {
            if progress.cancelled() {
                return None;
            }
            let stats = fight(cfg, party, encounter, settings, fight_seed(seed, i));
            progress.tick();
            Some(stats)
        })
        .collect();
    let results: Option<Vec<BattleStats>> = results.into_iter().collect();
    let names: Vec<String> = party.iter().map(|f| f.name.clone()).collect();
    results.map(|r| Report::of(&names, &r))
}

/// What random builds to try.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomBuilds {
    pub count: u32,
    pub fights_each: u32,
    /// Only this class, or any.
    pub class: Option<Class>,
    /// The party slot the build takes (appended when the party is shorter).
    pub slot: usize,
    pub seed: u64,
}

/// One random build and how it did.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildScore {
    pub hero: HeroPreset,
    /// Its damage per own turn over all its fights.
    pub dpt: f64,
    pub win_rate: f64,
    pub issues: Issues,
}

/// Tries `rb.count` random heroes (skills from the builder, valid by `skillcheck`) in `party`
/// and returns the 10 with the best damage per turn. `None` when cancelled.
pub fn random_builds(
    cfg: &BalanceConfig,
    presets: &Presets,
    party: &[Fighter],
    encounter: &Fight,
    settings: Settings,
    rb: RandomBuilds,
    progress: &Progress,
) -> Option<Vec<BuildScore>> {
    progress.done.store(0, Ordering::Relaxed);
    progress
        .total
        .store(rb.count as u64 * rb.fights_each as u64, Ordering::Relaxed);
    let slot = rb.slot.min(party.len()).min(PARTY_MAX - 1);
    let scores: Vec<Option<BuildScore>> = (0..rb.count as u64)
        .into_par_iter()
        .map(|b| {
            if progress.cancelled() {
                return None;
            }
            let mut rng = SeededRoll::new(fight_seed(rb.seed, b));
            let hero = random_hero(&mut rng, cfg, rb.class);
            let mut issues = Issues::default();
            let f = fighter(presets, "random", &hero, cfg, &mut issues);
            let mut team: Vec<Fighter> = party.to_vec();
            if slot < team.len() {
                team[slot] = f;
            } else {
                team.push(f);
            }
            let (mut damage, mut turns, mut wins) = (0u64, 0u64, 0u32);
            for i in 0..rb.fights_each as u64 {
                if progress.cancelled() {
                    return None;
                }
                let s = fight(cfg, &team, encounter, settings, fight_seed(rb.seed ^ b, i));
                damage += s.slots[slot].damage;
                turns += s.slots[slot].turns as u64;
                wins += (s.outcome == Outcome::Victory) as u32;
                progress.tick();
            }
            Some(BuildScore {
                hero,
                dpt: if turns == 0 {
                    0.0
                } else {
                    damage as f64 / turns as f64
                },
                win_rate: wins as f64 / rb.fights_each.max(1) as f64,
                issues,
            })
        })
        .collect();
    let mut scores: Vec<BuildScore> = scores.into_iter().collect::<Option<_>>()?;
    scores.sort_by(|a, b| {
        b.dpt
            .total_cmp(&a.dpt)
            .then(b.win_rate.total_cmp(&a.win_rate))
    });
    scores.truncate(10);
    Some(scores)
}
