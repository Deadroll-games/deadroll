//! The whole state of the bench as one JSON value, and what changed between two of them.
//!
//! A snapshot holds every number a Run depends on: the config, the heroes, the skills, the
//! monsters (the registry bestiary and its skill pool) and the encounters. Changes are read off the
//! flattened leaves, named by their path: `config.combat.crit_more_bp`, `heroes.elf_mage.level`.
//! An array of records is keyed by the record's `name` (or `id`) when it has one, so adding a
//! monster does not shift the names of the others.
use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

fn to_value(v: &impl serde::Serialize) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Everything a Run depends on. `monsters` is the bestiary as saved plus its skill pool.
pub fn snapshot(
    cfg: &impl serde::Serialize,
    heroes: &impl serde::Serialize,
    skills: &impl serde::Serialize,
    encounters: &impl serde::Serialize,
    monsters: Value,
) -> Value {
    json!({
        "config": to_value(cfg),
        "heroes": to_value(heroes),
        "skills": to_value(skills),
        "monsters": monsters,
        "encounters": to_value(encounters),
    })
}

/// The leaves of `value` by path.
pub fn flatten(value: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk(value, String::new(), &mut out);
    out
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn walk(value: &Value, path: String, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                walk(v, join(&path, key), out);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                let label = record_label(v).unwrap_or_else(|| i.to_string());
                walk(v, format!("{path}[{label}]"), out);
            }
        }
        Value::String(s) => {
            out.insert(path, s.clone());
        }
        other => {
            out.insert(path, other.to_string());
        }
    }
}

/// `name` or `id` of a record, so the array is keyed by it.
fn record_label(v: &Value) -> Option<String> {
    let map: &Map<String, Value> = v.as_object()?;
    ["name", "id"].iter().find_map(|k| match map.get(*k)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// One difference: `(name, old, new)`; `None` is a value that is not there.
pub type Change = (String, Option<String>, Option<String>);

/// Every leaf that differs between `old` and `new`, by path.
pub fn diff(old: &Value, new: &Value) -> Vec<Change> {
    let (a, b) = (flatten(old), flatten(new));
    let mut out = Vec::new();
    for (path, was) in &a {
        match b.get(path) {
            Some(now) if now == was => {}
            now => out.push((path.clone(), Some(was.clone()), now.cloned())),
        }
    }
    for (path, now) in &b {
        if !a.contains_key(path) {
            out.push((path.clone(), None, Some(now.clone())));
        }
    }
    out.sort_by(|x, y| x.0.cmp(&y.0));
    out
}

/// `name: old -> new` separated by `; `; empty when nothing changed.
pub fn changes(old: &Value, new: &Value) -> String {
    diff(old, new)
        .into_iter()
        .map(|(name, was, now)| {
            let show = |v: Option<String>| v.unwrap_or_else(|| "(none)".into());
            format!("{name}: {} -> {}", show(was), show(now))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_changed_is_empty() {
        let v = json!({"config": {"a": 1}, "heroes": {"x": {"level": 2}}});
        assert_eq!(changes(&v, &v), "");
    }

    #[test]
    fn a_changed_number_is_named_by_its_path() {
        let old = json!({"config": {"combat": {"crit": 5000}}, "heroes": {"mage": {"level": 1}}});
        let new = json!({"config": {"combat": {"crit": 6000}}, "heroes": {"mage": {"level": 1}}});
        assert_eq!(changes(&old, &new), "config.combat.crit: 5000 -> 6000");
    }

    #[test]
    fn several_changes_are_sorted_and_joined() {
        let old = json!({"b": 1, "a": 1, "c": "x"});
        let new = json!({"b": 2, "a": 3, "c": "y"});
        assert_eq!(changes(&old, &new), "a: 1 -> 3; b: 1 -> 2; c: x -> y");
    }

    #[test]
    fn records_are_keyed_by_name_so_an_insert_does_not_shift_the_rest() {
        let old = json!({"monsters": {"records": [{"name": "wolf", "hp": 10}, {"name": "rat", "hp": 3}]}});
        let new = json!({"monsters": {"records": [{"name": "bat", "hp": 1}, {"name": "wolf", "hp": 12}, {"name": "rat", "hp": 3}]}});
        assert_eq!(
            changes(&old, &new),
            "monsters.records[bat].hp: (none) -> 1; monsters.records[bat].name: (none) -> bat; \
             monsters.records[wolf].hp: 10 -> 12"
        );
    }

    #[test]
    fn plain_arrays_are_keyed_by_position_and_a_removed_value_shows() {
        let old = json!({"stats": [1, 2, 3], "gone": 5});
        let new = json!({"stats": [1, 9, 3]});
        assert_eq!(changes(&old, &new), "gone: 5 -> (none); stats[1]: 2 -> 9");
    }

    #[test]
    fn a_snapshot_has_the_five_parts() {
        let s = snapshot(
            &json!({"a": 1}),
            &json!({}),
            &json!({}),
            &json!({}),
            json!({"m": 2}),
        );
        let keys: Vec<_> = s.as_object().unwrap().keys().cloned().collect();
        for part in ["config", "heroes", "skills", "monsters", "encounters"] {
            assert!(keys.iter().any(|k| k == part), "{part}");
        }
    }
}
