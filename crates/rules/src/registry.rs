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
use deadroll_types::{Aspect, Family, StateKind};
use serde::Deserialize;

use crate::monster::{record, MonsterDefense};

/// Bestiary slots (registry `BESTIARY_MAX`).
pub const BESTIARY_MAX: usize = 32;

/// The tables a fight reads, decoded from the registry bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    pub tables: ResonanceTables,
    /// By bestiary id; `None` for an empty slot.
    pub defense: Vec<Option<MonsterDefense>>,
    /// The table bodies as the chain stores them.
    pub aspects: Vec<u8>,
    pub traits: Vec<u8>,
    pub resonances: Vec<u8>,
    pub bestiary: Vec<u8>,
}

impl Registry {
    /// Loads `aspects.json`, `traits.json`, `resonances.json` and `bestiary.json` from `dir`.
    pub fn load(dir: &Path) -> Result<Registry, String> {
        let read = |name: &str| {
            let path = dir.join(name);
            fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let aspects = encode_aspects(&read("aspects.json")?)?;
        let traits = encode_traits(&read("traits.json")?)?;
        let resonances = encode_resonances(&read("resonances.json")?)?;
        let bestiary = encode_bestiary(&read("bestiary.json")?)?;
        Registry::from_bytes(aspects, traits, resonances, bestiary)
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
    ) -> Result<Registry, String> {
        let tables = ResonanceTables::from_bytes(&aspects, &traits, &resonances)
            .map_err(|e| format!("resonance tables: {e:?}"))?;
        let defense = bestiary
            .chunks(record::SIZE)
            .map(MonsterDefense::from_record)
            .collect();
        Ok(Registry {
            tables,
            defense,
            aspects,
            traits,
            resonances,
            bestiary,
        })
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

#[derive(Deserialize)]
struct MonsterJson {
    id: u8,
    role: String,
    biome: String,
    size: u8,
    min_tier: u8,
    speed: u8,
    attack_aspect: String,
    attack_source: String,
    attack_form: String,
    predisposition: Option<String>,
    #[serde(default)]
    resistance: Vec<String>,
    #[serde(default)]
    immunity: Vec<String>,
    #[serde(default)]
    state_immunity: Vec<String>,
    #[serde(default)]
    traits: Vec<String>,
    armor: u16,
    evasion: u16,
    accuracy: u16,
    initiative: u16,
    hp: u32,
    hp_per_level: u32,
    damage_min: u16,
    damage_max: u16,
    xp: u32,
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
        MonsterDefense::from_record(&r).ok_or_else(|| format!("bestiary {i}: invalid"))?;
        out.extend_from_slice(&r);
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
