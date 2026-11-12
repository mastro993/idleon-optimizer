//! Static Royal Guardian data, extracted from the upstream JS calculator.
//!
//! `script/extract-royal-data.mjs` writes `data/royal-guardian.json` from the
//! upstream `ArmoryUpg`, `OrbletMarket`, `RoyalResources` and `MapDetails`
//! tables, the talent curve the clearing rate reads, and the Spelunk and Jelly
//! rows its cross-system factors multiply in.

use serde::Deserialize;
use std::sync::OnceLock;

const ROYAL_JSON: &str = include_str!("../../data/royal-guardian.json");

/// The tables the overview and the clear-rate maths read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    /// `ArmoryUpg[idx][5]`: an armory upgrade's bonus per level.
    pub armory_bonus_per_level: Vec<f64>,
    /// `OrbletMarket[idx][4]`: an orblet's bonus per level.
    pub orblet_bonus_per_level: Vec<f64>,
    pub resources: Vec<Resource>,
    /// One entry per map index; every index the game data carries has a row.
    pub maps: Vec<Map>,
    pub talent_231: Curve,
    /// `Spelunky[18]`: the base of each big-fish bonus.
    pub big_fish_base: Vec<f64>,
}

/// One `RoyalResources` row: the node's map anchor and currency slot.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub x: f64,
    pub y: f64,
    pub currency_slot: i64,
}

/// One map's Royal Guardian slice.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Map {
    /// `MapDispName[idx]`: the name the page prints.
    pub name: String,
    /// `MapDetails[idx][2]`: the map anchor.
    pub x: f64,
    pub y: f64,
    /// `royalMapEligible`: whether the map carries an outpost.
    pub eligible: bool,
    /// `outpostKillRequirement`: the override or the scaled base.
    pub kill_requirement: f64,
}

/// One `TalentDescriptions` formula: `formula(x1, x2, level)`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Curve {
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
}

/// The extracted tables, parsed once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(ROYAL_JSON).expect("crates/core/data/royal-guardian.json is valid")
    })
}

/// One map's static row.
pub fn map(index: usize) -> Option<&'static Map> {
    data().maps.get(index)
}

/// One resource node's static row.
pub fn resource(index: usize) -> Option<&'static Resource> {
    data().resources.get(index)
}

/// How many resource nodes exist, which is also the last index plus one.
pub fn resource_count() -> usize {
    data().resources.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extracted_tables_keep_their_upstream_shape() {
        let data = data();
        assert_eq!(
            data.armory_bonus_per_level.len(),
            83,
            "ArmoryUpg has 83 rows"
        );
        assert_eq!(
            data.orblet_bonus_per_level.len(),
            10,
            "OrbletMarket has 10 rows"
        );
        assert_eq!(data.resources.len(), 80, "RoyalResources has 80 rows");
        assert_eq!(data.maps.len(), 327, "MapDetails has 327 rows");
        assert_eq!(data.big_fish_base.len(), 7, "Spelunky[18] lists 7");
        assert_eq!(data.talent_231.formula, "decayMulti");
    }

    #[test]
    fn the_map_anchors_are_the_ones_the_upstream_reads() {
        // `MapDetails[1][2]` is the anchor W1's second outpost sits on, and the
        // range finder measures from it.
        let row = map(1).expect("map 1");
        assert_eq!((row.x, row.y), (603.0, 190.0));
        assert!(row.eligible, "map 1 carries an outpost");
        assert!(
            !map(0).expect("map 0").eligible,
            "the town anchor carries no outpost"
        );
        assert_eq!(
            map(1).expect("map 1").kill_requirement,
            156.39405897989235,
            "map 1's clear requirement"
        );
    }
}
