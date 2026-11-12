//! The weekly forecast: which encounters a save can enter in each upcoming week.
//!
//! Ports `killroyForecast` and `killroyWeek` from
//! `js/stats/systems/common/killroy-planner.js`. Every number here is a save
//! read or a game RNG draw; nothing predicts kills or skulls, which come from a
//! trial (see [`super::trial()`]).

use super::{
    KillroyError, MAX_FORECAST_WEEKS, WEEK_SECONDS, class_matches, data, form, game_random, number,
    options, required, room, world3,
};
use idleon_save::Save;
use idleon_save::at;
use serde::Serialize;
use serde_json::Value;

/// Upcoming weeks, their family and map counts, and every saved character's
/// three encounter slots per week.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub weeks: Vec<ForecastWeek>,
    pub family_counts: Vec<FamilyCount>,
    pub map_counts: Vec<MapCount>,
}

/// One week in the rotation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastWeek {
    pub week_offset: usize,
    pub week: i64,
    pub starts_at: f64,
    pub ends_at: f64,
    pub characters: Vec<ForecastCharacter>,
}

/// One saved character and the three encounters the week offers them.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastCharacter {
    pub ci: usize,
    pub name: String,
    pub encounters: Vec<Encounter>,
}

/// One encounter slot for one character.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    pub map_idx: Option<usize>,
    pub map_name: String,
    pub monster: Option<String>,
    pub slot: usize,
    pub seed: i64,
    pub week_offset: usize,
    pub week: i64,
    pub family: usize,
    pub family_name: String,
    pub available: Option<bool>,
    pub eligible: bool,
    pub reason: String,
    pub used: Option<bool>,
    /// The save's monster record for this room, shared across classes.
    pub best: Option<f64>,
    /// The save's record for this family, when it carries one.
    pub family_best: Option<f64>,
}

/// How many upcoming weeks offer a family.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCount {
    pub family: usize,
    pub name: String,
    pub count: usize,
}

/// How many upcoming weeks offer a room.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCount {
    pub map_idx: usize,
    pub name: String,
    pub count: usize,
}

/// The save's Killroy rotation and when its current week resets.
pub(super) struct Rotation {
    pub(super) week: i64,
    pub(super) reset_at: f64,
    pub(super) swap: f64,
}

/// Forecast `weeks` upcoming weeks for every saved character.
pub fn forecast(save: &Save, weeks: usize) -> Result<Forecast, KillroyError> {
    if !(1..=MAX_FORECAST_WEEKS).contains(&weeks) {
        return Err(KillroyError::new("Choose 1-52 upcoming weeks"));
    }
    let characters: Vec<(usize, String)> = (0..save.character_count())
        .map(|ci| (ci, save.character_name(ci)))
        .collect();
    let rotation = rotation(save)?;
    if characters.is_empty() {
        return Err(KillroyError::new("Load a save with characters"));
    }
    let mut family_counts: Vec<FamilyCount> = data()
        .families
        .iter()
        .enumerate()
        .map(|(family, name)| FamilyCount {
            family,
            name: name.clone(),
            count: 0,
        })
        .collect();
    let mut map_counts: Vec<MapCount> = Vec::new();
    let mut forecast = Vec::with_capacity(weeks + 1);

    for week_offset in 0..=weeks {
        let week = rotation.week + week_offset as i64;
        let saved = characters
            .iter()
            .map(|(ci, name)| ForecastCharacter {
                ci: *ci,
                name: name.clone(),
                encounters: encounters(save, &rotation, *ci, week_offset),
            })
            .collect::<Vec<_>>();
        if week_offset > 0 {
            for slot in 0..3 {
                let offered: Vec<&Encounter> = saved
                    .iter()
                    .filter_map(|character| character.encounters.get(slot))
                    .filter(|encounter| encounter.available == Some(true))
                    .collect();
                let Some(first) = offered.first() else {
                    continue;
                };
                family_counts[first.family].count += 1;
                let mut seen = Vec::new();
                for encounter in offered {
                    let Some(map_idx) = encounter.map_idx else {
                        continue;
                    };
                    if seen.contains(&map_idx) {
                        continue;
                    }
                    seen.push(map_idx);
                    match map_counts.iter_mut().find(|count| count.map_idx == map_idx) {
                        Some(count) => count.count += 1,
                        None => map_counts.push(MapCount {
                            map_idx,
                            name: encounter.map_name.clone(),
                            count: 1,
                        }),
                    }
                }
            }
        }
        forecast.push(ForecastWeek {
            week_offset,
            week,
            starts_at: rotation.reset_at + (week_offset as f64 - 1.0) * WEEK_SECONDS,
            ends_at: rotation.reset_at + week_offset as f64 * WEEK_SECONDS,
            characters: saved,
        });
    }
    map_counts.sort_by(|a, b| b.count.cmp(&a.count).then(a.map_idx.cmp(&b.map_idx)));

    Ok(Forecast {
        weeks: forecast,
        family_counts,
        map_counts,
    })
}

/// The one encounter a plan row runs, for a character and a week ahead.
pub(super) fn encounter(
    save: &Save,
    ci: usize,
    slot: usize,
    week_offset: usize,
) -> Result<Encounter, KillroyError> {
    if week_offset > MAX_FORECAST_WEEKS {
        return Err(KillroyError::new("Choose a week from 0 to 52 weeks ahead"));
    }
    if ci >= save.character_count() {
        return Err(KillroyError::new("Choose a saved character"));
    }
    let rotation = rotation(save)?;
    Ok(encounters(save, &rotation, ci, week_offset)[slot].clone())
}

/// The save's current rotation week and reset time, from its snapshot clock.
pub(super) fn rotation(save: &Save) -> Result<Rotation, KillroyError> {
    let options = options(save);
    let time = save.get("TimeAway");
    let global = required(
        time.as_ref().and_then(|time| time.get("GlobalTime")),
        "save snapshot time",
    )?;
    let restock = required(
        time.as_ref().and_then(|time| time.get("ShopRestock")),
        "weekly reset countdown",
    )?;
    let day = required(
        options.as_ref().and_then(|options| at(options, 39)),
        "weekly day counter",
    )?;
    let swap = required(
        save.get("serverVars")
            .as_ref()
            .and_then(|vars| vars.get("KillroySwap")),
        "server Killroy rotation (import a wrapped save)",
    )?;
    let reset_at = global + (restock + 86_400.0 * day).round();
    Ok(Rotation {
        week: (reset_at / WEEK_SECONDS).floor() as i64,
        reset_at,
        swap,
    })
}

/// The three encounters one character gets in one week.
fn encounters(save: &Save, rotation: &Rotation, ci: usize, week_offset: usize) -> Vec<Encounter> {
    let options = options(save);
    let extra = options
        .as_ref()
        .and_then(|options| at(options, 227))
        .and_then(number);
    let used = options
        .as_ref()
        .and_then(|options| at(options, 113))
        .and_then(number);
    let class = save
        .get(&format!("CharacterClass_{ci}"))
        .as_ref()
        .and_then(number);
    let best = save.get("KRbest");
    let tempest = form(save, ci, 420);
    let world3 = world3(save, ci);
    let week = rotation.week + week_offset as i64;

    (0..3)
        .map(|slot| {
            let seed = (week as f64 + (50.0 * slot as f64 + rotation.swap)).round() as i64;
            let mut random = game_random(seed);
            let r = random();
            let family = (3.0 * r - (slot / 2) as f64).ceil().clamp(0.0, 3.0) as usize;
            let roll = (1000.0 * r).floor() as usize;
            let mut pool = 68 + slot;
            let mut reason = String::new();
            if roll >= 300 && slot != 0 {
                let w5 = save
                    .get(&format!("KLA_{ci}"))
                    .as_ref()
                    .and_then(|kla| at(kla, 200))
                    .and_then(|row| at(row, 0))
                    .and_then(number);
                let summon = save
                    .get("Summon")
                    .as_ref()
                    .and_then(|summon| at(summon, 3))
                    .and_then(|row| at(row, 2))
                    .and_then(number);
                if roll < 400 && w5.is_none() {
                    reason = "World 5 portal progress is unavailable".into();
                } else if roll < 400 && w5.is_some_and(|w5| w5 <= 0.0) {
                    pool = 72;
                } else if roll < 500 && summon.is_none() {
                    reason = "Summoning world unlock is unavailable".into();
                } else if roll < 500 && summon.is_some_and(|summon| summon >= 4.0) {
                    pool = 99;
                } else {
                    pool = 69 + slot;
                }
            }
            let room = if reason.is_empty() {
                Some(room(pool, roll))
            } else {
                None
            };
            let available = if slot == 0 {
                Some(true)
            } else {
                match world3 {
                    None => None,
                    Some(false) => Some(false),
                    Some(true) if slot == 1 => Some(true),
                    Some(true) => extra.map(|extra| extra == 1.0),
                }
            };
            let matches = class.is_some_and(|class| class_matches(class, family));
            if reason.is_empty() && available.is_none() {
                reason = "Encounter unlock data is unavailable".into();
            }
            if reason.is_empty() && available == Some(false) {
                reason = if slot == 1 {
                    "Reach World 3 to unlock".into()
                } else {
                    "Third encounter is locked".into()
                };
            }
            if reason.is_empty() && !matches {
                reason = format!("Requires {}", data().families[family]);
            }
            if reason.is_empty() && tempest {
                reason = "Leave Tempest form before entering Killroy".into();
            }
            Encounter {
                map_idx: room.map(|room| room.map_idx),
                map_name: room.map_or_else(|| "Unavailable".into(), |room| room.name.clone()),
                monster: room.map(|room| room.monster.clone()),
                slot,
                seed,
                week_offset,
                week,
                family,
                family_name: data().families[family].clone(),
                available,
                eligible: reason.is_empty(),
                reason,
                used: if week_offset > 0 {
                    Some(false)
                } else {
                    used.map(|used| used.to_string().contains(&(slot + 1).to_string()))
                },
                best: match (&best, room) {
                    (Some(Value::Object(records)), Some(room)) => {
                        Some(records.get(&room.monster).and_then(number).unwrap_or(0.0))
                    }
                    _ => None,
                },
                family_best: if family == 0 {
                    None
                } else {
                    options
                        .as_ref()
                        .and_then(|options| at(options, 203 + family))
                        .and_then(number)
                },
            }
        })
        .collect()
}
