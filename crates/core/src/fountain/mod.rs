//! The Fountain Cavern (W5): the overview, income rates, lucky coins and ducks,
//! coin stacks and the cross-system bonuses.
//!
//! Ported from the upstream `js/stats/systems/w5/fountain.js` and the overview
//! and Income Rates halves of `fountain-calc.html`. The upstream reads a decoded
//! save through `holesData`; here the same rows come from the `Holes` field of
//! an imported save, so every accessor below is a direct transcription of one
//! upstream reader.
//!
//! The currency tab, upgrade list, planner and marble planner are FN-02 to
//! FN-04 and are not here yet. The external bonuses the overview reports read
//! the same account systems upstream does, and a source the save does not carry
//! is named by [`Fountain::missing_bonuses`] instead of silently reading zero.

mod currency;
mod data;
mod state;
mod upgrades;

pub use data::Upgrade;

use crate::save::{Roster, json};
use data::data;
use serde_json::{Map, Value};

/// How many currencies the fountain has.
pub const CURRENCIES: usize = 9;

/// How many waters the game implements.
pub const WATERS: usize = 3;

/// The `holesData` rows the fountain reads, by their upstream index.
mod row {
    /// `Holes[4 + tier][i]`: the Cosmo upgrade levels.
    pub const COSMO: usize = 4;
    /// `Holes[7][15]`: the fountain level.
    pub const LEVEL: usize = 7;
    /// `Holes[9][30 + t]`: the opal balance per currency.
    pub const CURRENCY: usize = 9;
    /// `Holes[11][...]`: marble, desire, ignore list, trench and gate keys.
    pub const STATE: usize = 11;
    /// `Holes[13][...]`: the purchased cavern schematics.
    pub const SCHEMATICS: usize = 13;
    /// `Holes[17][6]`: the bell coin bonus.
    pub const BELLS: usize = 17;
    /// `Holes[21][7]`: the lamp-wish level.
    pub const LAMP: usize = 21;
    /// `Holes[22][i]`: the measurement levels.
    pub const MEASUREMENTS: usize = 22;
    /// `Holes[26][i]`: the Bolaia study levels.
    pub const BOLAIA: usize = 26;
    /// `Holes[29][slot]`: the coin types on the ground.
    pub const STACKS: usize = 29;
    /// `Holes[30][t]`: the lucky coins collected per currency.
    pub const LUCKY: usize = 30;
    /// `Holes[31][water][upgrade]`: the fountain upgrade levels.
    pub const UPGRADES: usize = 31;
    /// `Holes[32][water][upgrade]`: the marble upgrade levels.
    pub const MARBLES: usize = 32;
    /// `Holes[33][bar]`: the coin, marble and duck bar progress.
    pub const BARS: usize = 33;
    /// `Holes[34][slot]`: whether a stack is royal.
    pub const ROYAL: usize = 34;
}

/// Cells of [`row::STATE`] the fountain reads.
mod saved {
    pub const TRENCH_DEPTH: usize = 7;
    pub const MARBLE: usize = 81;
    pub const DESIRED: usize = 82;
    pub const IGNORED: usize = 83;
}

/// Cells of the `Holes[4 + tier]` Cosmo rows.
mod cosmo {
    pub const MAX_STACK: usize = 4;
    pub const MEASUREMENT: usize = 3;
}

/// Schematics in [`row::SCHEMATICS`] the fountain reads.
mod schematic {
    pub const TRENCH: usize = 94;
    pub const SIZE: usize = 95;
    pub const STACK_TWO: usize = 97;
}

/// The currency type a fountain level is paid with: one per five levels.
pub fn level_currency(level: f64) -> usize {
    ((level / 5.0).floor().max(0.0) as usize).min(CURRENCIES)
}

/// A coin stack lying on the fountain floor.
#[derive(Clone, Debug, PartialEq)]
pub struct CoinStack {
    pub slot: usize,
    pub coins: Vec<usize>,
    pub royal: bool,
}

impl CoinStack {
    /// `count`: how many coins the stack holds.
    pub fn count(&self) -> usize {
        self.coins.len()
    }
}

/// One currency on the income table.
#[derive(Clone, Debug, PartialEq)]
pub struct IncomeRow {
    pub currency: usize,
    /// One coin's value.
    pub value: f64,
    /// Value per hour collected one coin at a time.
    pub single: f64,
    pub single_afk: f64,
    /// Value per hour collected in full stacks.
    pub full: f64,
    pub full_afk: f64,
    /// Value per hour collecting only full royal stacks.
    pub optimal: f64,
    pub optimal_afk: f64,
}

/// The income table and its totals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Income {
    pub rows: Vec<IncomeRow>,
    pub single: f64,
    pub single_afk: f64,
    pub full: f64,
    pub full_afk: f64,
    pub optimal: f64,
    pub optimal_afk: f64,
}

/// One cross-system bonus the fountain upgrades grant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossBonus {
    pub label: &'static str,
    pub percent: f64,
    /// The fountain upgrade that grants it.
    pub source: &'static str,
}

/// One row of the fountain level planner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelStep {
    pub level: usize,
    pub cost: f64,
    pub currency: usize,
    /// The spend within this currency type, as upstream accumulates it.
    pub cumulative: f64,
    pub affordable: bool,
}

/// The Fountain state one save carries.
#[derive(Clone, Debug, Default)]
pub struct Fountain {
    /// `holesData`: the cavern's saved rows, missing cells read as zero.
    holes: Vec<Value>,
    /// `OptionsListAccount`/`OptLacc`: ducks (`[601]`) and Cglunko (`[644]`).
    options: Vec<Value>,
    /// `Spelunk`: row 45 slot 2 scales the lamp wish.
    spelunk: Vec<Value>,
    /// `ArcadeUpg`: `[68]` is the fountain speed level.
    arcade: Vec<Value>,
    /// How many cards `Cards1` collects, for measurement type 8.
    cards: f64,
    /// The `it.json` companion roster.
    roster: Roster,
    /// The upgrade levels, `[water][upgrade]`.
    upg_lvs: Vec<Vec<f64>>,
    /// The marble upgrade levels, `[water][upgrade]`.
    marble_lvs: Vec<Vec<f64>>,
    /// Whether each raw source was in the save.
    present: Present,
    /// Bonus sources the save does not carry, named for the warning card.
    missing: Vec<String>,
}

/// Which raw sources the save carried, for the missing-bonus warnings.
#[derive(Clone, Copy, Debug, Default)]
struct Present {
    upgrades: bool,
    marbles: bool,
    schematics: bool,
    bells: bool,
    lamp: bool,
    measurements: bool,
    bolaia: bool,
    options: bool,
    spelunk: bool,
    arcade: bool,
    cards: bool,
}

/// The display name of a currency, e.g. `Bronze Coins`.
pub fn currency_name(currency: usize) -> &'static str {
    data()
        .currency_names
        .get(currency)
        .map(String::as_str)
        .unwrap_or("Unknown")
}

/// The short label a table column prints, e.g. `Bronze`.
pub fn currency_label(currency: usize) -> &'static str {
    data()
        .currency_short
        .get(currency)
        .map(String::as_str)
        .unwrap_or("?")
}

/// The water names, in upstream order.
pub fn water_names() -> &'static [String] {
    &data().water_names
}

/// One upgrade's static row.
pub fn upgrade(water: usize, upgrade: usize) -> Option<&'static Upgrade> {
    data().upgrades.get(water)?.get(upgrade)
}

/// The game's truncated `ln(10)`, which upstream writes out as a literal.
#[allow(clippy::approx_constant)]
const LN_10_TRUNCATED: f64 = 2.30259;

/// `getLOG`: the game's `ln(max(x, 1))` over its truncated `ln(10)`.
fn get_log(value: f64) -> f64 {
    value.max(1.0).ln() / LN_10_TRUNCATED
}

#[cfg(test)]
mod tests;
