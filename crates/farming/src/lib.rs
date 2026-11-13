//! Farming: the 9×4 field, per-plot metrics and the save-derived multipliers.
//!
//! Ported from the upstream `js/stats/data/w5/farming.js`,
//! `js/stats/systems/w6/farming.js`, `js/stats/systems/w6/farming-planner.js`
//! and `farming-calc.html`.
//!
//! The upstream account snapshot reads a dozen other systems (bubbles, meals,
//! cards, stamps, vials, star signs, achievements, summoning, rift, minehead,
//! sushi, gaming, hole, pristine and the lab mainframe). This port applies only
//! the terms the save itself carries — market levels, Land Rank, exotic
//! upgrades, crop quantities, `GemItemsPurchased`, Spelunk organic labels, the
//! emporium encoding (`Ninja[102][9]`), Grimoire 22/36 and Vault 79 — so
//! chances and values here are lower bounds. The importer warns about it.
//!
//! Everything here is pinned against the upstream calculator's own output:
//! `script/farming-signals-oracle.mjs` regenerates `fixtures/farming-signals.json` from
//! `farming-planner.js` and `farming.js`, and `crates/farming/tests/farming_signals.rs`
//! compares the snapshot, the diagnostics, the growth breakpoints, every plot's
//! metrics, every plot's overgrowth curve and the Crop Scientist table.
//! `script/farming-oracle.mjs` regenerates `fixtures/farming.json` from the same
//! planner, carrying the page's seven preset goals and the allocation and
//! comparison tables `crates/farming/tests/farming_planner.rs` compares.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const FARMING_JSON: &str = include_str!("../data/farming.json");

/// Land plots in the upstream field.
pub const PLOTS: usize = 36;
/// Land Rank upgrade rows.
pub const RANK_UPGRADES: usize = 20;

/// Everything the planner reads from the upstream game data.
#[derive(Debug, Deserialize)]
pub struct FarmingData {
    pub seeds: Vec<SeedFamily>,
    pub market: Vec<MarketUpgrade>,
    pub exotics: Vec<ExoticUpgrade>,
    #[serde(rename = "rankNames")]
    pub rank_names: Vec<String>,
    #[serde(rename = "rankUnlocks")]
    pub rank_unlocks: Vec<u32>,
    #[serde(rename = "ninjaRankBases")]
    pub ninja_rank_bases: Vec<f64>,
    #[serde(rename = "gmoThresholds")]
    pub gmo_thresholds: Vec<f64>,
    /// `N2L`: the character encoding of the emporium item list.
    #[serde(rename = "emporiumEncoding")]
    pub emporium_encoding: String,
    /// The emporium item each Crop Scientist bonus is gated behind.
    #[serde(rename = "cropScientistGates")]
    pub crop_scientist_gates: Vec<usize>,
    /// Vault 79's bonus per level, the Crop Scientist multiplier term.
    #[serde(rename = "vaultCropScientistPerLevel")]
    pub vault_crop_scientist_per_level: f64,
}

/// One seed family and the crop ids it evolves through.
#[derive(Debug, Deserialize)]
pub struct SeedFamily {
    pub id: usize,
    pub name: String,
    pub start: i64,
    pub end: i64,
    #[serde(rename = "unlockLevel")]
    pub unlock_level: u32,
    #[serde(rename = "listedBaseChance")]
    pub listed_base_chance: f64,
    #[serde(rename = "evolutionDenom")]
    pub evolution_denom: f64,
}

/// One Day or Night market upgrade.
#[derive(Debug, Deserialize)]
pub struct MarketUpgrade {
    pub page: usize,
    pub id: usize,
    pub name: String,
    pub description: String,
    #[serde(rename = "cropBase")]
    pub crop_base: f64,
    #[serde(rename = "cropRate")]
    pub crop_rate: f64,
    #[serde(rename = "costBase")]
    pub cost_base: f64,
    #[serde(rename = "costGrowth")]
    pub cost_growth: f64,
    #[serde(rename = "cropsRequired")]
    pub crops_required: f64,
    #[serde(rename = "maxLevel")]
    pub max_level: f64,
    #[serde(rename = "effectPerLevel")]
    pub effect_per_level: f64,
}

/// One exotic market upgrade row.
#[derive(Debug, Deserialize)]
pub struct ExoticUpgrade {
    pub id: usize,
    pub name: String,
    pub description: String,
    #[serde(rename = "cropId")]
    pub crop_id: u32,
    pub base: f64,
    pub decay: bool,
    #[serde(rename = "purchaseType")]
    pub purchase_type: f64,
    #[serde(rename = "farmSlot")]
    pub farm_slot: usize,
}

impl FarmingData {
    /// A seed family by its upstream index.
    pub fn seed(&self, id: usize) -> Option<&SeedFamily> {
        self.seeds.get(id)
    }

    /// The family a crop id belongs to.
    pub fn family_of(&self, crop: i64) -> Option<&SeedFamily> {
        self.seeds
            .iter()
            .find(|family| crop >= family.start && crop <= family.end)
    }

    /// A market row; page 99 reads the Night market, as the upstream does.
    pub fn market(&self, page: usize, id: usize) -> Option<&MarketUpgrade> {
        let page = if page == 99 { 1 } else { page };
        self.market
            .iter()
            .find(|row| row.page == page && row.id == id)
    }

    /// An exotic upgrade row.
    pub fn exotic(&self, id: usize) -> Option<&ExoticUpgrade> {
        self.exotics.get(id)
    }

    /// `emporiumBonus(t, Ninja[102][9])`: whether the encoding of the owned
    /// emporium items carries the character that item `t` is encoded as.
    pub fn emporium_owned(&self, owned: &str, item: usize) -> bool {
        self.emporium_encoding
            .chars()
            .nth(item)
            .is_some_and(|code| owned.contains(code))
    }
}

/// The bundled farming data, parsed once.
pub fn data() -> &'static FarmingData {
    static DATA: OnceLock<FarmingData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(FARMING_JSON).expect("bundled farming data is valid JSON")
    })
}

/// One plot's saved state (`farmPlotData[i]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plot {
    /// Seed family index, or `-1` for an empty plot.
    pub seed_family: i64,
    pub growth_progress: f64,
    pub evolution: u32,
    pub evolution_blocked: bool,
    pub crops_on_vine: u32,
    pub overgrowth: u32,
    pub overflow_progress: f64,
}

impl Default for Plot {
    fn default() -> Self {
        Self {
            seed_family: -1,
            growth_progress: 0.0,
            evolution: 0,
            evolution_blocked: false,
            crops_on_vine: 0,
            overgrowth: 0,
            overflow_progress: 0.0,
        }
    }
}

/// The farming state a save carries.
#[derive(Clone, Debug)]
pub struct Farm {
    pub plots: Vec<Plot>,
    /// Quantities per crop id, as `farmCropData` keys them.
    pub crop_quantities: BTreeMap<u32, f64>,
    /// `farmCropCount`: how many crop ids the save has discovered.
    pub crop_count: u32,
    /// Land Rank per plot (`farmRankData[0]`).
    pub rank_ranks: Vec<u32>,
    /// Land Rank progress per plot (`farmRankData[1]`).
    pub rank_progress: Vec<f64>,
    /// The twenty Land Rank upgrade levels (`farmRankData[2]`).
    pub rank_levels: Vec<u32>,
    /// Market upgrade levels (`farmUpgData`).
    pub market_levels: Vec<f64>,
    /// Organic-labeled exotic ids from `Spelunk[50]`.
    pub organic_exotics: Vec<u32>,
    /// `GemItemsPurchased`, for the vine bonus at slot 139.
    pub gem_items: Vec<f64>,
    /// The active character's Farming level (`Lv0_<n>[16]`).
    pub farm_level: u32,
    /// The emporium items the account owns: `Ninja[102][9]`, one character per
    /// item. It gates the Crop Scientist bonuses.
    pub emporium_owned: String,
    /// `Grimoire[22]` and its multiplier `Grimoire[36]`.
    pub grimoire_crop_scientist: f64,
    /// `UpgVault[79]`, `Properly Funded Research`, the Crop Scientist vault.
    pub vault_crop_scientist: f64,
}

impl Default for Farm {
    fn default() -> Self {
        Self {
            plots: vec![Plot::default(); PLOTS],
            crop_quantities: BTreeMap::new(),
            crop_count: 0,
            rank_ranks: vec![0; PLOTS],
            rank_progress: vec![0.0; PLOTS],
            rank_levels: vec![0; RANK_UPGRADES],
            market_levels: Vec::new(),
            organic_exotics: Vec::new(),
            gem_items: Vec::new(),
            farm_level: 0,
            emporium_owned: String::new(),
            grimoire_crop_scientist: 0.0,
            vault_crop_scientist: 0.0,
        }
    }
}

impl Farm {
    /// Whether any plot carries a seed.
    pub fn has_plots(&self) -> bool {
        self.plots.iter().any(|plot| plot.seed_family >= 0)
    }
}

mod account;
mod allocation;
mod assignment;
mod crop_scientist;
mod curves;
mod evolution;
mod growth;
mod plot;
mod presets;
mod signals;

pub use account::{RankTotals, Snapshot};
pub use allocation::{Allocation, Mode, UpgradeChange};
pub use assignment::{Assignment, CropRow, Objective};
pub use crop_scientist::{CropScientistBonus, CropScientistUnit};
pub use curves::{OvergrowthAdvice, OvergrowthCurve, OvergrowthRow};
pub use evolution::{EvolutionChain, EvolutionStep, ProbabilityPlan};
pub use growth::GrowthBreakpoint;
pub use plot::{CropValue, PlotMetrics, land_rank_requirement, overgrowth_multiplier};
pub use presets::Comparison;
pub use signals::Diagnostics;

#[cfg(test)]
mod tests {
    use super::assignment::crop_weight;
    use super::*;

    fn plot(family: i64, evolution: u32) -> Plot {
        Plot {
            seed_family: family,
            evolution,
            ..Plot::default()
        }
    }

    #[test]
    fn the_seed_families_cover_every_crop_once() {
        let data = data();
        assert_eq!(data.seeds.len(), 7);
        for (index, family) in data.seeds.iter().enumerate() {
            assert_eq!(family.id, index);
            if index > 0 {
                assert_eq!(family.start, data.seeds[index - 1].end + 1);
            }
        }
        assert_eq!(data.seeds.first().unwrap().start, 0);
        assert_eq!(data.seeds.last().unwrap().end, 329);
        assert!(data.family_of(231).is_some_and(|family| family.id == 6));
        assert!(data.family_of(330).is_none());
    }

    #[test]
    fn growth_requirement_follows_the_family_formula() {
        let farm = Farm::default();
        let snapshot = farm.snapshot();
        assert_eq!(farm.growth_requirement(&plot(0, 0), &snapshot), 14_400.0);
        assert_eq!(
            farm.growth_requirement(&plot(2, 0), &snapshot),
            14_400.0 * 2.25
        );
        assert_eq!(
            farm.growth_requirement(&plot(6, 0), &snapshot),
            25_200.0 * snapshot.growth_rate
        );
        assert_eq!(farm.growth_requirement(&plot(-1, 0), &snapshot), 0.0);
    }

    #[test]
    fn growth_seconds_never_go_negative() {
        let farm = Farm::default();
        let snapshot = farm.snapshot();
        let mut overgrown = plot(0, 0);
        overgrown.growth_progress = 20_000.0;
        assert_eq!(farm.growth_seconds(&overgrown, &snapshot), 0.0);
        assert_eq!(
            farm.growth_seconds(&plot(0, 0), &snapshot),
            14_400.0 / snapshot.growth_rate
        );
        assert!(farm.growth_seconds(&plot(-1, 0), &snapshot).is_infinite());
    }

    #[test]
    fn evolution_chance_uses_the_family_denominator_and_plot_rank() {
        let mut farm = Farm {
            farm_level: 2,
            ..Farm::default()
        };
        farm.plots[0] = plot(0, 0);
        let snapshot = farm.snapshot();
        assert!((farm.next_crop_chance(0, &plot(0, 0), &snapshot) - 0.3).abs() < 1e-12);
        let deep = plot(0, 2);
        assert!(
            (farm.next_crop_chance(0, &deep, &snapshot) - 0.3 * 0.75_f64.powi(2)).abs() < 1e-12
        );
        // The family's last crop cannot evolve further.
        assert_eq!(farm.next_crop_chance(0, &plot(0, 20), &snapshot), 0.0);
        // Farming level 2 is the upstream gate.
        farm.farm_level = 1;
        assert_eq!(farm.next_crop_chance(0, &plot(0, 0), &farm.snapshot()), 0.0);
    }

    #[test]
    fn overgrowth_chance_halves_per_level_and_the_multiplier_caps() {
        let farm = Farm::default();
        let snapshot = farm.snapshot();
        assert!((farm.next_overgrowth_chance(&plot(0, 0), &snapshot) - 0.4).abs() < 1e-12);
        let mut grown = plot(0, 0);
        grown.overgrowth = 2;
        assert!((farm.next_overgrowth_chance(&grown, &snapshot) - 0.4_f64.powi(3)).abs() < 1e-12);
        assert_eq!(overgrowth_multiplier(0), 1.0);
        assert_eq!(overgrowth_multiplier(3), 8.0);
        assert_eq!(overgrowth_multiplier(200), 1e9);
    }

    #[test]
    fn crop_value_applies_the_cap_and_the_doubler_roll() {
        let mut farm = Farm::default();
        farm.plots[0] = plot(0, 0);
        let value = farm.plot_metrics_with(0, &farm.snapshot()).crop_value;
        assert_eq!(value.cap, 10_000.0);
        assert!(!value.capped);
        assert_eq!(value.product_low, 1.0);
        assert_eq!(value.product_high_chance, 0.0);
        assert_eq!(value.expected, value.base.round());
        assert_eq!(value.expected_harvest, value.expected);
    }

    #[test]
    fn rank_xp_needs_the_land_rank_market_unlock() {
        let mut farm = Farm::default();
        farm.plots[1] = plot(0, 0);
        assert_eq!(farm.rank_xp_per_harvest(1, &farm.snapshot()), 0.0);
        // Night market row 5 (LAND RANK) at level 1 unlocks the plot XP, and
        // plot 2 reads plot 1's Land Rank.
        farm.market_levels = vec![0.0; 100];
        farm.market_levels[10 + 5] = 1.0;
        farm.rank_ranks[0] = 1;
        assert!(farm.rank_xp_per_harvest(1, &farm.snapshot()) > 0.0);
    }

    #[test]
    fn land_rank_requirement_and_fifth_column_follow_the_formulas() {
        assert_eq!(land_rank_requirement(0), 10.0);
        assert!((land_rank_requirement(1) - 18.87).abs() < 1e-9);
        assert_eq!(Farm::default().fifth_column_max(), 1);
        let mut farm = Farm::default();
        farm.rank_levels[19] = 1;
        assert_eq!(farm.land_rank_points_left(), -1);
    }

    #[test]
    fn the_market_pages_read_their_slots() {
        let mut farm = Farm {
            market_levels: vec![0.0; 100],
            ..Farm::default()
        };
        // Day market idx 2 (Nutritious Soil) is +1% growth per level.
        farm.market_levels[2 + 2] = 10.0;
        assert_eq!(farm.basket_upgrade(0, 2), 10.0);
        // Night market idx 3 (OG Fertilizer) is a multiplier of 1 + 1% per level.
        farm.market_levels[10 + 3] = 2.0;
        assert!((farm.basket_upgrade(1, 3) - 1.02).abs() < 1e-12);
        assert_eq!(farm.basket_upgrade(2, 0), 0.0);
    }

    #[test]
    fn the_objectives_and_modes_round_trip_their_upstream_ids() {
        for objective in Objective::ALL {
            assert_eq!(Objective::from_id(objective.id()), Some(objective));
        }
        assert_eq!(Objective::from_id("nope"), None);
        assert_eq!(Objective::default(), Objective::RankXp);
        for mode in Mode::ALL {
            assert_eq!(Mode::from_id(mode.id()), Some(mode));
        }
        assert_eq!(Mode::default(), Mode::Unspent);
    }

    #[test]
    fn bean_weight_grows_with_the_family_and_the_crop() {
        assert!((crop_weight(0) - 1.0).abs() < 1e-12);
        // Each family starts its own scale at 1 and multiplies by 2.5.
        assert!((crop_weight(21) - 2.5).abs() < 1e-12);
        assert!(crop_weight(1) > crop_weight(0));
        assert!(crop_weight(45) > crop_weight(44));
        assert!(crop_weight(230) > crop_weight(21));
        assert_eq!(crop_weight(330), 0.0);
    }

    #[test]
    fn the_growth_cycle_keeps_at_least_one_second() {
        let farm = Farm::default();
        let mut snapshot = farm.snapshot();
        let plot = plot(0, 0);
        snapshot.growth_rate = 1e9;
        assert_eq!(farm.growth_cycle_seconds(&plot, &snapshot), 1.0);
        snapshot.growth_rate = 1.0;
        assert_eq!(farm.growth_cycle_seconds(&plot, &snapshot), 14_400.0);
        assert!(
            farm.growth_cycle_seconds(&Plot::default(), &snapshot)
                .is_infinite()
        );
    }

    #[test]
    fn a_default_assignment_targets_nothing() {
        let assignment = Assignment::default();
        assert_eq!(assignment.rows.len(), PLOTS);
        assert_eq!(assignment.changes(), 0);
        assert!(assignment.rows.iter().all(|row| row.target_crop == -1));
    }
}
