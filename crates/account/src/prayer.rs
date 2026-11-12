//! The prayer bonus a character holds, ported from `computePrayerReal`
//! (`js/stats/systems/w3/prayer.js`).
//!
//! A prayer pays its base bonus scaled by its level while it is equipped. With
//! the character's every slot empty it pays the no-prayer passive instead: a
//! fifth of the base per unlocked Super Bit (bits 9, 39 and 53). The base comes
//! from the shared prayer table; `cost_column` selects its curse column, which
//! is the cost a curse prayer takes rather than the bonus it grants.

use idleon_save::fields;
use idleon_stats_data as data;
use serde_json::{Map, Value};

/// `computePrayerReal(index, costColumn, character, saveData)`.
pub fn prayer(save: &Map<String, Value>, character: usize, index: usize, cost_column: bool) -> f64 {
    let Some(row) = data::capacity().prayers.get(index) else {
        return 0.0;
    };
    let base = if cost_column { row[1] } else { row[0] };
    let level = fields::at(save, "PrayOwned", &[index]).unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    let scale = (1.0 + (level - 1.0) / 10.0).max(1.0);
    // `Number(cell)` reads a hole as 0, so a sparse slot counts as prayer 0.
    let equipped: Vec<f64> = fields::numbers(save, &format!("Prayers_{character}"))
        .unwrap_or_default()
        .into_iter()
        .map(|cell| cell.unwrap_or(0.0))
        .collect();
    if equipped.contains(&(index as f64)) {
        return (base * scale).round();
    }
    let no_prayers_equipped = !equipped.is_empty() && !equipped.iter().any(|value| *value >= 0.0);
    let passive_enabled =
        no_prayers_equipped && (superbit(save, 9) > 0.0 || superbit(save, 39) > 0.0);
    // Prayer 5 is the one the passive never pays, and neither does a curse.
    let passive = if passive_enabled && index != 5 && !cost_column {
        0.2 * (superbit(save, 9) + superbit(save, 39) + superbit(save, 53))
    } else {
        0.0
    };
    if passive > 0.0 {
        (passive * base * scale).round()
    } else {
        0.0
    }
}

/// `superBitType(idx, Gaming[12])`.
fn superbit(save: &Map<String, Value>, index: usize) -> f64 {
    let text = fields::field(save, "Gaming")
        .and_then(|value| {
            idleon_save::json::index(&value, &[12]).map(|cell| match cell {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
        })
        .unwrap_or_default();
    let letter = data::capacity().number_letter.chars().nth(index);
    if letter.is_some_and(|letter| text.contains(letter)) {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn save(prayers: &str, owned: &str, gaming: &str) -> Map<String, Value> {
        json!({
            "PrayOwned": owned,
            "Prayers_0": prayers,
            "Gaming": format!("[\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",{gaming}]"),
        })
        .as_object()
        .expect("an object")
        .clone()
    }

    #[test]
    fn an_equipped_prayer_pays_its_base_scaled_by_level() {
        // Prayer 8's base is 40; level 50 scales it by 5.9.
        let save = save("[8]", "[0,0,0,0,0,0,0,0,50]", "\"\"");
        assert_eq!(prayer(&save, 0, 8, false), 236.0);
    }

    #[test]
    fn empty_slots_pay_the_super_bit_passive() {
        let save = save("[-1,-1]", "[0,0,0,0,0,0,0,0,50]", "\"iM肥\"");
        assert_eq!(prayer(&save, 0, 8, false), 142.0);
    }

    #[test]
    fn a_worn_prayer_suppresses_the_passive() {
        let save = save("[0]", "[0,0,0,0,0,0,0,0,50]", "\"iM肥\"");
        assert_eq!(prayer(&save, 0, 8, false), 0.0);
    }

    #[test]
    fn an_unowned_prayer_pays_nothing() {
        let save = save("[8]", "[0]", "\"iM肥\"");
        assert_eq!(prayer(&save, 0, 8, false), 0.0);
    }

    #[test]
    fn the_cost_column_is_the_curse_value() {
        let save = save("[8]", "[0,0,0,0,0,0,0,0,50]", "\"\"");
        assert_eq!(prayer(&save, 0, 8, true), 1180.0);
    }
}
