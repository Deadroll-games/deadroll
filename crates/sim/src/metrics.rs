//! A [`Report`] as flat rows for the history log: one number per row, with the group it belongs
//! to, who it is about and what it is.
//!
//! Values are per-fight averages over the batch, except rates (percent), percentiles and the
//! largest hit (a maximum). The set of rows is the same for every Run of the same party, so two
//! Runs line up row by row. Zeros are included; the log drops the ones that are zero in both Runs.
use std::collections::HashMap;

use deadroll_combat::OpKind;
use deadroll_types::{Aspect, StateKind};

use crate::{
    named::Named,
    stats::{Percentiles, Report, SlotReport},
};

/// One number of a Run.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    /// `party`, `hero`, `skill`, `aspect` or `monster`.
    pub group: &'static str,
    /// A hero, monster, skill or aspect name, or `party`.
    pub who: String,
    pub metric: String,
    pub value: f64,
}

fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        0.0
    } else {
        a / b
    }
}

fn pct(a: f64, b: f64) -> f64 {
    ratio(a, b) * 100.0
}

/// Names that stay unique inside a list: a repeat gets ` #n` (n counts from 1).
fn unique(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let names: Vec<String> = names.into_iter().collect();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for name in &names {
        *seen.entry(name).or_default() += 1;
    }
    let mut next: HashMap<&str, usize> = HashMap::new();
    names
        .iter()
        .map(|name| {
            if seen[name.as_str()] == 1 {
                name.clone()
            } else {
                let n = next.entry(name).or_default();
                *n += 1;
                format!("{name} #{n}")
            }
        })
        .collect()
}

struct Rows(Vec<Metric>);

impl Rows {
    fn add(&mut self, group: &'static str, who: &str, metric: impl Into<String>, value: f64) {
        self.0.push(Metric {
            group,
            who: who.to_string(),
            metric: metric.into(),
            value,
        });
    }

    fn percentiles(&mut self, group: &'static str, who: &str, name: &str, p: &Percentiles) {
        self.add(group, who, format!("{name} p10"), p.p10);
        self.add(group, who, format!("{name} p50"), p.p50);
        self.add(group, who, format!("{name} p90"), p.p90);
    }
}

/// Every number of `report`, as rows.
pub fn metrics(report: &Report) -> Vec<Metric> {
    let n = report.fights as f64;
    let mut rows = Rows(Vec::new());
    party(&mut rows, report);
    let heroes = unique(report.slots.iter().map(|s| s.name.clone()));
    for (hero, slot) in heroes.iter().zip(&report.slots) {
        hero_rows(&mut rows, hero, slot, n);
    }
    // The party's aspects: the heroes' summed.
    let mut damage = [0u64; Aspect::COUNT];
    let mut resisted = [0u64; Aspect::COUNT];
    for slot in &report.slots {
        for (a, b) in damage.iter_mut().zip(slot.by_aspect) {
            *a += b;
        }
        for (a, b) in resisted.iter_mut().zip(slot.resisted) {
            *a += b;
        }
    }
    aspect_rows(&mut rows, "party", &damage, &resisted, n);
    for (hero, slot) in heroes.iter().zip(&report.slots) {
        aspect_rows(&mut rows, hero, &slot.by_aspect, &slot.resisted, n);
    }
    let monsters = unique(report.monsters.iter().map(|m| m.name.clone()));
    for (name, m) in monsters.iter().zip(&report.monsters) {
        let t = &m.tally;
        let g = "monster";
        rows.add(g, name, "damage dealt", ratio(t.damage_dealt as f64, n));
        rows.add(g, name, "damage taken", ratio(t.damage_taken as f64, n));
        rows.add(g, name, "hits", ratio(t.hits as f64, n));
        rows.add(g, name, "crits", ratio(t.crits as f64, n));
        rows.add(g, name, "skills used", ratio(t.skills as f64, n));
        rows.add(g, name, "reactions", ratio(t.reactions as f64, n));
        rows.add(g, name, "healed", ratio(t.healed as f64, n));
        rows.add(g, name, "turns skipped", ratio(t.turns_skipped as f64, n));
        rows.add(g, name, "kills of heroes", ratio(t.hero_kills as f64, n));
        rows.add(g, name, "death rate %", pct(t.died as f64, t.spawned as f64));
    }
    rows.0
}

fn party(rows: &mut Rows, r: &Report) {
    let g = "party";
    let w = "party";
    rows.add(g, w, "win rate %", r.win_rate * 100.0);
    rows.percentiles(g, w, "rounds to win", &r.rounds_to_win);
    rows.percentiles(g, w, "hits to kill", &r.hits_to_kill);
    rows.add(g, w, "HP left %", r.hp_left_share * 100.0);
    rows.add(g, w, "deaths", r.deaths_per_fight);
    rows.add(g, w, "kills", r.kills_per_fight);
    rows.add(g, w, "packs won", r.packs_won_per_fight);
    rows.add(g, w, "experience", r.xp_per_fight);
    for (kind, count) in StateKind::ALL.iter().zip(r.states_per_fight) {
        rows.add(g, w, format!("states landed: {}", kind.name()), count);
    }
    rows.add(g, w, "resonances", r.resonances_per_fight);
    rows.add(g, w, "triples", r.triples_per_fight);
    rows.add(g, w, "reactions", r.reactions_per_fight);
    for (k, count) in r.ops_per_fight.iter().enumerate() {
        if let Some(kind) = OpKind::from_u8(k as u8).filter(|k| *k != OpKind::None) {
            rows.add(g, w, format!("ops acted: {kind:?}"), *count);
        }
    }
    rows.add(g, w, "jumps", r.jumps_per_fight);
    rows.add(g, w, "knockbacks", r.knockbacks_per_fight);
    rows.add(g, w, "cells ignited", r.cells_per_fight);
    rows.add(g, w, "burned by cells", r.cell_burns_per_fight);
    rows.add(g, w, "line extra hits", r.line_extra_per_fight);
    rows.add(g, w, "aura reach", r.aura_reach_per_fight);
    rows.add(g, w, "control uptime %", r.control_uptime * 100.0);
}

fn hero_rows(rows: &mut Rows, who: &str, s: &SlotReport, n: f64) {
    let g = "hero";
    let per = |v: u64| ratio(v as f64, n);
    rows.add(g, who, "turns", per(s.turns));
    rows.add(g, who, "attacks", per(s.attacks));
    rows.add(g, who, "hits", per(s.hits));
    rows.add(g, who, "misses", per(s.misses));
    rows.add(g, who, "hit rate %", s.hit_rate * 100.0);
    rows.add(g, who, "crits", per(s.crits));
    rows.add(g, who, "crit rate %", s.crit_rate * 100.0);
    rows.add(g, who, "total damage", per(s.damage));
    rows.add(g, who, "damage per turn", ratio(s.damage as f64, s.turns as f64));
    rows.add(
        g,
        who,
        "average damage per hit",
        ratio(s.hit_damage as f64, s.hits as f64),
    );
    rows.add(g, who, "largest hit", s.largest_hit as f64);
    rows.add(g, who, "crit extra damage", per(s.crit_extra));
    rows.add(g, who, "damage over time", per(s.dot_damage));
    rows.add(g, who, "healed", per(s.healed));
    rows.add(g, who, "effects landed", per(s.effects));
    rows.add(g, who, "damage taken", per(s.damage_taken));
    rows.add(g, who, "death rate %", s.death_rate * 100.0);
    let skills = unique(s.skills.iter().map(|k| k.name.clone()));
    for (name, k) in skills.iter().zip(&s.skills) {
        let who = format!("{who} / {name}");
        let g = "skill";
        rows.add(g, &who, "uses", per(k.uses));
        rows.add(g, &who, "hits", per(k.hits));
        rows.add(g, &who, "misses", per(k.misses));
        rows.add(g, &who, "crits", per(k.crits));
        rows.add(g, &who, "damage", per(k.damage));
        rows.add(g, &who, "damage per use", ratio(k.damage as f64, k.uses as f64));
        rows.add(g, &who, "effects landed", per(k.effects));
    }
}

/// `owner / aspect` rows: damage, its share of the owner's damage, and the damage resistance cut.
fn aspect_rows(
    rows: &mut Rows,
    owner: &str,
    damage: &[u64; Aspect::COUNT],
    resisted: &[u64; Aspect::COUNT],
    n: f64,
) {
    let total: u64 = damage.iter().sum();
    for aspect in Aspect::ALL {
        let a = *aspect as usize;
        let who = format!("{owner} / {}", aspect.name());
        rows.add("aspect", &who, "damage", ratio(damage[a] as f64, n));
        rows.add("aspect", &who, "share %", pct(damage[a] as f64, total as f64));
        rows.add(
            "aspect",
            &who,
            "damage reduced by resistance",
            ratio(resisted[a] as f64, n),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::stats::{MonsterReport, SkillReport};

    fn report() -> Report {
        let mut by_aspect = [0u64; Aspect::COUNT];
        by_aspect[Aspect::Fire as usize] = 300;
        by_aspect[Aspect::Cold as usize] = 100;
        let slot = |name: &str| SlotReport {
            name: name.into(),
            damage: 400,
            turns: 40,
            attacks: 50,
            hits: 40,
            hit_damage: 360,
            hit_rate: 0.8,
            by_aspect,
            skills: vec![SkillReport {
                name: "Bolt".into(),
                uses: 20,
                damage: 300,
                ..Default::default()
            }],
            ..Default::default()
        };
        Report {
            fights: 10,
            win_rate: 0.5,
            slots: vec![slot("mage"), slot("mage")],
            monsters: vec![MonsterReport {
                id: 3,
                name: "wolf".into(),
                tally: crate::MonsterTally {
                    spawned: 20,
                    died: 10,
                    damage_dealt: 100,
                    ..Default::default()
                },
            }],
            ops_per_fight: vec![0.0; OpKind::COUNT as usize],
            ..Default::default()
        }
    }

    fn find<'a>(rows: &'a [Metric], group: &str, who: &str, metric: &str) -> &'a Metric {
        rows.iter()
            .find(|m| m.group == group && m.who == who && m.metric == metric)
            .unwrap_or_else(|| panic!("no row {group} / {who} / {metric}"))
    }

    #[test]
    fn rows_are_per_fight_averages_and_rates() {
        let rows = metrics(&report());
        assert_eq!(find(&rows, "party", "party", "win rate %").value, 50.0);
        assert_eq!(find(&rows, "hero", "mage #1", "total damage").value, 40.0);
        assert_eq!(find(&rows, "hero", "mage #2", "damage per turn").value, 10.0);
        assert_eq!(
            find(&rows, "hero", "mage #1", "average damage per hit").value,
            9.0
        );
        assert_eq!(find(&rows, "hero", "mage #1", "hit rate %").value, 80.0);
        assert_eq!(find(&rows, "skill", "mage #1 / Bolt", "damage per use").value, 15.0);
        assert_eq!(find(&rows, "monster", "wolf", "death rate %").value, 50.0);
        assert_eq!(find(&rows, "monster", "wolf", "damage dealt").value, 10.0);
    }

    #[test]
    fn aspects_are_per_hero_and_for_the_party_and_shares_add_up() {
        let rows = metrics(&report());
        assert_eq!(find(&rows, "aspect", "party / Fire", "damage").value, 60.0);
        assert_eq!(find(&rows, "aspect", "mage #1 / Fire", "share %").value, 75.0);
        let shares: f64 = rows
            .iter()
            .filter(|m| m.group == "aspect" && m.who.starts_with("party / ") && m.metric == "share %")
            .map(|m| m.value)
            .sum();
        assert!((shares - 100.0).abs() < 1e-9);
    }

    #[test]
    fn every_row_is_unique() {
        let rows = metrics(&report());
        let keys: HashSet<_> = rows
            .iter()
            .map(|m| (m.group, m.who.clone(), m.metric.clone()))
            .collect();
        assert_eq!(keys.len(), rows.len());
    }
}
