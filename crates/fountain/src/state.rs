//! Reading the fountain state out of an imported save.
//!
//! The upstream calculator receives one decoded save object; this module does
//! the same reads from `Holes`, the options list and the other account rows the
//! overview consumes, and records which bonus sources the save does not carry.

use super::saved;
use super::*;
use std::sync::Arc;

impl Fountain {
    /// Read the cavern out of a decoded save, or `None` when it has no `Holes`.
    pub fn from_save(save: &Map<String, Value>, roster: Roster) -> Option<Self> {
        let Value::Array(holes) = json::parsed_field(save, "Holes")? else {
            return None;
        };
        let has_row = |index: usize| {
            holes
                .get(index)
                .and_then(Value::as_array)
                .is_some_and(|cells| !cells.is_empty())
        };
        // The app writes `OptionsListAccount`; a raw game save writes `OptLacc`.
        let options = {
            let options = field(save, "OptionsListAccount");
            if options.is_empty() {
                field(save, "OptLacc")
            } else {
                options
            }
        };
        let spelunk = field(save, "Spelunk");
        let arcade = field(save, "ArcadeUpg");
        let cards_row = json::parsed_field(save, "Cards1");
        let cards = cards_row
            .as_ref()
            .and_then(|value| value.as_array().map(Vec::len))
            .unwrap_or(0) as f64;
        let present = Present {
            upgrades: has_row(row::UPGRADES),
            marbles: has_row(row::MARBLES),
            schematics: has_row(row::SCHEMATICS),
            bells: has_row(row::BELLS),
            lamp: has_row(row::LAMP),
            measurements: has_row(row::MEASUREMENTS),
            bolaia: has_row(row::BOLAIA),
            options: !options.is_empty(),
            spelunk: !spelunk.is_empty(),
            arcade: !arcade.is_empty(),
            cards: cards_row.is_some(),
        };
        let upg_lvs = levels(&holes, row::UPGRADES);
        let marble_lvs = levels(&holes, row::MARBLES);
        let mut fountain = Self {
            holes: Arc::new(holes),
            options,
            spelunk,
            arcade,
            cards,
            roster,
            upg_lvs,
            marble_lvs,
            present,
            missing: Vec::new(),
        };
        fountain.missing = fountain.find_missing();
        Some(fountain)
    }

    /// The bonus sources the save does not carry.
    pub fn missing_bonuses(&self) -> &[String] {
        &self.missing
    }

    fn find_missing(&self) -> Vec<String> {
        let present = self.present;
        let mut missing = Vec::new();
        if !present.upgrades {
            missing.push("Upgrade levels (Holes[31])".to_owned());
        }
        if !present.marbles {
            missing.push("Marble levels (Holes[32])".to_owned());
        }
        if !present.schematics {
            missing.push("Trench depth and Size Does Matter (Holes[13])".to_owned());
        }
        if !present.bells {
            missing.push("Bell coin bonus (Holes[17][6])".to_owned());
        }
        if !present.lamp {
            missing.push("Lamp wish level (Holes[21][7])".to_owned());
        }
        if !present.measurements {
            missing.push("Measurement bonus (Holes[22][16])".to_owned());
        }
        if !present.bolaia {
            missing.push("Bolaia marble rate (Holes[26][15])".to_owned());
        }
        if !present.options {
            missing.push("Ducks and Cglunko coin value (OptLacc)".to_owned());
        }
        if !present.spelunk {
            missing.push("Lamp wish scaling (Spelunk[45][2])".to_owned());
        }
        if !present.arcade {
            missing.push("Arcade fountain speed (ArcadeUpg[68])".to_owned());
        }
        if !present.cards {
            missing.push("Card measurement (Cards1)".to_owned());
        }
        if !self.roster.available {
            missing.push("Companion 27 arcade doubling (it.json companion)".to_owned());
        }
        missing
    }

    // ----- raw readers -----

    /// A cell of a `holesData` row; a missing row or cell reads as zero.
    pub(super) fn cell(&self, row: usize, cell: usize) -> f64 {
        nested_cell(&self.holes, row, cell)
    }

    /// A cell of the options list.
    pub(super) fn option(&self, index: usize) -> f64 {
        self.options
            .get(index)
            .and_then(json::number)
            .unwrap_or(0.0)
    }

    /// A cell of `Spelunk`.
    pub(super) fn spelunk_cell(&self, row: usize, cell: usize) -> f64 {
        nested_cell(&self.spelunk, row, cell)
    }

    /// `_bUpg`: a schematic's value once owned, zero before.
    pub(super) fn schematic(&self, index: usize, value: f64) -> f64 {
        if self.cell(row::SCHEMATICS, index) >= 1.0 {
            value
        } else {
            0.0
        }
    }

    /// `_cosmoBonus(t, i)`: `floor(base × level)`.
    pub(super) fn cosmo_bonus(&self, tier: usize, index: usize) -> f64 {
        let base = match (tier, index) {
            (0, cosmo::MAX_STACK) => data().cosmo.max_stack,
            (1, cosmo::MEASUREMENT) => data().cosmo.measurement,
            _ => 0.0,
        };
        (base * self.cell(row::COSMO + tier, index)).floor()
    }

    // ----- saved state -----

    /// `fountainLv`: the fountain level, the first cell of row 15 in row 7.
    pub fn level(&self) -> f64 {
        self.cell(row::LEVEL, 15)
    }

    /// `upgLvs`: one water's upgrade levels.
    pub fn upgrade_level(&self, water: usize, upgrade: usize) -> f64 {
        self.upg_lvs
            .get(water)
            .and_then(|levels| levels.get(upgrade))
            .copied()
            .unwrap_or(0.0)
    }

    /// `marbleLvs`: one water's marble upgrade levels.
    pub fn marble_level(&self, water: usize, upgrade: usize) -> f64 {
        self.marble_lvs
            .get(water)
            .and_then(|levels| levels.get(upgrade))
            .copied()
            .unwrap_or(0.0)
    }

    /// `marbleCurrency`: the marble balance.
    pub fn marble(&self) -> f64 {
        self.cell(row::STATE, saved::MARBLE)
    }

    /// `desiredCurrency`: the currency the save has desired.
    pub fn desired_currency(&self) -> usize {
        self.cell(row::STATE, saved::DESIRED).max(0.0) as usize
    }

    /// `fountCurrencyAvail`: the opal balance a fountain level is paid with.
    pub fn level_currency_balance(&self, currency: usize) -> f64 {
        self.cell(row::CURRENCY, 30 + currency)
    }

    /// `luckyCoinCount`: the lucky coins collected for a currency.
    pub fn lucky_coin_count(&self, currency: usize) -> f64 {
        self.cell(row::LUCKY, currency).max(0.0)
    }

    /// `duckCount`: the rubber ducks owned, `OptLacc[601]`.
    pub fn duck_count(&self) -> f64 {
        self.option(601)
    }

    /// `duckBarProgress`: the saved duck bar progress in seconds.
    pub fn duck_bar_progress(&self) -> f64 {
        self.cell(row::BARS, 2)
    }

    /// `marbleBarProgress`: the saved marble bar progress in seconds.
    pub fn marble_bar_progress(&self) -> f64 {
        self.cell(row::BARS, 1)
    }

    /// `coinStacks`: the stacks on the ground, in slot order.
    pub fn stacks(&self) -> Vec<CoinStack> {
        let Some(rows) = self.holes.get(row::STACKS).and_then(Value::as_array) else {
            return Vec::new();
        };
        rows.iter()
            .enumerate()
            .filter_map(|(slot, coins)| {
                let coins: Vec<usize> = coins
                    .as_array()?
                    .iter()
                    .filter_map(json::number)
                    .map(|coin| coin.max(0.0) as usize)
                    .collect();
                (!coins.is_empty()).then(|| CoinStack {
                    slot,
                    royal: self.cell(row::ROYAL, slot) >= 1.0,
                    coins,
                })
            })
            .collect()
    }
}

/// Read a top-level save field as an array of values.
fn field(save: &Map<String, Value>, key: &str) -> Vec<Value> {
    match json::parsed_field(save, key) {
        Some(Value::Array(cells)) => cells,
        _ => Vec::new(),
    }
}

/// One number of a row-of-rows; a missing row or cell reads as zero.
fn nested_cell(rows: &[Value], row: usize, cell: usize) -> f64 {
    rows.get(row)
        .and_then(Value::as_array)
        .and_then(|cells| cells.get(cell))
        .and_then(json::number)
        .unwrap_or(0.0)
}

/// `upgLvs`/`marbleLvs`: twenty levels per water, missing cells read as zero.
fn levels(holes: &[Value], row: usize) -> Vec<Vec<f64>> {
    let rows = holes.get(row).and_then(Value::as_array);
    (0..WATERS)
        .map(|water| {
            let cells = rows
                .and_then(|rows| rows.get(water))
                .and_then(Value::as_array);
            (0..data::UPGRADES_PER_WATER)
                .map(|upgrade| {
                    cells
                        .and_then(|cells| cells.get(upgrade))
                        .and_then(json::number)
                        .unwrap_or(0.0)
                })
                .collect()
        })
        .collect()
}
