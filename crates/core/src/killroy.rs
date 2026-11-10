//! Weekly Killroy forecasts, ported from the upstream planner.
//!
//! The maths follow `js/stats/systems/common/killroy-planner.js` and the room
//! pools in `js/stats/data/w2/killroy.js`: the weekly reset is derived from the
//! save's snapshot time, restock countdown and day counter, then each encounter
//! slot draws its family and room from the game's seeded random stream.
//!
//! Nothing here predicts kills or skulls. Those come from combat simulations
//! (KL-02/KL-03); this module only explains which encounters a save can enter in
//! each upcoming week and what its current monster record is.

use crate::save::{Save, at, number, row_values};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const KILLROY_JSON: &str = include_str!("../data/killroy.json");
const WEEK_SECONDS: f64 = 604_800.0;
const MAX_FORECAST_WEEKS: usize = 52;

/// The static Killroy data the forecast reads.
#[derive(Debug, Deserialize)]
struct Data {
    families: Vec<String>,
    pools: HashMap<String, Vec<Room>>,
}

/// One room a Killroy pool can roll.
#[derive(Clone, Debug, Deserialize)]
struct Room {
    #[serde(rename = "mapIdx")]
    map_idx: usize,
    name: String,
    monster: String,
}

/// Why a forecast could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KillroyError(String);

impl std::fmt::Display for KillroyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for KillroyError {}

impl KillroyError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

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
#[derive(Clone, Debug, Serialize)]
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
struct Rotation {
    week: i64,
    reset_at: f64,
    swap: f64,
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

/// The save's current rotation week and reset time, from its snapshot clock.
fn rotation(save: &Save) -> Result<Rotation, KillroyError> {
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

/// The save's quest-list values, matching `combatFarmingOptions`.
fn options(save: &Save) -> Option<Value> {
    save.get("OptionsListAccount")
        .filter(|values| !values.is_null())
        .or_else(|| save.get("OptLacc").filter(|values| !values.is_null()))
}

fn required(value: Option<&Value>, label: &str) -> Result<f64, KillroyError> {
    value
        .and_then(number)
        .ok_or_else(|| KillroyError::new(format!("Missing or invalid {label}")))
}

/// Whether a class can enter a family's encounters, matching
/// `killroyClassMatches`.
fn class_matches(class: f64, family: usize) -> bool {
    match family {
        0 => class < 6.0,
        1 => (7.0..18.0).contains(&class),
        2 => (19.0..30.0).contains(&class),
        3 => (31.0..41.0).contains(&class),
        _ => false,
    }
}

/// Whether the save has a buff active above the planner's threshold.
fn form(save: &Save, ci: usize, id: usize) -> bool {
    let Some(buffs) = save.get(&format!("BuffsActive_{ci}")) else {
        return false;
    };
    row_values(&buffs).iter().any(|buff| {
        at(buff, 0).and_then(number) == Some(id as f64)
            && at(buff, 1)
                .and_then(number)
                .is_some_and(|value| value > 100.0)
    })
}

/// World 3 reachability: the Tess form's rooms, or the character's portal gates.
fn world3(save: &Save, ci: usize) -> Option<bool> {
    if form(save, ci, 585) {
        let tess = save.get("Tess");
        let first = tess.as_ref().and_then(|tess| at(tess, 0));
        return match first {
            Some(row @ (Value::Object(_) | Value::Array(_))) => {
                Some([100, 150, 200].iter().any(|gate| {
                    row_values(row)
                        .iter()
                        .any(|room| room.as_str() == Some(&format!("{gate}_0")))
                }))
            }
            _ => None,
        };
    }
    let kla = save.get(&format!("KLA_{ci}"));
    let gates: Vec<Option<f64>> = [100, 150, 200, 250]
        .iter()
        .map(|gate| {
            kla.as_ref()
                .and_then(|kla| at(kla, *gate))
                .and_then(|row| at(row, 0))
                .and_then(number)
        })
        .collect();
    if gates
        .iter()
        .any(|gate| gate.is_some_and(|gate| gate <= 0.0))
    {
        Some(true)
    } else if gates.iter().all(Option::is_some) {
        Some(false)
    } else {
        None
    }
}

/// One room from a pool, matching `killroyRoom`.
fn room(pool: usize, roll: usize) -> &'static Room {
    let rooms = data()
        .pools
        .get(&pool.to_string())
        .expect("every Killroy pool is extracted");
    let length = rooms.len() as f64;
    let index = ((roll as f64 - (roll as f64 / length).floor() * length).round()) as usize;
    &rooms[index]
}

/// The game's seeded random stream, ported from `gameRandom` in `formulas.js`.
fn game_random(seed: i64) -> impl FnMut() -> f64 {
    let mut first = seed as i32;
    let mut value = first.wrapping_mul(-862_048_943);
    value = ((value << 15) | ((value as u32 >> 17) as i32)).wrapping_mul(461_845_907);
    let mut state = 5381 ^ value;
    state = ((state << 13) | ((state as u32 >> 19) as i32))
        .wrapping_mul(5)
        .wrapping_sub(430_675_100);
    state = (state ^ (state >> 16)).wrapping_mul(-2_048_144_789);
    state = (state ^ (state >> 13)).wrapping_mul(-1_028_477_387);
    let mut second = state ^ (state >> 16);
    if first == 0 {
        first = 1;
    }
    if second == 0 {
        second = 1;
    }
    move || {
        first = 36_969i32
            .wrapping_mul(65_535 & first)
            .wrapping_add(first >> 16);
        second = 18_000i32
            .wrapping_mul(65_535 & second)
            .wrapping_add(second >> 16);
        (f64::from(1_073_741_823 & (first << 16).wrapping_add(second)) % 10_007.0) / 10_007.0
    }
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(KILLROY_JSON).expect("bundled Killroy data is valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_random_stream_matches_upstream() {
        // node -e "import('./js/formulas.js').then(m => { const r = m.gameRandom(2974); console.log(JSON.stringify([...Array(4)].map(() => r()))); })"
        let mut random = game_random(2974);
        let values: Vec<f64> = (0..4).map(|_| random()).collect();
        for (got, want) in values.iter().zip([
            0.215_249_325_472_169_5,
            0.904_267_013_090_836_5,
            0.834_915_559_108_623_9,
            0.671_330_068_951_733_7,
        ]) {
            assert!((got - want).abs() < 1e-12, "{got} != {want}");
        }
    }

    #[test]
    fn every_extracted_pool_has_rooms() {
        for pool in ["68", "69", "70", "71", "72", "99"] {
            assert!(
                data()
                    .pools
                    .get(pool)
                    .is_some_and(|rooms| !rooms.is_empty())
            );
        }
        assert_eq!(data().families.len(), 4);
    }

    #[test]
    fn class_families_match_upstream_bounds() {
        for class in [0.0, 5.0] {
            assert!(class_matches(class, 0));
        }
        assert!(!class_matches(6.0, 0));
        assert!(class_matches(7.0, 1));
        assert!(class_matches(17.0, 1));
        assert!(!class_matches(18.0, 1));
        assert!(class_matches(19.0, 2));
        assert!(class_matches(29.0, 2));
        assert!(!class_matches(30.0, 2));
        assert!(class_matches(31.0, 3));
        assert!(class_matches(40.0, 3));
        assert!(!class_matches(41.0, 3));
    }

    #[test]
    fn a_room_roll_wraps_to_the_pool_length() {
        let rooms = &data().pools["68"];
        assert_eq!(room(68, 0).map_idx, rooms[0].map_idx);
        assert_eq!(room(68, 1000).map_idx, rooms[1000 % rooms.len()].map_idx);
    }
}
