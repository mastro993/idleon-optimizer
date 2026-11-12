//! Static Royal Guardian data, extracted from the upstream JS calculator.
//!
//! `script/extract-royal-data.mjs` writes `data/royal-guardian.json` from the
//! upstream `ArmoryUpg`, `OrbletMarket`, `RoyalResources` and `MapDetails`
//! tables, the talent curve the clearing rate reads, and the Spelunk and Jelly
//! rows its cross-system factors multiply in.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const ROYAL_JSON: &str = include_str!("../data/royal-guardian.json");

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
    /// The Royal talent curves by id: 225, 226, 230 and 231.
    pub talents: BTreeMap<String, Curve>,
    /// The single-row cross-system constants the collection rate reads.
    pub collection: Collection,
    /// `Spelunky[18]`: the base of each big-fish bonus.
    pub big_fish_base: Vec<f64>,
}

/// The cross-system rows the global collection rate multiplies in: RG-02 reads
/// only these three, so the rest of those systems stays out of this port.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// `Research[37][60]`: the RoG quantity sixty unique sushi unlock.
    pub rog_bonus60: f64,
    /// `ZenithMarket[10][4]`: the Zenith Market step per spelunk level.
    pub zenith_market10: f64,
    /// `ArcadeShopInfo[70][1..3]`: the arcade upgrade's curve.
    pub arcade70: Arcade,
}

/// One `ArcadeShopInfo` row: its curve type and the two coefficients.
#[derive(Debug, Deserialize)]
pub struct Arcade {
    pub kind: String,
    pub base: f64,
    pub denominator: f64,
}

/// One `RoyalResources` row: the node's map anchor, capacity and currency slot.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub x: f64,
    pub y: f64,
    /// `RoyalResources[idx][2]`: the capacity at grade zero, before the tier.
    pub base_capacity: f64,
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
        serde_json::from_str(ROYAL_JSON)
            .expect("crates/royal-guardian/data/royal-guardian.json is valid")
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

/// The talent curve at `index`, when the game data carries it.
pub fn talent(index: usize) -> Option<&'static Curve> {
    data().talents.get(&index.to_string())
}

/// The talent ids the section reads, in the order the game lists them.
pub fn talents() -> impl Iterator<Item = &'static usize> {
    static TALENTS: [usize; 4] = [225, 226, 230, 231];
    TALENTS.iter()
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
        assert_eq!(
            data.talents.len(),
            4,
            "the four Royal talents the section reads"
        );
        assert_eq!(
            talent(231).map(|curve| curve.formula.as_str()),
            Some("decayMulti")
        );
        assert_eq!(
            talent(225).map(|curve| curve.formula.as_str()),
            Some("decay")
        );
        // The collection rate's three cross-system constants.
        assert_eq!(data.collection.rog_bonus60, 30.0);
        assert_eq!(data.collection.zenith_market10, 1.0);
        assert_eq!(data.collection.arcade70.kind, "decay");
        assert_eq!(data.collection.arcade70.base, 6.0);
        assert_eq!(data.collection.arcade70.denominator, 100.0);
    }

    #[test]
    fn the_resource_rows_carry_the_capacity_the_tiers_scale() {
        // `RoyalResources[0]` is 1M at grade zero, times 5 per world tier.
        let row = resource(0).expect("resource 0");
        assert_eq!(row.base_capacity, 1_000_000.0);
        assert_eq!(row.currency_slot, 5);
        // A row that pays a currency is a real node with a capacity; the rest
        // are the game's zeroed placeholders.
        assert!(
            data()
                .resources
                .iter()
                .filter(|row| row.currency_slot >= 0)
                .all(|row| row.base_capacity > 0.0)
        );
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
