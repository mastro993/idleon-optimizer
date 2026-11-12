//! The fishing tables: pools, lures, lines, spots, locations and rods.
//!
//! Extracted from the upstream `js/stats/data/w2/fishing.js` and the game data
//! it reads by `script/extract-fishing-data.mjs`; nothing here is hand-typed.

use serde::Deserialize;
use std::sync::OnceLock;

const FISHING_JSON: &str = include_str!("../../data/fishing.json");

/// One toolkit piece's stats, as the upstream `FISHING_TOOLKIT_STATS` keys them.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE", default)]
pub struct ToolkitStats {
    #[serde(rename = "D0")]
    pub d0: f64,
    #[serde(rename = "D1")]
    pub d1: f64,
    #[serde(rename = "D2")]
    pub d2: f64,
    #[serde(rename = "D3")]
    pub d3: f64,
    #[serde(rename = "EXP")]
    pub exp: f64,
    #[serde(rename = "SPEED")]
    pub speed: f64,
    #[serde(rename = "POW")]
    pub pow: f64,
}

impl ToolkitStats {
    /// The depth stat at `0..=3`.
    pub fn depth(&self, tier: usize) -> f64 {
        match tier {
            0 => self.d0,
            1 => self.d1,
            2 => self.d2,
            _ => self.d3,
        }
    }

    /// This piece plus `other`, key by key.
    pub fn plus(self, other: Self) -> Self {
        Self {
            d0: self.d0 + other.d0,
            d1: self.d1 + other.d1,
            d2: self.d2 + other.d2,
            d3: self.d3 + other.d3,
            exp: self.exp + other.exp,
            speed: self.speed + other.speed,
            pow: self.pow + other.pow,
        }
    }
}

/// One lure or line from the upstream toolkit list.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ToolkitOption {
    pub idx: usize,
    pub name: String,
    pub stats: ToolkitStats,
}

/// One fish type of a pool.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fish {
    pub slot: usize,
    pub item_key: String,
    pub name: String,
    pub exp_per_catch: f64,
    pub bonus_item: String,
    pub bonus_odds: f64,
}

/// One AFK fishing pool.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
    pub key: String,
    pub name: String,
    pub panel_exp_per_catch: f64,
    pub exp_type: f64,
    pub eff_reqs: [f64; 3],
    pub fish: Vec<Fish>,
}

/// One fishing spot: a map and spot index with its depth bonus, or a pool that
/// is fished outside the normal maps (Equinox and the Trench).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub id: String,
    pub map_idx: Option<usize>,
    pub spot_idx: Option<usize>,
    pub pool_key: String,
    pub pool_name: String,
    pub map_name: String,
    pub label: String,
    pub bonus: [f64; 4],
}

/// One fishing rod, from the game's item table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Rod {
    pub key: String,
    pub name: String,
    pub speed: f64,
}

/// The Trench depth requirement's data, from the upstream `holesBolaiaPerLv`,
/// `legendTalentPerPt(29)` and the jar's collectible 5.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrenchData {
    /// `SKILLING_CAVERNS`, in the order a Bolaia study scales.
    pub caverns: Vec<String>,
    /// The Bolaia studies whose per-layer value scales with each cavern.
    pub studies: Vec<usize>,
    /// `holesBolaiaPerLv(study)`, one per entry of [`Self::studies`].
    pub bolaia_per_level: Vec<f64>,
    /// `legendTalentPerPt(29)`, the multiplier the jar collectible shares.
    pub legend_per_point: f64,
    /// `COLLECTIBLE_DATA[5][1]`, the skilling cavern requirement reduction.
    pub collectible5_per_level: f64,
}

/// Everything the calculator reads from the upstream game data.
#[derive(Debug, Deserialize)]
pub struct FishingData {
    pub pools: Vec<Pool>,
    pub lures: Vec<ToolkitOption>,
    pub lines: Vec<ToolkitOption>,
    pub locations: Vec<Location>,
    pub rods: Vec<Rod>,
    pub trench: TrenchData,
}

impl FishingData {
    /// A pool by its upstream key.
    pub fn pool(&self, key: &str) -> Option<&Pool> {
        self.pools.iter().find(|pool| pool.key == key)
    }

    /// One lure (`0`) or line (`1`) by index.
    pub fn toolkit(&self, kind: usize, index: usize) -> Option<&ToolkitOption> {
        let list = if kind == 0 { &self.lures } else { &self.lines };
        list.get(index).or_else(|| list.first())
    }

    /// A location by its stable id, `map:spot` or `x:pool`.
    pub fn location(&self, id: &str) -> Option<&Location> {
        self.locations.iter().find(|location| location.id == id)
    }

    /// A location by the row the section shows first.
    pub fn default_location(&self) -> &Location {
        self.location("72:FishBig")
            .or_else(|| self.locations.first())
            .expect("the fishing data ships at least one location")
    }

    /// A rod by its item key.
    pub fn rod(&self, key: &str) -> Option<&Rod> {
        self.rods.iter().find(|rod| rod.key == key)
    }
}

/// The bundled fishing data, parsed once.
pub fn data() -> &'static FishingData {
    static DATA: OnceLock<FishingData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(FISHING_JSON).expect("bundled fishing data is valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pool_has_four_fish_and_three_requirements() {
        let data = data();
        assert_eq!(data.pools.len(), 5);
        for pool in &data.pools {
            assert_eq!(pool.fish.len(), 4, "{}", pool.key);
            assert_eq!(pool.eff_reqs.len(), 3, "{}", pool.key);
        }
    }

    #[test]
    fn the_toolkit_lists_are_the_upstream_fifteen() {
        let data = data();
        assert_eq!(data.lures.len(), 15);
        assert_eq!(data.lines.len(), 15);
        assert_eq!(data.toolkit(0, 0).unwrap().name, "Fly");
        assert_eq!(data.toolkit(1, 0).unwrap().name, "Fishing Twine");
        assert_eq!(data.lures.len() * data.lines.len(), 225);
    }

    #[test]
    fn locations_carry_their_spot_depth_bonus() {
        let data = data();
        let spot = data.location("54:0").expect("Salty Shores spot 1");
        assert_eq!(spot.bonus, [0.0, 25.0, 5.0, 0.0]);
        assert_eq!(spot.map_name, "Salty Shores");
        assert_eq!(data.default_location().pool_key, "FishBig");
        assert!(data.location("x:MotherlodeFISH").is_some());
    }

    #[test]
    fn rods_carry_the_game_speed() {
        let data = data();
        assert_eq!(data.rods.len(), 13);
        assert_eq!(data.rod("FishingRod1").unwrap().speed, 3.0);
        assert_eq!(data.rod("FishingRod13").unwrap().speed, 8.0);
        assert!(data.rod("Blank").is_none());
    }

    #[test]
    fn the_trench_data_is_the_upstream_four_caverns() {
        let trench = &data().trench;
        assert_eq!(trench.caverns, ["Motherlode", "Hive", "Evertree", "Trench"]);
        assert_eq!(trench.studies, [1, 7, 11, 16]);
        assert_eq!(trench.bolaia_per_level, [2.0, 5.0, 5.0, 3.0]);
        assert_eq!(trench.legend_per_point, 20.0);
        assert_eq!(trench.collectible5_per_level, 10.0);
    }
}
