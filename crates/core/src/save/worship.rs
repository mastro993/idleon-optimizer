//! The Worship Tower Defense simulator's save-derived inputs.
//!
//! Mirrors `worshipTdInputs` in `js/stats/systems/w3/worship-td.js` for the
//! fields the live simulator reads: tower trial levels, the best-wave table,
//! `Atom 5`, `Dream 7`, Salt Lick 6, the Menagerie pre-cook floor and the
//! super-bit totem HP. Account bonuses the simulator does not model yet (start
//! points, kill-point meals, meritocracy) stay neutral and are named.

use super::json;
use crate::worship::{Inputs, data};
use serde_json::{Map, Value};

/// One cell of a decoded row.
fn cell(value: &Value, index: usize) -> Option<&Value> {
    match value {
        Value::Array(rows) => rows.get(index),
        Value::Object(map) => map.get(&index.to_string()),
        _ => None,
    }
}

fn field(save: &Map<String, Value>, row: &str) -> Option<Value> {
    json::parsed_field(save, row)
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(json::number)
}

fn level(value: f64) -> u32 {
    value.max(0.0).floor() as u32
}

/// Read every simulator input the save carries.
pub(crate) fn inputs(save: &Map<String, Value>, warnings: &mut Vec<String>) -> Inputs {
    let mut inputs = Inputs::default();

    match field(save, "Tower") {
        Some(tower) => {
            let mut levels = [0; 9];
            let mut any = false;
            for (index, level_slot) in levels.iter_mut().enumerate() {
                if let Some(value) = number(cell(&tower, 9 + index)) {
                    *level_slot = level(value);
                    any = true;
                }
            }
            if any {
                inputs.tower_levels = levels;
            } else {
                warnings.push("Tower: no worship tower levels 9-17".into());
            }
        }
        None => warnings.push("Tower: no tower trial levels".into()),
    }

    match field(save, "TotemInfo") {
        Some(totem) => {
            for (trial, best) in inputs.best_waves.iter_mut().enumerate() {
                let Some(waves) = cell(&totem, 0) else {
                    break;
                };
                *best = number(cell(waves, trial)).map(level).unwrap_or(0);
            }
        }
        None => warnings.push("TotemInfo: no best-wave table".into()),
    }

    if let Some(dream) = field(save, "Dream") {
        inputs.dream7 = number(cell(&dream, 7)).unwrap_or(0.0);
    } else {
        warnings.push("Dream: no Dream 7 wave-clock bonus".into());
    }

    if let Some(salt) = field(save, "SaltLick") {
        inputs.salt_lick_x = 1.0 + number(cell(&salt, 6)).unwrap_or(0.0) / 100.0;
    } else {
        warnings.push("SaltLick: no Salt Lick 6 skull multiplier".into());
    }

    // `Totem HP = 5 + 5 x Super Bit 6 + 10 x Super Bit 15`.
    let gaming = field(save, "Gaming").and_then(|gaming| {
        cell(&gaming, 12)
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let data = data();
    let has_super_bit = |index: usize| {
        let Some(token) = data.super_bits.get(&index) else {
            return false;
        };
        gaming
            .as_deref()
            .is_some_and(|flags| flags.contains(token.as_str()))
    };
    inputs.totem_hp =
        5.0 + if has_super_bit(6) { 5.0 } else { 0.0 } + if has_super_bit(15) { 10.0 } else { 0.0 };

    match field(save, "Menagerie") {
        Some(menagerie) => {
            let unlocked = cell(&menagerie, 2)
                .and_then(|row| number(cell(row, 18)))
                .is_some_and(|owned| owned >= 1.0);
            inputs.first_trait_multi = if unlocked { 2.0 } else { 1.0 };
            let boost = cell(&menagerie, 3)
                .and_then(|row| number(cell(row, 15)))
                .map(level)
                .unwrap_or(0);
            inputs.slow_roast_pre_cook_sec =
                (data.menagerie_boost_15_per_level * f64::from(boost)).min(14400.0);
        }
        None => warnings.push("Menagerie: no Slow Roast boost or first-trait unlock".into()),
    }

    inputs
}
