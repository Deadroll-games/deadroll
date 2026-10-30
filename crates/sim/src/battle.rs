//! One whole fight. The turn flow is the session program's, every rule call goes through
//! `deadroll_rules` (the same code the program runs):
//! - arena update: the initiative timeline of the arena (`deadroll_rules::arena`): the side the
//!   approach lets move first starts at 0, each turn adds the unit's delay, the lowest time acts;
//!   an encounter's `then` packs are fought after it, each in its own arena, the heroes' HP,
//!   states, buffs and cooldowns carried over, the pack's experience paid when it is won;
//! - each turn: `start_of_turn` (over-time damage, Freeze / Stun skip, durations, cooldowns),
//!   AP from speed with Haste / Slow and the auras over the unit, moves, at most one attack or
//!   active skill, each at its own AP cost;
//! - an action resolves like `consume_randomness` (`deadroll_rules::resolve`): a line hits every
//!   unit of its side on it, then the reactive skills it triggered fire on the same dice, the
//!   heroes' in slot order, then the monsters' in index order;
//! - passive skills work all the time (`deadroll_rules::aura`), reactive ones trigger by
//!   themselves (`deadroll_rules::reactive`): on an enemy's hit, an HP drop (a hit or over-time
//!   damage), hard control, and an enemy coming next to the bearer. Monsters (patch 6) have the
//!   skills of their spawn: actives chosen by `deadroll_rules::monster_ai`, a passive aura from the
//!   start, a reactive skill.
//!
//! Dice: one seeded stream per fight hands out one seed per action (the VRF value on-chain).
//! Ends: a boss dies (or every monster of the last pack) = victory, every hero dead = defeat,
//! `max_rounds` (arena rounds, `ArenaTuning::round_time`) in one arena = draw.
use deadroll_combat::{
    apply_mod, chill_slow_bp, speed_ap, AccuracyBuff, Applied, HitInput, HitResult, OpKind,
    Outcome as HitOutcome, ResonanceTables, RollSource, SeededRoll, StateRoll, States, TimedMods,
};
use deadroll_rules::{
    arena::{
        delay, next_unit, on_pack_won, pack_xp, round_at, start_time, turn_delay, FirstSide,
        TimelineUnit,
    },
    aura::{
        monster_passive_aura, passive_aura, tick_auras, Aura, AuraField, MonsterAuraField,
        MonsterAuras, MonsterBearer, PartyAuras, PlayerAuras, PASSIVE_AURA, REACTIVE_AURA,
    },
    input::hero,
    input::{effect_action, EffectAction, SkillStats},
    line::{line_cells, line_ends, targets_on_line},
    monster::{kin_reaches, monster_ap, PASSIVE_SLOT, REACTIVE_SLOT},
    monster_ai::{AiChoice, AiTarget},
    reactive::{entered_next_to, hit_events, nearby, reaction, Event, Reaction},
    resolve,
    spread::{spread, spreads, step_into, BurningCell, Field, BURNING_CELLS_MAX},
    units::cooldown_after_use,
    BalanceConfig, Footprint, MonsterDefense, MonsterProfile, MonsterSkillStats, Unit,
};
use deadroll_skillcheck::{Purpose, Target, ACTIVE_MAX};
use deadroll_types::{
    ActiveState, Cell, Monster, Stat, StateKind, BP, MONSTER_SLOTS, PARTY_MAX, STATES_MAX,
};

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
    /// States that landed, indexed by `StateKind` (Added, Refreshed or Froze).
    pub states_landed: [u32; StateKind::COUNT],
    /// Resonances that triggered (their target condition met), pairs and triples.
    pub resonances: u32,
    pub triples: u32,
    /// Ops of those resonances that acted, indexed by `OpKind`.
    pub ops_acted: [u32; OpKind::COUNT as usize],
    /// States a resonance passed on to other monsters, knockbacks, cells set on fire, and
    /// units that stepped into a burning cell.
    pub jumps: u32,
    pub knockbacks: u32,
    pub cells_ignited: u32,
    pub burned_by_cells: u32,
    /// Reactive skills that triggered (heroes' and monsters').
    pub reactions: u32,
    /// Units a line hit beyond its chosen target.
    pub line_extra_hits: u32,
    /// At each hero's turn start: the other units (allies, or enemies of a debuff aura) its
    /// auras covered.
    pub aura_reach: u32,
    /// Patch 6. Monster active skills used, monster heals (HP restored), monster reactives.
    pub monster_skills: u32,
    pub monster_healed: u64,
    pub monster_reactions: u32,
    /// Arena update: arenas fought and won, the pack experience each living hero got, and the
    /// party's HP at the end out of its maximum.
    pub packs: u32,
    pub packs_won: u32,
    pub xp: u32,
    pub hp_left: u64,
    pub hp_max: u64,
}

impl BattleStats {
    /// Adds the stats of the next arena of the same run.
    fn absorb(&mut self, o: BattleStats) {
        self.outcome = o.outcome;
        self.rounds += o.rounds;
        for (a, b) in self.slots.iter_mut().zip(&o.slots) {
            a.damage += b.damage;
            a.crit_extra += b.crit_extra;
            a.dot_damage += b.dot_damage;
            a.healed += b.healed;
            a.turns += b.turns;
            a.attacks += b.attacks;
            a.hits += b.hits;
            a.crits += b.crits;
            a.effects_landed += b.effects_landed;
            a.died |= b.died;
        }
        self.kills.extend(o.kills);
        self.monster_turns += o.monster_turns;
        self.monster_turns_skipped += o.monster_turns_skipped;
        for (a, b) in self.states_landed.iter_mut().zip(o.states_landed) {
            *a += b;
        }
        self.resonances += o.resonances;
        self.triples += o.triples;
        for (a, b) in self.ops_acted.iter_mut().zip(o.ops_acted) {
            *a += b;
        }
        self.jumps += o.jumps;
        self.knockbacks += o.knockbacks;
        self.cells_ignited += o.cells_ignited;
        self.burned_by_cells += o.burned_by_cells;
        self.reactions += o.reactions;
        self.line_extra_hits += o.line_extra_hits;
        self.aura_reach += o.aura_reach;
        self.monster_skills += o.monster_skills;
        self.monster_healed += o.monster_healed;
        self.monster_reactions += o.monster_reactions;
        self.packs += o.packs;
        self.packs_won += o.packs_won;
        self.xp += o.xp;
        self.hp_left = o.hp_left;
        self.hp_max = o.hp_max;
    }
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
    /// From a resonance (Pale Flame).
    pub accuracy: AccuracyBuff,
    /// Own turn starts until the reactive skill can trigger again.
    pub reactive_cd: u8,
    /// The passive skill's permanent aura, a reactive skill's temporary one.
    pub auras: PlayerAuras,
}

pub(crate) struct Foe {
    pub m: Monster,
    pub defense: MonsterDefense,
    pub mods: TimedMods,
    pub profile: MonsterProfile,
    pub boss: bool,
    pub max_ap: u8,
    pub initiative: u16,
    pub attacks_taken: u16,
    /// Its spawn's skills by slot, their cooldowns (the reactive's included), its auras.
    pub skills: [Option<MonsterSkillStats>; MONSTER_SLOTS],
    pub cooldowns: [u8; MONSTER_SLOTS],
    pub auras: MonsterAuras,
    /// Patch B: cells per side (`MonsterTemplate::size`).
    pub size: u8,
}

/// A hero's action this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// Weapon (`None`) or damage skill on a monster (every monster of its line for a line).
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
    tables: ResonanceTables,
    cells: [BurningCell; BURNING_CELLS_MAX],
    round: u16,
    rng: SeededRoll,
    input: Box<HitInput>,
    result: Box<HitResult>,
    stats: BattleStats,
    boss_dead: bool,
    /// Arena update: who starts the timeline.
    first: FirstSide,
}

const NO_MODS: TimedMods = [deadroll_combat::TimedMod {
    kind: deadroll_combat::ModKind::Haste,
    aspect: deadroll_types::Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
    weights_pct: [0; deadroll_types::Aspect::COUNT],
}; deadroll_combat::MODS_MAX];

/// Every unit's auras as the rules see them (`AuraField` borrows it).
pub(crate) struct PartyView {
    auras: PartyAuras,
    cells: [Cell; PARTY_MAX],
    alive: u8,
    bearers: Vec<MonsterBearer>,
}

impl PartyView {
    pub(crate) fn field(&self) -> AuraField<'_> {
        AuraField {
            auras: &self.auras,
            cells: &self.cells,
            alive: self.alive,
            monsters: MonsterAuraField {
                bearers: &self.bearers,
            },
        }
    }
}

/// What happened to a hero in this action, for its reactive skill.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReactEvent {
    bearer: usize,
    event: Event,
    /// The monster that caused it.
    source: Option<usize>,
}

/// What happened to a monster in this action, for its reactive skill.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FoeEvent {
    bearer: usize,
    event: Event,
    /// The hero that caused it.
    source: Option<usize>,
}

/// Runs one fight to its end: the encounter's arena, then (arena update) each of its `then` packs
/// while the party wins, the heroes carried from one arena to the next.
pub fn fight(
    cfg: &BalanceConfig,
    party: &[Fighter],
    fight: &Fight,
    settings: Settings,
    seed: u64,
) -> BattleStats {
    let mut b = Battle::new(cfg, party, fight, settings, seed);
    b.run();
    b.close_arena();
    let (mut stats, mut heroes, mut over) = (b.stats, b.heroes, b.boss_dead);
    for (k, next) in fight.then.iter().enumerate() {
        if over || stats.outcome != Outcome::Victory {
            break;
        }
        let seed = seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut b = Battle::new(cfg, party, next, settings, seed);
        b.carry(heroes);
        b.run();
        b.close_arena();
        over = b.boss_dead;
        heroes = core::mem::take(&mut b.heroes);
        stats.absorb(b.stats);
    }
    stats
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
                // The passive skill works from the start (docs/SKILL_BUILDER.md 3.3).
                let mut auras = [Aura::default(); 2];
                if let Some(a) = f
                    .passive
                    .as_ref()
                    .and_then(|p| passive_aura(cfg, &f.hero, &p.stats))
                {
                    auras[PASSIVE_AURA] = a;
                }
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
                    accuracy: AccuracyBuff::default(),
                    reactive_cd: 0,
                    auras,
                }
            })
            .collect();
        let first = 1 + arena.gap;
        // Each monster takes the first free place, column by column from `first`, each column's
        // rows from the middle outwards like the heroes; a large monster needs its whole square
        // inside and free (patch B).
        let mut placed: Vec<Footprint> = Vec::new();
        let mut place = |size: u8| -> Cell {
            for x in first..arena.width {
                for row in 0..arena.height as i16 {
                    let off = (row + 1) / 2 * if row % 2 == 1 { -1 } else { 1 };
                    let y = (mid as i16 + off).rem_euclid(arena.height as i16) as u8;
                    let f = Footprint::new(Cell { x, y }, size);
                    if f.inside(arena.width, arena.height) && !placed.iter().any(|p| p.overlaps(&f))
                    {
                        placed.push(f);
                        return f.at;
                    }
                }
            }
            // No room: a stack in the corner (an encounter bigger than its arena).
            Cell {
                x: arena.width.saturating_sub(size.max(1)),
                y: 0,
            }
        };
        let foes = fight
            .spawns
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let t = &s.template;
                let Cell { x, y } = place(t.size);
                let hp = t.hp_at(s.level, s.flags, &cfg.monsters);
                let profile = t.profile(s.level, &cfg.monsters);
                // The passive skill works from the start, like `open_session` copies it.
                let mut auras = [Aura::default(); 2];
                if let Some(a) = s.skills[PASSIVE_SLOT]
                    .as_ref()
                    .and_then(|k| monster_passive_aura(cfg, &profile, k))
                {
                    auras[PASSIVE_AURA] = a;
                }
                Foe {
                    m: Monster {
                        bestiary_id: i as u8,
                        level: s.level,
                        x,
                        y,
                        hp,
                        hp_max: hp,
                        alive: true,
                        flags: s.flags,
                        ..Monster::default()
                    },
                    mods: NO_MODS,
                    defense: t.defense,
                    profile,
                    boss: t.is_boss(),
                    max_ap: monster_ap(t.speed, &cfg.monsters),
                    initiative: t.initiative,
                    attacks_taken: 0,
                    skills: s.skills,
                    cooldowns: [0; MONSTER_SLOTS],
                    auras,
                    size: t.size,
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
            tables: fight.tables,
            cells: [BurningCell::default(); BURNING_CELLS_MAX],
            round: 1,
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
                states_landed: [0; StateKind::COUNT],
                resonances: 0,
                triples: 0,
                ops_acted: [0; OpKind::COUNT as usize],
                jumps: 0,
                knockbacks: 0,
                cells_ignited: 0,
                burned_by_cells: 0,
                reactions: 0,
                line_extra_hits: 0,
                aura_reach: 0,
                monster_skills: 0,
                monster_healed: 0,
                monster_reactions: 0,
                packs: 1,
                packs_won: 0,
                xp: 0,
                hp_left: 0,
                hp_max: 0,
            },
            boss_dead: false,
            first: fight.first,
        }
    }

    /// The heroes of the arena before: everything carries over but the cell
    /// (docs/PARTY_AND_TURNS.md 4.1.3: HP, states, buffs, cooldowns).
    fn carry(&mut self, before: Vec<Hero>) {
        for (h, old) in self.heroes.iter_mut().zip(before) {
            let cell = h.cell;
            *h = Hero { cell, ..old };
        }
    }

    /// The end of an arena: a won one pays the pack's experience to every living hero
    /// (`deadroll_rules::arena::on_pack_won`), and the party's HP is noted.
    fn close_arena(&mut self) {
        if self.stats.outcome == Outcome::Victory {
            self.stats.packs_won = 1;
            let xp = pack_xp(self.foes.iter().map(|f| f.profile.xp));
            let alive = self
                .heroes
                .iter()
                .enumerate()
                .filter(|(_, h)| h.alive)
                .fold(0u8, |m, (s, _)| m | 1 << s);
            let paid = on_pack_won(xp, alive, &[BP as u16; PARTY_MAX]);
            self.stats.xp = paid.iter().copied().max().unwrap_or(0);
        }
        self.stats.hp_left = self.heroes.iter().map(|h| h.hp as u64).sum();
        self.stats.hp_max = self.heroes.iter().map(|h| h.hp_max as u64).sum();
    }

    /// One seed per action, like one VRF value per action.
    pub(crate) fn action_rolls(&mut self) -> SeededRoll {
        SeededRoll::new(self.rng.next_u64())
    }

    /// Every unit's auras over the units' cells.
    pub(crate) fn party_view(&self) -> PartyView {
        let mut v = PartyView {
            auras: [[Aura::default(); 2]; PARTY_MAX],
            cells: [Cell::default(); PARTY_MAX],
            alive: 0,
            bearers: (0..self.foes.len())
                .filter(|&i| self.foes[i].m.alive && MonsterBearer::has_aura(&self.foes[i].auras))
                .map(|i| MonsterBearer {
                    index: i as u8,
                    cell: self.foe_cell_of(i),
                    kin: self.foes[i].profile.kin,
                    auras: self.foes[i].auras,
                    size: self.foes[i].size,
                })
                .collect(),
        };
        for (s, h) in self.heroes.iter().enumerate().take(PARTY_MAX) {
            v.auras[s] = h.auras;
            v.cells[s] = h.cell;
            if h.alive {
                v.alive |= 1 << s;
            }
        }
        v
    }

    /// Hero `s`'s buffs and debuffs with the auras over it (`basic`: for its basic attack).
    pub(crate) fn hero_mods(&self, s: usize, basic: bool) -> TimedMods {
        let h = &self.heroes[s];
        self.party_view()
            .field()
            .hero_mods(&h.mods, s as u8, h.cell, basic)
    }

    /// Monster `i`'s buffs and debuffs with the auras over it (`basic`: for its natural attack).
    pub(crate) fn foe_mods(&self, i: usize, basic: bool) -> TimedMods {
        let f = &self.foes[i];
        self.party_view().field().monster_mods(
            &f.mods,
            i as u8,
            self.foe_fp(i),
            f.profile.kin,
            basic,
        )
    }

    /// The arena timeline (docs/PARTY_AND_TURNS.md 6, `deadroll_rules::arena`), as the session
    /// program runs it: the first side starts at 0, the other at its delay; the living unit with
    /// the lowest time acts, a tie to the higher Dexterity (a monster: its initiative).
    fn run(&mut self) {
        let t = self.cfg.arena;
        let party_first = self.first == FirstSide::Party;
        let mut line: Vec<TimelineUnit> = Vec::new();
        let mut initiative: Vec<u16> = Vec::new();
        for (slot, h) in self.heroes.iter().enumerate() {
            line.push(TimelineUnit {
                id: Unit::Player(slot as u8).id(),
                time: start_time(party_first, h.initiative, &t),
                tie: self.party[h.fighter].hero.stats[Stat::Dexterity as usize],
                alive: h.alive,
            });
            initiative.push(h.initiative);
        }
        for (i, f) in self.foes.iter().enumerate() {
            line.push(TimelineUnit {
                id: Unit::Monster(i as u8).id(),
                time: start_time(!party_first, f.initiative, &t),
                tie: f.initiative,
                alive: f.m.alive,
            });
            initiative.push(f.initiative);
        }
        loop {
            for u in line.iter_mut() {
                u.alive = Unit::from_id(u.id).is_some_and(|unit| self.alive(unit));
            }
            let Some(k) = next_unit(&line) else {
                self.stats.outcome = self.ended().unwrap_or(Outcome::Draw);
                return;
            };
            let round = round_at(line[k].time, &t);
            if round > self.settings.max_rounds {
                self.stats.outcome = Outcome::Draw;
                self.stats.rounds = self.settings.max_rounds;
                return;
            }
            self.round = round;
            let step = match Unit::from_id(line[k].id) {
                Some(unit) => turn_delay(initiative[k], &self.turn(unit), &t),
                None => delay(initiative[k], &t),
            };
            line[k].time = line[k].time.saturating_add(step);
            if let Some(outcome) = self.ended() {
                self.stats.outcome = outcome;
                self.stats.rounds = round;
                return;
            }
        }
    }

    pub(crate) fn alive(&self, unit: Unit) -> bool {
        match unit {
            Unit::Player(s) => self.heroes.get(s as usize).is_some_and(|h| h.alive),
            Unit::Monster(i) => self.foes.get(i as usize).is_some_and(|f| f.m.alive),
        }
    }

    pub(crate) fn ended(&self) -> Option<Outcome> {
        if self.heroes.iter().all(|h| !h.alive) {
            Some(Outcome::Defeat)
        } else if self.boss_dead || self.foes.iter().all(|f| !f.m.alive) {
            Some(Outcome::Victory)
        } else {
            None
        }
    }

    /// One unit's turn. Returns its buffs and debuffs with the auras over it right after the turn
    /// start's tick: the timeline takes the turn's delay under them, as `start_turn` does
    /// (`deadroll_rules::arena::turn_delay`).
    pub(crate) fn turn(&mut self, unit: Unit) -> TimedMods {
        match unit {
            Unit::Player(s) => {
                let s = s as usize;
                let h = &mut self.heroes[s];
                let before = h.hp;
                let start = deadroll_rules::start_of_turn_with(
                    &mut h.states,
                    &mut h.mods,
                    &mut h.cooldowns,
                    h.hp,
                    &self.cfg.combat,
                );
                h.hp = start.hp;
                h.accuracy.tick();
                // The reactive skill's cooldown and the temporary aura count own turns.
                h.reactive_cd = h.reactive_cd.saturating_sub(1);
                tick_auras(&mut h.auras);
                self.stats.slots[s].turns += 1;
                self.credit_dot(start.dot_source, start.dot_damage);
                let delay_mods = self.hero_mods(s, false);
                if start.died {
                    self.hero_died(s);
                    return delay_mods;
                }
                self.note_aura_reach(s);
                if start.hp < before {
                    let source = match Unit::from_id(start.dot_source) {
                        Some(Unit::Monster(i)) => Some(i as usize),
                        _ => None,
                    };
                    let events = self.hp_drop_events(s, before, start.hp, source);
                    let mut src = self.action_rolls();
                    self.react(&events, &mut src);
                }
                if start.skip.is_some() || !self.heroes[s].alive {
                    return delay_mods;
                }
                let chill = chill_slow_bp(&self.heroes[s].states, &self.cfg.combat);
                let ap = speed_ap(self.heroes[s].max_ap, &self.hero_mods(s, false), chill);
                self.hero_turn(s, ap);
                delay_mods
            }
            Unit::Monster(i) => {
                let i = i as usize;
                let f = &mut self.foes[i];
                let before = f.m.hp;
                // Skill cooldowns (the reactive's too) and the temporary aura count own turns.
                let start = deadroll_rules::start_of_turn_with(
                    &mut f.m.states,
                    &mut f.mods,
                    &mut f.cooldowns,
                    f.m.hp,
                    &self.cfg.combat,
                );
                f.m.hp = start.hp;
                tick_auras(&mut f.auras);
                self.stats.monster_turns += 1;
                self.credit_dot(start.dot_source, start.dot_damage);
                let delay_mods = self.foe_mods(i, false);
                if start.died {
                    self.foe_died(i);
                    return delay_mods;
                }
                if start.hp < before {
                    let source = match Unit::from_id(start.dot_source) {
                        Some(Unit::Player(s)) => Some(s as usize),
                        _ => None,
                    };
                    let events = self.foe_hp_drop_events(i, before, start.hp, source);
                    if events.iter().any(|e| self.foe_reacts(e)) {
                        let mut src = self.action_rolls();
                        self.react_foes(&events, &mut src);
                    }
                }
                if start.skip.is_some() || !self.foes[i].m.alive || self.ended().is_some() {
                    if start.skip.is_some() {
                        self.stats.monster_turns_skipped += 1;
                    }
                    return delay_mods;
                }
                let chill = chill_slow_bp(&self.foes[i].m.states, &self.cfg.combat);
                let ap = speed_ap(self.foes[i].max_ap, &self.foe_mods(i, false), chill);
                self.foe_turn(i, ap);
                delay_mods
            }
        }
    }

    /// The other units hero `s`'s auras cover now.
    fn note_aura_reach(&mut self, s: usize) {
        let h = &self.heroes[s];
        let mut reach = 0;
        for a in h.auras.iter().filter(|a| !a.is_empty() && !a.self_only) {
            if a.hostile {
                reach += self
                    .foes
                    .iter()
                    .filter(|f| f.m.alive && a.reaches(h.cell, Cell { x: f.m.x, y: f.m.y }))
                    .count();
            } else {
                reach += self
                    .heroes
                    .iter()
                    .enumerate()
                    .filter(|(o, x)| *o != s && x.alive && a.reaches(h.cell, x.cell))
                    .count();
            }
        }
        self.stats.aura_reach += reach as u32;
    }

    fn credit_dot(&mut self, source: u8, damage: u32) {
        if let Some(Unit::Player(s)) = Unit::from_id(source) {
            if let Some(slot) = self.stats.slots.get_mut(s as usize) {
                slot.damage += damage as u64;
                slot.dot_damage += damage as u64;
            }
        }
    }

    pub(crate) fn hero_died(&mut self, s: usize) {
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

    pub(crate) fn foe_cell_of(&self, i: usize) -> Cell {
        Cell {
            x: self.foes[i].m.x,
            y: self.foes[i].m.y,
        }
    }

    /// Patch B: the cells monster `i` stands on.
    pub(crate) fn foe_fp(&self, i: usize) -> Footprint {
        Footprint::new(self.foe_cell_of(i), self.foes[i].size)
    }

    /// The living monsters on the line of `skill` from a caster on `from` through monster
    /// `target`, in line order (the arena has no walls).
    pub(crate) fn line_foes(
        &self,
        from: impl Into<Footprint>,
        target: usize,
        skill: &SkillStats,
    ) -> Vec<usize> {
        let reach = deadroll_rules::units::reach(skill.ext.range, &self.cfg.players);
        let (w, h) = (self.width, self.height);
        let (from, through) = line_ends(from, self.foe_fp(target));
        let line = line_cells(from, through, reach, |c| c.x < w && c.y < h);
        let units: Vec<(u8, Footprint)> = (0..self.foes.len())
            .filter(|&i| self.foes[i].m.alive)
            .map(|i| (i as u8, self.foe_fp(i)))
            .collect();
        let (t, n) = targets_on_line(&line, units.iter().copied());
        t[..n].iter().map(|&i| i as usize).collect()
    }

    /// The living heroes but `skip` on the line of `skill` from a caster on `from` through hero
    /// `target`.
    fn heroes_on_line(
        &self,
        from: impl Into<Footprint>,
        target: usize,
        skip: Option<usize>,
        skill: &SkillStats,
    ) -> Vec<usize> {
        let reach = deadroll_rules::units::reach(skill.ext.range, &self.cfg.players);
        let (w, h) = (self.width, self.height);
        let (from, through) = line_ends(from, self.heroes[target].cell);
        let line = line_cells(from, through, reach, |c| c.x < w && c.y < h);
        let units: Vec<(u8, Cell)> = (0..self.heroes.len())
            .filter(|&a| Some(a) != skip && self.heroes[a].alive)
            .map(|a| (a as u8, self.heroes[a].cell))
            .collect();
        let (t, n) = targets_on_line(&line, units.iter().copied());
        t[..n].iter().map(|&a| a as usize).collect()
    }

    /// The living heroes but `s` on the line of `skill` from hero `s` through hero `ally`.
    pub(crate) fn line_heroes(&self, s: usize, ally: usize, skill: &SkillStats) -> Vec<usize> {
        if ally == s {
            return vec![s];
        }
        self.heroes_on_line(self.heroes[s].cell, ally, Some(s), skill)
    }

    /// Resolves a hero's action (the AI picked it and moved into range), then the reactions it
    /// triggered (monsters pushed next to heroes, then the monsters' own), on the same dice.
    pub(crate) fn hero_act(&mut self, s: usize, action: Action) {
        let party = self.party;
        let fighter = &party[self.heroes[s].fighter];
        let skill_of = |k: usize| fighter.skills[k].stats;
        let mut src = self.action_rolls();
        if let Some(k) = match action {
            Action::Attack { skill, .. } => skill,
            Action::Debuff { skill, .. } | Action::Support { skill, .. } => Some(skill),
        } {
            self.heroes[s].cooldowns[k] = cooldown_after_use(skill_of(k).cooldown);
        }
        match action {
            Action::Attack { skill, target } => {
                let skill = skill.map(skill_of);
                let (moves, foe_events) = self.strike(s, skill.as_ref(), target, &mut src);
                let events = self.moved_events(&moves);
                self.react(&events, &mut src);
                self.react_foes(&foe_events, &mut src);
            }
            Action::Debuff { skill, target } => {
                let stats = skill_of(skill);
                if let Some(effect) = effect_action(stats.blocks.effect) {
                    let foe_events = self.debuff(s, &stats, effect, target, &mut src);
                    self.react_foes(&foe_events, &mut src);
                }
            }
            Action::Support { skill, ally } => {
                let stats = skill_of(skill);
                if let Some(effect) = effect_action(stats.blocks.effect) {
                    self.buff(s, &stats, effect, ally, &mut src);
                }
            }
        }
    }

    /// Hero `s`'s weapon or damage skill on monster `target` (every monster of its line for a
    /// line). Returns the knockbacks (monster, from, to) and what the hits did to the monsters.
    fn strike(
        &mut self,
        s: usize,
        skill: Option<&SkillStats>,
        target: usize,
        src: &mut SeededRoll,
    ) -> (Vec<(usize, Cell, Cell)>, Vec<FoeEvent>) {
        let targets = match skill.filter(|k| k.blocks.target == Target::Line) {
            Some(k) => self.line_foes(self.heroes[s].cell, target, k),
            None => vec![target],
        };
        self.stats.line_extra_hits += targets.len().saturating_sub(1) as u32;
        let mut moves = Vec::new();
        let mut events = Vec::new();
        for i in targets {
            if self.ended().is_some() || !self.foes[i].m.alive {
                continue;
            }
            let before = self.foes[i].m.hp;
            let moved = self.hit_foe(s, skill, i, src);
            if let Some((from, to)) = moved {
                moves.push((i, from, to));
            }
            events.extend(self.foe_hit_events(i, before, s));
        }
        (moves, events)
    }

    /// One hit of hero `s` on monster `target`, then its spread.
    fn hit_foe(
        &mut self,
        s: usize,
        skill: Option<&SkillStats>,
        target: usize,
        src: &mut SeededRoll,
    ) -> Option<(Cell, Cell)> {
        let cfg = self.cfg;
        let party = self.party;
        let fighter = &party[self.heroes[s].fighter];
        let caster = Unit::Player(s as u8).id();
        let hero_mods = self.hero_mods(s, skill.is_none());
        let view = self.foe_mods(target, false);
        let mut accuracy = self.heroes[s].accuracy;
        let foe = &mut self.foes[target];
        foe.attacks_taken = foe.attacks_taken.saturating_add(1);
        let out = resolve::attack_monster(
            cfg,
            &self.tables,
            caster,
            &fighter.hero,
            skill,
            &hero_mods,
            &mut accuracy,
            &foe.profile,
            &mut foe.m,
            &mut foe.mods,
            &view,
            &mut self.input,
            &mut self.result,
            src,
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
        self.heroes[s].accuracy = accuracy;
        note_states(&mut self.stats, &self.result);
        if out.died {
            self.foe_died(target);
        }
        self.spread(s, target, caster)
    }

    /// Hero `s`'s debuff on monster `target` (every monster of its line for a line). Returns
    /// the monsters it froze or stunned.
    fn debuff(
        &mut self,
        s: usize,
        stats: &SkillStats,
        effect: EffectAction,
        target: usize,
        src: &mut SeededRoll,
    ) -> Vec<FoeEvent> {
        let targets = if stats.blocks.target == Target::Line {
            self.line_foes(self.heroes[s].cell, target, stats)
        } else {
            vec![target]
        };
        self.stats.line_extra_hits += targets.len().saturating_sub(1) as u32;
        let caster = Unit::Player(s as u8).id();
        let party = self.party;
        let fighter = &party[self.heroes[s].fighter];
        let mut events = Vec::new();
        for i in targets {
            let foe = &mut self.foes[i];
            if !foe.m.alive {
                continue;
            }
            let out = resolve::debuff_monster(
                self.cfg,
                caster,
                &fighter.hero,
                stats,
                effect,
                &foe.profile,
                &mut foe.m,
                &mut foe.mods,
                src,
            );
            if out.roll.outcome == HitOutcome::Hit {
                self.stats.slots[s].effects_landed += 1;
            }
            if controls(effect, out.applied) {
                events.push(FoeEvent {
                    bearer: i,
                    event: Event::Controlled,
                    source: Some(s),
                });
            }
        }
        events
    }

    /// Hero `s`'s heal or buff on hero `ally` (every ally of its line for a line).
    fn buff(
        &mut self,
        s: usize,
        stats: &SkillStats,
        effect: EffectAction,
        ally: usize,
        src: &mut SeededRoll,
    ) {
        let targets = if stats.blocks.target == Target::Line {
            self.line_heroes(s, ally, stats)
        } else {
            vec![ally]
        };
        self.stats.line_extra_hits += targets.len().saturating_sub(1) as u32;
        let caster = Unit::Player(s as u8).id();
        let party = self.party;
        let fighter = &party[self.heroes[s].fighter];
        for a in targets {
            let h = &mut self.heroes[a];
            let before = h.hp;
            let out = resolve::support_ally(
                self.cfg,
                caster,
                &fighter.hero,
                stats,
                effect,
                a == s,
                &mut h.hp,
                h.hp_max,
                &mut h.states,
                &mut h.mods,
                src,
            );
            let slot = &mut self.stats.slots[s];
            slot.healed += (h.hp - before) as u64;
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

    /// Monster `i`'s action the AI chose (it moved into reach), then the reactions to it.
    pub(crate) fn foe_act(&mut self, i: usize, c: AiChoice) {
        let skill = c.slot.and_then(|k| self.foes[i].skills[k as usize]);
        if let (Some(k), Some(s)) = (c.slot, skill.as_ref()) {
            self.foes[i].cooldowns[k as usize] = cooldown_after_use(s.stats.cooldown);
            self.stats.monster_skills += 1;
        }
        let mut src = self.action_rolls();
        match c.target {
            AiTarget::Player(slot) => {
                let effect = skill.and_then(|s| effect_action(s.stats.blocks.effect));
                let events = match (skill.as_ref(), effect) {
                    (Some(s), Some(effect)) if s.stats.blocks.purpose == Purpose::Debuff => {
                        self.foe_debuff(i, s, effect, slot as usize, &mut src)
                    }
                    _ => self.foe_strike(i, skill.as_ref(), slot as usize, &mut src),
                };
                self.react(&events, &mut src);
            }
            AiTarget::Monster(ally) => {
                let effect = skill.and_then(|s| effect_action(s.stats.blocks.effect));
                if let (Some(s), Some(effect)) = (skill.as_ref(), effect) {
                    self.foe_support(i, s, effect, ally as usize, &mut src);
                }
            }
        }
    }

    /// Resolves a monster's natural attack on hero `s`, then the reactions to it.
    #[cfg(test)]
    pub(crate) fn foe_attack(&mut self, i: usize, s: usize) {
        let mut src = self.action_rolls();
        let events = self.foe_strike(i, None, s, &mut src);
        self.react(&events, &mut src);
    }

    /// Monster `i`'s natural attack (`skill = None`) or damage skill on hero `target` (every hero
    /// of its line for a line). Returns what it did to the heroes, for their reactives.
    fn foe_strike(
        &mut self,
        i: usize,
        skill: Option<&MonsterSkillStats>,
        target: usize,
        src: &mut SeededRoll,
    ) -> Vec<ReactEvent> {
        let from = self.foe_fp(i);
        let targets = match skill.filter(|k| k.stats.blocks.target == Target::Line) {
            Some(k) => self.heroes_on_line(from, target, None, &k.stats),
            None => vec![target],
        };
        self.stats.line_extra_hits += targets.len().saturating_sub(1) as u32;
        let mut events = Vec::new();
        for s in targets {
            if self.ended().is_some() || !self.heroes[s].alive {
                continue;
            }
            let cfg = self.cfg;
            let foe_mods = self.foe_mods(i, skill.is_none());
            let hero_mods = self.hero_mods(s, false);
            let foe = &self.foes[i];
            let party = self.party;
            let fighter = &party[self.heroes[s].fighter];
            let h = &mut self.heroes[s];
            let before = h.hp;
            let out = resolve::monster_attack(
                cfg,
                &self.tables,
                Unit::Monster(i as u8).id(),
                &foe.profile,
                skill.map(|k| &k.stats),
                &foe_mods,
                &fighter.hero,
                &mut h.hp,
                &mut h.states,
                &mut h.mods,
                &hero_mods,
                &mut self.input,
                &mut self.result,
                src,
            );
            let after = h.hp;
            let hp_max = h.hp_max;
            if out.died {
                self.hero_died(s);
            }
            note_states(&mut self.stats, &self.result);
            let rider_controls = out
                .rider
                .zip(skill.and_then(|k| effect_action(k.stats.blocks.effect)))
                .is_some_and(|(r, e)| controls(e, r.applied));
            events.extend(
                hit_events(&self.result, before, after, hp_max)
                    .into_iter()
                    .flatten()
                    .chain(rider_controls.then_some(Event::Controlled))
                    .map(|event| ReactEvent {
                        bearer: s,
                        event,
                        source: Some(i),
                    }),
            );
            if after < before {
                events.extend(
                    self.hp_drop_events(s, before, after, Some(i))
                        .into_iter()
                        .filter(|e| e.bearer != s),
                );
            }
        }
        events
    }

    /// Monster `i`'s debuff skill on hero `target` (every hero of its line for a line).
    fn foe_debuff(
        &mut self,
        i: usize,
        skill: &MonsterSkillStats,
        effect: EffectAction,
        target: usize,
        src: &mut SeededRoll,
    ) -> Vec<ReactEvent> {
        let from = self.foe_fp(i);
        let targets = if skill.stats.blocks.target == Target::Line {
            self.heroes_on_line(from, target, None, &skill.stats)
        } else {
            vec![target]
        };
        self.stats.line_extra_hits += targets.len().saturating_sub(1) as u32;
        let caster = Unit::Monster(i as u8).id();
        let mut events = Vec::new();
        for s in targets {
            if !self.heroes[s].alive {
                continue;
            }
            let view = self.hero_mods(s, false);
            let party = self.party;
            let fighter = &party[self.heroes[s].fighter];
            let profile = self.foes[i].profile;
            let h = &mut self.heroes[s];
            let out = resolve::monster_debuff_player(
                self.cfg,
                caster,
                &profile,
                &skill.stats,
                effect,
                &fighter.hero,
                h.hp,
                &mut h.states,
                &mut h.mods,
                &view,
                src,
            );
            if controls(effect, out.applied) {
                events.push(ReactEvent {
                    bearer: s,
                    event: Event::Controlled,
                    source: Some(i),
                });
            }
        }
        events
    }

    /// Monster `i`'s heal or buff on monster `ally` (every allied monster of its line for a line,
    /// itself left out; a kin-only skill skips other kins).
    fn foe_support(
        &mut self,
        i: usize,
        skill: &MonsterSkillStats,
        effect: EffectAction,
        ally: usize,
        src: &mut SeededRoll,
    ) {
        let from = self.foe_fp(i);
        let targets = if skill.stats.blocks.target == Target::Line && ally != i {
            self.line_foes(from, ally, &skill.stats)
                .into_iter()
                .filter(|&j| j != i)
                .collect()
        } else {
            vec![ally]
        };
        let kin = self.foes[i].profile.kin;
        let caster = Unit::Monster(i as u8).id();
        let profile = self.foes[i].profile;
        for j in targets {
            let f = &mut self.foes[j];
            if !f.m.alive || !kin_reaches(skill.kin_only, kin, f.profile.kin, j == i) {
                continue;
            }
            let before = f.m.hp;
            resolve::monster_support(
                self.cfg,
                caster,
                &profile,
                &skill.stats,
                effect,
                &mut f.m,
                &mut f.mods,
                src,
            );
            self.stats.monster_healed += (self.foes[j].m.hp - before) as u64;
        }
    }

    /// Hero `s`'s HP drop from `before` to `after`: its own event and its nearby allies'.
    pub(crate) fn hp_drop_events(
        &self,
        s: usize,
        before: u32,
        after: u32,
        source: Option<usize>,
    ) -> Vec<ReactEvent> {
        let hp_max = self.heroes[s].hp_max;
        let at = self.heroes[s].cell;
        let mut out = vec![ReactEvent {
            bearer: s,
            event: Event::HpDrop {
                before,
                after,
                hp_max,
            },
            source,
        }];
        for (b, h) in self.heroes.iter().enumerate() {
            if b != s && h.alive && nearby(self.cfg, h.cell, at) {
                out.push(ReactEvent {
                    bearer: b,
                    event: Event::AllyHpDrop {
                        ally: s as u8,
                        before,
                        after,
                        hp_max,
                    },
                    source,
                });
            }
        }
        out
    }

    /// What hero `s`'s last hit (`self.result`) did to monster `i` (HP `before` the hit).
    fn foe_hit_events(&self, i: usize, before: u32, s: usize) -> Vec<FoeEvent> {
        let f = &self.foes[i];
        let mut out: Vec<FoeEvent> = hit_events(&self.result, before, f.m.hp, f.m.hp_max)
            .into_iter()
            .flatten()
            .filter(|e| !matches!(e, Event::HpDrop { .. }))
            .map(|event| FoeEvent {
                bearer: i,
                event,
                source: Some(s),
            })
            .collect();
        if f.m.hp < before {
            out.extend(self.foe_hp_drop_events(i, before, f.m.hp, Some(s)));
        }
        out
    }

    /// Monster `i`'s HP drop: its own event and its nearby allied monsters'.
    fn foe_hp_drop_events(
        &self,
        i: usize,
        before: u32,
        after: u32,
        source: Option<usize>,
    ) -> Vec<FoeEvent> {
        let hp_max = self.foes[i].m.hp_max;
        let at = self.foe_fp(i);
        let mut out = vec![FoeEvent {
            bearer: i,
            event: Event::HpDrop {
                before,
                after,
                hp_max,
            },
            source,
        }];
        for (b, f) in self.foes.iter().enumerate() {
            if b != i && f.m.alive && nearby(self.cfg, self.foe_fp(b), at) {
                out.push(FoeEvent {
                    bearer: b,
                    event: Event::AllyHpDrop {
                        ally: i as u8,
                        before,
                        after,
                        hp_max,
                    },
                    source,
                });
            }
        }
        out
    }

    /// Heroes a monster came next to: monster, from, to.
    fn moved_events(&self, moves: &[(usize, Cell, Cell)]) -> Vec<ReactEvent> {
        let mut out = Vec::new();
        for &(i, from, to) in moves {
            for (b, h) in self.heroes.iter().enumerate() {
                let size = self.foes[i].size;
                let (from, to) = (Footprint::new(from, size), Footprint::new(to, size));
                if h.alive && entered_next_to(from, to, h.cell) {
                    out.push(ReactEvent {
                        bearer: b,
                        event: Event::EnemyEntered,
                        source: Some(i),
                    });
                }
            }
        }
        out
    }

    /// Every bearer's reaction to `events`, in hero slot order, each at most once.
    pub(crate) fn react(&mut self, events: &[ReactEvent], src: &mut SeededRoll) {
        let mut fired = 0u8;
        for b in 0..self.heroes.len() {
            for e in events.iter().filter(|e| e.bearer == b) {
                if fired & (1 << b) != 0 || self.ended().is_some() || !self.heroes[b].alive {
                    break;
                }
                if self.react_one(e, src) {
                    fired |= 1 << b;
                }
            }
        }
    }

    fn react_one(&mut self, e: &ReactEvent, src: &mut SeededRoll) -> bool {
        let b = e.bearer;
        let party = self.party;
        let fighter = &party[self.heroes[b].fighter];
        let Some(skill) = fighter.reactive.as_ref().map(|r| r.stats) else {
            return false;
        };
        let source = e
            .source
            .filter(|&i| self.foes[i].m.alive)
            .map(|i| (i as u8, self.foe_fp(i)));
        let ally_cell = match e.event {
            Event::AllyHpDrop { ally, .. } => Some(self.heroes[ally as usize].cell.into()),
            Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => {
                None
            }
        };
        let Some(r) = reaction(
            self.cfg,
            &skill,
            self.heroes[b].reactive_cd,
            b as u8,
            self.heroes[b].cell,
            &e.event,
            source,
            ally_cell,
        ) else {
            return false;
        };
        self.heroes[b].reactive_cd = cooldown_after_use(skill.cooldown);
        self.stats.reactions += 1;
        let effect = effect_action(skill.blocks.effect);
        match r {
            Reaction::Strike { monster } => {
                // What a reaction does triggers no further reactions.
                self.strike(b, Some(&skill), monster as usize, src);
            }
            Reaction::Debuff { monster } => {
                if let Some(effect) = effect {
                    self.debuff(b, &skill, effect, monster as usize, src);
                }
            }
            Reaction::Buff { slot } => {
                if let Some(effect) = effect {
                    self.buff(b, &skill, effect, slot as usize, src);
                }
            }
            Reaction::Aura => {
                let (_, aura) = resolve::start_aura(self.cfg, &fighter.hero, &skill, src);
                if let Some(a) = aura {
                    self.heroes[b].auras[REACTIVE_AURA] = a;
                    self.stats.slots[b].effects_landed += 1;
                }
            }
        }
        true
    }

    /// The reaction of monster `e.bearer`'s reactive skill to `e`, without acting on it.
    fn foe_reaction(&self, e: &FoeEvent) -> Option<(MonsterSkillStats, Reaction)> {
        let b = e.bearer;
        let f = &self.foes[b];
        let skill = f.skills[REACTIVE_SLOT]?;
        // A Dazed monster uses no reactive skill.
        if !f.m.alive || deadroll_combat::support::dazed(&f.mods) {
            return None;
        }
        let source = e
            .source
            .filter(|&s| self.heroes[s].alive)
            .map(|s| (s as u8, self.heroes[s].cell.into()));
        let ally_cell = match e.event {
            Event::AllyHpDrop { ally, .. } => {
                // A kin-only skill answers allies of its kin only.
                let a = &self.foes[ally as usize];
                if !kin_reaches(skill.kin_only, f.profile.kin, a.profile.kin, false) {
                    return None;
                }
                Some(self.foe_fp(ally as usize))
            }
            Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => {
                None
            }
        };
        let r = reaction(
            self.cfg,
            &skill.stats,
            f.cooldowns[REACTIVE_SLOT],
            b as u8,
            self.foe_fp(b),
            &e.event,
            source,
            ally_cell,
        )?;
        Some((skill, r))
    }

    /// Whether `e` would trigger its monster bearer's reactive (the program asks before it rolls).
    fn foe_reacts(&self, e: &FoeEvent) -> bool {
        self.foe_reaction(e).is_some()
    }

    /// Every monster bearer's reaction to `events`, in monster index order, each at most once.
    /// What a reaction does triggers no further reactions.
    pub(crate) fn react_foes(&mut self, events: &[FoeEvent], src: &mut SeededRoll) {
        for b in 0..self.foes.len() {
            for e in events.iter().filter(|e| e.bearer == b) {
                if self.ended().is_some() {
                    return;
                }
                let Some((skill, r)) = self.foe_reaction(e) else {
                    continue;
                };
                self.foes[b].cooldowns[REACTIVE_SLOT] = cooldown_after_use(skill.stats.cooldown);
                self.stats.reactions += 1;
                self.stats.monster_reactions += 1;
                let effect = effect_action(skill.stats.blocks.effect);
                match r {
                    Reaction::Strike { monster: slot } => {
                        self.foe_strike(b, Some(&skill), slot as usize, src);
                    }
                    Reaction::Debuff { monster: slot } => {
                        if let Some(effect) = effect {
                            self.foe_debuff(b, &skill, effect, slot as usize, src);
                        }
                    }
                    Reaction::Buff { slot: ally } => {
                        if let Some(effect) = effect {
                            self.foe_support(b, &skill, effect, ally as usize, src);
                        }
                    }
                    Reaction::Aura => {
                        let profile = self.foes[b].profile;
                        let (_, aura) =
                            resolve::monster_start_aura(self.cfg, &profile, &skill, src);
                        if let Some(a) = aura {
                            self.foes[b].auras[REACTIVE_AURA] = a;
                        }
                    }
                }
                break;
            }
        }
    }

    /// The jumps, knockback and burning cells of hero `s`'s last hit on monster `target`
    /// (`deadroll_rules::spread`, the code the program runs). Returns the knockback.
    fn spread(&mut self, s: usize, target: usize, caster: u8) -> Option<(Cell, Cell)> {
        let r = &self.result;
        if !spreads(r) {
            return None;
        }
        let mut monsters: Vec<Monster> = self.foes.iter().map(|f| f.m).collect();
        let defense: Vec<MonsterDefense> = self.foes.iter().map(|f| f.defense).collect();
        let field = OpenArena {
            width: self.width,
            height: self.height,
            units: self
                .heroes
                .iter()
                .filter(|h| h.alive)
                .map(|h| h.cell)
                .chain(
                    (0..self.foes.len())
                        .filter(|&i| monsters[i].alive)
                        .flat_map(|i| self.foe_fp(i).cells().collect::<Vec<_>>()),
                )
                .collect(),
        };
        let sizes: Vec<u8> = self.foes.iter().map(|f| f.size).collect();
        let out = spread(
            self.cfg,
            r,
            self.heroes[s].cell,
            target,
            &mut monsters,
            &sizes,
            &defense,
            &field,
            &mut self.cells,
            self.round,
            caster,
        );
        for (f, m) in self.foes.iter_mut().zip(monsters) {
            f.m = m;
        }
        for h in out.hits() {
            if h.applied != Applied::None {
                self.stats.states_landed[h.kind as usize] += 1;
            }
            for m in deadroll_rules::spread::modifiers_of(&self.result, h)
                .iter()
                .filter(|m| !m.is_empty())
            {
                apply_mod(&mut self.foes[h.monster as usize].mods, *m);
            }
        }
        for h in out.splashes() {
            self.stats.slots[s].damage += h.damage as u64;
            if h.died {
                self.foe_died(h.monster as usize);
            }
        }
        self.stats.jumps += out.hit_count as u32;
        self.stats.knockbacks += out.moved.is_some() as u32;
        self.stats.cells_ignited += out.ignited_count as u32;
        out.moved
    }

    /// Hero `s` moves to `cell`; a burning cell sets it on fire. The simulator moves in one
    /// jump, so only the cell it ends on counts (the program checks every step). Coming next to
    /// a monster triggers its `EnemyAdjacent` reactive on a roll of its own (`move_unit`).
    pub(crate) fn move_hero(&mut self, s: usize, cell: Cell) {
        let from = self.heroes[s].cell;
        let h = &mut self.heroes[s];
        if h.cell == cell {
            return;
        }
        h.cell = cell;
        if step_into(&self.cells, cell, self.round, &mut h.states, 0, self.cfg).is_some() {
            self.stats.burned_by_cells += 1;
        }
        let events: Vec<FoeEvent> = (0..self.foes.len())
            .filter(|&i| self.foes[i].m.alive && entered_next_to(from, cell, self.foe_fp(i)))
            .map(|i| FoeEvent {
                bearer: i,
                event: Event::EnemyEntered,
                source: Some(s),
            })
            .collect();
        if events.iter().any(|e| self.foe_reacts(e)) {
            let mut src = self.action_rolls();
            self.react_foes(&events, &mut src);
        }
    }

    /// Monster `i` moves to `cell`, as [`Self::move_hero`]. Coming next to a hero triggers its
    /// `EnemyAdjacent` reactive on a roll of its own (the program stops the walk on that step;
    /// the simulator moves in one jump).
    pub(crate) fn move_foe(&mut self, i: usize, cell: Cell) {
        let from = self.foe_cell_of(i);
        let f = &mut self.foes[i];
        if from == cell {
            return;
        }
        f.m.x = cell.x;
        f.m.y = cell.y;
        let immune = f.defense.immune_states;
        // Patch B: a large monster ends on every cell of its square; the first burning one counts.
        let burned = Footprint::new(cell, f.size).cells().any(|c| {
            step_into(
                &self.cells,
                c,
                self.round,
                &mut f.m.states,
                immune,
                self.cfg,
            )
            .is_some()
        });
        if burned {
            self.stats.burned_by_cells += 1;
        }
        let events = self.moved_events(&[(i, from, cell)]);
        if events.iter().any(|e| self.reacts(e)) {
            let mut src = self.action_rolls();
            self.react(&events, &mut src);
        }
    }

    /// Whether `e` would trigger its bearer's reactive (the program asks before it rolls).
    fn reacts(&self, e: &ReactEvent) -> bool {
        let h = &self.heroes[e.bearer];
        let Some(skill) = self.party[h.fighter].reactive.as_ref().map(|r| r.stats) else {
            return false;
        };
        let source = e
            .source
            .filter(|&i| self.foes[i].m.alive)
            .map(|i| (i as u8, self.foe_fp(i)));
        h.alive
            && reaction(
                self.cfg,
                &skill,
                h.reactive_cd,
                e.bearer as u8,
                h.cell,
                &e.event,
                source,
                None,
            )
            .is_some()
    }
}

/// Whether `effect` landing as `applied` put hard control (Freeze, Stun) on its target.
fn controls(effect: EffectAction, applied: Applied) -> bool {
    match effect {
        EffectAction::Stun | EffectAction::Freeze => applied != Applied::None,
        EffectAction::Heal | EffectAction::Mod(_) => false,
    }
}

/// The open arena as the spread sees it: no walls, living units block.
struct OpenArena {
    width: u8,
    height: u8,
    units: Vec<Cell>,
}

impl Field for OpenArena {
    fn passable(&self, c: Cell) -> bool {
        c.x < self.width && c.y < self.height
    }

    fn occupied(&self, c: Cell) -> bool {
        self.units.contains(&c)
    }
}

fn note_states(stats: &mut BattleStats, result: &HitResult) {
    let land = |counts: &mut [u32; StateKind::COUNT], roll: &StateRoll| {
        if roll.success && roll.applied != Applied::None {
            counts[roll.kind as usize] = counts[roll.kind as usize].saturating_add(1);
        }
    };
    for packet in result.packets() {
        land(&mut stats.states_landed, &packet.state);
    }
    for (slot, res) in result.resonances.iter().enumerate() {
        let Some(res) = res.filter(|r| r.active) else {
            continue;
        };
        stats.resonances += 1;
        stats.triples += res.second_tint.is_some() as u32;
        for op in result.ops(slot) {
            land(&mut stats.states_landed, &op.state);
            if op.acted {
                stats.ops_acted[op.kind as usize] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use deadroll_combat::ModKind;
    use deadroll_rules::{
        aura::{PASSIVE_AURA, REACTIVE_AURA},
        monster::REACTIVE_SLOT,
        HeroStats, SkillStats,
    };
    use deadroll_skillcheck::{Category, Effect, Purpose, SkillBlocks, SkillExt, Target, Trigger};
    use deadroll_types::{Aspect, Class, Form, Race, RangeTag, Source, StateKind};

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

    /// Projectiles shoot, strikes hit in melee (what the builder tests used to assume).
    fn ext_of(b: &SkillBlocks) -> SkillExt {
        SkillExt {
            range: match b.form {
                Form::Strike => RangeTag::Melee,
                Form::Projectile => RangeTag::Ranged,
            },
            ..SkillExt::MELEE
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
            class_tier: 14,
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
                    stats: SkillStats::derive(b, ext_of(&b), &CFG.skills),
                })
                .collect(),
            reactive: None,
            passive: None,
        }
    }

    fn goblins(n: usize) -> Fight {
        Fight {
            name: "goblins".into(),
            spawns: (0..n)
                .map(|_| Spawn {
                    name: "goblin".into(),
                    bestiary_id: 1,
                    template: registry().template(1).unwrap(),
                    level: 1,
                    flags: 0,
                    // The natural attack only.
                    skills: [None; deadroll_types::MONSTER_SLOTS],
                    skill_names: Default::default(),
                })
                .collect(),
            arena: Arena {
                width: 10,
                height: 6,
                gap: 3,
            },
            tables: registry().tables,
            first: FirstSide::Monsters,
            then: Vec::new(),
        }
    }

    fn registry() -> deadroll_rules::registry::Registry {
        deadroll_rules::registry::Registry::shipped().unwrap()
    }

    fn plan(b: &Battle, s: usize) -> Option<Action> {
        let ap = b.heroes[s].max_ap;
        let dist = b.distances(b.heroes[s].cell.into());
        b.plan(s, &dist, ap).map(|(_, a)| a)
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
            weights_pct: [0; Aspect::COUNT],
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

    #[test]
    fn a_large_monster_takes_its_whole_square_and_is_reached_by_any_cell() {
        // Patch B: the goblin chieftain (bestiary 2) is 2x2; goblins fill the rest around it.
        let party = [hero(Class::Warrior, vec![])];
        let mut fight = goblins(5);
        fight.spawns[0].bestiary_id = 2;
        fight.spawns[0].template = registry().template(2).unwrap();
        let b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        assert_eq!(b.foes[0].size, 2);
        let squares: Vec<Footprint> = (0..b.foes.len()).map(|i| b.foe_fp(i)).collect();
        for (i, f) in squares.iter().enumerate() {
            assert!(f.inside(b.width, b.height), "{f:?}");
            assert!(squares[..i].iter().all(|o| !o.overlaps(f)), "{f:?}");
        }
        // Next to its lower-right cell is melee reach.
        let boss = squares[0];
        let below = Cell {
            x: boss.at.x + 1,
            y: boss.at.y + 2,
        };
        assert!(deadroll_rules::units::in_range(
            deadroll_types::RangeTag::Melee,
            below,
            boss,
            &CFG.players
        ));
        // A hero can walk onto none of its cells.
        let dist = b.distances(b.heroes[0].cell.into());
        for c in boss.cells() {
            assert_eq!(
                dist[c.y as usize * b.width as usize + c.x as usize],
                u16::MAX
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // Patch 5: reactive and passive skills, auras, the line, action point costs.

    /// A starter preset as the fighter keeps it.
    fn preset(p: &deadroll_skillcheck::presets::Preset) -> Skill {
        Skill {
            name: p.name.into(),
            stats: SkillStats::derive(p.blocks, p.ext, &CFG.skills),
        }
    }

    fn with(
        mut f: Fighter,
        reactive: Option<&deadroll_skillcheck::presets::Preset>,
        passive: Option<&deadroll_skillcheck::presets::Preset>,
    ) -> Fighter {
        f.reactive = reactive.map(preset);
        f.passive = passive.map(preset);
        f
    }

    fn place_foe(b: &mut Battle, i: usize, at: Cell) {
        b.foes[i].m.x = at.x;
        b.foes[i].m.y = at.y;
    }

    #[test]
    fn riposte_answers_a_hit_and_waits_out_its_cooldown() {
        use deadroll_skillcheck::presets::WARRIOR;
        let party = [with(hero(Class::Warrior, vec![]), Some(&WARRIOR[4]), None)];
        let fight = goblins(1);
        let mut hit_once = false;
        for seed in 0..40 {
            let mut b = Battle::new(CFG, &party, &fight, Settings::default(), seed);
            let next = Cell {
                x: b.heroes[0].cell.x + 1,
                y: b.heroes[0].cell.y,
            };
            place_foe(&mut b, 0, next);
            b.foe_attack(0, 0);
            let landed = b.result.outcome == HitOutcome::Hit || b.stats.reactions > 0;
            if !landed {
                // A miss triggers nothing.
                assert_eq!(b.stats.reactions, 0);
                continue;
            }
            hit_once = true;
            assert_eq!(b.stats.reactions, 1, "seed {seed}");
            assert_eq!(b.foes[0].attacks_taken, 1, "the counterattack is a hit");
            let cooldown = party[0].reactive.as_ref().unwrap().stats.cooldown;
            assert_eq!(b.heroes[0].reactive_cd, cooldown_after_use(cooldown));
            // On cooldown: the next hit triggers nothing.
            if b.foes[0].m.alive && b.heroes[0].alive {
                b.foe_attack(0, 0);
                assert_eq!(b.stats.reactions, 1);
            }
        }
        assert!(hit_once);
    }

    #[test]
    fn a_melee_counterattack_does_not_reach_a_ranged_attacker() {
        use deadroll_skillcheck::presets::WARRIOR;
        let party = [with(hero(Class::Warrior, vec![]), Some(&WARRIOR[4]), None)];
        let fight = goblins(1);
        for seed in 0..20 {
            let mut b = Battle::new(CFG, &party, &fight, Settings::default(), seed);
            let far = Cell {
                x: b.heroes[0].cell.x + 3,
                y: b.heroes[0].cell.y,
            };
            place_foe(&mut b, 0, far);
            b.foe_attack(0, 0);
            assert_eq!(b.stats.reactions, 0);
            assert_eq!(
                b.heroes[0].reactive_cd, 0,
                "no reach: the cooldown is not spent"
            );
        }
    }

    #[test]
    fn cold_snap_answers_a_monster_coming_next_to_the_mage() {
        use deadroll_skillcheck::presets::MAGE;
        let party = [with(hero(Class::Mage, vec![]), Some(&MAGE[4]), None)];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 3);
        let at = b.heroes[0].cell;
        place_foe(
            &mut b,
            0,
            Cell {
                x: at.x + 3,
                y: at.y,
            },
        );
        // Moving around at a distance: nothing.
        b.move_foe(
            0,
            Cell {
                x: at.x + 2,
                y: at.y,
            },
        );
        assert_eq!(b.stats.reactions, 0);
        b.move_foe(
            0,
            Cell {
                x: at.x + 1,
                y: at.y,
            },
        );
        assert_eq!(b.stats.reactions, 1);
        // Staying next to it is not entering again.
        b.heroes[0].reactive_cd = 0;
        b.move_foe(
            0,
            Cell {
                x: at.x + 1,
                y: at.y + 1,
            },
        );
        assert_eq!(b.stats.reactions, 1);
    }

    #[test]
    fn a_passive_aura_covers_allies_in_its_radius_only() {
        use deadroll_skillcheck::presets::SUPPORT;
        let party = [
            with(hero(Class::Support, vec![]), None, Some(&SUPPORT[5])),
            hero(Class::Warrior, vec![]),
            hero(Class::Mage, vec![]),
        ];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let protection = |b: &Battle, s: usize| {
            let mods = b.hero_mods(s, false);
            deadroll_combat::support::find_mod(&mods, ModKind::Protection)
                .map_or(0, |i| mods[i].power_bp)
        };
        b.heroes[0].cell = Cell { x: 2, y: 2 };
        b.heroes[1].cell = Cell { x: 4, y: 3 };
        b.heroes[2].cell = Cell { x: 5, y: 2 };
        assert!(protection(&b, 0) > 0, "the bearer itself");
        assert_eq!(protection(&b, 1), protection(&b, 0), "two cells away");
        assert_eq!(protection(&b, 2), 0, "three cells away");
        // A dead bearer's aura is gone.
        b.hero_died(0);
        assert_eq!(protection(&b, 1), 0);
    }

    #[test]
    fn last_stand_starts_a_temporary_aura_when_hp_drops_below_its_threshold() {
        use deadroll_skillcheck::presets::SUPPORT;
        let party = [
            with(hero(Class::Support, vec![]), Some(&SUPPORT[4]), None),
            hero(Class::Warrior, vec![]),
        ];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        b.heroes[1].cell = Cell {
            x: b.heroes[0].cell.x + 1,
            y: b.heroes[0].cell.y,
        };
        let max = b.heroes[0].hp_max;
        let events = b.hp_drop_events(0, max, max / 2, Some(0));
        let mut src = SeededRoll::new(9);
        b.react(&events, &mut src);
        assert_eq!(b.stats.reactions, 0, "50% is not below 40%");
        let events = b.hp_drop_events(0, max / 2, max / 5, Some(0));
        b.heroes[0].hp = max / 5;
        b.react(&events, &mut src);
        assert_eq!(b.stats.reactions, 1);
        let aura = b.heroes[0].auras[REACTIVE_AURA];
        assert!(!aura.is_empty() && !aura.passive && aura.kind == ModKind::Protection);
        let mods = b.hero_mods(1, false);
        assert!(deadroll_combat::support::find_mod(&mods, ModKind::Protection).is_some());
        // It counts down with its bearer's turns.
        let turns = aura.turns;
        for _ in 0..turns {
            tick_auras(&mut b.heroes[0].auras);
        }
        assert!(b.heroes[0].auras[REACTIVE_AURA].is_empty());
    }

    #[test]
    fn a_line_hits_every_monster_on_it() {
        use deadroll_skillcheck::presets::MAGE;
        let arc = preset(&MAGE[3]).stats;
        let mut mage = hero(Class::Mage, vec![]);
        mage.skills = vec![Skill {
            name: "Arc".into(),
            stats: arc,
        }];
        let party = [mage];
        let fight = goblins(3);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let at = b.heroes[0].cell;
        for (i, dx) in [(0usize, 2u8), (1, 3), (2, 5)] {
            place_foe(
                &mut b,
                i,
                Cell {
                    x: at.x + dx,
                    y: at.y,
                },
            );
        }
        // Off the line: not hit.
        place_foe(
            &mut b,
            2,
            Cell {
                x: at.x + 2,
                y: at.y + 2,
            },
        );
        assert_eq!(b.line_foes(at, 0, &arc), vec![0, 1]);
        b.hero_act(
            0,
            Action::Attack {
                skill: Some(0),
                target: 0,
            },
        );
        assert_eq!(
            b.foes.iter().map(|f| f.attacks_taken).collect::<Vec<_>>(),
            vec![1, 1, 0]
        );
        assert_eq!(b.stats.line_extra_hits, 1);
    }

    #[test]
    fn a_skill_costs_its_own_action_points() {
        use deadroll_skillcheck::presets::WARRIOR;
        // Shield Bash (Stun) costs 3 AP; with 2 the warrior can only swing the sword.
        let bash = preset(&WARRIOR[1]).stats;
        assert_eq!(bash.ap_cost, 3);
        let mut warrior = hero(Class::Warrior, vec![]);
        warrior.skills = vec![Skill {
            name: "Bash".into(),
            stats: bash,
        }];
        let party = [warrior];
        let fight = goblins(1);
        let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let next = Cell {
            x: b.heroes[0].cell.x + 1,
            y: b.heroes[0].cell.y,
        };
        place_foe(&mut b, 0, next);
        let dist = b.distances(b.heroes[0].cell.into());
        assert!(matches!(
            b.plan(0, &dist, 2),
            Some((_, Action::Attack { skill: None, .. }))
        ));
        // With 3 AP the skill is a choice; the AI picks by expected value.
        assert!(b.plan(0, &dist, 3).is_some());
        let only_bash = {
            let mut f = party[0].clone();
            f.skills[0].stats.ap_cost = 2;
            f
        };
        let party2 = [only_bash];
        let mut b2 = Battle::new(CFG, &party2, &fight, Settings::default(), 1);
        place_foe(&mut b2, 0, next);
        let dist2 = b2.distances(b2.heroes[0].cell.into());
        assert!(b2.plan(0, &dist2, 2).is_some());
        assert_eq!(b.plan(0, &dist, 1), None);
    }

    // ---------------------------------------------------------------------------------------
    // Patch 6: monsters with their skills.

    /// Registry monsters by bestiary id, with their default skills (`setup::spawn`).
    fn pack(ids: &[(u8, bool)]) -> Fight {
        let reg = registry();
        Fight {
            name: "pack".into(),
            spawns: ids
                .iter()
                .map(|&(id, elite)| crate::setup::spawn(&reg, id, 1, elite, None, CFG).unwrap())
                .collect(),
            arena: Arena {
                width: 12,
                height: 8,
                gap: 6,
            },
            tables: reg.tables,
            first: FirstSide::Monsters,
            then: Vec::new(),
        }
    }

    const GOBLIN: u8 = 1;
    const CHIEFTAIN: u8 = 2;
    const SHAMAN: u8 = 4;
    const WOLF: u8 = 0;

    #[test]
    fn the_shaman_heals_a_hurt_goblin_but_not_a_wolf() {
        let party = [hero(Class::Warrior, vec![])];
        for (ally, healed) in [(GOBLIN, true), (WOLF, false)] {
            let fight = pack(&[(SHAMAN, false), (ally, false)]);
            let mut b = Battle::new(CFG, &party, &fight, Settings::default(), 3);
            place_foe(&mut b, 0, Cell { x: 8, y: 3 });
            place_foe(&mut b, 1, Cell { x: 9, y: 3 });
            b.foes[1].m.hp = 1;
            b.foe_turn(0, b.foes[0].max_ap);
            assert_eq!(b.foes[1].m.hp > 1, healed, "ally {ally}");
            assert_eq!(b.stats.monster_healed > 0, healed);
            // A skill was used: it waits out its cooldown.
            assert_eq!(b.stats.monster_skills, 1);
            assert!(b.foes[0].cooldowns.iter().any(|&c| c > 0));
        }
    }

    #[test]
    fn leader_works_from_the_start_and_battle_cry_answers_a_drop_below_half() {
        let party = [hero(Class::Warrior, vec![])];
        let fight = pack(&[(CHIEFTAIN, false), (GOBLIN, false)]);
        let b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        let leader = b.foes[0].auras[PASSIVE_AURA];
        assert!(!leader.is_empty() && leader.kin_only && leader.passive);
        let mut fired = false;
        for seed in 0..60 {
            let mut b = Battle::new(CFG, &party, &fight, Settings::default(), seed);
            let next = Cell {
                x: b.heroes[0].cell.x + 1,
                y: b.heroes[0].cell.y,
            };
            place_foe(&mut b, 0, next);
            // Just above half: a landed hit takes it below.
            b.foes[0].m.hp = b.foes[0].m.hp_max / 2 + 1;
            b.hero_act(
                0,
                Action::Attack {
                    skill: None,
                    target: 0,
                },
            );
            let f = &b.foes[0];
            if f.m.alive && f.m.hp * 2 < f.m.hp_max {
                assert_eq!(b.stats.monster_reactions, 1, "seed {seed}");
                assert!(!f.auras[REACTIVE_AURA].is_empty());
                assert!(f.cooldowns[REACTIVE_SLOT] > 0);
                fired = true;
            } else {
                assert_eq!(b.stats.monster_reactions, 0);
            }
        }
        assert!(fired);
    }

    #[test]
    fn a_dazed_monster_does_not_answer_with_its_reactive_skill() {
        let party = [hero(Class::Warrior, vec![])];
        let fight = pack(&[(CHIEFTAIN, false), (GOBLIN, false)]);
        let (mut reacted, mut dazed_reacted) = (0, 0);
        for seed in 0..60 {
            for dazed in [false, true] {
                let mut b = Battle::new(CFG, &party, &fight, Settings::default(), seed);
                let next = Cell {
                    x: b.heroes[0].cell.x + 1,
                    y: b.heroes[0].cell.y,
                };
                place_foe(&mut b, 0, next);
                b.foes[0].m.hp = b.foes[0].m.hp_max / 2 + 1;
                if dazed {
                    b.foes[0].mods[ModKind::Dazed as usize] = deadroll_combat::TimedMod {
                        kind: ModKind::Dazed,
                        turns: 3,
                        power_bp: 10_000,
                        ..Default::default()
                    };
                }
                b.hero_act(
                    0,
                    Action::Attack {
                        skill: None,
                        target: 0,
                    },
                );
                if dazed {
                    dazed_reacted += b.stats.monster_reactions;
                } else {
                    reacted += b.stats.monster_reactions;
                }
            }
        }
        assert!(reacted > 0, "a free chieftain reacts");
        assert_eq!(dazed_reacted, 0);
    }

    #[test]
    fn an_elite_has_more_hp_and_a_fourth_active() {
        let fight = pack(&[(WOLF, false), (WOLF, true)]);
        let (regular, elite) = (&fight.spawns[0], &fight.spawns[1]);
        let count = |s: &crate::setup::Spawn| s.skills[..5].iter().flatten().count();
        assert_eq!((count(regular), count(elite)), (3, 4));
        let party = [hero(Class::Warrior, vec![])];
        let b = Battle::new(CFG, &party, &fight, Settings::default(), 1);
        assert!(b.foes[1].m.hp_max > b.foes[0].m.hp_max);
    }

    #[test]
    fn monsters_fight_whole_encounters_with_their_skills() {
        let party = [hero(Class::Warrior, vec![]), hero(Class::Mage, vec![])];
        let fight = pack(&[
            (CHIEFTAIN, false),
            (GOBLIN, false),
            (SHAMAN, false),
            (WOLF, false),
        ]);
        let mut used = 0;
        for seed in 0..20 {
            let stats = fight_one(&party, &fight, seed);
            used += stats.monster_skills;
        }
        assert!(used > 0, "no monster ever used a skill");
    }

    fn fight_one(party: &[Fighter], fight: &Fight, seed: u64) -> BattleStats {
        super::fight(CFG, party, fight, Settings::default(), seed)
    }
}
