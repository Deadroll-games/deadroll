//! `BalanceConfig` as JSON. Loading lays the file over the defaults: missing keys keep the default,
//! arrays merge index by index (enum variants are append-only), so a file saved before a new
//! variant or a new field still loads. The item table ([`ItemTuning`]) also loads and saves on
//! its own: the Items tab keeps item presets in `presets/items/`.
use deadroll_rules::{BalanceConfig, ItemTuning};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

pub fn to_json(cfg: &BalanceConfig) -> String {
    serde_json::to_string_pretty(cfg).unwrap_or_default() + "\n"
}

pub fn from_json(text: &str) -> Result<BalanceConfig, String> {
    overlay(&BalanceConfig::DEFAULT, text)
}

pub fn items_to_json(items: &ItemTuning) -> String {
    serde_json::to_string_pretty(items).unwrap_or_default() + "\n"
}

pub fn items_from_json(text: &str) -> Result<ItemTuning, String> {
    overlay(&ItemTuning::DEFAULT, text)
}

/// `text` laid over `default`.
fn overlay<T: Serialize + DeserializeOwned>(default: &T, text: &str) -> Result<T, String> {
    let over: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut base = serde_json::to_value(default).map_err(|e| e.to_string())?;
    merge(&mut base, over);
    serde_json::from_value(base).map_err(|e| e.to_string())
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (key, value) in o {
                if let Some(slot) = b.get_mut(&key) {
                    merge(slot, value);
                }
            }
        }
        (Value::Array(b), Value::Array(o)) => {
            for (slot, value) in b.iter_mut().zip(o) {
                merge(slot, value);
            }
        }
        (slot, value) => *slot = value,
    }
}
