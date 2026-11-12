//! The static tables the stats context reads, extracted from the upstream JS by
//! `script/extract-stats-data.mjs`.
//!
//! `MapDispName`, `MapName`, `MapAFKtarget`, `MapAFKtargetSide`, `MapDetails`,
//! `SceneTransitions`, `ColosseumInfo` and `ClassNames` come from
//! `js/stats/data/game/customlists.js`, the target names, AFK types and combat
//! rows from `monsters.js`, the multi-character requirements from `custommaps.js`
//! and the dungeon maps from `js/stats/systems/w2/dungeon.js`. The equipment set
//! bonuses are `EquipmentSets[name][3][2]`, the lookup upstream's
//! `equipSetBonus` makes.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const STATS_JSON: &str = include_str!("../data/stats.json");

/// The static tables extracted from the upstream JS by
/// `script/extract-stats-data.mjs`.
#[derive(Debug, Deserialize)]
pub struct Data {
    pub maps: Vec<MapRow>,
    pub classes: Vec<String>,
    pub capacity: CapacityData,
    pub monsters: BTreeMap<String, Monster>,
    #[serde(rename = "equipmentSets")]
    pub equipment_sets: BTreeMap<String, EquipmentSet>,
}

/// One equipment set's pieces and bonus, from `EquipmentSets`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentSet {
    /// Row 0 slots any of these pieces fill.
    pub armor: Vec<String>,
    /// Row 1 slots, up to [`Self::tools_cap`] of them.
    pub tools: Vec<String>,
    /// Row 0 slots, one counts no matter how many match.
    pub special: Vec<String>,
    /// How many tool pieces the set counts.
    pub tools_cap: f64,
    /// Whether one special piece counts (`1` in the game data).
    pub special_cap: f64,
    /// `set[3][2]`: the set's bonus percentage.
    pub bonus: f64,
}

/// One monster's static combat row, from `js/stats/data/game/monsters.js`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Monster {
    /// The display name with its underscores, e.g. `Green_Mushroom`.
    pub name: String,
    /// `MonsterHPTotal`, the HP the overkill tier compares damage against.
    pub hp: f64,
    /// `Defence`, the accuracy gate reads it.
    pub defence: f64,
}

/// One map's static row.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapRow {
    pub display_name: String,
    pub internal_name: String,
    pub afk_target: String,
    pub target_name: String,
    pub afk_type: String,
    pub side_targets: Vec<String>,
    pub kill_req: f64,
    pub multichar_req: f64,
    pub colosseum: bool,
    pub dungeon: bool,
    /// `[portal index, destination internal name]` rows; portal `i` reads `i + 1`.
    pub transitions: Vec<(i64, String)>,
}

/// The static tables the carry-capacity breakdown reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityData {
    /// `N2L`, the game's `Number2Letter` alphabet.
    pub number_letter: String,
    /// `BOSS3B_CARD_PCT`, the critter card's boost per level.
    pub boss3b_card_pct: f64,
    pub guilds: Vec<Curve>,
    pub bribes: Vec<f64>,
    pub prayers: Vec<[f64; 2]>,
    pub shrines: Vec<[f64; 2]>,
    pub talents: BTreeMap<String, Curve>,
    pub pristine_charms: BTreeMap<String, f64>,
    pub stamps: Vec<StampRow>,
    pub compass_per_level: Vec<f64>,
    pub emperor_set: f64,
    pub rog_bonus: Vec<f64>,
    pub legend_per_point: Vec<f64>,
    pub palette23: PaletteParam,
    pub jelly_reward50: f64,
    pub exotic49: ExoticParam,
    pub pet_shiny_type: Vec<Vec<f64>>,
    pub shiny_type_to_cat: Vec<u64>,
    pub shiny_bonus_per_level: Vec<f64>,
    pub star_sign_names: Vec<String>,
    pub carry_star_signs: Vec<[f64; 2]>,
}

/// `paletteParams(23)`: the Honey Yellow palette behind the stamp doubler.
#[derive(Debug, Deserialize)]
pub struct PaletteParam {
    pub coeff: f64,
    pub denom: f64,
    pub decay: bool,
}

/// `exoticParams(49)`: the Exalted Eldou upgrade behind the stamp doubler.
#[derive(Debug, Deserialize)]
pub struct ExoticParam {
    pub base: f64,
    pub decay: bool,
}

/// One `ArbitraryCode5Inputs` curve: its kind and two coefficients.
#[derive(Debug, Deserialize)]
pub struct Curve {
    pub formula: String,
    pub x1: f64,
    pub x2: f64,
}

/// One stamp row, built from the stamp item's description upstream.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StampRow {
    pub item: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub cat: usize,
    pub idx: usize,
    pub formula: String,
    pub x1: f64,
    pub x2: f64,
    pub overlevel_step: f64,
    pub skill_idx: i64,
}

/// The extracted tables, parsed once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(STATS_JSON).expect("crates/stats-data/data/stats.json is valid")
    })
}

/// How many maps the game data carries.
pub fn map_count() -> usize {
    data().maps.len()
}

/// The carry-capacity tables the breakdowns read.
pub fn capacity() -> &'static CapacityData {
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

/// One monster's static row, by the key the maps and saves name it with.
pub fn monster(key: &str) -> Option<&'static Monster> {
    data().monsters.get(key)
}

/// How many monsters the game data carries.
pub fn monster_count() -> usize {
    data().monsters.len()
}

/// An equipment set's bonus percentage, as `equipSetBonus` returns it.
pub fn set_bonus(name: &str) -> f64 {
    equipment_set(name).map(|set| set.bonus).unwrap_or(0.0)
}

/// An equipment set's pieces and bonus, as `EquipmentSets` carries them.
pub fn equipment_set(name: &str) -> Option<&'static EquipmentSet> {
    data().equipment_sets.get(name)
}

/// The AFK target key a map fights, when the table carries one.
pub fn afk_target(map: usize) -> Option<&'static str> {
    data()
        .maps
        .get(map)
        .map(|row| row.afk_target.as_str())
        .filter(|target| !target.is_empty())
}

/// A map's kill requirement, `MapDetails[map][0][0]`.
pub fn kill_requirement(map: usize) -> f64 {
    data().maps.get(map).map(|row| row.kill_req).unwrap_or(0.0)
}

/// A map's display name with its underscores, e.g. `Blunder_Hills`.
pub fn map_name(map: usize) -> String {
    data()
        .maps
        .get(map)
        .map(|row| row.display_name.replace('_', " "))
        .unwrap_or_else(|| "Unknown map".to_string())
}

/// A monster's display name with its underscores replaced, or `Unavailable`.
///
/// `coin-multi-calc.html`'s `_targetName` trims the display name and treats an
/// empty one and `Nothing` as unavailable.
pub fn target_name(key: &str) -> String {
    let name = monster(key)
        .map(|monster| monster.name.replace('_', " ").trim().to_string())
        .unwrap_or_default();
    if name.is_empty() || name == "Nothing" {
        "Unavailable".to_string()
    } else {
        name
    }
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

    #[test]
    fn the_monster_table_carries_the_combat_rows_and_set_bonuses() {
        assert_eq!(monster_count(), 418);
        let mushroom = monster("mushG").expect("the green mushroom row is in the table");
        assert_eq!(mushroom.name, "Green_Mushroom");
        assert_eq!(mushroom.hp, 32.0);
        assert_eq!(monster("frogG").map(|row| row.hp), Some(120.0));
        assert_eq!(monster("not a monster"), None);
        assert_eq!(target_name("mushG"), "Green Mushroom");
        assert_eq!(target_name("Nothing"), "Unavailable");
        assert_eq!(target_name("not a monster"), "Unavailable");
        assert_eq!(set_bonus("GOLD_SET"), 50.0);
        assert_eq!(set_bonus("NOT_A_SET"), 0.0);
        let gold = equipment_set("GOLD_SET").expect("the gold set rows are in the table");
        assert_eq!(gold.armor[0], "EquipmentHats28");
        assert_eq!(gold.tools_cap, 1.0);
        assert_eq!(gold.special_cap, 1.0);
        assert_eq!(map_name(0), "Blunder Hills");
        assert_eq!(afk_target(1), Some("mushG"));
        assert_eq!(afk_target(999), None);
    }
}
