//! From presets to fight inputs, with every problem explained. Invalid skills never reach a
//! fight: they are dropped with the reason (`skillcheck` rules, the same as in the game).
use deadroll_rules::{
    input::{hero, skill_use, SkillStats, SkillUse},
    items::{check_equip, starter_loadout, EquipError, Equipment},
    monster::{select_skills, NO_SKILL},
    BalanceConfig, HeroStats, MonsterSkillStats, MonsterTemplate,
};
use deadroll_skillcheck::{check_loadout, Category, SkillBlocks, SkillExt, ACTIVE_MAX};
use deadroll_types::{MONSTERS_MAX, MONSTER_SLOTS, PARTY_MAX, SPAWN_ELITE};

use deadroll_rules::arena::FirstSide;

use crate::presets::{Approach, Arena, HeroPreset, Presets, SkillPreset};

/// Problems found while resolving a selection. Errors drop something from the fight; warnings
/// do not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Issues {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Issues {
    pub fn extend(&mut self, other: Issues) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }
}

/// A hero ready to fight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fighter {
    pub name: String,
    pub hero: HeroStats,
    pub hp_max: u32,
    /// Usable actions: valid active skills, at most `ACTIVE_MAX`, in preset order.
    pub skills: Vec<Skill>,
    /// The reactive and the passive skill taken onto the map (the first valid of each).
    pub reactive: Option<Skill>,
    pub passive: Option<Skill>,
}

/// A skill in a fighter's loadout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub stats: SkillStats,
}

/// A monster ready to spawn: its registry template and the skills it takes in, as the program
/// copies them at `open_session`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub name: String,
    pub bestiary_id: u8,
    pub template: MonsterTemplate,
    pub level: u8,
    /// `Spawn.flags` (`SPAWN_ELITE`).
    pub flags: u8,
    /// By slot (`deadroll_rules::monster::ACTIVE_SLOT..`).
    pub skills: [Option<MonsterSkillStats>; MONSTER_SLOTS],
    pub skill_names: [String; MONSTER_SLOTS],
}

/// The pool bits that fill a spawn's slots with the first skills of each category.
pub fn default_skills(pool: &[Category], slots: [u8; Category::COUNT]) -> u16 {
    select_skills(pool, u16::MAX, slots)
        .iter()
        .filter(|&&i| i != NO_SKILL)
        .fold(0u16, |m, &i| m | 1 << i)
}

/// A registry monster at `level` with the pool bits `mask` (`None` = [`default_skills`]): the
/// skills are chosen like `open_session` does (mask and pool, at most the rank's slots).
pub fn spawn(
    reg: &deadroll_rules::registry::Registry,
    bestiary_id: u8,
    level: u8,
    elite: bool,
    mask: Option<u16>,
    cfg: &BalanceConfig,
) -> Result<Spawn, String> {
    let template = reg
        .template(bestiary_id)
        .ok_or_else(|| format!("no bestiary record {bestiary_id}"))?;
    let pool = reg.pool(&template);
    let mut categories = Vec::new();
    for (i, k) in pool.iter().enumerate() {
        categories.push(
            k.ok_or_else(|| format!("pool skill {i} of {bestiary_id} is missing"))?
                .blocks
                .category,
        );
    }
    let flags = if elite { SPAWN_ELITE } else { 0 };
    let slots = template.slots(flags, &cfg.monsters);
    // A chosen mask passes the map builder's rule (`finalize_map`).
    if let Some(m) = mask {
        deadroll_skillcheck::check_monster_selection(&categories, m, slots)
            .map_err(|e| format!("skills {m:#b}: {}", e.message()))?;
    }
    let mask = mask.unwrap_or_else(|| default_skills(&categories, slots));
    let chosen = select_skills(&categories, mask, slots);
    let mut skills = [None; MONSTER_SLOTS];
    let mut skill_names: [String; MONSTER_SLOTS] = Default::default();
    for (slot, &i) in chosen.iter().enumerate() {
        if let Some(k) = (i != NO_SKILL).then(|| pool[i as usize]).flatten() {
            skills[slot] = Some(MonsterSkillStats::new(&k, &cfg.skills));
            let at = template.pool_start as usize + i as usize;
            skill_names[slot] = reg.skill_names.get(at).cloned().unwrap_or_default();
        }
    }
    Ok(Spawn {
        name: reg
            .monster_names
            .get(bestiary_id as usize)
            .cloned()
            .unwrap_or_else(|| format!("monster {bestiary_id}")),
        bestiary_id,
        template,
        level,
        flags,
        skills,
        skill_names,
    })
}

/// An encounter ready to fight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fight {
    pub name: String,
    pub spawns: Vec<Spawn>,
    pub arena: Arena,
    /// The registry resonance tables (`Presets::registry`).
    pub tables: deadroll_combat::ResonanceTables,
    /// Arena update: the side that starts the timeline at 0.
    pub first: FirstSide,
    /// The packs fought after this one, the party carried over (`EncounterPreset::then`).
    pub then: Vec<Fight>,
    /// Patch B: the party retreated from this pack before: its monsters start with the retreat
    /// buff (`deadroll_rules::exit::retreat_buff`).
    pub guarded: bool,
}

/// The side that moves first coming from `approach`; a pack on guard (the party retreated from it)
/// moves first from any side (`deadroll_rules::exit::first_side_guarded`).
pub fn first_side(approach: Approach, guarded: bool) -> FirstSide {
    if guarded {
        return FirstSide::Monsters;
    }
    match approach {
        Approach::Back => FirstSide::Party,
        Approach::Front => FirstSide::Monsters,
    }
}

/// One hero preset with its skills looked up in `presets`.
pub fn resolve_hero(presets: &Presets, id: &str, cfg: &BalanceConfig) -> (Option<Fighter>, Issues) {
    let mut issues = Issues::default();
    let Some(p) = presets.heroes.get(id) else {
        issues.errors.push(format!("hero `{id}`: no such preset"));
        return (None, issues);
    };
    (Some(fighter(presets, id, p, cfg, &mut issues)), issues)
}

/// A hero preset (maybe not saved yet, e.g. a random build) with its skills.
pub fn fighter(
    presets: &Presets,
    id: &str,
    p: &HeroPreset,
    cfg: &BalanceConfig,
    issues: &mut Issues,
) -> Fighter {
    for w in p.warnings(&cfg.players) {
        issues.warnings.push(format!("hero `{id}`: {w}"));
    }
    let mut picked: Vec<(String, SkillBlocks, SkillExt)> = Vec::new();
    for (i, skill_id) in p.skills.iter().enumerate() {
        let preset: Option<&SkillPreset> = presets
            .skills
            .get(skill_id)
            .or_else(|| p.inline_skills.iter().find(|s| &s.name == skill_id));
        let Some(skill) = preset else {
            issues.errors.push(format!(
                "hero `{id}`, skill {}: no skill `{skill_id}`",
                i + 1
            ));
            continue;
        };
        match skill.check_for(p.class) {
            Ok((blocks, ext)) => picked.push((skill.name.clone(), blocks, ext)),
            Err(e) => issues.errors.push(format!(
                "hero `{id}`, skill `{skill_id}`: {} ({e:?}), not used",
                e.message()
            )),
        }
    }
    // Loadout limits: keep skills in order while they fit.
    let mut kept: Vec<(String, SkillBlocks, SkillExt)> = Vec::new();
    for (name, blocks, ext) in picked {
        let mut trial: Vec<SkillBlocks> = kept.iter().map(|(_, b, _)| *b).collect();
        trial.push(blocks);
        match check_loadout(trial.iter()) {
            Ok(()) => kept.push((name, blocks, ext)),
            Err(e) => issues.errors.push(format!(
                "hero `{id}`, skill `{name}`: {} ({e:?}), not used",
                e.message()
            )),
        }
    }
    let mut skills = Vec::new();
    let (mut reactive, mut passive) = (None, None);
    for (name, blocks, ext) in kept {
        let stats = SkillStats::derive(blocks, ext, &cfg.skills);
        match skill_use(&stats) {
            SkillUse::Ok => skills.push(Skill { name, stats }),
            // A reactive skill triggers by itself, a passive one works all the time
            // (docs/SKILL_BUILDER.md 3.2, 3.3): one of each is taken onto the map.
            SkillUse::NotActive => match blocks.category {
                Category::Reactive => reactive = reactive.or(Some(Skill { name, stats })),
                Category::Passive => passive = passive.or(Some(Skill { name, stats })),
                Category::Active => {}
            },
            SkillUse::NoEffect => issues
                .errors
                .push(format!("hero `{id}`, skill `{name}`: no effect, not used")),
        }
    }
    skills.truncate(ACTIVE_MAX);
    let level = p.level.max(1);
    let hero = HeroStats {
        race: p.race,
        class: p.class,
        level,
        stats: p.stats,
        predisposition: p.predisposition,
        predisposition_tier: p.tier.clamp(1, 14),
        class_tier: p.class_tier.clamp(1, 14),
        equipment: equipment(id, p, level, cfg, issues),
    };
    Fighter {
        name: p.name.clone(),
        hp_max: hero::hp_max(cfg, &hero),
        hero,
        skills,
        reactive,
        passive,
    }
}

/// What a hero preset wears: its own list (each item checked by the character program's rule), or
/// the class starter kit when it has none.
pub fn equipment(
    id: &str,
    p: &HeroPreset,
    level: u16,
    cfg: &BalanceConfig,
    issues: &mut Issues,
) -> Equipment {
    let Some(listed) = p.equipment else {
        return starter_loadout(p.class, &cfg.items).equipment();
    };
    let mut out = Equipment::EMPTY;
    for (slot, base) in listed.worn() {
        let item_level = cfg.items.base(base).level;
        match check_equip(level, base, item_level, slot, &cfg.items) {
            Ok(()) => out.set(slot, Some(base)),
            Err(e) => issues.errors.push(format!(
                "hero `{id}`: {base:?} in {slot:?}: {}, not worn",
                match e {
                    EquipError::WrongSlot => "not worn in this slot",
                    EquipError::LevelTooLow => "character level too low",
                }
            )),
        }
    }
    out
}

/// A party from hero ids (at most `PARTY_MAX`).
pub fn resolve_party(
    presets: &Presets,
    ids: &[String],
    cfg: &BalanceConfig,
) -> (Vec<Fighter>, Issues) {
    let mut issues = Issues::default();
    let mut out = Vec::new();
    if ids.len() > PARTY_MAX {
        issues.errors.push(format!(
            "party: at most {PARTY_MAX} heroes, the rest is left out"
        ));
    }
    for id in ids.iter().take(PARTY_MAX) {
        let (f, i) = resolve_hero(presets, id, cfg);
        issues.extend(i);
        out.extend(f);
    }
    if out.is_empty() {
        issues.errors.push("party: no heroes".into());
    }
    (out, issues)
}

/// An encounter from its id (at most `MONSTERS_MAX` monsters).
pub fn resolve_fight(presets: &Presets, id: &str, cfg: &BalanceConfig) -> (Option<Fight>, Issues) {
    let (fight, mut issues) = resolve_pack(presets, id, cfg);
    let Some(mut fight) = fight else {
        return (None, issues);
    };
    let then = presets
        .encounters
        .get(id)
        .map(|e| e.then.clone())
        .unwrap_or_default();
    for step in then {
        let (next, more) = resolve_pack(presets, &step.encounter, cfg);
        issues.extend(more);
        match next {
            Some(mut next) => {
                next.first = first_side(step.approach, step.guarded);
                next.guarded = step.guarded;
                fight.then.push(next);
            }
            None => issues.errors.push(format!(
                "encounter `{id}`: pack `{}` left out",
                step.encounter
            )),
        }
    }
    (Some(fight), issues)
}

/// One encounter as one pack, without its `then`.
fn resolve_pack(presets: &Presets, id: &str, cfg: &BalanceConfig) -> (Option<Fight>, Issues) {
    let mut issues = Issues::default();
    let Some(e) = presets.encounters.get(id) else {
        issues
            .errors
            .push(format!("encounter `{id}`: no such preset"));
        return (None, issues);
    };
    let mut spawns = Vec::new();
    let reg = &presets.registry;
    for slot in &e.monsters {
        let Some(bestiary_id) = reg.monster_id(&slot.monster) else {
            issues.errors.push(format!(
                "encounter `{id}`: no monster `{}` in bestiary.json, left out",
                slot.monster
            ));
            continue;
        };
        match spawn(reg, bestiary_id, slot.level, slot.elite, slot.skills, cfg) {
            Ok(s) => spawns.extend(std::iter::repeat_n(s, slot.count as usize)),
            Err(err) => issues.errors.push(format!(
                "encounter `{id}`, monster `{}`: {err}",
                slot.monster
            )),
        }
    }
    if spawns.len() > MONSTERS_MAX {
        issues.errors.push(format!(
            "encounter `{id}`: at most {MONSTERS_MAX} monsters, the rest is left out"
        ));
        spawns.truncate(MONSTERS_MAX);
    }
    let arena = e.arena;
    let fits = arena.width >= 3
        && arena.height >= 2
        && (arena.gap as usize + 2) <= arena.width as usize
        && spawns.len() <= monster_cells(&arena)
        && PARTY_MAX <= arena.height as usize;
    if !fits {
        issues.errors.push(format!(
            "encounter `{id}`: arena {}x{} gap {} does not fit the party and {} monsters",
            arena.width,
            arena.height,
            arena.gap,
            spawns.len()
        ));
        return (None, issues);
    }
    if spawns.is_empty() {
        issues.errors.push(format!("encounter `{id}`: no monsters"));
        return (None, issues);
    }
    (
        Some(Fight {
            name: e.name.clone(),
            spawns,
            arena,
            tables: presets.registry.tables,
            first: first_side(e.approach, e.guarded),
            then: Vec::new(),
            guarded: e.guarded,
        }),
        issues,
    )
}

/// Cells from the monster column to the right edge.
pub fn monster_cells(a: &Arena) -> usize {
    let first = 1 + a.gap as usize;
    (a.width as usize).saturating_sub(first) * a.height as usize
}
