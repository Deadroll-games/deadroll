//! Enums by name. Every list comes from the enum's own `ALL`, so a new variant appears in JSON
//! presets and in the UI without code changes here.
use std::fmt::Debug;

use deadroll_combat::ModKind;
use deadroll_skillcheck::{Category, Effect, Purpose, Target, Trigger};
use deadroll_types::{Aspect, Class, Form, Race, Source, Stat, StateKind};
use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

/// An enum with every variant listed in `ALL`; the name is the `Debug` text.
pub trait Named: Copy + Debug + PartialEq + 'static {
    fn all() -> &'static [Self];

    fn name(self) -> String {
        format!("{self:?}")
    }

    fn parse(text: &str) -> Option<Self> {
        Self::all().iter().copied().find(|v| v.name() == text)
    }

    fn index(self) -> usize {
        Self::all().iter().position(|v| *v == self).unwrap_or(0)
    }
}

macro_rules! named {
    ($($ty:ty),* $(,)?) => {$(
        impl Named for $ty {
            fn all() -> &'static [Self] {
                <$ty>::ALL
            }
        }
    )*};
}

named!(
    Race, Class, Aspect, Source, Form, Stat, StateKind, Category, Purpose, Target, Effect, Trigger
);

impl Named for ModKind {
    fn all() -> &'static [Self] {
        &ModKind::ALL
    }
}

fn unknown<E: Named>(text: &str) -> String {
    let names: Vec<String> = E::all().iter().map(|v| v.name()).collect();
    format!(
        "unknown {} `{text}`, expected one of: {}",
        std::any::type_name::<E>()
            .rsplit("::")
            .next()
            .unwrap_or("value"),
        names.join(", ")
    )
}

/// `#[serde(with = "named::variant")]`: one variant as its name.
pub mod variant {
    use super::*;

    pub fn serialize<E: Named, S: Serializer>(v: &E, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.name())
    }

    pub fn deserialize<'de, E: Named, D: Deserializer<'de>>(d: D) -> Result<E, D::Error> {
        let text = String::deserialize(d)?;
        E::parse(&text).ok_or_else(|| D::Error::custom(unknown::<E>(&text)))
    }
}

/// `#[serde(with = "named::list")]`: a list of variants by name.
pub mod list {
    use serde::ser::SerializeSeq;

    use super::*;

    pub fn serialize<E: Named, S: Serializer>(v: &[E], s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for e in v {
            seq.serialize_element(&e.name())?;
        }
        seq.end()
    }

    pub fn deserialize<'de, E: Named, D: Deserializer<'de>>(d: D) -> Result<Vec<E>, D::Error> {
        Vec::<String>::deserialize(d)?
            .iter()
            .map(|t| E::parse(t).ok_or_else(|| D::Error::custom(unknown::<E>(t))))
            .collect()
    }
}

/// A table indexed by an enum, as a JSON object keyed by variant name. Missing keys stay 0.
fn ser_keyed<E: Named, T: serde::Serialize, S: Serializer>(
    v: &[T],
    s: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = s.serialize_map(Some(v.len()))?;
    for (e, t) in E::all().iter().zip(v) {
        map.serialize_entry(&e.name(), t)?;
    }
    map.end()
}

fn de_keyed<'de, E: Named, T, const N: usize, D>(d: D) -> Result<[T; N], D::Error>
where
    T: Deserialize<'de> + Default + Copy,
    D: Deserializer<'de>,
{
    let map = std::collections::BTreeMap::<String, T>::deserialize(d)?;
    let mut out = [T::default(); N];
    for (key, value) in map {
        let e = E::parse(&key).ok_or_else(|| D::Error::custom(unknown::<E>(&key)))?;
        if let Some(slot) = out.get_mut(e.index()) {
            *slot = value;
        }
    }
    Ok(out)
}

/// `#[serde(with = "named::by_stat")]`: `[u16; STATS_COUNT]` keyed by `Stat`.
pub mod by_stat {
    use super::*;
    use deadroll_types::STATS_COUNT;

    pub fn serialize<S: Serializer>(v: &[u16; STATS_COUNT], s: S) -> Result<S::Ok, S::Error> {
        ser_keyed::<Stat, _, _>(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u16; STATS_COUNT], D::Error> {
        de_keyed::<Stat, _, STATS_COUNT, _>(d)
    }
}

/// `#[serde(with = "named::by_aspect")]`: `[u32; Aspect::COUNT]` keyed by `Aspect`, zeros left out.
pub mod by_aspect {
    use super::*;
    use serde::ser::SerializeMap;

    pub fn serialize<S: Serializer>(v: &[u32; Aspect::COUNT], s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for (e, t) in Aspect::ALL.iter().zip(v) {
            if *t != 0 {
                map.serialize_entry(&e.name(), t)?;
            }
        }
        map.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u32; Aspect::COUNT], D::Error> {
        de_keyed::<Aspect, _, { Aspect::COUNT }, _>(d)
    }
}
