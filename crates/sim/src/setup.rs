//! From presets to fight inputs, with every problem explained. Invalid skills never reach a
//! fight: they are dropped with the reason (`skillcheck` rules, the same as in the game).
use deadroll_rules::{
    input::{hero, skill_use, SkillStats, SkillUse},
    items::{check_equip, starter_loadout, EquipError, Equipment},
    BalanceConfig, HeroStats, MonsterBase,
};
use deadroll_skillcheck::{check_loadout, Category, SkillBlocks, ACTIVE_MAX};
use deadroll_types::{MONSTERS_MAX, PARTY_MAX};

use crate::presets::{Arena, HeroPreset, Presets, SkillPreset};

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
}

/// A skill in a fighter's loadout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub stats: SkillStats,
}

/// A monster ready to spawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub name: String,
    pub base: MonsterBase,
    pub level: u8,
}

/// An encounter ready to fight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fight {
    pub name: String,
    pub spawns: Vec<Spawn>,
    pub arena: Arena,
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
    let mut picked: Vec<(String, SkillBlocks)> = Vec::new();
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
            Ok(blocks) => picked.push((skill.name.clone(), blocks)),
            Err(e) => issues.errors.push(format!(
                "hero `{id}`, skill `{skill_id}`: {} ({e:?}), not used",
                e.message()
            )),
        }
    }
    // Loadout limits: keep skills in order while they fit.
    let mut kept: Vec<(String, SkillBlocks)> = Vec::new();
    for (name, blocks) in picked {
        let mut trial: Vec<SkillBlocks> = kept.iter().map(|(_, b)| *b).collect();
        trial.push(blocks);
        match check_loadout(trial.iter()) {
            Ok(()) => kept.push((name, blocks)),
            Err(e) => issues.errors.push(format!(
                "hero `{id}`, skill `{name}`: {} ({e:?}), not used",
                e.message()
            )),
        }
    }
    let mut skills = Vec::new();
    for (name, blocks) in kept {
        let stats = SkillStats::derive(blocks, &cfg.skills);
        match skill_use(&stats) {
            SkillUse::Ok => skills.push(Skill { name, stats }),
            SkillUse::NotActive => issues.warnings.push(format!(
                "hero `{id}`, skill `{name}`: {} skills are not actions in the session yet",
                match blocks.category {
                    Category::Active => "active",
                    Category::Reactive => "reactive",
                    Category::Passive => "passive",
                }
            )),
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
        equipment: equipment(id, p, level, cfg, issues),
    };
    Fighter {
        name: p.name.clone(),
        hp_max: hero::hp_max(cfg, &hero),
        hero,
        skills,
    }
}

/// What a hero preset wears: its own list (each item checked by the items program's rule), or
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
pub fn resolve_fight(presets: &Presets, id: &str) -> (Option<Fight>, Issues) {
    let mut issues = Issues::default();
    let Some(e) = presets.encounters.get(id) else {
        issues
            .errors
            .push(format!("encounter `{id}`: no such preset"));
        return (None, issues);
    };
    let mut spawns = Vec::new();
    for slot in &e.monsters {
        let Some(m) = presets.monsters.get(&slot.monster) else {
            issues.errors.push(format!(
                "encounter `{id}`: no monster `{}`, left out",
                slot.monster
            ));
            continue;
        };
        for w in m.warnings() {
            issues
                .warnings
                .push(format!("monster `{}`: {w}", slot.monster));
        }
        for _ in 0..slot.count {
            spawns.push(Spawn {
                name: m.name.clone(),
                base: m.base(),
                level: slot.level,
            });
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
        }),
        issues,
    )
}

/// Cells from the monster column to the right edge.
pub fn monster_cells(a: &Arena) -> usize {
    let first = 1 + a.gap as usize;
    (a.width as usize).saturating_sub(first) * a.height as usize
}
