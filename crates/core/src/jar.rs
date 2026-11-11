//! The Jar Cavern (W5): the jar summary, rupie balances and production.
//!
//! Ported from the upstream `js/stats/systems/w5/jar.js` and its
//! `jar-calc.html` page. The upstream reads a decoded save through
//! `holesData`; here the same rows come from the `Holes` field of an imported
//! save, so every accessor below is a direct transcription of one upstream
//! reader.
//!
//! The overview and the production tab are complete. Rupie value, the break
//! chances, the upgrade plan and the enchant simulator are JR-02 to JR-04 and
//! are not here yet.
//!
//! Two upstream inputs are not in a save and stay at their upstream defaults:
//! the Legend-points multiplier on collectible bonuses (`ext.legendPts29`) and
//! the external bonuses JR-02 adds (bells, Cosmo, Cglunko, monument, stamps).

use crate::save::json;
use serde_json::{Map, Value};

/// `JAR_TYPE_NAMES`: the ten jar types, in save order.
pub const TYPE_NAMES: [&str; 10] = [
    "Basic Rupie",
    "Opal",
    "Collectible",
    "Decent Rupie",
    "Enchant",
    "White Rupie",
    "Elegant Rupie",
    "Gilded",
    "Dark Rupie",
    "Master Rupie",
];

/// What breaking a jar type gives, as the upstream `JAR_TYPE_CATS` classes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeCategory {
    /// Rupies of one or more colors.
    Rupie,
    /// A chance at an opal, a collectible or an enchant.
    Chance,
    /// The gilded doubler bonus.
    Gilded,
}

impl TypeCategory {
    /// The word the slot table's badge shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rupie => "rupie",
            Self::Chance => "chance",
            Self::Gilded => "gilded",
        }
    }
}

/// `JAR_TYPE_CATS`, one category per type.
pub const TYPE_CATEGORIES: [TypeCategory; 10] = [
    TypeCategory::Rupie,
    TypeCategory::Chance,
    TypeCategory::Chance,
    TypeCategory::Rupie,
    TypeCategory::Chance,
    TypeCategory::Rupie,
    TypeCategory::Rupie,
    TypeCategory::Gilded,
    TypeCategory::Rupie,
    TypeCategory::Rupie,
];

/// `RUPIE_NAMES`: the ten standard rupies, then white and dark.
pub const RUPIE_NAMES: [&str; 12] = [
    "Red", "Blue", "Green", "Purple", "Yellow", "Orange", "Cyan", "Pink", "Crimson", "Master",
    "White", "Dark",
];

/// `RUPIE_COLORS`, the dot each rupie wears, in the same order as [`RUPIE_NAMES`].
pub const RUPIE_COLORS: [&str; 12] = [
    "#f44", "#4488ff", "#4caf50", "#bb86fc", "#ffe066", "#ff9800", "#4dd0e1", "#f48fb1", "#e53935",
    "#ffd700", "#eee", "#666",
];

/// `COLLECTIBLE_DATA[i][0]`: the forty collectibles, in save order.
pub const COLLECTIBLE_NAMES: [&str; 40] = [
    "Abnormal Rupie",
    "Sapphire Droplet",
    "Effervescent Diamond",
    "Tortole Rock",
    "Natural Pearl",
    "Amethyst Heartstone",
    "Amber Square",
    "Verdant Thorns",
    "Violent Violets",
    "Blue Faberge Egg",
    "Shadow Prism",
    "Big Beef Rock",
    "Emerald Ore",
    "Dawn Prism",
    "Swampstone",
    "Frost Spirestone",
    "Rosemerald",
    "Blood Glass",
    "Sunrise Diamond",
    "Minceraft Gem",
    "Crimson Spade",
    "Stained Glassdrop",
    "Tabula Rasastone",
    "Deep Blue Square",
    "Earthbound Geode",
    "Inferno Droplet",
    "Octogonal Gem",
    "Solarfang",
    "Mystic Ore",
    "Arcane Prism",
    "Murky Fabrege Egg",
    "Corpore Rock",
    "Twilight Prism",
    "Tewball Orbstone",
    "Mad Muscle Rock",
    "Sunroot Splinters",
    "Twisted Rupie",
    "Overloaded Relic",
    "Sunburst Pearl",
    "Bloodfang Spires",
];

/// `COLLECTIBLE_DATA[i][1]`: the bonus each collectible level grants.
const COLLECTIBLE_BONUS_PER_LEVEL: [f64; 40] = [
    20.0, 10.0, 20.0, 25.0, 15.0, 10.0, 25.0, 25.0, 20.0, 15.0, 20.0, 25.0, 30.0, 30.0, 25.0, 12.0,
    10.0, 40.0, 25.0, 20.0, 20.0, 35.0, 32.0, 1.0, 15.0, 40.0, 30.0, 32.0, 50.0, 38.0, 1.0, 1.0,
    1.0, 40.0, 40.0, 40.0, 75.0, 1.0, 1.0, 1.0,
];

/// The collectible page two unlocks with `Advanced Collection`.
const PAGE_TWO_COLLECTIBLES: usize = 16;
const COLLECTIBLES: usize = 40;

/// Progress a slot makes per hour before any bonus (`productionPerHR`).
pub const BASE_PRODUCTION_PER_HOUR: f64 = 36_000.0;

/// The `holesData` rows the jar reads, by their upstream index.
mod row {
    pub const COSMO: usize = 4;
    pub const OPALS: usize = 7;
    pub const RUPIES: usize = 9;
    pub const STATE: usize = 11;
    pub const UPGRADES: usize = 13;
    pub const MEASUREMENTS: usize = 22;
    pub const COLLECTIBLES: usize = 24;
    pub const SLOTS: usize = 25;
}

/// Cells of [`row::STATE`] the jar reads.
mod state {
    pub const TYPES_OWNED: usize = 37;
    pub const WHITE_RUPIES: usize = 38;
    pub const DARK_RUPIES: usize = 39;
    pub const JARS_PRODUCED: usize = 40;
    pub const DOUBLER: usize = 60;
    pub const TARGETED_ENCHANT: usize = 62;
}

/// Cells of [`row::RUPIES`] the ten standard rupies start at.
const STANDARD_RUPIES: usize = 20;

/// Schematic indices in [`row::UPGRADES`] (`B_UPG_DATA`).
mod upgrade {
    pub const SLOTS_TWO: usize = 63;
    pub const SLOTS_THREE: usize = 66;
    pub const REQUIREMENT: usize = 67;
    pub const BREAK_ALL: usize = 69;
    pub const LIGHT_SPEED: usize = 72;
    pub const PRODUCTION_LINE: usize = 74;
    pub const PAGE_TWO: usize = 75;
}

/// The measurement that scales jar production, and its `HolesInfo` inputs.
mod measurement {
    /// `Holes[22][12]`, "Jar Production Speed".
    pub const PRODUCTION: usize = 12;
    /// `HolesInfo[55][12]` is `30TOT`: a TOT base scales as `level / (100 + level)`.
    pub const PRODUCTION_BASE: f64 = 30.0;
    /// `HolesInfo[52][12]` is type 2, `tomeQTY(5) / 500`: the account level sum.
    pub const PRODUCTION_KIND: u32 = 2;
}

/// One production bonus, as `productionPerHRBreakdown` lists it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProductionTerm {
    /// The system that grants it.
    pub label: &'static str,
    /// The percentage points it adds to the base rate.
    pub percent: f64,
}

/// The Jar Cavern state one save carries.
#[derive(Clone, Debug, Default)]
pub struct Jar {
    /// `holesData`: the cavern's saved rows, missing cells read as zero.
    rows: Vec<Vec<f64>>,
    /// The sum of `Lv0_<n>[0]` across characters, which measurement 12 reads as
    /// its Tome input.
    account_levels: f64,
}

impl Jar {
    /// Read the cavern out of a decoded save, or `None` when it has no `Holes`.
    pub fn from_save(save: &Map<String, Value>) -> Option<Self> {
        let holes = json::parsed_field(save, "Holes")?;
        let rows = holes
            .as_array()?
            .iter()
            .map(|row| {
                row.as_array()
                    .map(|cells| {
                        cells
                            .iter()
                            .map(|cell| json::number(cell).unwrap_or(0.0))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        Some(Self {
            rows,
            account_levels: account_levels(save),
        })
    }

    /// A cell of a `holesData` row; a missing row or cell reads as zero.
    fn cell(&self, row: usize, cell: usize) -> f64 {
        self.rows
            .get(row)
            .and_then(|cells| cells.get(cell))
            .copied()
            .unwrap_or(0.0)
    }

    /// Whether a `B_UPG_DATA` schematic is owned (`holesData[13][idx] >= 1`).
    fn upgrade(&self, index: usize) -> bool {
        self.cell(row::UPGRADES, index) >= 1.0
    }

    /// `_bUpgVal`: the schematic's value once owned, zero before.
    fn upgrade_value(&self, index: usize, value: f64) -> f64 {
        if self.upgrade(index) { value } else { 0.0 }
    }

    // ----- summary -----

    /// `jarTypesOwned`: the jar types bought above the free base jar.
    pub fn types_purchased(&self) -> usize {
        (self.cell(row::STATE, state::TYPES_OWNED).round().max(0.0) as usize).min(9)
    }

    /// `totalJarTypes`: the base jar plus the purchased ones.
    pub fn types_available(&self) -> usize {
        self.types_purchased() + 1
    }

    /// The jar types the account has not bought yet, in save order.
    pub fn missing_types(&self) -> impl Iterator<Item = usize> {
        self.types_available()..TYPE_NAMES.len()
    }

    /// `newJarCost`: what the next jar type costs, in the rupie it is paid with.
    pub fn next_type_cost(&self) -> f64 {
        let owned = self.types_purchased() as f64;
        50.0 * (1.0 + owned).powf(1.5) * 4.8_f64.powf(owned)
    }

    /// `canAffordNewJar`: whether the rupie the next type is paid with covers it.
    pub fn can_afford_next_type(&self) -> bool {
        self.types_purchased() < 9 && self.rupies(self.types_purchased()) >= self.next_type_cost()
    }

    /// `opalsFromJars`: opals the cavern has banked from jar breaks.
    pub fn opals(&self) -> f64 {
        self.cell(row::OPALS, 10)
    }

    /// `doublerBonus`: the gilded jar bonus waiting on the next rupie break.
    pub fn doubler_bonus(&self) -> f64 {
        self.cell(row::STATE, state::DOUBLER)
    }

    /// `breakAllUnlocked`.
    pub fn break_all_unlocked(&self) -> bool {
        self.upgrade(upgrade::BREAK_ALL)
    }

    /// `page2Unlocked`: whether the second collectible page is open.
    pub fn page_two_unlocked(&self) -> bool {
        self.upgrade(upgrade::PAGE_TWO)
    }

    /// The collectible pages the account can find.
    pub fn collectible_slots(&self) -> usize {
        if self.page_two_unlocked() {
            COLLECTIBLES
        } else {
            PAGE_TWO_COLLECTIBLES
        }
    }

    /// `targetedEnchantIdx`: the collectible the next enchant targets, if any.
    pub fn targeted_enchant(&self) -> Option<usize> {
        let index = self.cell(row::STATE, state::TARGETED_ENCHANT).round();
        (0.0..COLLECTIBLES as f64)
            .contains(&index)
            .then_some(index as usize)
    }

    /// `targetedEnchantProb`: the chance an enchant lands on the target.
    pub fn targeted_enchant_chance(&self) -> f64 {
        let found = self.collectibles_found();
        if found == 0 {
            return 0.0;
        }
        5.0 / (found as f64 + 3.0)
    }

    // ----- rupies and collectibles -----

    /// `rupieBalance`: ten standard rupies at `Holes[9][20 + n]`, then white and
    /// dark from their own storage cells.
    pub fn rupies(&self, rupie: usize) -> f64 {
        match rupie {
            10 => self.cell(row::STATE, state::WHITE_RUPIES),
            11 => self.cell(row::STATE, state::DARK_RUPIES),
            other => self.cell(row::RUPIES, STANDARD_RUPIES + other),
        }
    }

    /// `whiteRupies`.
    pub fn white_rupies(&self) -> f64 {
        self.rupies(10)
    }

    /// `collectibleLevel`.
    fn collectible_level(&self, index: usize) -> u32 {
        self.cell(row::COLLECTIBLES, index).max(0.0) as u32
    }

    /// `collectibleBonus`: a collectible's total bonus at its saved level.
    fn collectible_bonus(&self, index: usize) -> f64 {
        let per_level = COLLECTIBLE_BONUS_PER_LEVEL
            .get(index)
            .copied()
            .unwrap_or(0.0);
        f64::from(self.collectible_level(index)) * per_level
    }

    /// `collectiblesFound`: how many of the forty collectibles are found.
    pub fn collectibles_found(&self) -> usize {
        (0..COLLECTIBLES)
            .filter(|index| self.collectible_level(*index) >= 1)
            .count()
    }

    /// `totalEnchantLevels`: every level past the first, which the enchant
    /// chance counts.
    pub fn enchant_levels(&self) -> u32 {
        (0..COLLECTIBLES)
            .map(|index| self.collectible_level(index).saturating_sub(1))
            .sum()
    }

    // ----- production -----

    /// `productionSlots`: one slot, two with Big Jar Mach III, three with VI.
    pub fn production_slots(&self) -> usize {
        if self.upgrade(upgrade::SLOTS_THREE) {
            3
        } else if self.upgrade(upgrade::SLOTS_TWO) {
            2
        } else {
            1
        }
    }

    /// `productionSlotType`: the jar type a slot is filling.
    pub fn slot_type(&self, slot: usize) -> usize {
        (self.cell(row::SLOTS, slot).round().max(0.0) as usize).min(TYPE_NAMES.len() - 1)
    }

    /// `productionSlotProgress`: the progress a slot has banked toward its next jar.
    pub fn slot_progress(&self, slot: usize) -> f64 {
        self.cell(row::SLOTS, slot + 3)
    }

    /// `jarsProducedOfType`: how many jars of a type the account has broken.
    pub fn jars_produced(&self, jar_type: usize) -> f64 {
        self.cell(row::STATE, state::JARS_PRODUCED + jar_type)
    }

    /// `productionPerHR`: progress every slot shares, before any bonus.
    pub fn production_per_hour(&self) -> f64 {
        let collectibles =
            self.collectible_bonus(1) + self.collectible_bonus(15) + self.collectible_bonus(24);
        let bonus = collectibles + self.measurement_bonus(measurement::PRODUCTION);
        let light_speed =
            self.upgrade_value(upgrade::LIGHT_SPEED, 10.0) * log10(self.white_rupies()) / 100.0;
        BASE_PRODUCTION_PER_HOUR * (1.0 + bonus / 100.0) * (1.0 + light_speed)
    }

    /// `productionPerHRBreakdown`: the bonuses [`Self::production_per_hour`]
    /// applies, in the upstream order, without the ones that grant nothing.
    pub fn production_terms(&self) -> Vec<ProductionTerm> {
        let mut terms = Vec::new();
        for (label, index) in [
            ("Sapphire Droplet", 1),
            ("Frost Spirestone", 15),
            ("Earthbound Geode", 24),
        ] {
            let bonus = self.collectible_bonus(index);
            if bonus > 0.0 {
                terms.push(ProductionTerm {
                    label,
                    percent: bonus,
                });
            }
        }
        let measurement = self.measurement_bonus(measurement::PRODUCTION);
        if measurement > 0.0 {
            terms.push(ProductionTerm {
                label: "Jar Production Speed Measurement",
                percent: measurement,
            });
        }
        if self.upgrade(upgrade::LIGHT_SPEED) {
            terms.push(ProductionTerm {
                label: "Light Speed",
                percent: self.upgrade_value(upgrade::LIGHT_SPEED, 10.0)
                    * log10(self.white_rupies()),
            });
        }
        terms
    }

    /// `productionREQ`: the progress a jar of `jar_type` needs, after Big Jar
    /// Mach VII and the production line.
    pub fn production_requirement(&self, jar_type: usize) -> f64 {
        let (mach_seven, line) = self.requirement_reduction(jar_type);
        (1000.0 + 2000.0 * jar_type as f64) / ((1.0 + mach_seven / 100.0) * (1.0 + line / 100.0))
    }

    /// What Big Jar Mach VII and the production line take off a requirement, as
    /// the requirement table lists them.
    pub fn requirement_reduction(&self, jar_type: usize) -> (f64, f64) {
        let mach_seven = self.upgrade_value(upgrade::REQUIREMENT, 30.0);
        let previous = self.jars_produced(jar_type.saturating_sub(1));
        let line = self.upgrade_value(upgrade::PRODUCTION_LINE, 5.0) * log10(previous);
        (mach_seven, line)
    }

    /// `timeToTier`: hours one slot needs to reach `tier` of `jar_type`, counting
    /// the tiers below it.
    pub fn time_to_tier(&self, jar_type: usize, tier: u32) -> f64 {
        let needed = self.production_requirement(jar_type) * 10_f64.powi(tier as i32);
        needed / self.production_per_hour()
    }

    /// `_measBonus`: a measurement's base at its saved level, scaled by the
    /// multiplier its input type grants.
    fn measurement_bonus(&self, index: usize) -> f64 {
        let level = self.cell(row::MEASUREMENTS, index);
        if level <= 0.0 {
            return 0.0;
        }
        // `_cosmoBonus(1, 3)`: the tier one measurement upgrade is 25 a level.
        let cosmo = 1.0 + (25.0 * self.cell(row::COSMO + 1, 3)).floor() / 100.0;
        // Only the production measurement is read here; its base is a `TOT` one.
        let base = cosmo * measurement::PRODUCTION_BASE * level / (100.0 + level);
        base * self.measurement_multi(measurement::PRODUCTION_KIND)
    }

    /// `_measMulti`: what a measurement's input is worth. Type 2 is the account
    /// level sum over 500; the other types arrive with the systems that own them.
    fn measurement_multi(&self, kind: u32) -> f64 {
        let quantity = match kind {
            measurement::PRODUCTION_KIND => self.account_levels / 500.0,
            _ => return 1.0,
        };
        if quantity < 5.0 {
            1.0 + 18.0 * quantity / 100.0
        } else {
            1.0 + (18.0 * quantity + 8.0 * (quantity - 5.0)) / 100.0
        }
    }
}

/// `getLOG`: the game's log helper, which divides by a truncated `ln(10)`.
///
/// The literal is deliberately not `f64::consts::LN_10`: the game writes
/// `Math.log(x) / 2.30259`, and the last digits move a requirement.
#[allow(clippy::approx_constant)]
fn log10(value: f64) -> f64 {
    value.max(1.0).ln() / 2.30259
}

/// The sum of `Lv0_<n>[0]`, which measurement 12 reads as `tomeQTY(5)`.
fn account_levels(save: &Map<String, Value>) -> f64 {
    save.iter()
        .filter(|(key, _)| {
            key.strip_prefix("Lv0_")
                .is_some_and(|index| index.parse::<usize>().is_ok())
        })
        .filter_map(|(key, _)| json::parsed_field(save, key))
        .filter_map(|row| json::index(&row, &[0]).and_then(json::number))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jar(holes: Value) -> Jar {
        let mut save = Map::new();
        save.insert("Holes".into(), json!(holes.to_string()));
        Jar::from_save(&save).expect("the save carries Holes")
    }

    #[test]
    fn a_save_without_holes_has_no_jar_state() {
        assert!(Jar::from_save(&Map::new()).is_none());
        let mut save = Map::new();
        save.insert("Holes".into(), json!("not an array"));
        assert!(Jar::from_save(&save).is_none());
    }

    #[test]
    fn missing_rows_read_as_zero_instead_of_panicking() {
        let empty = jar(json!([]));
        assert_eq!(empty.types_available(), 1);
        assert_eq!(empty.production_slots(), 1);
        assert_eq!(empty.rupies(0), 0.0);
        assert_eq!(empty.collectibles_found(), 0);
        assert_eq!(empty.production_per_hour(), BASE_PRODUCTION_PER_HOUR);
    }

    #[test]
    fn a_slot_needs_one_schematic_per_extra_slot() {
        let mut rows = vec![vec![0.0; 150]; 26];
        assert_eq!(jar(json!(rows)).production_slots(), 1);
        rows[13][63] = 1.0;
        assert_eq!(jar(json!(rows)).production_slots(), 2);
        rows[13][66] = 1.0;
        assert_eq!(jar(json!(rows)).production_slots(), 3);
    }

    #[test]
    fn a_jar_type_costs_more_in_the_rupies_the_one_below_it_stores() {
        let empty = jar(json!([]));
        assert_eq!(empty.next_type_cost(), 50.0);
        assert!(!empty.can_afford_next_type());

        let mut rows = vec![vec![0.0; 150]; 26];
        rows[9][20] = 50.0;
        assert!(jar(json!(rows)).can_afford_next_type());

        // All nine bought: there is no next type to pay for.
        let mut rows = vec![vec![0.0; 150]; 26];
        rows[11][37] = 9.0;
        rows[9][29] = 1e30;
        assert!(!jar(json!(rows)).can_afford_next_type());
    }

    #[test]
    fn the_production_measurement_scales_with_the_account_levels() {
        let mut rows = vec![vec![0.0; 150]; 26];
        rows[22][12] = 100.0;
        let mut save = Map::new();
        save.insert("Holes".into(), json!(json!(rows).to_string()));
        // 3000 levels is a Tome quantity of 6: `18 × 6 + 8 × 1` percent.
        save.insert("Lv0_0".into(), json!(json!([3000]).to_string()));
        let jar = Jar::from_save(&save).expect("the save carries Holes");
        // A TOT base at level 100 is half of 30, and six Tome quantities add 116%.
        assert!((jar.measurement_bonus(12) - 15.0 * 2.16).abs() < 1e-9);
        assert_eq!(jar.production_terms().len(), 1);
    }

    #[test]
    fn the_production_line_divides_the_requirement_by_its_log() {
        let mut rows = vec![vec![0.0; 150]; 26];
        assert_eq!(jar(json!(rows.clone())).production_requirement(3), 7000.0);
        rows[13][67] = 1.0;
        assert_eq!(
            jar(json!(rows.clone())).production_requirement(3),
            7000.0 / 1.3
        );
        rows[13][74] = 1.0;
        rows[11][42] = 1000.0;
        // `5 × log10(1000)` is 15 percent off the requirement.
        let expected = 7000.0 / (1.3 * (1.0 + 5.0 * log10(1000.0) / 100.0));
        assert!((jar(json!(rows)).production_requirement(3) - expected).abs() < 1e-9);
    }

    #[test]
    fn the_targeted_enchant_is_only_read_when_it_names_a_collectible() {
        let mut rows = vec![vec![0.0; 150]; 26];
        // The game writes the target into the cell; zero names the first collectible.
        assert_eq!(jar(json!(rows.clone())).targeted_enchant(), Some(0));
        rows[11][62] = 30.0;
        assert_eq!(jar(json!(rows.clone())).targeted_enchant(), Some(30));
        rows[11][62] = 40.0;
        assert_eq!(jar(json!(rows)).targeted_enchant(), None);
    }

    #[test]
    fn page_two_decides_how_many_collectibles_the_account_can_find() {
        let mut rows = vec![vec![0.0; 150]; 26];
        let first_page = jar(json!(rows.clone()));
        assert_eq!(first_page.collectible_slots(), 16);
        rows[13][75] = 1.0;
        assert_eq!(jar(json!(rows)).collectible_slots(), 40);
    }

    #[test]
    fn the_game_log_floors_at_one_like_the_upstream_helper() {
        assert_eq!(log10(0.0), 0.0);
        assert!((log10(1000.0) - 3.0).abs() < 1e-4);
    }
}
