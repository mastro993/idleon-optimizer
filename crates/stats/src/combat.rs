//! The combat simulator's save-derived inputs (CS-06).
//!
//! Ported from `combatProgressInputs` in
//! `js/stats/systems/common/combat-sim-progress.js` and the Den's config in
//! `js/stats/systems/common/combat-sim-special.js`. The account-wide terms are
//! read through [`idleon_account::Decoder`], which sits beside this crate, and
//! the stamp, bribe and shiny lookups reuse the capacity breakdown's ports.
//!
//! The terms this build cannot decode are returned as warnings rather than
//! guessed: the star-sign 57b term of the shiny-pet speed and the per-kill
//! forge, pet and alchemy grants (talents 280, 362 and 490) need the talent and
//! star-sign formula tables, so they read neutral here and the panel names them.

use crate::breakdowns::capacity;
use idleon_account::Decoder;
use idleon_combat::den;
use idleon_combat::progress::{Config, Coral, Forge, Omega, Pets, Scurge};
use idleon_research::model::Simulation;
use idleon_save::fields;
use idleon_save::{Save, json};
use serde_json::Value;
use std::collections::BTreeMap;

/// The star-sign term `computeShinyBonusS` leaves out here.
pub const STAR_SIGN: &str = "Star sign 57b is not decoded for the shiny-pet speed; its term reads 0 and the speed can be low.";
/// The per-kill grants the talent formulas would need.
pub const TALENT_GRANTS: &str = "Per-kill forge, pet and alchemy time (talents 280, 362 and 490) is not \
    decoded; the furnace still advances with the run's clock and the panel reports the credit as zero.";

/// The progress config and the missing inputs for one character.
pub fn progress(save: &Save, character: usize) -> (Config, Vec<String>) {
    let mut warnings = vec![STAR_SIGN.to_string(), TALENT_GRANTS.to_string()];
    let Some((fields_map, root)) = save.parts() else {
        return (Config::default(), warnings);
    };
    let characters = idleon_save::characters(fields_map);
    let simulation = Simulation::default();
    let mut decoder = Decoder::new(fields_map, root, &characters, &simulation, &mut warnings);
    let forge = forge(save, &mut decoder);
    let pets = pets(save, &mut decoder);
    let scurge = Scurge {
        unlocked: decoder.menagerie_unlock(4) == 1.0,
        menagerie: rows(save, "Menagerie"),
    };
    let coral = Coral {
        unlocked: decoder.menagerie_unlock(2) == 1.0,
        kills: option(save, 623),
    };
    let balls = option_value(save, 625);
    let drops = option_value(save, 626);
    let omega = Omega {
        unlocked: decoder.menagerie_unlock(6) == 1.0,
        available: balls.is_some() && drops.is_some(),
        balls: balls.unwrap_or(0.0),
        drops: drops.unwrap_or(0.0),
    };
    let _ = character;
    (
        Config {
            forge,
            pets,
            scurge,
            coral,
            omega,
        },
        warnings,
    )
}

/// The Den attempt's inputs, when the save carries its `Holes` rows.
pub fn den(save: &Save, _character: usize) -> Option<den::Config> {
    fields::at(save.as_object()?, "Holes", &[0, 0])?;
    let mut amps = [0.0; 8];
    for (index, level) in amps.iter_mut().enumerate() {
        *level = fields::at(save.as_object()?, "Holes", &[12, index]).unwrap_or(0.0);
    }
    Some(den::Config {
        amps,
        b_upg23: if fields::at(save.as_object()?, "Holes", &[13, 23]).unwrap_or(0.0) >= 1.0 {
            1.0
        } else {
            0.0
        },
        bolaia_level: fields::at(save.as_object()?, "Holes", &[26, 2]).unwrap_or(0.0),
        best_score: fields::at(save.as_object()?, "Holes", &[11, 8]).unwrap_or(0.0),
        opal_level: fields::at(save.as_object()?, "Holes", &[7, 2]).unwrap_or(0.0),
        hound_hp: den::hound_hp(),
    })
}

/// `combatProgressInputs`'s forge half.
fn forge(save: &Save, decoder: &mut Decoder) -> Forge {
    let Some(fields_map) = save.as_object() else {
        return Forge::default();
    };
    let order = string_rows(fields_map, "ForgeItemOrder");
    let qty = flat(save, "ForgeItemQty");
    let progress_rows = flat(save, "ForgeIntProg");
    let levels: Vec<f64> = fields::numbers(fields_map, "ForgeLV")
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.unwrap_or(0.0))
        .collect();
    let level = |index: usize| levels.get(index).copied().unwrap_or(0.0);
    let (storage, storage_available) = chest(fields_map);
    let godshard = decoder.card_level("Godshard");
    let cap = (decoder.cosmo_bonus(2, 1)
        * (20.0 + 10.0 + 10.0 * level(1).round())
        * (1.0 + (decoder.arcade(26) + 30.0 * godshard) / 100.0)
        * (1.0 + decoder.stamp("ForgeCap") / 100.0)
        * (1.0 + (bribe(save, 35) + decoder.vault(26)) / 100.0)
        * (1.0
            + (50.0 * decoder.achievement(281)
                + 25.0 * f64::from(decoder.rift_skill_rank(1) > 1))
                / 100.0))
        .min(2e9);
    Forge {
        available: order.len() >= 48 && qty.len() >= 48 && progress_rows.len() >= 16,
        order,
        qty,
        progress: progress_rows,
        storage,
        storage_available,
        slots: level(0),
        cap,
        refill: fields::at(fields_map, "Lab", &[16, 0]).unwrap_or(0.0) == 1.0,
        gem_slots: fields::row(fields_map, "GemItemsPurchased", &[])
            .map(|row| fields::number_at(&row, 104))
            .unwrap_or(0.0),
        speed: ((5.0 * level(2) + 30.0 * godshard + 100.0)
            * (1.0 + decoder.menagerie_boost_qty(11) / 100.0))
            .round()
            / 100.0,
        bars: 1.0 + level(4).round() / 100.0,
        oil_use: 1.0 - bribe(save, 9) / 100.0,
    }
}

/// `combatProgressInputs`'s pet half.
fn pets(save: &Save, decoder: &mut Decoder) -> Pets {
    let Some(fields_map) = save.as_object() else {
        return Pets::default();
    };
    let breeding = rows(save, "Breeding");
    let deployed = deployed_pets(fields_map);
    let available = fields::field(fields_map, "Pets").is_some()
        && fields::field(fields_map, "Breeding").is_some();
    let unlock = fields::at(fields_map, "Menagerie", &[2, 11]).unwrap_or(0.0);
    let thresholds = (19.0 + if unlock >= 1.0 { 10.0 } else { 0.0 }) as usize;
    let over_levels = over_levels(&breeding, thresholds);
    let breeding_2_12 = breeding
        .get(2)
        .and_then(|row| row.get(12))
        .copied()
        .unwrap_or(0.0);
    let shiny_speed = (1.0
        + (decoder.mainframe(115).unwrap_or(0.0)
            + (capacity::shiny_bonus(save, 16)
                + (decoder.crop_scientist(5) + 5.0 * breeding_2_12)))
            / 100.0)
        * (1.0 + decoder.win_bonus(17) / 100.0)
        * (1.0 + 10.0 * over_levels / 100.0)
        * (1.0 + decoder.lamp(0, 1) / 100.0);
    Pets {
        available,
        deployed,
        breeding,
        shiny_speed,
    }
}

/// `overLevels`: the shiny levels above 20 across every pet world.
fn over_levels(breeding: &[Vec<f64>], thresholds: usize) -> f64 {
    let worlds = capacity::shiny_pet_worlds();
    let mut total = 0.0;
    for (world, pets) in worlds.iter().take(4).enumerate() {
        let Some(row) = breeding.get(22 + world) else {
            continue;
        };
        for pet in 0..*pets {
            let exp = row.get(pet).copied().unwrap_or(0.0);
            total += (capacity::shiny_pet_level(exp, thresholds) - 20.0).max(0.0);
        }
    }
    total.round()
}

/// The `Pets` rows as the strings the panel's slots name.
fn deployed_pets(fields_map: &serde_json::Map<String, Value>) -> Vec<Vec<String>> {
    let Some(Value::Array(pets)) = fields::field(fields_map, "Pets") else {
        return Vec::new();
    };
    pets.iter()
        .map(|row| {
            row.as_array()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| match cell {
                            Value::String(text) => text.clone(),
                            other => json::number(other)
                                .map(|value| format!("{value}"))
                                .unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// The `ChestOrder`/`ChestQuantity` pair as the refill rule reads it.
fn chest(fields_map: &serde_json::Map<String, Value>) -> (BTreeMap<String, f64>, bool) {
    let order = string_rows(fields_map, "ChestOrder");
    let quantity = fields::numbers(fields_map, "ChestQuantity").unwrap_or_default();
    let available = fields::field(fields_map, "ChestOrder").is_some()
        && fields::field(fields_map, "ChestQuantity").is_some();
    let mut storage = BTreeMap::new();
    for (index, key) in order.iter().enumerate() {
        *storage.entry(key.clone()).or_insert(0.0) +=
            quantity.get(index).copied().flatten().unwrap_or(0.0);
    }
    (storage, available)
}

/// A flat-array field as numbers, coercing the save's string cells.
fn flat(save: &Save, key: &str) -> Vec<f64> {
    let Some(Value::Array(cells)) = save.get(key) else {
        return Vec::new();
    };
    cells
        .iter()
        .map(|cell| json::number(cell).unwrap_or(0.0))
        .collect()
}

/// A nested-array field as numeric rows, coercing the save's string cells.
fn rows(save: &Save, key: &str) -> Vec<Vec<f64>> {
    let Some(Value::Array(rows)) = save.get(key) else {
        return Vec::new();
    };
    rows.iter()
        .map(|row| {
            row.as_array()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| json::number(cell).unwrap_or(0.0))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

fn string_rows(fields_map: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    let Some(Value::Array(rows)) = fields::field(fields_map, key) else {
        return Vec::new();
    };
    rows.iter()
        .map(|entry| entry.as_str().unwrap_or_default().to_string())
        .collect()
}

/// `OptionsListAccount[index]`.
fn option_value(save: &Save, index: usize) -> Option<f64> {
    let options = save
        .get("OptionsListAccount")
        .or_else(|| save.get("OptLacc"))?;
    json::index(&options, &[index]).and_then(json::number)
}

fn option(save: &Save, index: usize) -> f64 {
    option_value(save, index).unwrap_or(0.0)
}

/// `getBribeBonus(index)` through the capacity table.
fn bribe(save: &Save, index: usize) -> f64 {
    capacity::bribe_bonus(save, index)
}
