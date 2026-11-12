//! Currency values, income rates, stacks and the lucky-coin and duck systems.
//!
//! Every function transcribes one upstream reader from
//! `js/stats/systems/w5/fountain.js`. The per-currency income table is the one
//! block the upstream page computes inline; it follows the same order of
//! operations, including the royal weighting and the full-stack bonus.

use super::data::data;
use super::{
    CURRENCIES, CoinStack, CurrencyRow, DesiredRow, Fountain, Income, IncomeRow, UnlockRow, WATERS,
    WaterMultiplier, cosmo, row, saved, schematic,
};
use idleon_common::curve::log;

impl Fountain {
    /// `currencyFountainMulti`: the bonuses the fountain's own upgrades grant.
    pub fn currency_fountain_multi(&self) -> f64 {
        let blue = 1.0 + self.bon_tot(0, 0) / 100.0;
        let yellow = 1.0 + self.bon_tot(1, 0) / 100.0;
        let green = 1.0 + self.bon_tot(2, 0) / 100.0;
        let boosters = 1.0 + self.boosters() / 100.0;
        blue * boosters * yellow * green
    }

    /// The four currency boosters, which the fountain multiplier sums.
    pub(super) fn boosters(&self) -> f64 {
        self.bon_tot(0, 1) + self.bon_tot(1, 1) + self.bon_tot(2, 1) + self.bon_tot(1, 12)
    }

    /// `_bellBonus`: the bell coin bonus at slot 6.
    fn bell_bonus(&self) -> f64 {
        let level = self.cell(row::BELLS, 6);
        level * data().holes.bell_per_level.get(6).copied().unwrap_or(0.0)
    }

    /// `_lampBonus99`: the lamp-wish bonus, scaled by the Spelunk wish level.
    fn lamp_bonus(&self) -> f64 {
        let level = self.cell(row::LAMP, 7);
        let wish = self.spelunk_cell(45, 2);
        data().lamp_per_level * level * (1.0 + wish / 100.0)
    }

    /// `_bolaiaBonus`: the Bolaia study bonus at one measurement slot.
    fn bolaia_bonus(&self, index: usize) -> f64 {
        self.cell(row::BOLAIA, index)
            * data()
                .holes
                .bolaia_per_level
                .get(index)
                .copied()
                .unwrap_or(0.0)
    }

    /// `_measBonus`: one measurement's bonus, its base and its multiplier.
    fn measurement_bonus(&self, index: usize) -> f64 {
        let level = self.cell(row::MEASUREMENTS, index);
        if level <= 0.0 {
            return 0.0;
        }
        let cosmo = self.cosmo_bonus(1, cosmo::MEASUREMENT);
        let (number, is_total) = super::measurement(index).unwrap_or((0.0, false));
        let base = if is_total {
            (1.0 + cosmo / 100.0) * (number * level / (100.0 + level))
        } else {
            (1.0 + cosmo / 100.0) * number * level
        };
        base * self.measurement_multi(super::measurement_type(index))
    }

    /// `_measMulti`: the account quantity a measurement type counts.
    fn measurement_multi(&self, kind: u32) -> f64 {
        let cards = self.cards;
        let qty = match kind {
            // Only the card count is read here; every other type stays neutral.
            8 => cards / data().cards_per_measurement_unit,
            _ => 0.0,
        };
        if qty < 5.0 {
            1.0 + 18.0 * qty / 100.0
        } else {
            1.0 + (18.0 * qty + 8.0 * (qty - 5.0)) / 100.0
        }
    }

    /// `_cglunkoUpgBon(14)`: the Cglunko coin-value upgrade at level × 10.
    fn cglunko_bonus(&self) -> f64 {
        self.option(644) * 10.0
    }

    /// `currencyExternalMulti`'s terms, in the order the breakdown lists them.
    pub(super) fn external_terms(&self) -> Vec<(&'static str, f64, String)> {
        let trench_level = self.schematic(schematic::TRENCH, 1.0);
        let depth = self.cell(row::STATE, saved::TRENCH_DEPTH);
        let trench = (trench_level * 1.1_f64.powf(depth)).max(1.0);
        let cosmo_04 = self.cosmo_bonus(0, cosmo::MAX_STACK);
        let lamp = self.lamp_bonus();
        let measurement = self.measurement_bonus(16);
        let bell = self.bell_bonus();
        let cglunko = self.cglunko_bonus();
        vec![
            ("Trench depth", trench, format!("Owned × 1.1^{depth}")),
            (
                "Cosmo Max Coin Stack",
                1.0 + 25.0 * cosmo_04 / 100.0,
                format!("Level {cosmo_04} × 25%"),
            ),
            ("Lamp wish", 1.0 + lamp / 400.0, format!("{lamp} / 400")),
            (
                "Measurement 16",
                1.0 + measurement / 100.0,
                format!("+{measurement}% from Holes[22][16] and the card count"),
            ),
            (
                "Bell coin bonus",
                1.0 + bell / 100.0,
                format!("+{bell}% from Holes[17][6]"),
            ),
            (
                "Cglunko coin value",
                1.0 + cglunko / 100.0,
                format!("Cglunko upgrade 14 × 10 = +{cglunko}%"),
            ),
        ]
    }

    /// `currencyExternalMulti`: everything outside the fountain that scales coins.
    pub fn currency_external_multi(&self) -> f64 {
        self.external_terms()
            .iter()
            .map(|(_, value, _)| *value)
            .product()
    }

    /// `currencyAllMulti`: the whole currency value multiplier.
    pub fn currency_all_multi(&self) -> f64 {
        self.currency_fountain_multi() * self.currency_external_multi()
    }

    /// `currencyBaseValue`: one currency's own base value.
    pub fn currency_base_value(&self, currency: usize) -> f64 {
        let mythril_log = self.mythril_log();
        let sharp_log = self.sharp_log();
        let rupie_log = self.rupie_log();
        match currency {
            0 => 1.0 + self.bon_tot(0, 2) * (1.0 + self.bon_tot(0, 2) / 100.0) * mythril_log,
            1 => 1.0 + self.bon_tot(0, 3) * (1.0 + self.bon_tot(0, 5) / 100.0) * mythril_log,
            2 => 1.0 + self.bon_tot(0, 4) * (1.0 + self.bon_tot(0, 6) / 100.0) * mythril_log,
            3 => 1.0 + self.bon_tot(1, 2) * (1.0 + self.bon_tot(0, 7) / 100.0) * sharp_log,
            4 => 1.0 + self.bon_tot(1, 3) * (1.0 + self.bon_tot(1, 5) / 100.0) * sharp_log,
            5 => 1.0 + self.bon_tot(1, 4) * (1.0 + self.bon_tot(1, 6) / 100.0) * sharp_log,
            6 => 1.0 + self.bon_tot(2, 2) * (1.0 + self.bon_tot(1, 7) / 100.0) * rupie_log,
            7 => 1.0 + self.bon_tot(2, 3) * (1.0 + self.bon_tot(2, 5) / 100.0) * rupie_log,
            8 => 1.0 + self.bon_tot(2, 4) * (1.0 + self.bon_tot(2, 6) / 100.0) * rupie_log,
            _ => 1.0,
        }
    }

    /// The Mythril Transience log: Bronze, Silver and Gold scale with the POW 10
    /// Mythril the save carries.
    fn mythril_log(&self) -> f64 {
        self.pow_log(3, 0)
    }

    /// The Sharp Surrogate log: Dollar, Credit and Treasury scale with POW 10
    /// Sharp Notes.
    fn sharp_log(&self) -> f64 {
        self.pow_log(16, 1)
    }

    /// The Rupie Monie log: Moolah, Shilling and Greane scale with POW 10 Red
    /// Rupie.
    fn rupie_log(&self) -> f64 {
        self.pow_log(20, 2)
    }

    /// `1 + log10(POW 10 <resource>) × bonTOT / 100`.
    fn pow_log(&self, cell: usize, water: usize) -> f64 {
        1.0 + log(self.cell(row::CURRENCY, cell)) * self.bon_tot(water, 19) / 100.0
    }

    /// `luckyCoinChance`: the chance a collected coin is lucky.
    pub fn lucky_coin_chance(&self, currency: usize) -> f64 {
        let base = self.bon_tot(2, 8);
        if base == 0.0 {
            return 0.0;
        }
        let boost = self.bon_tot(2, 10);
        let count = self.lucky_coin_count(currency);
        (0.001 * (1.0 + (base + boost) / 100.0) * 0.25_f64.powf(count)).min(1.0)
    }

    /// `luckyCoinValue`: the value multiplier a currency's lucky coins give.
    pub fn lucky_coin_value(&self, currency: usize) -> f64 {
        let count = self.lucky_coin_count(currency);
        if count <= 0.0 {
            return 1.0;
        }
        1.0 + count * self.lucky_coin_value_per() / 100.0
    }

    /// `duckChance`: the chance a duck bar fill gives a duck.
    pub fn duck_chance(&self) -> f64 {
        if !self.duck_bar_unlocked() {
            return 0.0;
        }
        let boost = self.bon_tot(2, 12) + self.bon_tot(2, 10);
        let count = self.duck_count();
        (1.0 / 3.0 * (1.0 + boost / 100.0) * 0.2_f64.powf(count)).min(1.0)
    }

    /// `duckMulti`: the currency multiplier the owned ducks give.
    pub fn duck_multi(&self) -> f64 {
        let count = self.duck_count();
        if count <= 0.0 {
            return 1.0;
        }
        (1.0 + self.bon_tot(2, 11) / 100.0).powf(count)
    }

    /// `duckOddsDisplay`: the odds ratio and percentage of the next duck.
    pub fn duck_odds(&self) -> (String, String) {
        let chance = self.duck_chance();
        if chance <= 0.0 {
            return ("-".into(), "-".into());
        }
        let percent = chance * 100.0;
        let percent = if percent < 0.1 {
            format!("{percent:.4}")
        } else {
            format!("{percent:.2}")
        };
        (format!("{:.2}", 1.0 / chance), percent)
    }

    /// `currencyTotalValue`: one coin's value at the save's desired currency.
    pub fn currency_total_value(&self, currency: usize) -> f64 {
        self.currency_total_value_with(currency, self.desired_currency())
    }

    /// `currencyTotalValue` with the desired currency chosen by the caller, so
    /// the section can simulate switching it.
    pub fn currency_total_value_with(&self, currency: usize, desired: usize) -> f64 {
        let base = self.currency_base_value(currency);
        let all = self.currency_all_multi();
        let desire = self.desire_multi();
        let lucky = self.lucky_coin_value(currency);
        let duck = self.duck_multi();
        if currency >= 6 {
            // Green currencies: desire always applies, inside the square root.
            (desire * base * all).sqrt() * lucky * duck
        } else {
            let desire = if currency == desired { desire } else { 1.0 };
            desire * base * all * lucky * duck
        }
    }

    /// `marblePerFill`: the marble one marble-bar fill gives.
    pub fn marble_per_fill(&self) -> f64 {
        100.0
            * (1.0 + self.bon_tot(1, 10) / 100.0)
            * (1.0 + self.bolaia_bonus(15) / 100.0)
            * (1.0 + 10.0 * self.cosmo_bonus(0, cosmo::MAX_STACK) / 100.0)
    }

    /// `marblePerHr`: the marble an hour of active play gives.
    pub fn marble_per_hr(&self) -> f64 {
        if !self.marble_bar_unlocked() {
            return 0.0;
        }
        self.marble_per_fill() * 3600.0 * self.active_speed() / Self::marble_bar_fill_time()
    }

    /// `optimalStackValueForCurrency`: one full stack of a currency, with the
    /// royal multiplier and the Size Does Matter bonus.
    pub fn optimal_stack_value_for_currency(&self, currency: usize, desired: usize) -> f64 {
        let stack = self.max_stack();
        stack
            * self.currency_total_value_with(currency, desired)
            * self.effective_royal_multi()
            * (1.0 + stack * self.size_matter_bonus() / 100.0)
    }

    /// `optimalStackValue`: one full stack of the desired currency.
    pub fn optimal_stack_value(&self, desired: usize) -> f64 {
        self.optimal_stack_value_for_currency(desired, desired)
    }

    /// `optimalStacksPerHr`: the full royal stacks a collector can bank each hour.
    pub fn optimal_stacks_per_hr(&self) -> f64 {
        let chance = self.royal_chance();
        let stack = self.max_stack();
        let fills_per_hr = 3600.0 * self.active_speed() / self.bar_fill_time();
        if chance <= 0.0 {
            return fills_per_hr / stack;
        }
        let royal_during_fill = 1.0 - (1.0 - chance).powf(stack);
        let wait = if royal_during_fill < 1.0 {
            (1.0 - royal_during_fill) / chance
        } else {
            0.0
        };
        fills_per_hr / (stack + wait)
    }

    /// `flushTime`: the seconds to collect every space once.
    pub fn flush_time(&self) -> f64 {
        let stacks = self.optimal_stacks_per_hr();
        if stacks <= 0.0 {
            return f64::INFINITY;
        }
        self.spaces() as f64 / stacks * 3600.0
    }

    /// `stackRedeemValue`: what one ground stack is worth when collected.
    pub fn stack_redeem_value(&self, stack: &CoinStack) -> f64 {
        let size = self.size_matter_bonus();
        let royal = self.effective_royal_multi();
        stack
            .coins
            .iter()
            .map(|currency| {
                let value = self.currency_total_value(*currency);
                let coin = value * (1.0 + stack.count() as f64 * size / 100.0);
                if stack.royal { coin * royal } else { coin }
            })
            .sum()
    }

    /// `totalRedeemValue`: every ground stack's value.
    pub fn total_redeem_value(&self) -> f64 {
        self.stacks()
            .iter()
            .map(|stack| self.stack_redeem_value(stack))
            .sum()
    }

    /// The redeem value split by currency, as the overview card's caption reads it.
    pub fn redeem_by_currency(&self) -> [f64; CURRENCIES] {
        let mut values = [0.0; CURRENCIES];
        for stack in self.stacks() {
            let total = self.stack_redeem_value(&stack);
            let mut counts = [0usize; CURRENCIES];
            for currency in &stack.coins {
                if let Some(count) = counts.get_mut(*currency) {
                    *count += 1;
                }
            }
            let kinds: Vec<usize> = (0..CURRENCIES)
                .filter(|currency| counts[*currency] > 0)
                .collect();
            if kinds.len() == 1 {
                values[kinds[0]] += total;
                continue;
            }
            let weights: Vec<f64> = kinds
                .iter()
                .map(|currency| self.currency_total_value(*currency) * counts[*currency] as f64)
                .collect();
            let weight: f64 = weights.iter().sum();
            for (currency, share) in kinds.iter().zip(&weights) {
                values[*currency] += if weight > 0.0 {
                    total * share / weight
                } else {
                    0.0
                };
            }
        }
        values
    }

    /// `activeEarnRatesPerHr`: every currency's active rate, optionally focused.
    ///
    /// A focus assumes Turn and Push hides every other currency; `None` reads
    /// the save's own active set. The desired currency is the save's.
    pub fn active_earn_rates(&self, focus: Option<usize>) -> [f64; CURRENCIES] {
        self.active_earn_rates_with(self.desired_currency(), focus)
    }

    /// `activeEarnRatesPerHr` with the desired currency chosen by the caller.
    pub fn active_earn_rates_with(
        &self,
        desired: usize,
        focus: Option<usize>,
    ) -> [f64; CURRENCIES] {
        let active = match focus {
            Some(currency) if self.ignore_unlocked() && self.currency_unlocked(currency) => {
                vec![currency]
            }
            _ => self.active_currencies(),
        };
        let share = 1.0 / active.len() as f64;
        let stacks = self.optimal_stacks_per_hr();
        let royal = self.effective_royal_multi();
        let stack = self.max_stack();
        let size = 1.0 + stack * self.size_matter_bonus() / 100.0;
        let mut rates = [0.0; CURRENCIES];
        for currency in active {
            rates[currency] = stacks
                * stack
                * self.currency_total_value_with(currency, desired)
                * royal
                * size
                * share;
        }
        rates
    }

    /// The income table: one row per active currency and its totals.
    pub fn income(&self) -> Income {
        let active = self.active_currencies();
        let royal_chance = self.royal_chance();
        let royal = self.effective_royal_multi();
        let stack = self.max_stack();
        let size = self.size_matter_bonus();
        let single_stack = 1.0 + size / 100.0;
        let full_stack = 1.0 + stack * size / 100.0;
        let active_speed = self.active_speed();
        let active_fills = 3600.0 / (self.bar_fill_time() / active_speed);
        let afk_fills = 3600.0 / self.bar_fill_time();
        let optimal = self.active_earn_rates(None);
        let share = 1.0 / active.len() as f64;
        let rows: Vec<IncomeRow> = active
            .iter()
            .map(|currency| {
                let value = self.currency_total_value(*currency);
                let royal_weighted = value * (1.0 - royal_chance) + value * royal * royal_chance;
                let active_share = active_fills * share;
                let afk_share = afk_fills * share;
                IncomeRow {
                    currency: *currency,
                    value,
                    single: active_share * royal_weighted * single_stack,
                    single_afk: afk_share * royal_weighted * single_stack,
                    full: active_share * royal_weighted * full_stack,
                    full_afk: afk_share * royal_weighted * full_stack,
                    optimal: optimal[*currency],
                    optimal_afk: optimal[*currency] / active_speed,
                }
            })
            .collect();
        let total = |read: fn(&IncomeRow) -> f64| rows.iter().map(read).sum::<f64>();
        Income {
            single: total(|row| row.single),
            single_afk: total(|row| row.single_afk),
            full: total(|row| row.full),
            full_afk: total(|row| row.full_afk),
            optimal: total(|row| row.optimal),
            optimal_afk: total(|row| row.optimal_afk),
            rows,
        }
    }

    /// `activeSpeedMulti`'s fill time: one coin bar while standing at the fountain.
    pub fn active_fill_time(&self) -> f64 {
        self.bar_fill_time() / self.active_speed()
    }

    /// `stackFillTime`: one space's coins at the active rate.
    pub fn stack_fill_time(&self) -> f64 {
        self.active_fill_time() * self.max_stack()
    }

    /// `allStacksFillTime`: every space, at the active rate.
    pub fn all_stacks_fill_time(&self) -> f64 {
        self.stack_fill_time() * self.spaces() as f64
    }

    /// `afkStackFillTime`: one space's coins while away.
    pub fn afk_stack_fill_time(&self) -> f64 {
        self.bar_fill_time() * self.max_stack()
    }

    /// `afkAllStacksFillTime`: every space, while away.
    pub fn afk_all_stacks_fill_time(&self) -> f64 {
        self.afk_stack_fill_time() * self.spaces() as f64
    }

    // ----- currency tab -----

    /// `earnRatesPerHr`: every unlocked currency's optimal /hr, as if it were
    /// both the desired currency and the only one generating when Turn and Push
    /// is bought.
    pub fn earn_rates(&self) -> [f64; CURRENCIES] {
        let focus = self.ignore_unlocked();
        let mut rates = [0.0; CURRENCIES];
        for (currency, rate) in rates.iter_mut().enumerate() {
            if !self.currency_unlocked(currency) {
                continue;
            }
            *rate = self.active_earn_rates_with(currency, focus.then_some(currency))[currency];
        }
        rates
    }

    /// The share a full stack adds on top of its coins.
    fn stack_value_multi(&self) -> f64 {
        1.0 + self.max_stack() * self.size_matter_bonus() / 100.0
    }

    /// The Currency Values table, every type in game order.
    ///
    /// `desired` is the simulated desired currency; pass
    /// [`Fountain::desired_currency`] for the save's own choice.
    pub fn currency_rows(&self, desired: usize) -> Vec<CurrencyRow> {
        let active = self.active_currencies();
        let stack = self.max_stack() * self.stack_value_multi();
        let royal = self.effective_royal_multi();
        (0..CURRENCIES)
            .map(|currency| {
                let total = self.currency_total_value_with(currency, desired);
                let full_stack = total * stack;
                let unlocked = self.currency_unlocked(currency);
                let is_active = active.contains(&currency);
                CurrencyRow {
                    currency,
                    base: self.currency_base_value(currency),
                    lucky: self.lucky_coin_value(currency),
                    total,
                    full_stack,
                    royal_stack: full_stack * royal,
                    unlocked,
                    ignored: self.currency_ignored(currency),
                    active: is_active,
                    create: if unlocked && is_active {
                        1.0 / active.len() as f64
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }

    /// The per-water POW-10 log multipliers, one row per water.
    pub fn water_multipliers(&self) -> Vec<WaterMultiplier> {
        let shared = self.currency_all_multi();
        let own = [self.mythril_log(), self.sharp_log(), self.rupie_log()];
        (0..WATERS)
            .map(|water| WaterMultiplier {
                water,
                shared,
                own: own[water],
            })
            .collect()
    }

    /// The Optimal Desired Currency simulator: what a full ground redeems for
    /// at each unlocked desired currency.
    pub fn desired_rows(&self, desired: usize) -> Vec<DesiredRow> {
        let active = self.active_currencies();
        let royal_chance = self.royal_chance();
        let royal = self.effective_royal_multi();
        let ground = self.spaces() as f64 * self.max_stack() * self.stack_value_multi();
        let mut rows: Vec<DesiredRow> = (0..CURRENCIES)
            .filter(|currency| self.currency_unlocked(*currency))
            .map(|currency| {
                let average = active
                    .iter()
                    .map(|active| self.currency_total_value_with(*active, currency))
                    .sum::<f64>()
                    / active.len() as f64;
                let weighted = average * (1.0 - royal_chance) + average * royal * royal_chance;
                DesiredRow {
                    currency,
                    average,
                    weighted,
                    ground: weighted * ground,
                    current: currency == desired,
                    best: false,
                }
            })
            .collect();
        let mut best: Option<usize> = None;
        let mut best_ground = 0.0;
        for (index, row) in rows.iter().enumerate() {
            if row.ground > best_ground {
                best_ground = row.ground;
                best = Some(index);
            }
        }
        if let Some(best) = best {
            rows[best].best = true;
        }
        rows
    }

    /// The Next Unlock Tracker: every currency the save has not unlocked, with
    /// the upgrade that grants it.
    pub fn next_unlocks(&self) -> Vec<UnlockRow> {
        const UNLOCKS: [(usize, usize, usize); 8] = [
            (1, 0, 3),
            (2, 0, 4),
            (3, 1, 2),
            (4, 1, 3),
            (5, 1, 4),
            (6, 2, 2),
            (7, 2, 3),
            (8, 2, 4),
        ];
        UNLOCKS
            .iter()
            .filter(|(currency, _, _)| !self.currency_unlocked(*currency))
            .map(|(currency, water, upgrade)| UnlockRow {
                currency: *currency,
                upgrade: data().upgrades[*water][*upgrade].name.as_str(),
                water: *water,
                level: self.upgrade_level(*water, *upgrade),
                prerequisite_unlocked: self.upgrade_accessible(*water, *upgrade),
            })
            .collect()
    }
}

/// The leading number of a `HolesInfo` base string, as `parseFloat` reads it.
pub(super) fn leading_number(text: &str) -> f64 {
    let digits: String = text
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    digits.parse().unwrap_or(0.0)
}
