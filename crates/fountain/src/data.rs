//! Static Fountain data, extracted from the upstream JS calculator.
//!
//! `script/extract-fountain-data.mjs` writes `data/fountain.json` from the
//! upstream `UPG_DATA` table and the `HolesInfo`/`CosmoUpgrades` rows the
//! cross-system bonuses read.

use serde::Deserialize;
use std::sync::OnceLock;

const FOUNTAIN_JSON: &str = include_str!("../data/fountain.json");

/// How many upgrades each water has in `UPG_DATA`.
pub(crate) const UPGRADES_PER_WATER: usize = 20;

/// The tables the fountain overview and income rates read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub water_names: Vec<String>,
    pub currency_names: Vec<String>,
    pub currency_short: Vec<String>,
    pub bar_fill_times: Vec<f64>,
    pub upgrades: Vec<Vec<Upgrade>>,
    pub holes: Holes,
    pub cosmo: Cosmo,
    pub arcade_fountain_speed: Arcade,
    pub lamp_per_level: f64,
    pub cards_per_measurement_unit: f64,
}

/// One `UPG_DATA` row: `[name, prereqIdx, xy, currencyTypeIdx, baseCost,
/// costScale, bonPerLv, description]`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub name: String,
    /// The upgrade that must reach level 10 first; `-1` for none.
    pub prereq: i32,
    pub x: f64,
    pub y: f64,
    pub currency: usize,
    pub base_cost: f64,
    pub cost_scale: f64,
    pub per_level: f64,
    pub description: String,
}

/// The `HolesInfo` rows the fountain reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holes {
    pub measurement_type: Vec<f64>,
    pub measurement_base: Vec<String>,
    pub bolaia_per_level: Vec<f64>,
    pub bell_per_level: Vec<f64>,
}

/// `CosmoBonus` bases: the max coin stack row and the measurement multiplier.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cosmo {
    pub max_stack: f64,
    pub measurement: f64,
    /// `CosmoUpgrades[tier][index]`: the base each level grants.
    pub base: Vec<Vec<f64>>,
}

/// `ArcadeShopInfo[68]` "Fountain Speed": `[formula, base, denominator]`.
#[derive(Debug, Deserialize)]
pub struct Arcade {
    #[serde(rename = "type")]
    pub kind: String,
    pub base: f64,
    pub denom: f64,
}

/// The parsed fountain data, extracted once.
pub(crate) fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(FOUNTAIN_JSON)
            .expect("crates/fountain/data/fountain.json is extracted")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extracted_tables_keep_the_upstream_shape() {
        let data = data();
        assert_eq!(data.water_names.len(), 3);
        assert_eq!(data.currency_names.len(), 9);
        assert_eq!(data.currency_short.len(), 9);
        assert_eq!(data.upgrades.len(), 3);
        assert!(data.upgrades.iter().all(|water| water.len() == 20));
        assert_eq!(data.holes.measurement_base.len(), 17);
        assert_eq!(data.holes.measurement_type.len(), 17);
        assert_eq!(data.holes.bell_per_level[6], 0.2);
        assert_eq!(data.holes.bolaia_per_level[15], 5.0);
        // Water Bender is upgrade 12 of the first water, +10% per level.
        let bender = &data.upgrades[0][12];
        assert_eq!(bender.name, "Water Bender");
        assert_eq!(bender.prereq, 2);
        assert_eq!(bender.per_level, 10.0);
        assert_eq!(bender.currency, 0);
        assert_eq!(data.cosmo.max_stack, 1.0);
        assert_eq!(data.cosmo.measurement, 25.0);
        assert_eq!(data.arcade_fountain_speed.kind, "decay");
        assert_eq!(data.arcade_fountain_speed.base, 30.0);
        assert_eq!(data.arcade_fountain_speed.denom, 100.0);
    }
}
