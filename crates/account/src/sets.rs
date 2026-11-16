//! Set bonuses.
//!
//! A set pays when the save lists it as a permanent unlock (`OptionsListAccount
//! [379]`) or when the character has the pieces equipped; the bonus itself comes
//! from `EquipmentSets[name][3][2]`.

use crate::Decoder;
use idleon_save::fields;
use idleon_stats_data as stats;
use serde_json::Value;

/// `getSetBonus(name, character)`: the permanent unlock, or the equipped set.
pub fn set_bonus(decoder: &Decoder<'_>, name: &str, character: usize) -> f64 {
    if decoder.ola_text(379).contains(name) || set_equipped(decoder, name, character) {
        stats::set_bonus(name)
    } else {
        0.0
    }
}

pub fn set_equipped(decoder: &Decoder<'_>, name: &str, character: usize) -> bool {
    let Some(set) = stats::equipment_set(name) else {
        return false;
    };
    let Some(order) = fields::char_field(decoder.save(), "EquipOrder", character) else {
        return false;
    };
    let row = |index: usize| -> &[Value] {
        order
            .get(index)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    };
    let (armor, tools) = (row(0), row(1));
    let item = |row: &[Value], slot: usize| -> Option<String> {
        row.get(slot).and_then(Value::as_str).map(str::to_owned)
    };

    let mut parts = 0;
    for slot in 0..16 {
        if item(armor, slot).is_some_and(|name| set.armor.contains(&name)) {
            parts += 1;
        }
    }
    if set.tools_cap > 0.0 {
        let mut found = 0;
        for slot in 0..8 {
            if found as f64 >= set.tools_cap {
                break;
            }
            if item(tools, slot).is_some_and(|name| set.tools.contains(&name)) {
                parts += 1;
                found += 1;
            }
        }
    }
    if set.special_cap == 1.0
        && (0..16).any(|slot| item(armor, slot).is_some_and(|name| set.special.contains(&name)))
    {
        parts += 1;
    }
    let required = set.armor.len() as f64 + set.tools_cap + set.special_cap;
    parts as f64 >= required
}
