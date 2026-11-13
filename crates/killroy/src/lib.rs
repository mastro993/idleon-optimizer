//! Weekly Killroy forecasts and +1 upgrade comparisons.
//!
//! The maths follow `js/stats/systems/common/killroy-planner.js` and the room
//! pools and upgrade list in `js/stats/data/w2/killroy.js`: the weekly reset is
//! derived from the save's snapshot time, restock countdown and day counter,
//! then each encounter slot draws its family and room from the game's seeded
//! random stream.
//!
//! [`forecast()`] explains which encounters a save can enter in each upcoming
//! week. [`plan`] turns a selection of encounters into the baseline and `+1`
//! upgrade rows the comparison runs, [`trial()`] runs one row for one seed and
//! [`summarize`] averages the records and ranks the options. [`saved_buffs`]
//! reports the save's `BuffsActive` rows a run starts from, and [`results_json`]
//! writes the run's results as the section's export document.
//!
//! The player's damage comes from the combat simulator upstream; that section is
//! not built here yet (M2), so a trial takes the character's kills per second as
//! an input. Everything else in a trial — the arrival schedule, the respawn
//! bonus, the 100 ms spawn tick, the run duration, the per-kill drop rolls and
//! the bag those drops share — is the upstream formula.

mod buffs;
mod export;
mod forecast;
mod inventory;
mod planner;
mod summary;
mod trial;

pub use buffs::{CharacterBuffs, SavedBuff, character_buffs, saved_buffs};
pub use export::{document as results_document, to_json as results_json};
pub use forecast::{Encounter, FamilyCount, Forecast, ForecastCharacter, ForecastWeek, forecast};
pub use inventory::Bag;
pub use planner::{
    Comparisons, Job, KillroyError, Plan, PlanInput, Row, Selection, Upgrade, UpgradeChoice, Week,
    plan, trial_inputs, upgrade_choices,
};
pub use summary::{Delta, Means, RowSummary, run, summarize, trial_seeds};
pub use trial::{Record, TrialInputs, UPGRADES, trial};

use idleon_save::Save;
use idleon_save::at;
use idleon_save::number;
use idleon_save::row_values;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const KILLROY_JSON: &str = include_str!("../data/killroy.json");
const WEEK_SECONDS: f64 = 604_800.0;
const MAX_FORECAST_WEEKS: usize = 52;

/// One upgrade the Killroy results screen offers, in the game's own order.
#[derive(Clone, Debug, Deserialize)]
struct UpgradeDef {
    name: String,
    effect: String,
}

/// The static Killroy data the planner reads.
#[derive(Debug, Deserialize)]
struct Data {
    families: Vec<String>,
    upgrades: Vec<UpgradeDef>,
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

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(KILLROY_JSON).expect("bundled Killroy data is valid JSON")
    })
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
    fn the_upgrade_list_matches_upstream() {
        let names: Vec<&str> = data()
            .upgrades
            .iter()
            .map(|upgrade| upgrade.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Time",
                "Talent points",
                "Skulls",
                "Respawns",
                "Dungeon credits",
                "Pearls"
            ]
        );
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

#[cfg(test)]
pub(crate) mod test_support {
    use idleon_save::Save;
    use serde_json::json;

    /// The committed fixture, wrapped like an `it.json` with the Killroy swap
    /// server variable the planner reads.
    pub(crate) fn wrapped_fixture() -> serde_json::Value {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        json!({ "data": save, "serverVars": { "KillroySwap": 7 } })
    }

    /// [`wrapped_fixture`] as a save.
    pub(crate) fn fixture() -> Save {
        Save::new(wrapped_fixture())
    }
}
