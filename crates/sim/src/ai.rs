//! A simple AI, the same for every run:
//! - heroes: heal the weakest ally below `heal_below_pct` if a heal skill reaches it; else the
//!   action with the best expected value that is reachable this turn (damage, a debuff the target
//!   lacks, a buff the ally lacks); else walk toward the nearest monster;
//! - monsters: `deadroll_rules::monster_ai` picks the action and its target (the website's auto
//!   mode asks the same function), then they walk into its reach; else toward the nearest hero.
//!
//! Scores are integers in "expected damage x 400" (two d20 factors of 20). Every effect kind has
//! an explicit value below: a new `EffectAction` or `ModKind` does not compile until it has one.
use deadroll_combat::{preview_range, support::find_mod, HitInput, ModKind};
use deadroll_rules::{
    input::{
        action_ap, effect_action, effect_range, monster_attacker, monster_defender,
        player_attacker, player_defender, player_range, support_input, target_side, EffectAction,
        Side,
    },
    monster::NO_KIN,
    monster_ai::{plan as monster_plan, AiMonster, AiTarget, AiUnit},
    units::{in_ally_range, in_range},
};
use deadroll_skillcheck::{Purpose, Target};
use deadroll_types::{Cell, StateKind, BP};

use crate::battle::{Action, Battle};

const UNREACHABLE: u16 = u16::MAX;

/// d20 faces (of 20) that land against `defense`: a natural 1 misses, a natural 20 lands.
fn faces(bonus: u32, defense: u32) -> u64 {
    1 + (2..=19u32)
        .filter(|d| d.saturating_add(bonus) >= defense)
        .count() as u64
}

fn mid((lo, hi): (u32, u32)) -> u64 {
    (lo as u64 + hi as u64) / 2
}

impl Battle<'_> {
    fn cell_index(&self, c: Cell) -> usize {
        c.y as usize * self.width as usize + c.x as usize
    }

    fn occupied(&self, c: Cell, mover: Cell) -> bool {
        c != mover
            && (self.heroes.iter().any(|h| h.alive && h.cell == c)
                || self
                    .foes
                    .iter()
                    .any(|f| f.m.alive && f.m.x == c.x && f.m.y == c.y))
    }

    /// Steps from `from` to every cell (4 directions, floor costs 1, living units block).
    pub(crate) fn distances(&self, from: Cell) -> Vec<u16> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut dist = vec![UNREACHABLE; w * h];
        let mut queue = std::collections::VecDeque::new();
        dist[self.cell_index(from)] = 0;
        queue.push_back(from);
        while let Some(at) = queue.pop_front() {
            let d = dist[self.cell_index(at)];
            let steps = [(0i16, -1i16), (1, 0), (0, 1), (-1, 0)];
            for (dx, dy) in steps {
                let (x, y) = (at.x as i16 + dx, at.y as i16 + dy);
                if x < 0 || y < 0 || x >= w as i16 || y >= h as i16 {
                    continue;
                }
                let next = Cell {
                    x: x as u8,
                    y: y as u8,
                };
                let i = self.cell_index(next);
                if dist[i] == UNREACHABLE && !self.occupied(next, from) {
                    dist[i] = d + 1;
                    queue.push_back(next);
                }
            }
        }
        dist
    }

    /// The cheapest cell within `budget` steps from which `reaches(cell)` holds.
    fn best_cell(
        &self,
        dist: &[u16],
        budget: u8,
        reaches: impl Fn(Cell) -> bool,
    ) -> Option<(Cell, u16)> {
        let mut best: Option<(Cell, u16)> = None;
        for y in 0..self.height {
            for x in 0..self.width {
                let c = Cell { x, y };
                let d = dist[self.cell_index(c)];
                if d <= budget as u16 && reaches(c) && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((c, d));
                }
            }
        }
        best
    }

    fn foe_cell(&self, i: usize) -> Cell {
        Cell {
            x: self.foes[i].m.x,
            y: self.foes[i].m.y,
        }
    }

    /// Walks toward the closest of `targets` as far as `budget` allows.
    fn approach(&self, from: Cell, dist: &[u16], budget: u8, targets: &[Cell]) -> Cell {
        let gap = |c: Cell| {
            targets
                .iter()
                .map(|t| deadroll_rules::units::chebyshev(c, *t) as u16)
                .min()
                .unwrap_or(0)
        };
        let mut best = (from, gap(from), 0u16);
        for y in 0..self.height {
            for x in 0..self.width {
                let c = Cell { x, y };
                let d = dist[self.cell_index(c)];
                if d > budget as u16 {
                    continue;
                }
                let g = gap(c);
                if (g, d) < (best.1, best.2) {
                    best = (c, g, d);
                }
            }
        }
        best.0
    }

    // -----------------------------------------------------------------------------------------
    // Heroes

    pub(crate) fn hero_turn(&mut self, s: usize, ap: u8) {
        let from = self.heroes[s].cell;
        let dist = self.distances(from);
        if let Some((cell, action)) = self.plan(s, &dist, ap) {
            self.move_hero(s, cell);
            self.hero_act(s, action);
            return;
        }
        let targets: Vec<Cell> = (0..self.foes.len())
            .filter(|&i| self.foes[i].m.alive)
            .map(|i| self.foe_cell(i))
            .collect();
        let to = self.approach(from, &dist, ap, &targets);
        self.move_hero(s, to);
    }

    /// The best action affordable with `ap` (its own cost, docs/PARTY_AND_TURNS.md 5.1; the
    /// rest buys steps), and the cell to act from. A line is worth the sum over the units on it.
    pub(crate) fn plan(&self, s: usize, dist: &[u16], ap: u8) -> Option<(Cell, Action)> {
        let fighter = &self.party[self.heroes[s].fighter];
        let ready = |k: usize| self.heroes[s].cooldowns[k] == 0;
        let budget_for =
            |stats: Option<&deadroll_rules::SkillStats>| ap.checked_sub(action_ap(self.cfg, stats));

        // 1. A heal on the weakest ally below the threshold.
        let pct = self.settings.heal_below_pct as u64;
        let mut wounded: Vec<usize> = (0..self.heroes.len())
            .filter(|&a| {
                let h = &self.heroes[a];
                h.alive && (h.hp as u64) * 100 < pct * h.hp_max as u64
            })
            .collect();
        wounded.sort_by_key(|&a| {
            let h = &self.heroes[a];
            (h.hp as u64 * 10_000 / h.hp_max.max(1) as u64, a)
        });
        for k in 0..fighter.skills.len() {
            let skill = &fighter.skills[k].stats;
            if !ready(k) || effect_action(skill.blocks.effect) != Some(EffectAction::Heal) {
                continue;
            }
            let Some(budget) = budget_for(Some(skill)) else {
                continue;
            };
            for &a in &wounded {
                let to = self.heroes[a].cell;
                let reach = |c| in_ally_range(skill.ext.range, c, to, &self.cfg.players);
                if let Some((cell, _)) = self.best_cell(dist, budget, reach) {
                    return Some((cell, Action::Support { skill: k, ally: a }));
                }
            }
        }

        // 2. The best expected value.
        let mut best: Option<(u64, u16, Cell, Action)> = None;
        let mut consider = |score: u64, cell: Cell, cost: u16, action: Action| {
            if score > 0
                && best.is_none_or(|(bs, bc, ..)| {
                    (score, std::cmp::Reverse(cost)) > (bs, std::cmp::Reverse(bc))
                })
            {
                best = Some((score, cost, cell, action));
            }
        };
        let choices =
            std::iter::once(None).chain((0..fighter.skills.len()).filter(|&k| ready(k)).map(Some));
        for skill in choices {
            let stats = skill.map(|k| fighter.skills[k].stats);
            let Some(budget) = budget_for(stats.as_ref()) else {
                continue;
            };
            let range = player_range(self.cfg, &fighter.hero, stats.as_ref());
            let line = stats.filter(|st| st.blocks.target == Target::Line);
            match target_side(stats.as_ref()) {
                Side::Enemy => {
                    for i in (0..self.foes.len()).filter(|&i| self.foes[i].m.alive) {
                        let to = self.foe_cell(i);
                        let reach = |c| in_range(range, c, to, &self.cfg.players);
                        let Some((cell, cost)) = self.best_cell(dist, budget, reach) else {
                            continue;
                        };
                        let targets = match &line {
                            Some(st) => self.line_foes(cell, i, st),
                            None => vec![i],
                        };
                        let debuff = stats.filter(|s| s.blocks.purpose == Purpose::Debuff);
                        match (skill, debuff) {
                            (Some(k), Some(_)) => {
                                let score = targets.iter().map(|&t| self.debuff_score(s, k, t));
                                consider(
                                    score.sum(),
                                    cell,
                                    cost,
                                    Action::Debuff {
                                        skill: k,
                                        target: i,
                                    },
                                );
                            }
                            _ => {
                                let score = targets.iter().map(|&t| self.attack_score(s, skill, t));
                                consider(
                                    score.sum(),
                                    cell,
                                    cost,
                                    Action::Attack { skill, target: i },
                                )
                            }
                        }
                    }
                }
                Side::Ally => {
                    let Some(k) = skill else { continue };
                    for a in (0..self.heroes.len()).filter(|&a| self.heroes[a].alive) {
                        let to = self.heroes[a].cell;
                        let reach = |c| in_ally_range(range, c, to, &self.cfg.players);
                        let Some((cell, cost)) = self.best_cell(dist, budget, reach) else {
                            continue;
                        };
                        let score: u64 = match &line {
                            Some(st) => self
                                .line_heroes(s, a, st)
                                .iter()
                                .map(|&t| self.buff_score(s, k, t))
                                .sum(),
                            None => self.buff_score(s, k, a),
                        };
                        consider(score, cell, cost, Action::Support { skill: k, ally: a });
                    }
                }
            }
        }
        best.map(|(_, _, cell, action)| (cell, action))
    }

    /// Expected damage x 400 of hero `s` hitting monster `i` with `skill`, plus the expected
    /// value of the skill's effect.
    fn attack_score(&self, s: usize, skill: Option<usize>, i: usize) -> u64 {
        let h = &self.heroes[s];
        let fighter = &self.party[h.fighter];
        let stats = skill.map(|k| fighter.skills[k].stats);
        let foe = &self.foes[i];
        let input = HitInput {
            attacker: player_attacker(
                self.cfg,
                0,
                &fighter.hero,
                stats.as_ref(),
                &self.hero_mods(s, stats.is_none()),
                &h.accuracy,
            ),
            defender: monster_defender(&foe.profile, &foe.m.states, &self.foe_mods(i, false)),
        };
        let f = faces(input.attacker.attack_bonus, input.defender.defense);
        let mut score = mid(preview_range(&input, &self.cfg.combat)) * f * 20;
        if let Some(st) = stats {
            if let Some(effect) = effect_action(st.blocks.effect) {
                let value = self.foe_effect_value(s, st.power_bp, effect, i);
                score += value * f / 20 * self.cfg.support.rider_chance_pct as u64 / 100;
            }
        }
        score
    }

    fn debuff_score(&self, s: usize, k: usize, i: usize) -> u64 {
        let fighter = &self.party[self.heroes[s].fighter];
        let st = fighter.skills[k].stats;
        let Some(effect) = effect_action(st.blocks.effect) else {
            return 0;
        };
        let foe = &self.foes[i];
        let defense = monster_defender(&foe.profile, &foe.m.states, &foe.mods).defense;
        let input = support_input(self.cfg, &fighter.hero, &st, effect, Some(defense));
        self.foe_effect_value(s, st.power_bp, effect, i) * faces(input.attack_bonus, defense) / 20
    }

    fn buff_score(&self, s: usize, k: usize, a: usize) -> u64 {
        let fighter = &self.party[self.heroes[s].fighter];
        let st = fighter.skills[k].stats;
        let Some(effect) = effect_action(st.blocks.effect) else {
            return 0;
        };
        let ally = &self.heroes[a];
        let (lo, hi) = effect_range(self.cfg, &fighter.hero, st.power_bp, effect);
        let avg = (lo as u64 + hi as u64) / 2;
        match effect {
            EffectAction::Heal => {
                let pct = self.settings.heal_below_pct as u64;
                if (ally.hp as u64) * 100 < pct * ally.hp_max as u64 {
                    avg.min((ally.hp_max - ally.hp) as u64) * 400
                } else {
                    0
                }
            }
            EffectAction::Mod(kind) => {
                if find_mod(&ally.mods, kind).is_some() {
                    return 0;
                }
                let turns = self.cfg.support.duration(kind) as u64;
                match kind {
                    // One action per turn: Haste only buys steps.
                    ModKind::Haste => self.best_damage(a) * avg / BP as u64 * turns / 4,
                    ModKind::Empowerment => self.best_damage(a) * avg / BP as u64 * turns,
                    ModKind::Protection => self.threat_on(a) * avg / (BP as u64 + avg) * turns,
                    // Only its own aspects: about half of what comes at the ally.
                    ModKind::Resistance => self.threat_on(a) * avg / (BP as u64 + avg) * turns / 2,
                    ModKind::Slow | ModKind::Weakening | ModKind::Vulnerability => 0,
                }
            }
            EffectAction::Stun | EffectAction::Freeze => 0,
        }
    }

    /// Expected value x 400 of `effect` landing on monster `i`.
    fn foe_effect_value(&self, s: usize, power_bp: u32, effect: EffectAction, i: usize) -> u64 {
        let foe = &self.foes[i];
        let caster = &self.party[self.heroes[s].fighter].hero;
        let (lo, hi) = effect_range(self.cfg, caster, power_bp, effect);
        let avg = (lo as u64 + hi as u64) / 2;
        let controlled = foe
            .m
            .states
            .iter()
            .any(|st| st.turns > 0 && matches!(st.kind, StateKind::Freeze | StateKind::Stun));
        match effect {
            EffectAction::Stun | EffectAction::Freeze => {
                if controlled {
                    return 0;
                }
                let kind = match effect {
                    EffectAction::Stun => StateKind::Stun,
                    _ => StateKind::Freeze,
                };
                self.threat_of(i) * self.cfg.combat.turns(kind) as u64
            }
            EffectAction::Mod(kind) => {
                if find_mod(&foe.mods, kind).is_some() {
                    return 0;
                }
                let turns = self.cfg.support.duration(kind) as u64;
                match kind {
                    ModKind::Weakening => self.threat_of(i) * avg / (BP as u64 + avg) * turns,
                    // Fewer steps, not fewer attacks.
                    ModKind::Slow => self.threat_of(i) * avg / (BP as u64 + avg) * turns / 4,
                    ModKind::Vulnerability => {
                        let party: u64 = (0..self.heroes.len())
                            .filter(|&a| self.heroes[a].alive)
                            .map(|a| self.damage_on(a, i))
                            .sum();
                        party * avg / BP as u64 * turns
                    }
                    ModKind::Haste
                    | ModKind::Empowerment
                    | ModKind::Protection
                    | ModKind::Resistance => 0,
                }
            }
            EffectAction::Heal => 0,
        }
    }

    /// Hero `a`'s best expected damage x 400 on monster `i` (weapon or a ready damage skill).
    fn damage_on(&self, a: usize, i: usize) -> u64 {
        let fighter = &self.party[self.heroes[a].fighter];
        std::iter::once(None)
            .chain(
                (0..fighter.skills.len())
                    .filter(|&k| fighter.skills[k].stats.blocks.purpose == Purpose::Damage)
                    .map(Some),
            )
            .map(|k| {
                let stats = k.map(|k| fighter.skills[k].stats);
                let foe = &self.foes[i];
                let input = HitInput {
                    attacker: player_attacker(
                        self.cfg,
                        0,
                        &fighter.hero,
                        stats.as_ref(),
                        &self.hero_mods(a, k.is_none()),
                        &self.heroes[a].accuracy,
                    ),
                    defender: monster_defender(
                        &foe.profile,
                        &foe.m.states,
                        &self.foe_mods(i, false),
                    ),
                };
                mid(preview_range(&input, &self.cfg.combat))
                    * faces(input.attacker.attack_bonus, input.defender.defense)
                    * 20
            })
            .max()
            .unwrap_or(0)
    }

    /// Hero `a`'s best expected damage on the nearest living monster.
    fn best_damage(&self, a: usize) -> u64 {
        let at = self.heroes[a].cell;
        (0..self.foes.len())
            .filter(|&i| self.foes[i].m.alive)
            .min_by_key(|&i| deadroll_rules::units::chebyshev(at, self.foe_cell(i)))
            .map_or(0, |i| self.damage_on(a, i))
    }

    /// Expected damage x 400 of monster `i` on the living hero it hurts most.
    fn threat_of(&self, i: usize) -> u64 {
        (0..self.heroes.len())
            .filter(|&a| self.heroes[a].alive)
            .map(|a| self.foe_damage_on(i, a))
            .max()
            .unwrap_or(0)
    }

    /// Expected damage x 400 on hero `a` from the living monster that hurts it most.
    fn threat_on(&self, a: usize) -> u64 {
        (0..self.foes.len())
            .filter(|&i| self.foes[i].m.alive)
            .map(|i| self.foe_damage_on(i, a))
            .max()
            .unwrap_or(0)
    }

    fn foe_damage_on(&self, i: usize, a: usize) -> u64 {
        let foe = &self.foes[i];
        let h = &self.heroes[a];
        let input = HitInput {
            attacker: monster_attacker(self.cfg, 0, &foe.profile, None, &self.foe_mods(i, true)),
            defender: player_defender(
                self.cfg,
                &self.party[h.fighter].hero,
                &h.states,
                &self.hero_mods(a, false),
            ),
        };
        mid(preview_range(&input, &self.cfg.combat))
            * faces(input.attacker.attack_bonus, input.defender.defense)
            * 20
    }

    // -----------------------------------------------------------------------------------------
    // Monsters

    pub(crate) fn foe_turn(&mut self, i: usize, ap: u8) {
        let from = self.foe_cell(i);
        let dist = self.distances(from);
        let unit = |cell, hp, hp_max, alive, states, mods, kin| AiUnit {
            cell,
            hp,
            hp_max,
            alive,
            states,
            mods,
            kin,
        };
        let heroes: Vec<AiUnit> = self
            .heroes
            .iter()
            .map(|h| unit(h.cell, h.hp, h.hp_max, h.alive, h.states, h.mods, NO_KIN))
            .collect();
        let monsters: Vec<AiUnit> = (0..self.foes.len())
            .map(|j| {
                let f = &self.foes[j];
                unit(
                    self.foe_cell(j),
                    f.m.hp,
                    f.m.hp_max,
                    f.m.alive,
                    f.m.states,
                    f.mods,
                    f.profile.kin,
                )
            })
            .collect();
        let foe = &self.foes[i];
        let me = AiMonster {
            index: i as u8,
            ap,
            profile: &foe.profile,
            skills: &foe.skills,
            cooldowns: &foe.cooldowns,
        };
        if let Some(choice) = monster_plan(self.cfg, &me, &heroes, &monsters) {
            let (to, ally) = match choice.target {
                AiTarget::Player(s) => (self.heroes[s as usize].cell, false),
                AiTarget::Monster(j) => (self.foe_cell(j as usize), true),
            };
            let players = &self.cfg.players;
            let reach = |c: Cell| {
                if ally {
                    in_ally_range(choice.range, c, to, players)
                } else {
                    in_range(choice.range, c, to, players)
                }
            };
            if let Some((cell, _)) = self.best_cell(&dist, ap - choice.ap, reach) {
                self.move_foe(i, cell);
                if self.foes[i].m.alive && self.ended().is_none() {
                    self.foe_act(i, choice);
                }
                return;
            }
        }
        let cells: Vec<Cell> = (0..self.heroes.len())
            .filter(|&a| self.heroes[a].alive)
            .map(|a| self.heroes[a].cell)
            .collect();
        let to = self.approach(from, &dist, ap, &cells);
        self.move_foe(i, to);
    }
}
