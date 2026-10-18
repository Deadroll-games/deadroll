//! Heroes, skills and encounters as JSON files: `<root>/{heroes,skills,encounters}/<id>.json`,
//! the id is the file stem. Monsters and their skills are registry data
//! (`assets/registry/bestiary.json`, `monster_skills.json`): the bytes the program copies at
//! `open_session`, no copy here.
//!
//! Nothing here is game content: the files are. Enums are written by variant name (see
//! [`crate::named`]), so new variants work in presets without code changes.
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use deadroll_rules::{
    items::Equipment, player::predisposition_allowed, registry::Registry, PlayerTuning,
};
use deadroll_skillcheck::{
    validate_skill, validate_starting, Category, Effect, Purpose, SkillBlocks, SkillError,
    SkillExt, Target, Trigger, SKILL_ASPECTS_MAX,
};
use deadroll_types::{Aspect, Class, Form, Race, RangeTag, Source, Tier, TimeTag, STATS_COUNT};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::named::{self, Named};

/// One skill as the in-game builder makes it (`SkillBlocks` plus a name).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPreset {
    pub name: String,
    #[serde(with = "named::variant")]
    pub category: Category,
    #[serde(with = "named::variant")]
    pub purpose: Purpose,
    #[serde(with = "named::variant")]
    pub source: Source,
    #[serde(with = "named::variant")]
    pub form: Form,
    #[serde(with = "named::variant")]
    pub target: Target,
    /// In order, the first is the main one. The builder allows 1..=3.
    #[serde(with = "named::list")]
    pub aspects: Vec<Aspect>,
    #[serde(with = "named::variant")]
    pub effect: Effect,
    #[serde(with = "named::variant", default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub aura_radius: u8,
    /// Patch 5 blocks (`SkillExt`): range and timing tags, the HP threshold of `HpBelow` /
    /// `AllyHpBelow` in percent. Left out: melee, instant, none.
    #[serde(with = "named::variant", default)]
    pub range: RangeTag,
    #[serde(with = "named::variant", default)]
    pub timing: TimeTag,
    #[serde(default)]
    pub hp_below_pct: u8,
    /// The player's text, up to 200 characters; mechanics never read it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl SkillPreset {
    /// The builder blocks. More than `SKILL_ASPECTS_MAX` aspects is `AspectCount`.
    pub fn blocks(&self) -> Result<SkillBlocks, SkillError> {
        if self.aspects.len() > SKILL_ASPECTS_MAX {
            return Err(SkillError::AspectCount);
        }
        let mut aspects = [Aspect::default(); SKILL_ASPECTS_MAX];
        aspects[..self.aspects.len()].copy_from_slice(&self.aspects);
        Ok(SkillBlocks {
            category: self.category,
            purpose: self.purpose,
            source: self.source,
            form: self.form,
            target: self.target,
            aspects,
            aspect_count: self.aspects.len() as u8,
            effect: self.effect,
            trigger: self.trigger,
            aura_radius: self.aura_radius,
        })
    }

    /// The patch 5 blocks.
    pub fn ext(&self) -> SkillExt {
        SkillExt {
            range: self.range,
            timing: self.timing,
            hp_below_pct: self.hp_below_pct,
        }
    }

    /// The builder rules (`skillcheck::validate_skill`).
    pub fn check(&self) -> Result<(SkillBlocks, SkillExt), SkillError> {
        let blocks = self.blocks()?;
        validate_skill(&blocks, &self.ext())?;
        Ok((blocks, self.ext()))
    }

    /// The builder rules plus the class limits on starting skills (`validate_starting`).
    pub fn check_for(&self, class: Class) -> Result<(SkillBlocks, SkillExt), SkillError> {
        let blocks = self.blocks()?;
        validate_starting(&blocks, &self.ext(), class)?;
        Ok((blocks, self.ext()))
    }

    pub fn from_blocks(name: &str, b: &SkillBlocks, ext: &SkillExt) -> Self {
        SkillPreset {
            name: name.to_string(),
            category: b.category,
            purpose: b.purpose,
            source: b.source,
            form: b.form,
            target: b.target,
            aspects: b.used_aspects().to_vec(),
            effect: b.effect,
            trigger: b.trigger,
            aura_radius: b.aura_radius,
            range: ext.range,
            timing: ext.timing,
            hp_below_pct: ext.hp_below_pct,
            description: String::new(),
        }
    }

    /// A short line: `Active Damage Projectile Cold+Fire Line Freeze`.
    pub fn summary(&self) -> String {
        let aspects: Vec<String> = self.aspects.iter().map(|a| a.name()).collect();
        let mut out = format!(
            "{} {} {} {} {}",
            self.category.name(),
            self.purpose.name(),
            self.source.name(),
            self.form.name(),
            aspects.join("+")
        );
        if self.target != Target::default() {
            out += &format!(" {}", self.target.name());
        }
        if self.effect != Effect::None {
            out += &format!(" {}", self.effect.name());
        }
        if self.trigger != Trigger::None {
            out += &format!(" on {}", self.trigger.name());
        }
        if self.aura_radius > 0 {
            out += &format!(" aura {}", self.aura_radius);
        }
        out
    }
}

/// A hero: a character at a level with its skills (ids of skill presets).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroPreset {
    pub name: String,
    #[serde(with = "named::variant")]
    pub race: Race,
    #[serde(with = "named::variant")]
    pub class: Class,
    pub level: u16,
    #[serde(with = "named::by_stat")]
    pub stats: [u16; STATS_COUNT],
    #[serde(with = "named::variant")]
    pub predisposition: Aspect,
    /// Race trait (predisposition) tier, 1 best .. 14 worst.
    pub tier: u8,
    /// Class trait tier, 1 best .. 14 worst. Left out = 14 (a new character).
    #[serde(default = "worst_tier")]
    pub class_tier: u8,
    pub skills: Vec<String>,
    /// Skills written inline (random builds); used when an id is not in the skill presets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inline_skills: Vec<SkillPreset>,
    /// What the hero wears, by slot. Left out = the class starter kit, as the character program
    /// gives a new character (`create_character`); `{}` = nothing.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "named::equipment"
    )]
    pub equipment: Option<Equipment>,
}

impl HeroPreset {
    /// Creation rules the character program enforces; reported, not blocking (heroes above
    /// level 1 have spent more points).
    pub fn warnings(&self, t: &PlayerTuning) -> Vec<String> {
        let mut out = Vec::new();
        if !predisposition_allowed(self.race, self.predisposition) {
            out.push(format!(
                "predisposition {} is not allowed for {}",
                self.predisposition.name(),
                self.race.name()
            ));
        }
        if Tier::new(self.tier).is_err() {
            out.push(format!("tier {} is outside 1..=14", self.tier));
        }
        if Tier::new(self.class_tier).is_err() {
            out.push(format!("class tier {} is outside 1..=14", self.class_tier));
        }
        if self.level <= 1 && !deadroll_rules::player::stats_valid(&self.stats, t) {
            out.push(format!(
                "level 1 stats must be {}..={} each with {} points spent",
                t.stat_base, t.stat_max_start, t.stat_points_start
            ));
        }
        if self.level == 0 {
            out.push("level 0".into());
        }
        out
    }
}

/// A fight: registry monsters (by their `bestiary.json` name) on an open arena.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterPreset {
    pub name: String,
    pub monsters: Vec<EncounterSlot>,
    #[serde(default)]
    pub arena: Arena,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterSlot {
    /// A `bestiary.json` name.
    pub monster: String,
    pub level: u8,
    pub count: u8,
    /// The elite version (docs/BESTIARY.md 5).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub elite: bool,
    /// The pool bits the DM chose (`Spawn.skills`); left out = the first of each category that
    /// fill the slots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<u16>,
}

/// An open floor: the party in column 1, the monsters `gap` columns to the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arena {
    pub width: u8,
    pub height: u8,
    pub gap: u8,
}

impl Default for Arena {
    fn default() -> Self {
        Arena {
            width: 14,
            height: 10,
            gap: 6,
        }
    }
}

/// Which preset folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Heroes,
    Skills,
    Encounters,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Heroes, Kind::Skills, Kind::Encounters];

    pub fn dir(self) -> &'static str {
        match self {
            Kind::Heroes => "heroes",
            Kind::Skills => "skills",
            Kind::Encounters => "encounters",
        }
    }
}

/// All presets, by id, and the registry data they fight with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presets {
    pub heroes: BTreeMap<String, HeroPreset>,
    pub skills: BTreeMap<String, SkillPreset>,
    pub encounters: BTreeMap<String, EncounterPreset>,
    /// `assets/registry`: aspects, traits, resonances, the bestiary and the monster skills, the
    /// tables the program copies at `open_session`.
    pub registry: Registry,
}

/// The registry folder for a presets folder: `DEADROLL_REGISTRY`, else `assets/registry` of the
/// repository that holds `tools/balance/presets`, else the one this crate was built in.
pub fn registry_dir(root: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("DEADROLL_REGISTRY") {
        return PathBuf::from(dir);
    }
    let beside = root.join("../../../assets/registry");
    if beside.join("resonances.json").exists() {
        return beside;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/registry")
}

impl Presets {
    /// Reads every `*.json` under `root`. Files that do not parse are skipped and reported.
    pub fn load(root: &Path) -> (Presets, Vec<String>) {
        let mut errors = Vec::new();
        let registry = Registry::load(&registry_dir(root)).unwrap_or_else(|e| {
            errors.push(format!("registry: {e}"));
            Registry::default()
        });
        let p = Presets {
            heroes: load_dir(&root.join(Kind::Heroes.dir()), &mut errors),
            skills: load_dir(&root.join(Kind::Skills.dir()), &mut errors),
            encounters: load_dir(&root.join(Kind::Encounters.dir()), &mut errors),
            registry,
        };
        (p, errors)
    }

    /// Writes every preset to `<root>/<kind>/<id>.json` and removes the `*.json` files whose id
    /// is gone (deleted in the UI).
    pub fn save(&self, root: &Path) -> io::Result<()> {
        save_dir(&root.join(Kind::Heroes.dir()), &self.heroes)?;
        save_dir(&root.join(Kind::Skills.dir()), &self.skills)?;
        save_dir(&root.join(Kind::Encounters.dir()), &self.encounters)
    }

    pub fn ids(&self, kind: Kind) -> Vec<String> {
        match kind {
            Kind::Heroes => self.heroes.keys().cloned().collect(),
            Kind::Skills => self.skills.keys().cloned().collect(),
            Kind::Encounters => self.encounters.keys().cloned().collect(),
        }
    }

    /// A free id from `name` (lowercase, `_` for anything else, `_2`, `_3` on clashes).
    pub fn free_id(&self, kind: Kind, name: &str) -> String {
        let base = slug(name);
        let taken = self.ids(kind);
        if !taken.contains(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}_{n}"))
            .find(|id| !taken.contains(id))
            .unwrap_or(base)
    }
}

/// `"Frost Bolt!"` -> `"frost_bolt"`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "preset".into()
    } else {
        out
    }
}

fn load_dir<T: DeserializeOwned>(dir: &Path, errors: &mut Vec<String>) -> BTreeMap<String, T> {
    let mut out = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    for path in paths {
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        match fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<T>(&text).map_err(|e| e.to_string()))
        {
            Ok(v) => {
                out.insert(id, v);
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    out
}

fn save_dir<T: Serialize>(dir: &Path, items: &BTreeMap<String, T>) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for (id, item) in items {
        let text = serde_json::to_string_pretty(item).map_err(io::Error::other)?;
        fs::write(dir.join(format!("{id}.json")), text + "\n")?;
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let stale = path.extension().is_some_and(|x| x == "json")
            && path
                .file_stem()
                .is_some_and(|s| !items.contains_key(&*s.to_string_lossy()));
        if stale {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn worst_tier() -> u8 {
    deadroll_types::Tier::WORST.get()
}
