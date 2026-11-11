//! The static tables the stats context reads, extracted from the upstream JS by
//! `script/extract-stats-data.mjs`.
//!
//! `MapDispName`, `MapName`, `MapAFKtarget`, `MapAFKtargetSide`, `MapDetails`,
//! `SceneTransitions`, `ColosseumInfo` and `ClassNames` come from
//! `js/stats/data/game/customlists.js`, the target names and AFK types from
//! `monsters.js`, the multi-character requirements from `custommaps.js` and the
//! dungeon maps from `js/stats/systems/w2/dungeon.js`.

use serde::Deserialize;
use std::sync::OnceLock;

const STATS_JSON: &str = include_str!("../../data/stats.json");

/// The static tables extracted from the upstream JS by
/// `script/extract-stats-data.mjs`.
#[derive(Debug, Deserialize)]
pub(super) struct Data {
    pub(super) maps: Vec<MapRow>,
    pub(super) classes: Vec<String>,
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
}
