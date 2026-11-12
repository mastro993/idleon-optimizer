//! The static tables the stats context reads, extracted from the upstream JS by
//! `script/extract-stats-data.mjs`.
//!
//! `MapDispName`, `MapName`, `MapAFKtarget`, `MapAFKtargetSide`, `MapDetails`,
//! `SceneTransitions`, `ColosseumInfo` and `ClassNames` come from
//! `js/stats/data/game/customlists.js`, the target names and AFK types from
//! `monsters.js`, the multi-character requirements from `custommaps.js` and the
//! dungeon maps from `js/stats/systems/w2/dungeon.js`.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const STATS_JSON: &str = include_str!("../../data/stats.json");

/// The static tables extracted from the upstream JS by
/// `script/extract-stats-data.mjs`.
#[derive(Debug, Deserialize)]
pub(super) struct Data {
    pub(super) maps: Vec<MapRow>,
    pub(super) classes: Vec<String>,
    pub(super) capacity: CapacityData,
}

/// One map's static row.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MapRow {
    pub(super) display_name: String,
    pub(super) internal_name: String,
    pub(super) afk_target: String,
    pub(super) target_name: String,
    pub(super) afk_type: String,
    pub(super) side_targets: Vec<String>,
    pub(super) kill_req: f64,
    pub(super) multichar_req: f64,
    pub(super) colosseum: bool,
    pub(super) dungeon: bool,
    /// `[portal index, destination internal name]` rows; portal `i` reads `i + 1`.
    pub(super) transitions: Vec<(i64, String)>,
}

/// The static tables the carry-capacity breakdown reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapacityData {
    /// `N2L`, the game's `Number2Letter` alphabet.
    pub(super) number_letter: String,
    /// `BOSS3B_CARD_PCT`, the critter card's boost per level.
    pub(super) boss3b_card_pct: f64,
    pub(super) guilds: Vec<Curve>,
    pub(super) bribes: Vec<f64>,
    pub(super) prayers: Vec<[f64; 2]>,
    pub(super) shrines: Vec<[f64; 2]>,
    pub(super) talents: BTreeMap<String, Curve>,
    pub(super) pristine_charms: BTreeMap<String, f64>,
    pub(super) stamps: Vec<StampRow>,
    pub(super) compass_per_level: Vec<f64>,
    pub(super) emperor_set: f64,
    pub(super) rog_bonus: Vec<f64>,
    pub(super) legend_per_point: Vec<f64>,
    pub(super) palette23: PaletteParam,
    pub(super) jelly_reward50: f64,
    pub(super) exotic49: ExoticParam,
    pub(super) pet_shiny_type: Vec<Vec<f64>>,
    pub(super) shiny_type_to_cat: Vec<u64>,
    pub(super) shiny_bonus_per_level: Vec<f64>,
    pub(super) star_sign_names: Vec<String>,
    pub(super) carry_star_signs: Vec<[f64; 2]>,
}

/// `paletteParams(23)`: the Honey Yellow palette behind the stamp doubler.
#[derive(Debug, Deserialize)]
pub(super) struct PaletteParam {
    pub(super) coeff: f64,
    pub(super) denom: f64,
    pub(super) decay: bool,
}

/// `exoticParams(49)`: the Exalted Eldou upgrade behind the stamp doubler.
#[derive(Debug, Deserialize)]
pub(super) struct ExoticParam {
    pub(super) base: f64,
    pub(super) decay: bool,
}

/// One `ArbitraryCode5Inputs` curve: its kind and two coefficients.
#[derive(Debug, Deserialize)]
pub(super) struct Curve {
    pub(super) formula: String,
    pub(super) x1: f64,
    pub(super) x2: f64,
}

/// One stamp row, built from the stamp item's description upstream.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StampRow {
    pub(super) item: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) cat: usize,
    pub(super) idx: usize,
    pub(super) formula: String,
    pub(super) x1: f64,
    pub(super) x2: f64,
    pub(super) overlevel_step: f64,
    pub(super) skill_idx: i64,
}

/// The extracted tables, parsed once.
pub(super) fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(STATS_JSON).expect("crates/core/data/stats.json is valid")
    })
}

/// How many maps the game data carries.
pub fn map_count() -> usize {
    data().maps.len()
}

/// The carry-capacity tables the breakdowns read.
pub(super) fn capacity() -> &'static CapacityData {
    &data().capacity
}

/// One class id's upstream display name.
pub fn class_name(id: usize) -> &'static str {
    data()
        .classes
        .get(id)
        .map(String::as_str)
        .unwrap_or("Unknown Class")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_class_names_are_the_upstream_table() {
        assert_eq!(class_name(0), "0");
        assert_eq!(class_name(1), "BEGINNER");
        assert_eq!(class_name(53), "LABORATORY");
        assert_eq!(class_name(9_999), "Unknown Class");
    }

    #[test]
    fn the_map_table_carries_the_upstream_map_count() {
        assert_eq!(map_count(), 327);
        assert_eq!(data().maps[0].display_name, "Blunder_Hills");
        assert_eq!(data().maps[0].internal_name, "GrasslandaTown");
    }

    #[test]
    fn the_capacity_tables_name_every_upstream_source() {
        let capacity = capacity();
        assert_eq!(capacity.guilds.len(), 18);
        assert_eq!(capacity.bribes.len(), 41);
        assert_eq!(capacity.prayers.len(), 25);
        assert_eq!(capacity.shrines.len(), 9);
        assert_eq!(capacity.star_sign_names.len(), 94);
        // Rucksack is guild 2 and Bottomless Bags is bribe 23.
        let guild = &capacity.guilds[2];
        assert_eq!(guild.formula, "decay");
        assert_eq!((guild.x1, guild.x2), (70.0, 50.0));
        assert_eq!(capacity.bribes[23], 5.0);
        // Pantheon Shrine is shrine 3.
        assert_eq!(capacity.shrines[3], [10.0, 2.0]);
        // The carry stamps are the six groups upstream's TYPE_CONFIG names.
        for kind in [
            "MinCap",
            "ChopCap",
            "MatCap",
            "FishCap",
            "CatchCap",
            "AllCarryCap",
        ] {
            assert!(
                capacity.stamps.iter().any(|stamp| stamp.kind == kind),
                "{kind} has no stamp"
            );
        }
    }
}
