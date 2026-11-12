//! Minehead (Depth Charge): the formulas, the floor simulation, the strategy
//! optimizer and the save-derived inputs the research page's Minehead tab
//! renders.
//!
//! Ported from `js/stats/systems/w7/minehead.js`, `js/stats/data/w7/minehead.js`
//! and `js/minehead/sim.js`. The static tables come from
//! `script/extract-minehead-data.mjs`; the tests compare the port with
//! `script/minehead-oracle.mjs`, which runs the upstream calculator itself.

pub mod data;
pub mod infer;
pub mod inputs;
pub mod math;
pub mod optimize;
pub mod rank;
pub mod sim;

pub use data::game;
pub use infer::{Inferred, Recorded, Spatial, SpatialReport, infer};
pub use inputs::{Inputs, inputs};
pub use math::{ShapeContext, ShapeKind, ShapeValue};
pub use optimize::{
    Candidate, Grid, Optimized, Path, PathInputs, Step, cost_path, default_grid, get, optimize, set,
};
pub use rank::{RankOutcome, RankRow, rank_upgrades};
pub use sim::{
    Ctx, Decision, Game, GameResult, MonteCarlo, Options, Outcome, Params, Reveal, Rng,
    monte_carlo, simulate_game,
};

/// The upgrade description with its markers filled in, as the dashboard shows
/// it. Ported from `fmtUpgDesc` (`js/ui/minehead-helpers.js`).
pub fn upgrade_desc(inputs: &Inputs, t: usize) -> String {
    let data = game();
    let Some(upgrade) = data.upgrade(t) else {
        return String::new();
    };
    let levels = &inputs.levels;
    let level = levels.get(t).copied().unwrap_or(0.0);
    let qty = math::upgrade_qty(data, t, level, math::doubler_multi(&inputs.doublers, t));
    let mut desc = upgrade
        .desc
        .replace('{', &thousands(qty))
        .replace('}', &format!("{:.2}", 1.0 + qty / 100.0));
    match t {
        1 => {
            if level >= 17.0 {
                desc = "Your max possible number is 19, the MAXIMUM!".into();
            } else {
                let max = (1.0 + (qty + 1.0 + ((qty + 1.0) / 9.0).floor().min(1.0))).round();
                desc = desc.replace('$', &max.to_string());
            }
        }
        2 => {
            if level >= 16.0 {
                desc = "Your grid is 12x6, a full grid!".into();
            } else {
                let next = data
                    .grid_dims
                    .get(level as usize + 1)
                    .or_else(|| data.grid_dims.get(level as usize))
                    .map(String::as_str)
                    .unwrap_or("3,3");
                let current = data
                    .grid_dims
                    .get(level as usize)
                    .map(String::as_str)
                    .unwrap_or("3,3");
                let dims = |text: &str| {
                    let mut parts = text
                        .split(',')
                        .map(|part| part.parse::<f64>().unwrap_or(0.0));
                    (parts.next().unwrap_or(0.0), parts.next().unwrap_or(0.0))
                };
                let (next_cols, next_rows) = dims(next);
                desc = format!(
                    "Expands grid to {next_cols}x{next_rows} ({} tiles), current {current}",
                    next_cols * next_rows
                );
            }
        }
        12 => {
            if qty == 0.0 {
                desc = "Unlocks Multiplier Tiles!".into();
            } else {
                const VALUES: [f64; 14] = [
                    1.0, 1.2, 1.4, 1.6, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0, 8.0, 8.0,
                ];
                let value = VALUES.get(qty as usize).copied().unwrap_or(1.0);
                desc = desc.replace('$', &format!("{value}x"));
            }
        }
        14 => {
            let multi = math::bluecrown_multi(data, &inputs.qty_levels);
            desc = desc.replace('$', &format!("{multi:.2}x"));
        }
        17 => {
            if qty == 0.0 {
                desc = "Unlocks Additive Tiles!".into();
            } else {
                const ADDITIVE: [f64; 11] = [
                    0.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
                ];
                let value = ADDITIVE.get(qty as usize).copied().unwrap_or(0.0);
                desc = desc.replace('$', &format!("{value}%"));
            }
        }
        23 => {
            let odds = math::jackpot_odds(data, &inputs.qty_levels);
            if odds == 0.0 {
                desc = "Unlocks JACKPOT Tiles!".into();
            } else {
                desc = desc.replace('$', &format!("{}", (1.0 / odds).ceil() as i64));
            }
        }
        24 => {
            let tiles = math::jackpot_tiles(data, &inputs.qty_levels);
            desc = desc.replace('$', &format!("{}", tiles.ceil() as i64));
        }
        26 => {
            let discount = (1e4 * (1.0 - 1.0 / (1.0 + qty / 100.0))).round() / 100.0;
            desc = desc.replace('$', &format!("{discount}"));
        }
        28 => {
            let log10 = if inputs.highest_dmg > 0.0 {
                inputs.highest_dmg.log10()
            } else {
                0.0
            };
            desc = format!(
                "+{}% Mine Currency per POW 10 of best hit, +{}% total",
                thousands(qty),
                format_number(qty * log10)
            );
        }
        _ => {}
    }
    desc
}

/// A number with the thousands separators `toLocaleString` writes.
pub fn thousands(value: f64) -> String {
    let rounded = value.round();
    let negative = rounded < 0.0;
    let text = format!("{}", rounded.abs() as i64);
    let mut out = String::with_capacity(text.len() + text.len() / 3 + 1);
    for (index, character) in text.chars().enumerate() {
        if index > 0 && (text.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(character);
    }
    if negative { format!("-{out}") } else { out }
}

/// The dashboard's number format: two significant decimals under a thousand,
/// then a rounded integer.
pub fn format_number(value: f64) -> String {
    if value.abs() < 1000.0 {
        let rounded = (value * 100.0).round() / 100.0;
        if (rounded.fract()).abs() < f64::EPSILON {
            format!("{rounded:.0}")
        } else {
            format!("{rounded:.2}")
        }
    } else {
        thousands(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_matches_the_locale_string() {
        assert_eq!(thousands(1234.0), "1,234");
        assert_eq!(thousands(12.0), "12");
        assert_eq!(thousands(1_234_567.0), "1,234,567");
    }
}
