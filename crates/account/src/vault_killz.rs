//! The vault kill totals.
//!
//! Indices 0..3 are the raw kills on maps 14, 24, 13 and 8; 4..7 their floored
//! logs; 8 the completed tasks; 9 the capped bubble levels; 10 and 11 the same
//! pair for map 101.

use crate::Decoder;
use idleon_save::fields;
use idleon_stats_data as stats;

/// The vault kill-count columns, by the index the vault upgrades read.
pub fn vault_killz(decoder: &Decoder<'_>) -> [f64; 12] {
    let mut result = [0.0; 12];
    let kills = |map: usize| -> f64 {
        let mut total = 0.0;
        for character in 0..decoder.characters().len() {
            let Some(row) = fields::char_field(decoder.save(), "KLA", character) else {
                continue;
            };
            let Some(row) = fields::object_row(&row, map) else {
                continue;
            };
            let left = row.first().copied().flatten().unwrap_or(0.0);
            total += (stats::kill_requirement(map) - left).max(0.0);
        }
        total
    };

    for (index, map) in [14usize, 24, 13, 8].into_iter().enumerate() {
        result[index] = kills(map);
    }
    for index in 0..4 {
        result[4 + index] = floored_log(result[index]);
    }

    // `tasksGlobalData[3]` is `TaskZZ3`: its rows hold the completed flag per task.
    let mut completed = 0.0;
    if let Some(rows) = fields::field(decoder.save(), "TaskZZ3") {
        for world in 0..4 {
            let Some(tasks) = fields::object_row(&rows, world) else {
                continue;
            };
            completed += tasks.iter().filter(|task| **task == Some(1.0)).count() as f64;
        }
    }
    result[8] = completed;

    let mut bubbles = 0.0;
    if let Some(cauldrons) = fields::field(decoder.save(), "CauldronInfo") {
        for cauldron in 0..4 {
            let Some(levels) = fields::object_row(&cauldrons, cauldron) else {
                continue;
            };
            for level in levels.into_iter().flatten() {
                bubbles += level.min(100.0);
            }
        }
    }
    result[9] = bubbles;

    result[10] = kills(101);
    result[11] = floored_log(result[10]);
    result
}

/// `getLOG(value)`: the floored base-10 log, `0` below ten.
fn floored_log(value: f64) -> f64 {
    if value < 10.0 {
        0.0
    } else {
        value.log10().floor()
    }
}
