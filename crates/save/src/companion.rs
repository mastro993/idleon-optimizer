//! The `it.json` companion roster, shared by the sections that read companions.
//!
//! The wrapper lists one `"<id>,<name>,…,<level-two flag>"` string per owned
//! companion beside the save. A plain `save.json` has no roster, so a section
//! reading a companion bonus reports it as unavailable instead of guessing.

use serde_json::Value;
use std::collections::BTreeSet;

/// The companions an `it.json` wrapper carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roster {
    /// Companion ids owned.
    pub owned: BTreeSet<u32>,
    /// Companion ids at level two, whose bonus differs from level one.
    pub enhanced: BTreeSet<u32>,
    /// Whether the wrapper carried a roster at all.
    pub available: bool,
}

impl Roster {
    /// Whether the save owns a companion.
    pub fn owns(&self, id: u32) -> bool {
        self.owned.contains(&id)
    }

    /// Whether a companion is at level two.
    pub fn is_enhanced(&self, id: u32) -> bool {
        self.enhanced.contains(&id)
    }

    /// Read the roster beside a save; a missing or unreadable list is unavailable.
    pub fn read(companion: Option<&Value>) -> Roster {
        let Some(entries) = companion
            .and_then(|root| at_key(root, "l"))
            .and_then(companion_entries)
        else {
            return Roster::default();
        };
        let mut roster = Roster {
            available: true,
            ..Roster::default()
        };
        for (id, enhanced) in entries {
            roster.owned.insert(id);
            if enhanced {
                roster.enhanced.insert(id);
            }
        }
        roster
    }
}

/// The entries of a `companion.l` list in list order: each companion id and
/// whether it is at level two. `None` when the list is neither an array nor
/// an object with numeric keys.
pub fn companion_entries(list: &Value) -> Option<Vec<(u32, bool)>> {
    let entries: Vec<&str> = match list {
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort_by_key(|key| key.parse::<u64>().unwrap_or(u64::MAX));
            keys.into_iter()
                .filter_map(|key| fields.get(key).and_then(Value::as_str))
                .collect()
        }
        _ => return None,
    };
    Some(
        entries
            .into_iter()
            .filter_map(|entry| {
                let fields: Vec<&str> = entry.split(',').collect();
                let id = fields.first()?.trim().parse::<u32>().ok()?;
                let level_two = fields
                    .get(4)
                    .and_then(|flag| flag.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                Some((id, level_two >= 1.0))
            })
            .collect(),
    )
}

/// Index into an array, or a named field in an object.
fn at_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
        _ => None,
    }
}
