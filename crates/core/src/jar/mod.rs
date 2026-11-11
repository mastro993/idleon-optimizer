//! The Jar Cavern (W5): the summary, production, rupie value and break chances.
//!
//! Ported from the upstream `js/stats/systems/w5/jar.js` and its
//! `jar-calc.html` page. The upstream reads a decoded save through
//! `holesData`; here the same rows come from the `Holes` field of an imported
//! save, so every accessor below is a direct transcription of one upstream
//! reader.
//!
//! The overview, the production tab, the rupie value and the break chances are
//! complete. The collectibles, the upgrade plan and the enchant simulator are
//! JR-03 and JR-04.
//!
//! The external bonuses the formulas read live in [`Ext`]. Everything the save
//! itself carries is decoded here; two bonuses (the Cavern Resource stamp and
//! the farming Exotic 52) belong to systems this app has not ported yet, so
//! they stay at their upstream default and the section says the total leaves
//! them out.

mod bonus;
mod chance;
mod data;
mod ext;
mod game_data;
mod production;
mod rupies;

pub use chance::ChanceEvent;
pub use ext::Ext;
pub use game_data::{
    BASE_PRODUCTION_PER_HOUR, COLLECTIBLE_NAMES, RUPIE_COLORS, RUPIE_NAMES, TYPE_CATEGORIES,
    TYPE_NAMES, TypeCategory,
};
use game_data::{
    COLLECTIBLE_BONUS_PER_LEVEL, COLLECTIBLES, PAGE_TWO_COLLECTIBLES, STANDARD_RUPIES,
};
pub use rupies::{INCOME_TIERS, VALUE_TIERS};

use crate::save::json;
use data::data;
use serde_json::{Map, Value};

/// The `holesData` rows the jar reads, by their upstream index.
mod row {
    /// `Holes[4]`: the tier zero Cosmo upgrades.
    pub const COSMO: usize = 4;
    /// `Holes[5]`: the tier one Cosmo upgrades.
    pub const COSMO_TIER_ONE: usize = 5;
    pub const OPALS: usize = 7;
    pub const RUPIES: usize = 9;
    pub const STATE: usize = 11;
    pub const UPGRADES: usize = 13;
    /// `Holes[15]`: the monument levels, ten per water.
    pub const MONUMENTS: usize = 15;
    /// `Holes[17]`: the bell upgrades.
    pub const BELLS: usize = 17;
    /// `Holes[21]`: the lamp upgrades, index 7 being Infinite Resources.
    pub const LAMP: usize = 21;
    pub const MEASUREMENTS: usize = 22;
    pub const COLLECTIBLES: usize = 24;
    pub const SLOTS: usize = 25;
    /// `Holes[26]`: the Bolaia studies, whose sum measurement type 9 reads.
    pub const BOLAIA: usize = 26;
    /// `Holes[31]` and `Holes[32]`: the Fountain's upgrade and marble levels,
    /// written as three water rows of twenty upgrades.
    pub const FOUNTAIN_UPGRADES: usize = 31;
    pub const FOUNTAIN_MARBLES: usize = 32;
}

/// Upgrades in the fountain rows, per water.
const FOUNTAIN_UPGRADES_PER_WATER: usize = 20;

/// Cells of [`row::STATE`] the jar reads.
mod state {
    pub const TYPES_OWNED: usize = 37;
    pub const WHITE_RUPIES: usize = 38;
    pub const DARK_RUPIES: usize = 39;
    pub const JARS_PRODUCED: usize = 40;
    /// `Holes[11][5]`: the Evertree trunks the rupie synergy counts.
    pub const EVERTREE_TRUNKS: usize = 5;
    pub const DOUBLER: usize = 60;
    pub const TARGETED_ENCHANT: usize = 62;
}

/// Schematic indices in [`row::UPGRADES`] (`B_UPG_DATA`).
mod upgrade {
    pub const SLOTS_TWO: usize = 63;
    pub const SLOTS_THREE: usize = 66;
    pub const REQUIREMENT: usize = 67;
    pub const BREAK_ALL: usize = 69;
    pub const LIGHT_SPEED: usize = 72;
    pub const DARK_LUCK: usize = 73;
    pub const PRODUCTION_LINE: usize = 74;
    pub const PAGE_TWO: usize = 75;
    pub const COLLECT_ALL: usize = 76;
    pub const BASE_RUPIE_TWO: usize = 62;
    pub const BASE_RUPIE_FOUR: usize = 64;
    pub const BASE_RUPIE_FIVE: usize = 65;
    pub const BASE_RUPIE_EIGHT: usize = 68;
    pub const EVER_TREE_SYNERGY: usize = 80;
}

/// Cells of `Spelunk` the jar reads.
mod spelunk {
    /// `Spelunk[0][8]`: the lore episode that raises palette bonuses.
    pub const LORE: usize = 8;
    /// `Spelunk[6]`: the lore episodes found.
    pub const LORE_ROW: usize = 6;
    /// `Spelunk[9]`: the palette levels.
    pub const PALETTES: usize = 9;
    /// `Spelunk[18]`: the legend talent points.
    pub const LEGEND: usize = 18;
    /// `Spelunk[45][2]`: the lamp level that scales Infinite Resources.
    pub const LAMP: usize = 45;
    /// `Spelunk[49]`: the black stars placed on each palette.
    pub const BLACK_STARS: usize = 49;
}

/// One production bonus, as `productionPerHRBreakdown` lists it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProductionTerm {
    /// The system that grants it.
    pub label: &'static str,
    /// The percentage points it adds to the base rate.
    pub percent: f64,
}

/// The Fountain's saved levels, `Holes[31]` and `Holes[32]`.
#[derive(Clone, Debug, Default)]
struct Fountain {
    upgrades: Vec<Vec<f64>>,
    marbles: Vec<Vec<f64>>,
}

/// The Jar Cavern state one save carries.
#[derive(Clone, Debug, Default)]
pub struct Jar {
    /// `holesData`: the cavern's saved rows, missing cells read as zero.
    rows: Vec<Vec<f64>>,
    /// `Spelunk`: the row of the W7 cave the cavern bonuses read.
    spelunk: Vec<Vec<f64>>,
    /// `OptLacc`: the account option list.
    opt_lacc: Vec<f64>,
    /// The Fountain's saved rows, which the game writes nested.
    fountain: Fountain,
    /// `Gaming[12]`: the SuperBit keys the account owns.
    gaming: String,
    /// The sum of `Lv0_<n>[0]` across characters, which measurement 12 reads as
    /// its Tome input.
    account_levels: f64,
    /// `tasksGlobalData[0][1][0]`, the tasks measurement type 7 reads.
    tasks_completed: f64,
    /// The bonuses other systems grant the formulas.
    pub ext: Ext,
}

impl Jar {
    /// Read the cavern out of a decoded save, or `None` when it has no `Holes`.
    pub fn from_save(save: &Map<String, Value>) -> Option<Self> {
        let holes = json::parsed_field(save, "Holes")?;
        let holes = holes.as_array()?;
        let mut rows = Vec::with_capacity(holes.len());
        let mut fountain = Fountain::default();
        for (index, row) in holes.iter().enumerate() {
            if index == row::FOUNTAIN_UPGRADES || index == row::FOUNTAIN_MARBLES {
                let waters = nested_rows(row);
                match index {
                    row::FOUNTAIN_UPGRADES => fountain.upgrades = waters,
                    _ => fountain.marbles = waters,
                }
                rows.push(Vec::new());
                continue;
            }
            rows.push(
                row.as_array()
                    .map(|cells| cells.iter().filter_map(json::number).collect())
                    .unwrap_or_default(),
            );
        }
        let mut jar = Self {
            rows,
            spelunk: rows_of(save, "Spelunk"),
            opt_lacc: numbers_of(save, "OptLacc"),
            fountain,
            gaming: cell_text(save, "Gaming", 12),
            account_levels: account_levels(save),
            tasks_completed: nested_number(save, "tasksGlobalData", &[0, 1, 0]),
            ext: Ext::default(),
        };
        jar.ext = Ext::read(&jar);
        Some(jar)
    }

    /// A cell of a `holesData` row; a missing row or cell reads as zero.
    fn cell(&self, row: usize, cell: usize) -> f64 {
        self.rows
            .get(row)
            .and_then(|cells| cells.get(cell))
            .copied()
            .unwrap_or(0.0)
    }

    /// A cell of a `Spelunk` row.
    fn spelunk(&self, row: usize, cell: usize) -> f64 {
        self.spelunk
            .get(row)
            .and_then(|cells| cells.get(cell))
            .copied()
            .unwrap_or(0.0)
    }

    /// `Spelunk[row]` as a whole.
    fn spelunk_row(&self, row: usize) -> &[f64] {
        self.spelunk.get(row).map(Vec::as_slice).unwrap_or_default()
    }

    /// A cell of `OptLacc`, which the game writes as one flat row.
    fn opt_lacc(&self, cell: usize) -> f64 {
        self.opt_lacc.get(cell).copied().unwrap_or(0.0)
    }

    /// Whether a `B_UPG_DATA` schematic is owned (`holesData[13][idx] >= 1`).
    fn upgrade(&self, index: usize) -> bool {
        self.cell(row::UPGRADES, index) >= 1.0
    }

    /// `_bUpgVal`: the schematic's value once owned, zero before.
    fn upgrade_value(&self, index: usize, value: f64) -> f64 {
        if self.upgrade(index) { value } else { 0.0 }
    }

    /// `computeCosmoBonus`: a Cosmo upgrade's base times its saved level, floored.
    fn cosmo_bonus(&self, row: usize, cell: usize, base: f64) -> f64 {
        (base * self.cell(row, cell)).floor()
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

    /// `darkRupies`.
    pub fn dark_rupies(&self) -> f64 {
        self.rupies(11)
    }

    /// `collectibleLevel`.
    fn collectible_level(&self, index: usize) -> u32 {
        self.cell(row::COLLECTIBLES, index).max(0.0) as u32
    }

    /// `collectibleBonus`: a collectible's total bonus at its saved level, raised
    /// by the legend talent points every collectible level shares.
    fn collectible_bonus(&self, index: usize, legend_pts: f64) -> f64 {
        let per_level = COLLECTIBLE_BONUS_PER_LEVEL
            .get(index)
            .copied()
            .unwrap_or(0.0);
        f64::from(self.collectible_level(index)) * per_level * (1.0 + legend_pts / 100.0)
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

    /// `computeMeasurementBonus`: a measurement's base at its saved level, scaled
    /// by the multiplier its input type grants.
    fn measurement_bonus(&self, index: usize) -> f64 {
        let level = self.cell(row::MEASUREMENTS, index);
        if level <= 0.0 {
            return 0.0;
        }
        let cosmo =
            1.0 + self.cosmo_bonus(row::COSMO_TIER_ONE, 3, data().cosmo.measurement) / 100.0;
        let base_text = data().measurement_base(index);
        let base = parse_base(base_text);
        let meas_base = if base_text.contains("TOT") {
            cosmo * (base * level / (100.0 + level))
        } else {
            cosmo * base * level
        };
        meas_base * self.measurement_multi(data().measurement_type(index))
    }

    /// `_measMulti`: what a measurement's input is worth. The types whose
    /// systems are not ported stay at the neutral multiplier the game uses for
    /// an empty input.
    fn measurement_multi(&self, kind: u32) -> f64 {
        let quantity = match kind {
            // Account levels, which the Tome reads from `Lv0_<n>[0]`.
            2 => self.account_levels / 500.0,
            // Tasks completed, `tasksGlobalData[0][1][0]`.
            7 => log10(self.tasks_completed) / 2.0,
            // The Bolaia study total, `Holes[26]` summed.
            9 => self.bolaia_total() / 6.0,
            _ => 0.0,
        };
        if quantity < 5.0 {
            1.0 + 18.0 * quantity / 100.0
        } else {
            1.0 + (18.0 * quantity + 8.0 * (quantity - 5.0)) / 100.0
        }
    }

    /// The sum of `Holes[26]`, the Bolaia studies measurement type 9 reads.
    fn bolaia_total(&self) -> f64 {
        self.rows
            .get(row::BOLAIA)
            .map(|cells| cells.iter().sum())
            .unwrap_or(0.0)
    }

    // ----- bonuses other systems grant -----
}

/// `getLOG`: the game's log helper, which divides by a truncated `ln(10)`.
///
/// The literal is deliberately not `f64::consts::LN_10`: the game writes
/// `Math.log(x) / 2.30259`, and the last digits move a requirement.
#[allow(clippy::approx_constant)]
fn log10(value: f64) -> f64 {
    value.max(1.0).ln() / 2.30259
}

/// `parseFloat` on a measurement base, which the game writes as text.
fn parse_base(text: &str) -> f64 {
    let digits: String = text
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    digits.parse().unwrap_or(0.0)
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

/// A top-level array field as one row of numbers, missing cells read as zero.
fn numbers_of(save: &Map<String, Value>, key: &str) -> Vec<f64> {
    json::parsed_field(save, key)
        .and_then(|value| {
            value.as_array().map(|cells| {
                cells
                    .iter()
                    .map(|cell| json::number(cell).unwrap_or(0.0))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// A top-level array field as rows of numbers, missing cells read as zero.
fn rows_of(save: &Map<String, Value>, key: &str) -> Vec<Vec<f64>> {
    json::parsed_field(save, key)
        .and_then(|value| {
            value.as_array().map(|rows| {
                rows.iter()
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
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// A row the game writes as arrays of arrays, as the Fountain rows are.
fn nested_rows(value: &Value) -> Vec<Vec<f64>> {
    value
        .as_array()
        .map(|waters| {
            waters
                .iter()
                .map(|row| {
                    let mut cells: Vec<f64> = row
                        .as_array()
                        .map(|cells| {
                            cells
                                .iter()
                                .map(|cell| json::number(cell).unwrap_or(0.0))
                                .collect()
                        })
                        .unwrap_or_default();
                    cells.resize(FOUNTAIN_UPGRADES_PER_WATER, 0.0);
                    cells
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One nested number, `Number(value) || 0` at every step.
fn nested_number(save: &Map<String, Value>, key: &str, path: &[usize]) -> f64 {
    json::parsed_field(save, key)
        .and_then(|value| json::index(&value, path).and_then(json::number))
        .unwrap_or(0.0)
}

/// One cell of a top-level array field as text, which `Gaming[12]` needs.
fn cell_text(save: &Map<String, Value>, key: &str, index: usize) -> String {
    json::parsed_field(save, key)
        .and_then(|value| {
            json::index(&value, &[index]).map(|cell| match cell {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
        })
        .unwrap_or_default()
}

/// The production measurement's `HolesInfo` index and the ones the rupie value
/// reads, kept named here because they are plain data indices.
mod measurement {
    /// `Holes[22][12]`, "Jar Production Speed".
    pub const PRODUCTION: usize = 12;
}

#[cfg(test)]
mod tests;
