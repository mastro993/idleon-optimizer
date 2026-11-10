//! The JSON helpers both sides of the import read the save through.

use serde_json::{Map, Value};

/// A save field that may be a JSON value or a string holding one, as the
/// upstream `parseSaveKey` helper reads it.
pub(crate) fn parsed_field(save: &Map<String, Value>, key: &str) -> Option<Value> {
    match save.get(key)? {
        Value::String(text) => serde_json::from_str(text).ok(),
        value => Some(value.clone()),
    }
}

/// Index into nested arrays, returning `None` for a missing or non-array step.
pub(crate) fn index<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for step in path {
        current = current.as_array()?.get(*step)?;
    }
    Some(current)
}

/// A number, or a string holding one; the game writes some values as strings.
pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}
