//! The registry data of `assets/registry/*.json` as the bytes the chain holds (tools only, with
//! the `serde` feature). `tools/scripts/seed-registry.ts` uploads the same files with its own
//! encoder; `deadroll-rules` tests pin that both give the same bytes (a fixture shared with the
//! TS test), so the simulator, the website and the program read one source.
use std::{fs, path::Path, string::String, vec::Vec};

use deadroll_combat::tables::{
    decode_aspect, decode_pair, decode_trait, encode_aspect, encode_pair, encode_trait, ArgKind,
    AspectEntry, Op, OpKind, PairEntry, ResonanceTables, TraitEntry, ANY_STATE, NATIVE, PAIR_OPS,
    TEMPLATE_OPS, TRAITS,
};
use deadroll_combat::{ModKind, ResonanceCondition};
use deadroll_skillcheck::{
    monster::{MonsterSkill, KIN_ONLY, RECORDS_MAX, RECORD_SIZE as SKILL_RECORD},
    Category, Effect, Purpose, SkillBlocks, SkillExt, SkillTuning, Target, Trigger,
    SKILL_ASPECTS_MAX,
};
use deadroll_types::{
    Aspect, Family, Form, RangeTag, RewardKind, Source, StateKind, TileKind, TimeTag, NO_TABLE,
};
use serde::{Deserialize, Serialize};

use crate::loot::{self, LootEntry, LootLevel, LootTable};
use crate::monster::{record, MonsterDefense, MonsterTemplate, NO_KIN};
use crate::tiles;

/// Bestiary slots (registry `BESTIARY_MAX`).
pub const BESTIARY_MAX: usize = 32;

/// The tables a fight reads, decoded from the registry bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    pub tables: ResonanceTables,
    /// By bestiary id; `None` for an empty slot.
    pub defense: Vec<Option<MonsterDefense>>,
    /// By bestiary id; `None` for an empty slot.
    pub templates: Vec<Option<MonsterTemplate>>,
    /// By `monster_skills` index; `None` for an empty slot.
    pub skills: Vec<Option<MonsterSkill>>,
    /// Names from the JSON (text is not on the chain): bestiary by id, skills by index.
    pub monster_names: Vec<String>,
    pub skill_names: Vec<String>,
    /// The table bodies as the chain stores them.
    pub aspects: Vec<u8>,
    pub traits: Vec<u8>,
    pub resonances: Vec<u8>,
    pub bestiary: Vec<u8>,
    pub monster_skills: Vec<u8>,
    /// Patch 11: the `rewards` table body, its tables by id (`None` for an empty id) and their
    /// names from the JSON.
    pub rewards: Vec<u8>,
    pub reward_tables: Vec<Option<LootTable>>,
    pub reward_names: Vec<String>,
    /// Patch 13: the `tileset` table body (`tiles::tile_rules` decodes it).
    pub tileset: Vec<u8>,
}

impl Registry {
    /// Loads `aspects.json`, `traits.json`, `resonances.json`, `bestiary.json` and
    /// `monster_skills.json` from `dir`.
    pub fn load(dir: &Path) -> Result<Registry, String> {
        let read = |name: &str| {
            let path = dir.join(name);
            fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let aspects = encode_aspects(&read("aspects.json")?)?;
        let traits = encode_traits(&read("traits.json")?)?;
        let resonances = encode_resonances(&read("resonances.json")?)?;
        let bestiary_json = read("bestiary.json")?;
        let skills_json = read("monster_skills.json")?;
        let bestiary = encode_bestiary(&bestiary_json)?;
        let monster_skills = encode_monster_skills(&skills_json)?;
        let mut r = Registry::from_bytes(aspects, traits, resonances, bestiary, monster_skills)?;
        r.monster_names = names::<MonsterJson>(&bestiary_json, |m| m.name.clone())?;
        r.skill_names = names::<MonsterSkillJson>(&skills_json, |m| m.name.clone())?;
        let rewards_json = read("reward_tables.json")?;
        r.set_rewards(encode_rewards(&rewards_json)?);
        r.reward_names = names::<RewardTableJson>(&rewards_json, |t| t.name.clone())?;
        r.tileset = encode_tileset(&read("tileset.json")?)?;
        Ok(r)
    }

    /// Sets the `rewards` table body and decodes its tables.
    pub fn set_rewards(&mut self, rewards: Vec<u8>) {
        self.reward_tables = rewards
            .chunks(loot::record::SIZE)
            .map(LootTable::from_record)
            .collect();
        self.rewards = rewards;
    }

    /// Reward table `id`.
    pub fn reward_table(&self, id: u8) -> Option<LootTable> {
        self.reward_tables.get(id as usize).copied().flatten()
    }

    /// The default location: `assets/registry` of this repository.
    pub fn shipped() -> Result<Registry, String> {
        Registry::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/registry"))
    }

    pub fn from_bytes(
        aspects: Vec<u8>,
        traits: Vec<u8>,
        resonances: Vec<u8>,
        bestiary: Vec<u8>,
        monster_skills: Vec<u8>,
    ) -> Result<Registry, String> {
        let tables = ResonanceTables::from_bytes(&aspects, &traits, &resonances)
            .map_err(|e| format!("resonance tables: {e:?}"))?;
        let templates: Vec<Option<MonsterTemplate>> = bestiary
            .chunks(record::SIZE)
            .map(MonsterTemplate::from_record)
            .collect();
        let defense = templates.iter().map(|t| t.map(|t| t.defense)).collect();
        let skills = monster_skills
            .chunks(SKILL_RECORD)
            .enumerate()
            .map(|(i, r)| {
                MonsterSkill::decode_record(r, &SkillTuning::DEFAULT)
                    .map_err(|e| format!("monster skill {i}: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Registry {
            tables,
            defense,
            templates,
            skills,
            monster_names: Vec::new(),
            skill_names: Vec::new(),
            aspects,
            traits,
            resonances,
            bestiary,
            monster_skills,
            rewards: Vec::new(),
            reward_tables: Vec::new(),
            reward_names: Vec::new(),
            tileset: Vec::new(),
        })
    }

    /// The template of bestiary id `id`.
    pub fn template(&self, id: u8) -> Option<MonsterTemplate> {
        self.templates.get(id as usize).copied().flatten()
    }

    /// The bestiary record bytes of `id` (what `MonsterState.records` copies).
    pub fn record(&self, id: u8) -> Option<[u8; record::SIZE]> {
        self.template(id)?;
        let at = id as usize * record::SIZE;
        let mut out = [0u8; record::SIZE];
        out.copy_from_slice(self.bestiary.get(at..at + record::SIZE)?);
        Some(out)
    }

    /// The pool of `t` in pool order (`None` for a slot past the table or empty).
    pub fn pool(&self, t: &MonsterTemplate) -> Vec<Option<MonsterSkill>> {
        (0..t.pool_len as usize)
            .map(|i| {
                self.skills
                    .get(t.pool_start as usize + i)
                    .copied()
                    .flatten()
            })
            .collect()
    }

    /// The builder rules of bestiary id `id` (`deadroll_mapcheck::check_map`).
    pub fn spawn_rules(
        &self,
        id: u8,
        s: &crate::config::MonsterScaling,
    ) -> Option<deadroll_mapcheck::roster::SpawnRules> {
        self.template(id)?.spawn_rules(
            |i| {
                self.skills
                    .get(i)
                    .copied()
                    .flatten()
                    .map(|k| k.blocks.category)
            },
            s,
        )
    }

    /// The bestiary id named `name` in `bestiary.json`.
    pub fn monster_id(&self, name: &str) -> Option<u8> {
        self.monster_names
            .iter()
            .position(|n| n == name)
            .map(|i| i as u8)
    }

    /// The defenses of bestiary id `id` (none for an unknown id).
    pub fn defense_of(&self, id: u8) -> MonsterDefense {
        self.defense
            .get(id as usize)
            .copied()
            .flatten()
            .unwrap_or(MonsterDefense::NONE)
    }
}

fn parse<T: Copy + core::fmt::Debug>(name: &str, all: &[T], label: &str) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|v| format!("{v:?}") == name)
        .ok_or_else(|| format!("unknown {label} `{name}`"))
}

fn aspect(name: &str) -> Result<Aspect, String> {
    parse(name, Aspect::ALL, "aspect")
}

fn state(name: &str) -> Result<StateKind, String> {
    parse(name, StateKind::ALL, "state")
}

const CONDITIONS: [ResonanceCondition; 9] = [
    ResonanceCondition::Always,
    ResonanceCondition::CanBurn,
    ResonanceCondition::CanBeChilled,
    ResonanceCondition::CanBeFrozen,
    ResonanceCondition::IsFrozen,
    ResonanceCondition::CanBeShocked,
    ResonanceCondition::CanBleed,
    ResonanceCondition::CanBeStunned,
    ResonanceCondition::HasArmor,
];

fn condition(name: &str) -> Result<ResonanceCondition, String> {
    parse(name, &CONDITIONS, "condition")
}

/// Trait names, by trait id (docs/DAMAGE_TYPES_AND_MIXES.md 3.2).
pub const TRAIT_NAMES: [&str; TRAITS] = [
    "Fury",
    "Composure",
    "Swiftness",
    "Wasting",
    "Crushing",
    "Keenness",
];

#[derive(Deserialize)]
struct Records<T> {
    records: Vec<T>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AspectJson {
    aspect: String,
    family: String,
    #[serde(default)]
    flags: u8,
    native: Option<String>,
    #[serde(rename = "trait")]
    trait_name: String,
    condition: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpJson {
    kind: String,
    #[serde(default)]
    arg: Option<String>,
    #[serde(default)]
    value: u32,
    #[serde(default)]
    crit: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitJson {
    #[serde(rename = "trait")]
    trait_name: String,
    aspect: String,
    #[allow(dead_code)]
    epithet: String,
    hit: Option<OpJson>,
    template: Vec<OpJson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairJson {
    base: String,
    tint: String,
    #[allow(dead_code)]
    name: Option<String>,
    #[serde(rename = "override")]
    set: bool,
    condition: String,
    ops: Vec<OpJson>,
}

/// One record of `bestiary.json` (programs/registry README byte table), as the file writes it:
/// the balance bench edits these and saves the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterJson {
    pub id: u8,
    pub name: String,
    pub role: String,
    pub biome: String,
    pub size: u8,
    pub min_tier: u8,
    pub speed: u8,
    pub attack_aspect: String,
    pub attack_source: String,
    pub attack_form: String,
    pub predisposition: Option<String>,
    #[serde(default)]
    pub resistance: Vec<String>,
    #[serde(default)]
    pub immunity: Vec<String>,
    #[serde(default)]
    pub state_immunity: Vec<String>,
    /// Patch 9: defenses by tag, power sources (`Magical`, `Combat`) and forms (`Strike`,
    /// `Projectile`): "immunity to magical" (docs/DAMAGE_TYPES_AND_MIXES.md 9).
    #[serde(default)]
    pub tag_resistance: Vec<String>,
    #[serde(default)]
    pub tag_immunity: Vec<String>,
    #[serde(default)]
    pub traits: Vec<String>,
    pub armor: u16,
    pub evasion: u16,
    pub accuracy: u16,
    pub initiative: u16,
    pub hp: u32,
    pub hp_per_level: u32,
    pub damage_min: u16,
    pub damage_max: u16,
    pub xp: u32,
    /// Patch 6: the skill pool in `monster_skills.json` (`[start, start + len)`) and the kin.
    #[serde(default)]
    pub pool_start: u8,
    #[serde(default)]
    pub pool_len: u8,
    #[serde(default)]
    pub kin: Option<String>,
    /// Patch 11: the body loot table (`reward_tables.json` id), none = no body loot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot_table: Option<u8>,
}

/// The whole `bestiary.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BestiaryJson {
    #[serde(default)]
    pub note: String,
    pub records: Vec<MonsterJson>,
}

/// `dir/bestiary.json`.
pub fn read_bestiary_json(dir: &Path) -> Result<BestiaryJson, String> {
    let path = dir.join("bestiary.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes `dir/bestiary.json` after the encoder accepts it (the bytes the seed script uploads).
pub fn write_bestiary_json(dir: &Path, b: &BestiaryJson) -> Result<(), String> {
    let text = serde_json::to_string_pretty(b).map_err(|e| e.to_string())? + "\n";
    encode_bestiary(&text)?;
    let path = dir.join("bestiary.json");
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MonsterSkillJson {
    id: u8,
    name: String,
    category: String,
    purpose: String,
    source: String,
    form: String,
    #[serde(default = "default_target")]
    target: String,
    aspects: Vec<String>,
    #[serde(default = "default_none")]
    effect: String,
    #[serde(default = "default_none")]
    trigger: String,
    #[serde(default)]
    aura_radius: u8,
    range: String,
    #[serde(default = "default_timing")]
    timing: String,
    #[serde(default)]
    hp_below_pct: u8,
    #[serde(default)]
    kin_only: bool,
}

fn default_target() -> String {
    "Single".into()
}

fn default_none() -> String {
    "None".into()
}

fn default_timing() -> String {
    "Instant".into()
}

fn names<T: serde::de::DeserializeOwned>(
    json: &str,
    name: impl Fn(&T) -> String,
) -> Result<Vec<String>, String> {
    let list: Records<T> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(list.records.iter().map(name).collect())
}

fn records<'a, T: Deserialize<'a>>(json: &'a str, table: &str) -> Result<Vec<T>, String> {
    serde_json::from_str::<Records<T>>(json)
        .map(|r| r.records)
        .map_err(|e| format!("{table}: {e}"))
}

fn op(j: &OpJson) -> Result<Op, String> {
    const KINDS: [OpKind; OpKind::COUNT as usize] = {
        let mut out = [OpKind::None; OpKind::COUNT as usize];
        let mut i = 0;
        while i < OpKind::COUNT {
            out[i as usize] = match OpKind::from_u8(i) {
                Some(k) => k,
                None => OpKind::None,
            };
            i += 1;
        }
        out
    };
    let kind = parse(&j.kind, &KINDS, "op kind")?;
    let arg = j.arg.as_deref();
    let need = || arg.ok_or_else(|| format!("{}: needs an arg", j.kind));
    let arg = match kind.arg() {
        ArgKind::Zero => match arg {
            None => 0,
            Some(a) => return Err(format!("{}: takes no arg, got `{a}`", j.kind)),
        },
        ArgKind::State | ArgKind::StateOnly => match need()? {
            "Native" if kind.arg() == ArgKind::State => NATIVE,
            name => state(name)? as u8,
        },
        ArgKind::StateOrAny => match need()? {
            "Any" => ANY_STATE,
            name => state(name)? as u8,
        },
        ArgKind::Mod => parse(need()?, &ModKind::ALL, "modifier")? as u8,
        ArgKind::Aspect => aspect(need()?)? as u8,
    };
    let op = Op {
        kind,
        crit_only: j.crit,
        arg,
        value: j.value,
    };
    Op::decode(&op.encode()).ok_or_else(|| format!("{}: invalid op", j.kind))
}

fn ops<const N: usize>(list: &[OpJson], label: &str) -> Result<[Op; N], String> {
    if list.len() > N {
        return Err(format!("{label}: at most {N} ops"));
    }
    let mut out = [Op::NONE; N];
    for (slot, j) in out.iter_mut().zip(list) {
        *slot = op(j).map_err(|e| format!("{label}: {e}"))?;
    }
    Ok(out)
}

/// `aspects.json` as the `aspects` table body.
pub fn encode_aspects(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<AspectJson> = records(json, "aspects")?;
    if list.len() != Aspect::COUNT {
        return Err(format!("aspects: {} records", Aspect::COUNT));
    }
    let mut out = Vec::new();
    for (i, r) in list.iter().enumerate() {
        let a = aspect(&r.aspect)?;
        if a as usize != i {
            return Err(format!("aspects: record {i} must be {:?}", Aspect::ALL[i]));
        }
        let trait_id = TRAIT_NAMES
            .iter()
            .position(|n| *n == r.trait_name)
            .ok_or_else(|| format!("unknown trait `{}`", r.trait_name))?;
        let entry = AspectEntry {
            family: parse(&r.family, Family::ALL, "family")?,
            flags: r.flags,
            native: r.native.as_deref().map(state).transpose()?,
            trait_id: trait_id as u8,
            condition: condition(&r.condition)?,
        };
        let bytes = encode_aspect(a, &entry);
        decode_aspect(i, &bytes).ok_or_else(|| format!("aspects: record {i} invalid"))?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// `traits.json` as the `traits` table body (records in trait id order).
pub fn encode_traits(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<TraitJson> = records(json, "traits")?;
    if list.len() != TRAITS {
        return Err(format!("traits: {TRAITS} records"));
    }
    let mut out = Vec::new();
    for (i, r) in list.iter().enumerate() {
        if r.trait_name != TRAIT_NAMES[i] {
            return Err(format!("traits: record {i} must be {}", TRAIT_NAMES[i]));
        }
        aspect(&r.aspect)?;
        let label = format!("trait {}", r.trait_name);
        let entry = TraitEntry {
            present: true,
            hit: match &r.hit {
                Some(j) => op(j).map_err(|e| format!("{label}: {e}"))?,
                None => Op::NONE,
            },
            template: ops::<TEMPLATE_OPS>(&r.template, &label)?,
        };
        let bytes = encode_trait(&entry);
        decode_trait(&bytes).ok_or_else(|| format!("{label}: invalid"))?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// `resonances.json` as the `resonances` table body (index `base * 6 + tint`).
pub fn encode_resonances(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<PairJson> = records(json, "resonances")?;
    if list.len() != Aspect::COUNT * Aspect::COUNT {
        return Err(format!(
            "resonances: {} records",
            Aspect::COUNT * Aspect::COUNT
        ));
    }
    let mut out = Vec::new();
    for (i, r) in list.iter().enumerate() {
        let (base, tint) = (aspect(&r.base)?, aspect(&r.tint)?);
        if base as usize * Aspect::COUNT + tint as usize != i {
            return Err(format!("resonances: record {i} out of order"));
        }
        let label = format!("{:?} + {:?}", base, tint);
        let entry = PairEntry {
            condition: condition(&r.condition)?,
            set: r.set,
            ops: ops::<PAIR_OPS>(&r.ops, &label)?,
        };
        let bytes = encode_pair(&entry);
        decode_pair(&bytes).ok_or_else(|| format!("{label}: invalid"))?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

const ROLES: [&str; 6] = ["Swarm", "Fast", "Tank", "Loner", "Boss", "Helper"];
const BIOMES: [&str; 2] = ["Forest", "Caves and Ruins"];
/// Kin names by kin id (`record::KIN`): the families kin-only skills look at (docs/BESTIARY.md
/// 7-8). Append only.
pub const KINS: [&str; 6] = ["Wolf", "Goblin", "Bear", "Treant", "Fairy", "Troll"];
const MONSTER_TRAITS: [(&str, u16); 4] = [
    ("swarm", 1),
    ("regeneration", 2),
    ("leader", 4),
    ("hunt", 8),
];

fn mask<T: Copy + core::fmt::Debug + Into<u8>>(
    names: &[String],
    all: &[T],
    label: &str,
) -> Result<u8, String> {
    names.iter().try_fold(0u8, |m, n| {
        let v: u8 = parse(n, all, label)?.into();
        Ok(m | 1 << v)
    })
}

/// A defense tag mask (`deadroll_combat::source_bit` / `form_bit`) from source and form names.
fn tag_mask(names: &[String]) -> Result<u8, String> {
    names.iter().try_fold(0u8, |m, n| {
        let source = deadroll_types::Source::ALL
            .iter()
            .find(|s| format!("{s:?}") == *n);
        let form = deadroll_types::Form::ALL
            .iter()
            .find(|f| format!("{f:?}") == *n);
        match (source, form) {
            (Some(s), _) => Ok(m | deadroll_combat::source_bit(*s)),
            (_, Some(f)) => Ok(m | deadroll_combat::form_bit(*f)),
            _ => Err(format!(
                "unknown tag `{n}`, expected one of: Magical, Combat, Strike, Projectile"
            )),
        }
    })
}

/// `bestiary.json` as the `bestiary` table body (programs/registry README byte table).
pub fn encode_bestiary(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<MonsterJson> = records(json, "bestiary")?;
    if list.len() > BESTIARY_MAX {
        return Err(format!("bestiary: at most {BESTIARY_MAX} records"));
    }
    let mut out = Vec::new();
    for (i, m) in list.iter().enumerate() {
        if m.id as usize != i {
            return Err(format!("bestiary: record {i} must have id {i}"));
        }
        let index = |list: &[&str], v: &str, label: &str| {
            list.iter()
                .position(|n| *n == v)
                .map(|p| p as u8)
                .ok_or_else(|| format!("unknown {label} `{v}`"))
        };
        let mut r = [0u8; record::SIZE];
        r[0] = 1;
        r[1] = index(&ROLES, &m.role, "role")?;
        r[2] = index(&BIOMES, &m.biome, "biome")?;
        r[3] = m.size;
        r[4] = m.min_tier;
        r[5] = m.speed;
        r[6] = aspect(&m.attack_aspect)? as u8;
        r[7] = parse(&m.attack_source, deadroll_types::Source::ALL, "source")? as u8;
        r[8] = parse(&m.attack_form, deadroll_types::Form::ALL, "form")? as u8;
        r[9] = match m.predisposition.as_deref() {
            None | Some("None") => 0xFF,
            Some(a) => aspect(a)? as u8,
        };
        r[record::RESIST] = mask(&m.resistance, Aspect::ALL, "aspect")?;
        r[record::IMMUNE] = mask(&m.immunity, Aspect::ALL, "aspect")?;
        let traits = m.traits.iter().try_fold(0u16, |bits, t| {
            MONSTER_TRAITS
                .iter()
                .find(|(n, _)| n == t)
                .map(|(_, b)| bits | b)
                .ok_or_else(|| format!("unknown trait `{t}`"))
        })?;
        r[12..14].copy_from_slice(&traits.to_le_bytes());
        r[14..16].copy_from_slice(&m.armor.to_le_bytes());
        r[16..18].copy_from_slice(&m.evasion.to_le_bytes());
        r[18..20].copy_from_slice(&m.accuracy.to_le_bytes());
        r[20..22].copy_from_slice(&m.initiative.to_le_bytes());
        r[22..26].copy_from_slice(&m.hp.to_le_bytes());
        r[26..30].copy_from_slice(&m.hp_per_level.to_le_bytes());
        r[30..32].copy_from_slice(&m.damage_min.to_le_bytes());
        r[32..34].copy_from_slice(&m.damage_max.to_le_bytes());
        r[34..38].copy_from_slice(&m.xp.to_le_bytes());
        r[record::IMMUNE_STATES] = mask(&m.state_immunity, StateKind::ALL, "state")?;
        r[record::RESIST_TAGS] = tag_mask(&m.tag_resistance)?;
        r[record::IMMUNE_TAGS] = tag_mask(&m.tag_immunity)?;
        r[record::POOL_START] = m.pool_start;
        r[record::POOL_LEN] = m.pool_len;
        r[record::KIN] = match m.kin.as_deref() {
            None => NO_KIN,
            Some(k) => index(&KINS, k, "kin")?,
        };
        r[record::LOOT_TABLE] = m.loot_table.unwrap_or(NO_TABLE);
        MonsterTemplate::from_record(&r).ok_or_else(|| format!("bestiary {i}: invalid"))?;
        out.extend_from_slice(&r);
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RewardEntryJson {
    kind: String,
    id: u16,
    qty: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RewardTableJson {
    id: u8,
    name: String,
    level_base: u8,
    level_max: u8,
    cost: u16,
    levels: Vec<Vec<RewardEntryJson>>,
}

/// Patch 11: `reward_tables.json` as the `rewards` table body (programs/registry README), each
/// record checked by `LootTable::from_record`.
pub fn encode_rewards(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<RewardTableJson> = records(json, "reward tables")?;
    if list.len() > loot::record::RECORDS_MAX {
        return Err(format!(
            "reward tables: at most {} records",
            loot::record::RECORDS_MAX
        ));
    }
    let mut out = Vec::new();
    for (i, t) in list.iter().enumerate() {
        let label = format!("reward table {i} ({})", t.name);
        if t.id as usize != i {
            return Err(format!("{label} must have id {i}"));
        }
        if t.levels.len() != (t.level_max.saturating_sub(t.level_base) as usize + 1) {
            return Err(format!("{label}: one list per level from base to max"));
        }
        if t.levels.len() > loot::record::LEVELS {
            return Err(format!("{label}: at most {} levels", loot::record::LEVELS));
        }
        let mut table = LootTable {
            level_base: t.level_base,
            level_max: t.level_max,
            cost: t.cost,
            levels: [LootLevel::default(); loot::record::LEVELS],
        };
        for (level, entries) in table.levels.iter_mut().zip(&t.levels) {
            if entries.len() > loot::record::ENTRIES {
                return Err(format!(
                    "{label}: at most {} entries per level",
                    loot::record::ENTRIES
                ));
            }
            level.count = entries.len() as u8;
            for (e, j) in level.entries.iter_mut().zip(entries) {
                *e = LootEntry {
                    kind: parse(&j.kind, RewardKind::ALL, "reward kind")?,
                    id: j.id,
                    qty: j.qty,
                };
            }
        }
        let bytes = table.to_record();
        LootTable::from_record(&bytes).ok_or_else(|| format!("{label}: invalid"))?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TileJson {
    tile: String,
    passable: u8,
    cost: u8,
    blocks_los: u8,
    flags: u8,
}

/// Patch 13: `tileset.json` as the `tileset` table body (`crate::tiles::record`): one record
/// per tile type in id order (`TileKind`, docs/MAP_DESCRIPTION.md 4.2).
pub fn encode_tileset(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<TileJson> = records(json, "tileset")?;
    if list.len() != TileKind::COUNT {
        return Err(format!("tileset: {} records, one per tile type", TileKind::COUNT));
    }
    let mut out = Vec::new();
    for (i, t) in list.iter().enumerate() {
        if parse(&t.tile, TileKind::ALL, "tile")? as usize != i {
            return Err(format!("tileset: record {i} must be {:?}", TileKind::ALL[i]));
        }
        let mut r = [0u8; tiles::record::SIZE];
        r[tiles::record::PASSABLE] = t.passable;
        r[tiles::record::COST] = t.cost;
        r[tiles::record::BLOCKS_LOS] = t.blocks_los;
        r[tiles::record::FLAGS] = t.flags;
        if !tiles::validate_record(&r) {
            return Err(format!("tileset: record {i} ({}) is invalid", t.tile));
        }
        out.extend_from_slice(&r);
    }
    Ok(out)
}

/// `monster_skills.json` as the `monster_skills` table body (programs/registry README), each
/// record checked by the monster builder (`MonsterSkill::decode_record`).
pub fn encode_monster_skills(json: &str) -> Result<Vec<u8>, String> {
    let list: Vec<MonsterSkillJson> = records(json, "monster skills")?;
    if list.len() > RECORDS_MAX {
        return Err(format!("monster skills: at most {RECORDS_MAX} records"));
    }
    let mut out = Vec::new();
    for (i, m) in list.iter().enumerate() {
        if m.id as usize != i {
            return Err(format!("monster skill {i} must have id {i}"));
        }
        let label = format!("monster skill {i} ({})", m.name);
        let count = m.aspects.len();
        if count == 0 || count > SKILL_ASPECTS_MAX {
            return Err(format!("{label}: 1..={SKILL_ASPECTS_MAX} aspects"));
        }
        let mut aspects = [Aspect::default(); SKILL_ASPECTS_MAX];
        for (slot, a) in aspects.iter_mut().zip(&m.aspects) {
            *slot = aspect(a)?;
        }
        let skill = MonsterSkill {
            blocks: SkillBlocks {
                category: parse(&m.category, Category::ALL, "category")?,
                purpose: parse(&m.purpose, Purpose::ALL, "purpose")?,
                source: parse(&m.source, Source::ALL, "source")?,
                form: parse(&m.form, Form::ALL, "form")?,
                target: parse(&m.target, Target::ALL, "target")?,
                aspects,
                aspect_count: count as u8,
                effect: parse(&m.effect, Effect::ALL, "effect")?,
                trigger: parse(&m.trigger, Trigger::ALL, "trigger")?,
                aura_radius: m.aura_radius,
            },
            ext: SkillExt {
                range: parse(&m.range, RangeTag::ALL, "range")?,
                timing: parse(&m.timing, TimeTag::ALL, "timing")?,
                hp_below_pct: m.hp_below_pct,
            },
            flags: if m.kin_only { KIN_ONLY } else { 0 },
        };
        let bytes = skill.encode_record();
        MonsterSkill::decode_record(&bytes, &SkillTuning::DEFAULT)
            .map_err(|e| format!("{label}: {e:?}"))?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_registry_loads() {
        let r = Registry::shipped().unwrap();
        assert_eq!(r.tables.native(Aspect::Cold), Some(StateKind::Chill));
        assert_eq!(r.defense_of(0).resist, 1 << Aspect::Cold as u8);
        assert_eq!(r.defense_of(31), MonsterDefense::NONE);
    }

    #[test]
    fn bad_json_is_refused() {
        assert!(op(&OpJson {
            kind: "Linked".into(),
            arg: None,
            value: 1,
            crit: false
        })
        .is_err());
        assert!(op(&OpJson {
            kind: "NativeTurns".into(),
            arg: Some("Fire".into()),
            value: 1,
            crit: false
        })
        .is_err());
        assert!(op(&OpJson {
            kind: "Nope".into(),
            arg: None,
            value: 1,
            crit: false
        })
        .is_err());
        assert!(encode_aspects(r#"{"records":[]}"#).is_err());
        assert!(encode_resonances(r#"{"records":[]}"#).is_err());
    }
}
