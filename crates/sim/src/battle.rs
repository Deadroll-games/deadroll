//! One whole fight. The turn flow is the session program's, every rule call goes through
//! `deadroll_rules` (the same code the program runs):
//! - initiative from one roll stream (`turns::begin`), players by slot then monsters by index;
//! - each turn: `start_of_turn` (over-time damage, Freeze / Stun skip, durations, cooldowns),
//!   AP from speed with Haste / Slow, moves, at most one attack or active skill;
//! - an action resolves like `consume_randomness` (`deadroll_rules::resolve`).
//!
//! Dice: one seeded stream per fight hands out one seed per action (the VRF value on-chain).
//! Ends: a boss dies (or every monster, when there is no boss) = victory, every hero dead =
//! defeat, `max_rounds` = draw.
use deadroll_combat::{
    speed_ap, HitInput, HitResult, Outcome as HitOutcome, RollSource, SeededRoll, States, TimedMods,
};
use deadroll_rules::{
    input::hero,
    input::{effect_action, EffectAction},
    monster::monster_ap,
    resolve,
    units::{cooldown_after_use, roll_initiative},
    BalanceConfig, MonsterProfile, Unit,
};
use deadroll_skillcheck::ACTIVE_MAX;
use deadroll_types::{ActiveState, Cell, Monster, BP, PARTY_MAX, STATES_MAX};

use crate::setup::{Fight, Fighter};

/// Knobs of a fight that are not game rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// A fight still going after this many rounds is a draw.
    pub max_rounds: u16,
    /// Heroes heal an ally below this share of its HP, percent.
    pub heal_below_pct: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_rounds: 60,
            heal_below_pct: 40,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Victory,
    Defeat,
    Draw,
}

/// What one hero did in one fight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotStats {
    /// Hit damage plus over-time damage from the hero's states.
    pub damage: u64,
    /// The part of hit damage that crits added.
    pub crit_extra: u64,
    /// Over-time damage (Burning, Bleeding) from the hero's states.
    pub dot_damage: u64,
    pub healed: u64,
    /// Own turns started alive (skipped ones included).
    pub turns: u32,
    pub attacks: u32,
    pub hits: u32,
    pub crits: u32,
    /// Debuffs and buffs that landed (skill effects on hits included).
    pub effects_landed: u32,
    pub died: bool,
}

/// What one fight did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleStats {
    pub outcome: Outcome,
    pub rounds: u16,
    pub slots: Vec<SlotStats>,
    /// Attacks each killed monster took before it died.
    pub kills: Vec<u16>,
    pub monster_turns: u32,
    /// Monster turns lost to Freeze / Stun.
    pub monster_turns_skipped: u32,
}

pub(crate) struct Hero {
    pub fighter: usize,
    pub cell: Cell,
    pub hp: u32,
    pub hp_max: u32,
    pub alive: bool,
    pub states: States,
    pub mods: TimedMods,
    pub cooldowns: [u8; ACTIVE_MAX],
    pub max_ap: u8,
    pub initiative: u16,
}

pub(crate) struct Foe {
    pub m: Monster,
    pub mods: TimedMods,
    pub profile: MonsterProfile,
    pub boss: bool,
    pub max_ap: u8,
    pub initiative: u16,
    pub attacks_taken: u16,
}

/// A hero's action this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// Weapon (`None`) or damage skill on a monster.
    Attack { skill: Option<usize>, target: usize },
    /// Debuff skill on a monster.
    Debuff { skill: usize, target: usize },
    /// Heal or buff skill on a hero (the caster included).
    Support { skill: usize, ally: usize },
}

pub(crate) struct Battle<'a> {
    pub cfg: &'a BalanceConfig,
    pub party: &'a [Fighter],
    pub settings: Settings,
    pub width: u8,
    pub height: u8,
    pub heroes: Vec<Hero>,
    pub foes: Vec<Foe>,
    rng: SeededRoll,
    input: Box<HitInput>,
    result: Box<HitResult>,
    stats: BattleStats,
    boss_dead: bool,
}

const NO_MODS: TimedMods = [deadroll_combat::TimedMod {
    kind: deadroll_combat::ModKind::Haste,
    aspect: deadroll_types::Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; deadroll_combat::MODS_MAX];

/// Runs one fight to its end.
pub fn fight(
    cfg: &BalanceConfig,
    party: &[Fighter],
    fight: &Fight,
    settings: Settings,
    seed: u64,
) -> BattleStats {
    let mut b = Battle::new(cfg, party, fight, settings, seed);
    b.run();
    b.stats
}

impl<'a> Battle<'a> {
    pub(crate) fn new(
        cfg: &'a BalanceConfig,
        party: &'a [Fighter],
        fight: &Fight,
        settings: Settings,
        seed: u64,
    ) -> Self {
        let party = &party[..party.len().min(PARTY_MAX)];
        let arena = fight.arena;
        let mid = arena.height / 2;
        let heroes = party
            .iter()
            .enumerate()
            .map(|(i, f)| {
                // Column 1, around the middle row: mid, mid - 1, mid + 1, mid - 2, ...
                let off = (i as i16 + 1) / 2 * if i % 2 == 1 { -1 } else { 1 };
                let y = (mid as i16 + off).clamp(0, arena.height as i16 - 1) as u8;
                Hero {
                    fighter: i,
                    cell: Cell { x: 1, y },
                    hp: f.hp_max,
                    hp_max: f.hp_max,
                    alive: true,
                    states: [ActiveState::default(); STATES_MAX],
                    mods: NO_MODS,
                    cooldowns: [0; ACTIVE_MAX],
                    max_ap: hero::ap(cfg, &f.hero),
                    initiative: hero::initiative(cfg, &f.hero),
                }
            })
            .collect();
        let first = 1 + arena.gap;
        let foes = fight
            .spawns
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let col = i / arena.height as usize;
                let row = i % arena.height as usize;
                // Rows from the middle outwards, like the heroes.
                let off = (row as i16 + 1) / 2 * if row % 2 == 1 { -1 } else { 1 };
                let y = (mid as i16 + off).rem_euclid(arena.height as i16) as u8;
                let hp = s.base.hp_at(s.level);
                Foe {
                    m: Monster {
                        bestiary_id: i as u8,
                        level: s.level,
                        x: first + col as u8,
                        y,
                        hp,
                        hp_max: hp,
                        alive: true,
                        ..Monster::default()
                    },
                    mods: NO_MODS,
                    profile: s.base.profile(s.level, &cfg.monsters),
                    boss: s.base.boss,
                    max_ap: monster_ap(s.base.speed, &cfg.monsters),
                    initiative: s.base.initiative,
                    attacks_taken: 0,
                }
            })
            .collect();
        Battle {
            cfg,
            party,
            settings,
            width: arena.width,
            height: arena.height,
            heroes,
            foes,
            rng: SeededRoll::new(seed),
            input: Box::default(),
            result: Box::default(),
            stats: BattleStats {
                outcome: Outcome::Draw,
                rounds: 0,
                slots: vec![SlotStats::default(); party.len()],
                kills: Vec::new(),
                monster_turns: 0,
                monster_turns_skipped: 0,
            },
            boss_dead: false,
        }
    }

    /// One seed per action, like one VRF value per action.
    pub(crate) fn action_rolls(&mut self) -> SeededRoll {
        SeededRoll::new(self.rng.next_u64())
    }

    fn run(&mut self) {
        let mut units: Vec<(u8, u16)> = Vec::new();
        for (slot, h) in self.heroes.iter().enumerate() {
            units.push((Unit::Player(slot as u8).id(), h.initiative));
        }
        for (i, f) in self.foes.iter().enumerate() {
            units.push((Unit::Monster(i as u8).id(), f.initiative));
        }
        let order = roll_initiative(&units, &mut self.action_rolls());
        let len = order.len as usize;
        let mut round: u16 = 1;
        let mut index = 0;
        loop {
            if round > self.settings.max_rounds {
                self.stats.outcome = Outcome::Draw;
                self.stats.rounds = self.settings.max_rounds;
                return;
            }
            if let Some(unit) = Unit::from_id(order.order[index]) {
                if self.alive(unit) {
                    self.turn(unit);
                    if let Some(outcome) = self.ended() {
                        self.stats.outcome = outcome;
                        self.stats.rounds = round;
                        return;
                    }
                }
            }
            index += 1;
            if index == len {
                index = 0;
                round += 1;
            }
        }
    }

    pub(crate) fn alive(&self, unit: Unit) -> bool {
        match unit {
            Unit::Player(s) => self.heroes.get(s as usize).is_some_and(|h| h.alive),
            Unit::Monster(i) => self.foes.get(i as usize).is_some_and(|f| f.m.alive),
        }
    }

    fn ended(&self) -> Option<Outcome> {
        if self.heroes.iter().all(|h| !h.alive) {
            Some(Outcome::Defeat)
        } else if self.boss_dead || self.foes.iter().all(|f| !f.m.alive) {
            Some(Outcome::Victory)
        } else {
            None
        }
    }

    fn turn(&mut self, unit: Unit) {
        match unit {
            Unit::Player(s) => {
                let s = s as usize;
                let h = &mut self.heroes[s];
                let start = deadroll_rules::start_of_turn(
                    &mut h.states,
                    &mut h.mods,
                    &mut h.cooldowns,
                    h.hp,
                );
                h.hp = start.hp;
                self.stats.slots[s].turns += 1;
                self.credit_dot(start.dot_source, start.dot_damage);
                if start.died {
                    self.hero_died(s);
                    return;
                }
                if start.skip.is_some() {
                    return;
                }
                let ap = speed_ap(self.heroes[s].max_ap, &self.heroes[s].mods);
                self.hero_turn(s, ap);
            }
            Unit::Monster(i) => {
                let i = i as usize;
                let f = &mut self.foes[i];
                let mut cooldowns = [0u8; 0];
                let start = deadroll_rules::start_of_turn(
                    &mut f.m.states,
                    &mut f.mods,
                    &mut cooldowns,
                    f.m.hp,
                );
                f.m.hp = start.hp;
                self.stats.monster_turns += 1;
                self.credit_dot(start.dot_source, start.dot_damage);
                if start.died {
                    self.foe_died(i);
                    return;
                }
                if start.skip.is_some() {
                    self.stats.monster_turns_skipped += 1;
                    return;
                }
                let ap = speed_ap(self.foes[i].max_ap, &self.foes[i].mods);
                self.foe_turn(i, ap);
            }
        }
    }

    fn credit_dot(&mut self, source: u8, damage: u32) {
        if let Some(Unit::Player(s)) = Unit::from_id(source) {
            if let Some(slot) = self.stats.slots.get_mut(s as usize) {
                slot.damage += damage as u64;
                slot.dot_damage += damage as u64;
            }
        }
    }

    fn hero_died(&mut self, s: usize) {
        self.heroes[s].alive = false;
        self.heroes[s].hp = 0;
        self.stats.slots[s].died = true;
    }

    fn foe_died(&mut self, i: usize) {
        let f = &mut self.foes[i];
        f.m.alive = false;
        self.stats.kills.push(f.attacks_taken);
        if f.boss {
            self.boss_dead = true;
        }
    }

    /// Resolves a hero's action (the AI picked it and moved into range).
    pub(crate) fn hero_act(&mut self, s: usize, action: Action) {
        let cfg = self.cfg;
        let fighter = &self.party[self.heroes[s].fighter];
        let caster = Unit::Player(s as u8).id();
        let mut src = self.action_rolls();
        let skill_of = |k: usize| fighter.skills[k].stats;
        if let Some(k) = match action {
            Action::Attack { skill, .. } => skill,
            Action::Debuff { skill, .. } | Action::Support { skill, .. } => Some(skill),
        } {
            self.heroes[s].cooldowns[k] = cooldown_after_use(skill_of(k).cooldown);
        }
        match action {
            Action::Attack { skill, target } => {
                let skill = skill.map(skill_of);
                let hero_mods = self.heroes[s].mods;
                let foe = &mut self.foes[target];
                foe.attacks_taken = foe.attacks_taken.saturating_add(1);
                let out = resolve::attack_monster(
                    cfg,
                    caster,
                    &fighter.hero,
                    skill.as_ref(),
                    &hero_mods,
                    &foe.profile,
                    &mut foe.m,
                    &mut foe.mods,
                    &mut self.input,
                    &mut self.result,
                    &mut src,
                );
                let slot = &mut self.stats.slots[s];
                slot.attacks += 1;
                if self.result.outcome == HitOutcome::Hit {
                    let damage = self.result.damage as u64;
                    slot.hits += 1;
                    slot.damage += damage;
                    if self.result.crit {
                        slot.crits += 1;
                        let more = cfg.combat.crit_more_bp as u64;
                        slot.crit_extra += damage - damage * BP as u64 / (BP as u64 + more);
                    }
                }
                if out.rider.is_some_and(|r| r.roll.outcome == HitOutcome::Hit) {
                    slot.effects_landed += 1;
                }
                if out.died {
                    self.foe_died(target);
                }
            }
            Action::Debuff { skill, target } => {
                let stats = skill_of(skill);
                let Some(effect) = effect_action(stats.blocks.effect) else {
                    return;
                };
                let foe = &mut self.foes[target];
                let out = resolve::debuff_monster(
                    cfg,
                    caster,
                    &fighter.hero,
                    &stats,
                    effect,
                    &foe.profile,
                    &mut foe.m,
                    &mut foe.mods,
                    &mut src,
                );
                if out.roll.outcome == HitOutcome::Hit {
                    self.stats.slots[s].effects_landed += 1;
                }
            }
            Action::Support { skill, ally } => {
                let stats = skill_of(skill);
                let Some(effect) = effect_action(stats.blocks.effect) else {
                    return;
                };
                let a = &mut self.heroes[ally];
                let before = a.hp;
                let out = resolve::support_ally(
                    cfg,
                    caster,
                    &fighter.hero,
                    &stats,
                    effect,
                    &mut a.hp,
                    a.hp_max,
                    &mut a.states,
                    &mut a.mods,
                    &mut src,
                );
                let slot = &mut self.stats.slots[s];
                slot.healed += (a.hp - before) as u64;
                match effect {
                    EffectAction::Heal => {}
                    EffectAction::Mod(_) | EffectAction::Stun | EffectAction::Freeze => {
                        if out.roll.outcome == HitOutcome::Hit {
                            slot.effects_landed += 1;
                        }
                    }
                }
            }
        }
    }

    /// Resolves a monster's natural attack on hero `s`.
    pub(crate) fn foe_attack(&mut self, i: usize, s: usize) {
        let cfg = self.cfg;
        let mut src = self.action_rolls();
        let foe = &self.foes[i];
        let fighter = &self.party[self.heroes[s].fighter];
        let h = &mut self.heroes[s];
        let hero_mods = h.mods;
        let died = resolve::monster_attack(
            cfg,
            Unit::Monster(i as u8).id(),
            &foe.profile,
            &foe.mods,
            &fighter.hero,
            &mut h.hp,
            &mut h.states,
            &hero_mods,
            &mut self.input,
            &mut self.result,
            &mut src,
        );
        if died {
            self.hero_died(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use deadroll_combat::ModKind;
    use deadroll_rules::{monster::p0, HeroStats, SkillStats};
    use deadroll_skillcheck::{Category, Effect, Purpose, SkillBlocks, Target, Trigger};
    use deadroll_types::{Aspect, Class, Form, Race, Source, StateKind};

    use super::*;
    use crate::{
        presets::Arena,
        setup::{Skill, Spawn},
    };

    const CFG: &BalanceConfig = &BalanceConfig::DEFAULT;

    fn blocks(purpose: Purpose, source: Source, effect: Effect, aspect: Aspect) -> SkillBlocks {
        SkillBlocks {
            category: Category::Active,
            purpose,
            source,
            form: Form::Projectile,
            target: Target::Single,
            aspects: [aspect, Aspect::default(), Aspect::default()],
            aspect_count: 1,
            effect,
            trigger: Trigger::None,
            aura_radius: 0,
        }
    }

    fn hero(class: Class, skills: Vec<SkillBlocks>) -> Fighter {
        let hero = HeroStats {
            race: Race::Human,
            class,
            level: 1,
            stats: [8, 8, 8, 6],
            predisposition: Aspect::Blunt,
            predisposition_tier: 14,
            equipment: deadroll_rules::items::starter_loadout(class, &CFG.items).equipment(),
        };
        Fighter {
            name: format!("{class:?}"),
            hp_max: hero::hp_max(CFG, &hero),
            hero,
            skills: skills
                .into_iter()
                .map(|b| Skill {
                    name: "s".into(),
                    stats: SkillStats::derive(b, &CFG.skills),
                })
                .collect(),
        }
    }

    fn goblins(n: usize) -> Fight {
        Fight {
            name: "goblins".into(),
            spawns: (0..n)
                .map(|_| Spawn {
                    name: "goblin".into(),
                    base: p0::monster_base(1).unwrap(),
                    level: 1,
                })
                .collect(),
            arena: Arena {
                width: 10,
                height: 6,
                gap: 3,
            },
        }
    }

    fn plan(b: &Battle, s: usize) -> Option<Action> {
        let ap = b.heroes[s].max_ap;
        let dist = b.distances(b.heroes[s].cell);
        b.plan(s, &dist, ap - CFG.players.act_ap).map(|(_, a)| a)
    }

    #[test]
    fn heals_an_ally_below_the_threshold_only() {
        let heal = blocks(Purpose::Buff, Source::Magical, Effect::Heal, Aspect::Blood);
        let party = [
            hero(Class::Support, vec![heal]),
            hero(Class::Warrior, vec![]),
        ];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let max = b.heroes[1].hp_max;
        b.heroes[1].hp = max * 39 / 100;
        assert_eq!(plan(&b, 0), Some(Action::Support { skill: 0, ally: 1 }));
        b.heroes[1].hp = max / 2;
        assert_ne!(plan(&b, 0), Some(Action::Support { skill: 0, ally: 1 }));
        // The threshold is a setting.
        b.settings.heal_below_pct = 60;
        assert_eq!(plan(&b, 0), Some(Action::Support { skill: 0, ally: 1 }));
    }

    #[test]
    fn a_skill_on_cooldown_is_not_used() {
        let expose = blocks(
            Purpose::Debuff,
            Source::Magical,
            Effect::Vulnerability,
            Aspect::Fire,
        );
        let party = [
            hero(Class::Support, vec![expose]),
            hero(Class::Warrior, vec![]),
        ];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        // Walk into range first: the goblin stands 4 columns away, projectiles reach 6.
        assert_eq!(
            plan(&b, 0),
            Some(Action::Debuff {
                skill: 0,
                target: 0
            })
        );
        b.hero_act(
            0,
            Action::Debuff {
                skill: 0,
                target: 0,
            },
        );
        let cooldown = party[0].skills[0].stats.cooldown;
        assert_eq!(b.heroes[0].cooldowns[0], cooldown_after_use(cooldown));
        assert!(matches!(
            plan(&b, 0),
            Some(Action::Attack { skill: None, .. })
        ));
        // A debuff the monster already carries is worth nothing.
        b.heroes[0].cooldowns[0] = 0;
        b.foes[0].mods[0] = deadroll_combat::TimedMod {
            kind: ModKind::Vulnerability,
            aspect: Aspect::Blood,
            turns: 2,
            power_bp: 3_000,
            source: 0,
        };
        assert!(matches!(
            plan(&b, 0),
            Some(Action::Attack { skill: None, .. })
        ));
    }

    #[test]
    fn freeze_and_stun_skip_a_monster_turn() {
        let party = [hero(Class::Warrior, vec![])];
        let fight = goblins(2);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        for (i, kind) in [(0usize, StateKind::Freeze), (1, StateKind::Stun)] {
            b.foes[i].m.states[0] = ActiveState {
                kind,
                turns: 1,
                power: 0,
                source: 0,
            };
            let at = (b.foes[i].m.x, b.foes[i].m.y);
            b.turn(Unit::Monster(i as u8));
            assert_eq!((b.foes[i].m.x, b.foes[i].m.y), at, "{kind:?}");
            // The state ran out at the start of that turn: the next one moves.
            b.turn(Unit::Monster(i as u8));
            assert_ne!((b.foes[i].m.x, b.foes[i].m.y), at, "{kind:?}");
        }
        assert_eq!(b.stats.monster_turns, 4);
        assert_eq!(b.stats.monster_turns_skipped, 2);
    }

    #[test]
    fn monsters_close_in_and_attack() {
        let party = [hero(Class::Warrior, vec![])];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 5);
        let hp = b.heroes[0].hp;
        for _ in 0..4 {
            b.turn(Unit::Monster(0));
        }
        let gap = deadroll_rules::units::chebyshev(
            b.heroes[0].cell,
            Cell {
                x: b.foes[0].m.x,
                y: b.foes[0].m.y,
            },
        );
        assert_eq!(gap, 1);
        assert!(b.heroes[0].hp < hp || b.heroes[0].states.iter().all(|s| s.turns == 0));
    }

    #[test]
    fn units_start_on_their_sides_without_overlap() {
        let party = [
            hero(Class::Warrior, vec![]),
            hero(Class::Mage, vec![]),
            hero(Class::Support, vec![]),
        ];
        let fight = goblins(9);
        let b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let mut cells: Vec<(u8, u8)> = b.heroes.iter().map(|h| (h.cell.x, h.cell.y)).collect();
        cells.extend(b.foes.iter().map(|f| (f.m.x, f.m.y)));
        let n = cells.len();
        cells.sort();
        cells.dedup();
        assert_eq!(cells.len(), n);
        assert!(b.heroes.iter().all(|h| h.cell.x == 1));
        assert!(b.foes.iter().all(|f| f.m.x >= 4 && f.m.x < 10 && f.m.y < 6));
    }
}
