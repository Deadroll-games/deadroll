//! Aggregates over many fights. Floats only here, for the report: the fights are integers.
use crate::battle::{BattleStats, Outcome};

/// p10 / p50 / p90 (nearest rank) and the mean; all 0 without samples.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percentiles {
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub mean: f64,
    pub samples: usize,
}

impl Percentiles {
    pub fn of(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        let mut v = values.to_vec();
        v.sort_by(|a, b| a.total_cmp(b));
        let rank = |p: f64| v[(((p * v.len() as f64).ceil() as usize).max(1) - 1).min(v.len() - 1)];
        Percentiles {
            p10: rank(0.1),
            p50: rank(0.5),
            p90: rank(0.9),
            mean: v.iter().sum::<f64>() / v.len() as f64,
            samples: v.len(),
        }
    }
}

/// Counts per bucket: `(bucket start, count)`, buckets of `width`, ascending.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    pub width: f64,
    pub bins: Vec<(f64, u64)>,
}

impl Histogram {
    /// Integer-valued samples get width-1 buckets up to 60 distinct values, wider beyond.
    pub fn of(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min).floor();
        let hi = values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil();
        let width = ((hi - lo + 1.0) / 60.0).ceil().max(1.0);
        let n = ((hi - lo) / width).floor() as usize + 1;
        let mut counts = vec![0u64; n];
        for v in values {
            let i = (((v - lo) / width).floor() as usize).min(n - 1);
            counts[i] += 1;
        }
        Histogram {
            width,
            bins: counts
                .into_iter()
                .enumerate()
                .map(|(i, c)| (lo + i as f64 * width, c))
                .collect(),
        }
    }
}

/// One hero over all fights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotReport {
    pub name: String,
    /// Damage per own turn, per fight.
    pub dpt: Percentiles,
    pub dpt_hist: Histogram,
    /// Totals over all fights.
    pub damage: u64,
    pub turns: u64,
    /// Shares of the damage: added by crits, dealt over time.
    pub crit_share: f64,
    pub dot_share: f64,
    pub hit_rate: f64,
    pub crit_rate: f64,
    pub healed_per_turn: f64,
    pub effects_per_fight: f64,
    pub death_rate: f64,
}

/// A batch of fights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub fights: u32,
    pub victories: u32,
    pub defeats: u32,
    pub draws: u32,
    pub win_rate: f64,
    /// Rounds of the fights won.
    pub rounds_to_win: Percentiles,
    pub rounds_hist: Histogram,
    /// Attacks a monster took until it died.
    pub hits_to_kill: Percentiles,
    pub hits_hist: Histogram,
    /// Party damage per hero turn, per fight.
    pub party_dpt: Percentiles,
    pub party_dpt_hist: Histogram,
    pub slots: Vec<SlotReport>,
    pub crit_share: f64,
    /// Over-time damage share (Burning, Bleeding).
    pub effect_share: f64,
    /// Monster turns lost to Freeze / Stun.
    pub control_uptime: f64,
}

fn ratio(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

impl Report {
    pub fn of(names: &[String], fights: &[BattleStats]) -> Self {
        let n = fights.len();
        let count = |o: Outcome| fights.iter().filter(|f| f.outcome == o).count() as u32;
        let rounds: Vec<f64> = fights
            .iter()
            .filter(|f| f.outcome == Outcome::Victory)
            .map(|f| f.rounds as f64)
            .collect();
        let hits: Vec<f64> = fights
            .iter()
            .flat_map(|f| f.kills.iter().map(|&k| k as f64))
            .collect();
        let party_dpt: Vec<f64> = fights
            .iter()
            .map(|f| {
                let damage: u64 = f.slots.iter().map(|s| s.damage).sum();
                let turns: u64 = f.slots.iter().map(|s| s.turns as u64).sum();
                ratio(damage, turns)
            })
            .collect();
        let slots: Vec<SlotReport> = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let per: Vec<_> = fights.iter().filter_map(|f| f.slots.get(i)).collect();
                let dpt: Vec<f64> = per
                    .iter()
                    .map(|s| ratio(s.damage, s.turns as u64))
                    .collect();
                let sum = |g: fn(&&crate::battle::SlotStats) -> u64| per.iter().map(g).sum::<u64>();
                let damage = sum(|s| s.damage);
                let turns = sum(|s| s.turns as u64);
                let attacks = sum(|s| s.attacks as u64);
                let hits = sum(|s| s.hits as u64);
                SlotReport {
                    name: name.clone(),
                    dpt: Percentiles::of(&dpt),
                    dpt_hist: Histogram::of(&dpt),
                    damage,
                    turns,
                    crit_share: ratio(sum(|s| s.crit_extra), damage),
                    dot_share: ratio(sum(|s| s.dot_damage), damage),
                    hit_rate: ratio(hits, attacks),
                    crit_rate: ratio(sum(|s| s.crits as u64), hits),
                    healed_per_turn: ratio(sum(|s| s.healed), turns),
                    effects_per_fight: ratio(sum(|s| s.effects_landed as u64), per.len() as u64),
                    death_rate: ratio(
                        per.iter().filter(|s| s.died).count() as u64,
                        per.len() as u64,
                    ),
                }
            })
            .collect();
        let total: u64 = slots.iter().map(|s| s.damage).sum();
        let crit: u64 = fights
            .iter()
            .flat_map(|f| f.slots.iter())
            .map(|s| s.crit_extra)
            .sum();
        let dot: u64 = fights
            .iter()
            .flat_map(|f| f.slots.iter())
            .map(|s| s.dot_damage)
            .sum();
        let monster_turns: u64 = fights.iter().map(|f| f.monster_turns as u64).sum();
        let skipped: u64 = fights.iter().map(|f| f.monster_turns_skipped as u64).sum();
        let victories = count(Outcome::Victory);
        Report {
            fights: n as u32,
            victories,
            defeats: count(Outcome::Defeat),
            draws: count(Outcome::Draw),
            win_rate: ratio(victories as u64, n as u64),
            rounds_to_win: Percentiles::of(&rounds),
            rounds_hist: Histogram::of(&rounds),
            hits_to_kill: Percentiles::of(&hits),
            hits_hist: Histogram::of(&hits),
            party_dpt: Percentiles::of(&party_dpt),
            party_dpt_hist: Histogram::of(&party_dpt),
            slots,
            crit_share: ratio(crit, total),
            effect_share: ratio(dot, total),
            control_uptime: ratio(skipped, monster_turns),
        }
    }
}
