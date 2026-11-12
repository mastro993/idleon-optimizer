//! Upgrade levels, unlocks and the summary multipliers they feed.
//!
//! One method per upstream reader. [`Fountain::bon_tot`] is the percentage an
//! upgrade grants once its marble levels are counted, and every summary figure
//! the overview shows is built from it.

use super::data::{self, data};
use super::{
    CURRENCIES, CapHint, CrossBonus, Fountain, LevelStep, UpgradeRow, WATERS, cosmo, json,
    level_currency, row, saved, schematic, short_number,
};
use idleon_common::formula::{FormulaNode, ValueFormat};
use serde_json::Value;

impl Fountain {
    // ----- bonuses -----

    /// `marbleBon`: the marble multiplier a marble level grants.
    pub fn marble_bonus(level: f64) -> f64 {
        if level <= 0.0 { 1.0 } else { 1.5 + 0.5 * level }
    }

    /// `bonTOT`: the percentage one upgrade grants at its current levels.
    pub fn bon_tot(&self, water: usize, upgrade: usize) -> f64 {
        let level = self.upgrade_level(water, upgrade);
        let marble = Self::marble_bonus(self.marble_level(water, upgrade));
        let per_level = super::upgrade(water, upgrade)
            .map(|upgrade| upgrade.per_level)
            .unwrap_or(0.0);
        (marble * level * per_level).round()
    }

    /// `upgUnlocked`: whether an upgrade's prerequisite is met.
    pub fn upgrade_unlocked(&self, water: usize, upgrade: usize) -> bool {
        let Some(entry) = super::upgrade(water, upgrade) else {
            return false;
        };
        if entry.prereq < 0 {
            return true;
        }
        let prereq = self.upgrade_level(water, entry.prereq as usize);
        if water == 0 && (upgrade == 2 || upgrade == 14) {
            prereq >= 1.0
        } else {
            prereq >= 10.0
        }
    }

    /// `highestWaterIndex`: how many of the three water unlocks are bought.
    fn highest_water(&self) -> usize {
        let mut highest = 0;
        for water in 0..WATERS {
            highest += usize::from(self.upgrade_level(water, 0) >= 1.0);
        }
        highest.min(WATERS - 1)
    }

    /// `watersOwned`: the implemented waters the account can use.
    pub fn waters_owned(&self) -> usize {
        self.highest_water() + 1
    }

    /// `currencyUnlocked`: whether a currency can be found.
    pub fn currency_unlocked(&self, currency: usize) -> bool {
        match currency {
            0 => true,
            1 => self.upgrade_unlocked(0, 3),
            2 => self.upgrade_unlocked(0, 4),
            3 => self.upgrade_unlocked(1, 2),
            4 => self.upgrade_unlocked(1, 3),
            5 => self.upgrade_unlocked(1, 4),
            6 => self.upgrade_unlocked(2, 2),
            7 => self.upgrade_unlocked(2, 3),
            8 => self.upgrade_unlocked(2, 4),
            _ => false,
        }
    }

    /// `waterAccessible`: the water itself is unlocked.
    pub fn water_accessible(&self, water: usize) -> bool {
        water <= self.highest_water()
    }

    /// `upgradeAccessible`: the water and the upgrade's prerequisite are met.
    pub fn upgrade_accessible(&self, water: usize, upgrade: usize) -> bool {
        self.water_accessible(water) && self.upgrade_unlocked(water, upgrade)
    }

    /// `ignoreUnlocked`: whether Turn and Push can hide a currency.
    pub fn ignore_unlocked(&self) -> bool {
        self.upgrade_level(1, 12) >= 1.0
    }

    /// `currencyIgnored`: whether the save hides one currency type.
    ///
    /// `Holes[11][83]` holds the ignored types as letters (`_abcdefgh`); the
    /// game writes it as a string, and a zero cell means nothing is hidden.
    pub fn currency_ignored(&self, currency: usize) -> bool {
        const LETTERS: [char; 9] = ['_', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
        let raw = self
            .holes
            .get(row::STATE)
            .and_then(Value::as_array)
            .and_then(|cells| cells.get(saved::IGNORED));
        let text = match raw {
            Some(Value::String(text)) => text.clone(),
            Some(value) => match json::number(value) {
                Some(number) if number != 0.0 => format!("{number}"),
                _ => return false,
            },
            None => return false,
        };
        LETTERS
            .get(currency)
            .is_some_and(|letter| text.contains(*letter))
    }

    /// `activeCurrencyTypes`: the currencies that can appear, falling back to bronze.
    pub fn active_currencies(&self) -> Vec<usize> {
        let active: Vec<usize> = (0..CURRENCIES)
            .filter(|currency| self.currency_unlocked(*currency))
            .filter(|currency| !self.currency_ignored(*currency))
            .collect();
        if active.is_empty() { vec![0] } else { active }
    }

    /// `spacesOwned`: how many coins can lie on the ground at once.
    pub fn spaces(&self) -> usize {
        (1.0 + self.bon_tot(0, 10)).min(16.0) as usize
    }

    /// `maxStackSize`: how many coins one space holds.
    pub fn max_stack(&self) -> f64 {
        (3.0 + self.schematic(schematic::STACK_TWO, 3.0)
            + self.bon_tot(0, 11)
            + self.cosmo_bonus(0, cosmo::MAX_STACK))
        .min(50.0)
    }

    /// `maxCoins`: the spaces times the stack size.
    pub fn max_coins(&self) -> f64 {
        self.spaces() as f64 * self.max_stack()
    }

    /// `barSpeed`: the AFK fill rate multiplier.
    pub fn bar_speed(&self) -> f64 {
        (1.0 + self.bon_tot(0, 9) / 100.0) * (1.0 + self.arcade_fountain_speed() / 100.0)
    }

    /// `_arcadeBonus68`: the arcade's Fountain Speed bonus.
    fn arcade_fountain_speed(&self) -> f64 {
        let level = self.arcade.get(68).and_then(json::number).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let params = &data().arcade_fountain_speed;
        let raw = if params.kind == "add" {
            if params.denom != 0.0 {
                ((params.base + params.denom) / params.denom + 0.5 * (level - 1.0))
                    / (params.base / params.denom)
                    * level
                    * params.base
            } else {
                params.base * level
            }
        } else {
            params.base * level / (level + params.denom)
        };
        let maxed = if level == 101.0 {
            2.0
        } else if level == 102.0 {
            6.0
        } else {
            1.0
        };
        // Companion 27 doubles a maxed arcade upgrade. Its base bonus is 1 and
        // its level-two bonus is 1.5, so only the un-enhanced companion matches.
        let companion = if self.roster.owns(27) && !self.roster.is_enhanced(27) {
            2.0
        } else {
            1.0
        };
        maxed * companion * raw
    }

    /// `barFillTime`: the AFK seconds one coin bar takes.
    pub fn bar_fill_time(&self) -> f64 {
        data().bar_fill_times[0] / self.bar_speed()
    }

    /// `marbleBarFillTime`: the AFK seconds the marble bar takes.
    pub fn marble_bar_fill_time() -> f64 {
        data().bar_fill_times[1]
    }

    /// `duckBarFillTime`: the AFK seconds the duck bar takes.
    pub fn duck_bar_fill_time() -> f64 {
        data().bar_fill_times[2]
    }

    /// `activeSpeedMultiDisplay`: the Water Bender fill multiplier alone.
    pub fn active_speed_display(&self) -> f64 {
        let bonus = self.bon_tot(0, 12);
        1.0 + (4.0 * bonus).min(4.0) + bonus / 100.0
    }

    /// `activeSpeedMulti`: the AFK and active fill rates combined.
    pub fn active_speed(&self) -> f64 {
        1.0 + self.active_speed_display()
    }

    /// `currencyKeep`: the Money Back Guarantee share of a full board's coins.
    pub fn currency_keep(&self) -> f64 {
        let bonus = self.bon_tot(0, 8);
        0.1 + bonus / (100.0 + bonus) * 0.5
    }

    /// `royalChance`: the chance a fill crowns a stack.
    pub fn royal_chance(&self) -> f64 {
        let bonus = self.bon_tot(1, 8);
        if bonus <= 0.0 {
            return 0.0;
        }
        (1.0 / 300.0 * (1.0 + bonus / 100.0)).min(1.0)
    }

    /// `royalMulti`: what a royal stack is worth.
    pub fn royal_multi(&self) -> f64 {
        5.0 + self.bon_tot(1, 9) / 100.0
    }

    /// `effectiveRoyalMulti`: the royal multiplier once Royal Stacks is bought.
    pub fn effective_royal_multi(&self) -> f64 {
        if self.royal_chance() > 0.0 {
            self.royal_multi()
        } else {
            1.0
        }
    }

    /// `desireMulti`: what a desired currency is worth.
    pub fn desire_multi(&self) -> f64 {
        1.0 + self.bon_tot(1, 11) / 100.0
    }

    /// `_sizeMatterBonus`: the stack-size coin bonus once the schematic is owned.
    pub fn size_matter_bonus(&self) -> f64 {
        self.schematic(schematic::SIZE, 10.0)
    }

    /// `marbleBarUnlocked`: whether Marble Filling is bought.
    pub fn marble_bar_unlocked(&self) -> bool {
        self.bon_tot(1, 10) >= 1.0
    }

    /// `duckBarUnlocked`: whether Rubber Ducky is bought.
    pub fn duck_bar_unlocked(&self) -> bool {
        self.upgrade_level(2, 12) >= 1.0
    }

    /// `luckyCoinBaseChance`: the lucky chance before each coin grows rarer.
    pub fn lucky_coin_base_chance(&self) -> f64 {
        let base = self.bon_tot(2, 8);
        if base == 0.0 {
            return 0.0;
        }
        let boost = self.bon_tot(2, 10);
        (0.001 * (1.0 + (base + boost) / 100.0)).min(1.0)
    }

    /// `luckyCoinValuePer`: the value a lucky coin adds, as a percentage.
    pub fn lucky_coin_value_per(&self) -> f64 {
        25.0 + self.bon_tot(2, 9)
    }

    // ----- planner -----

    /// `upgCost`: the currency the next upgrade level costs.
    ///
    /// A base cost of one is the game's per-level row, whose cost grows with the
    /// level as well as the scale.
    pub fn upgrade_cost(water: usize, upgrade: usize, level: f64) -> f64 {
        let Some(row) = super::upgrade(water, upgrade) else {
            return 0.0;
        };
        if row.base_cost == 1.0 {
            (level + 1.0) * row.cost_scale.powf(level)
        } else {
            row.base_cost * row.cost_scale.powf(level)
        }
    }

    /// `marbleCost`: the marble the next marble level costs.
    pub fn marble_cost(water: usize, upgrade: usize, marble_level: f64) -> f64 {
        if water >= 2 {
            return 250_000.0 * (2.0 * (5.0 + marble_level)).powf(marble_level);
        }
        if upgrade == 13 {
            return 1_500.0 * (10.0 + 5.0 * marble_level).powf(marble_level);
        }
        500.0 * (5.0 + marble_level).powf(marble_level)
    }

    /// `canMarble`: the upgrades `HolesInfo[76]` leaves without a marble track.
    pub fn can_marble(water: usize, upgrade: usize) -> bool {
        !matches!(
            (water, upgrade),
            (0, 8) | (0, 10) | (0, 11) | (1, 8) | (1, 10)
        )
    }

    /// The upgrade table for one water, in game order.
    pub fn upgrade_rows(&self, water: usize) -> Vec<UpgradeRow> {
        let rates = self.earn_rates();
        (0..data::UPGRADES_PER_WATER)
            .filter_map(|upgrade| self.upgrade_row_with(water, upgrade, &rates))
            .collect()
    }

    /// One upgrade row, for a section that only shows the selected upgrade.
    pub fn upgrade_row(&self, water: usize, upgrade: usize) -> Option<UpgradeRow> {
        self.upgrade_row_with(water, upgrade, &self.earn_rates())
    }

    fn upgrade_row_with(
        &self,
        water: usize,
        upgrade: usize,
        rates: &[f64; CURRENCIES],
    ) -> Option<UpgradeRow> {
        let row = super::upgrade(water, upgrade)?;
        let level = self.upgrade_level(water, upgrade);
        let marble = self.marble_level(water, upgrade);
        let next_cost = Self::upgrade_cost(water, upgrade, level);
        let currency = row.currency;
        let remaining = (next_cost - self.level_currency_balance(currency)).max(0.0);
        let rate = rates.get(currency).copied().unwrap_or(0.0);
        let seconds_to_afford = if remaining <= 0.0 {
            0.0
        } else if rate > 0.0 {
            remaining / rate * 3600.0
        } else {
            f64::INFINITY
        };
        let bonus = self.bon_tot(water, upgrade);
        Some(UpgradeRow {
            water,
            upgrade,
            name: row.name.as_str(),
            level,
            marble,
            marble_bonus: Self::marble_bonus(marble),
            bonus,
            accessible: self.upgrade_accessible(water, upgrade),
            next_cost,
            currency,
            seconds_to_afford,
            marble_cost: Self::marble_cost(water, upgrade, marble),
            marbable: Self::can_marble(water, upgrade),
            description: self.upgrade_description(water, upgrade),
            cap: self.upgrade_cap(water, upgrade, bonus),
        })
    }

    /// `_formatUpgradeDescription`: the saved numbers filled into the row text.
    pub fn upgrade_description(&self, water: usize, upgrade: usize) -> String {
        let Some(row) = super::upgrade(water, upgrade) else {
            return String::new();
        };
        let bonus = self.bon_tot(water, upgrade);
        let mut description = row
            .description
            .replace("}x", &format!("{}x", short_number(1.0 + bonus / 100.0)))
            .replace("+{%", &format!("+{}%", short_number(bonus)))
            .replace("+{", &format!("+{}", short_number(bonus)));
        let token = match (water, upgrade) {
            (0, 8) => format!("{:.1}", self.currency_keep() * 100.0),
            (0, 10) => short_number(self.spaces() as f64),
            (0, 11) => short_number(self.max_stack()),
            (0, 12) => format!("{:.2}", self.active_speed_display()),
            (1, 8) => {
                let mut chance = self.royal_chance();
                if chance <= 0.0 {
                    let mut preview = self.clone();
                    preview.upg_lvs[1][8] = 1.0;
                    chance = preview.royal_chance();
                    description.push_str(" Level 1 chance shown.");
                } else {
                    description.push_str(" Current chance shown.");
                }
                short_number(1.0 / chance)
            }
            (1, 9) => format!("{:.2}", self.royal_multi()),
            (1, 10) => short_number(self.marble_per_fill()),
            (2, 8) => {
                let mut chance = self.lucky_coin_base_chance();
                if chance <= 0.0 {
                    let mut preview = self.clone();
                    preview.upg_lvs[2][8] = 1.0;
                    chance = preview.lucky_coin_base_chance();
                    description.push_str(
                        " Level 1 base chance shown; each lucky coin of that currency makes the \
                         next one 4x rarer.",
                    );
                } else {
                    description.push_str(
                        " Current base chance shown; each lucky coin of that currency makes the \
                         next one 4x rarer.",
                    );
                }
                short_number(1.0 / chance)
            }
            (2, 9) => short_number(self.lucky_coin_value_per()),
            (2, 12) => {
                let mut chance = self.duck_chance();
                if chance <= 0.0 {
                    let mut preview = self.clone();
                    preview.upg_lvs[2][12] = 1.0;
                    chance = preview.duck_chance();
                    description.push_str(
                        " Level 1 chance shown; each owned duck makes the next one 5x rarer.",
                    );
                } else {
                    description.push_str(
                        " Current chance shown; each owned duck makes the next one 5x rarer.",
                    );
                }
                short_number(1.0 / chance)
            }
            _ => plain(bonus),
        };
        description.replace('$', &token)
    }

    /// `_upgBreakpoint`: the cap an upgrade's effect is near, if any.
    pub fn upgrade_cap(&self, water: usize, upgrade: usize, bonus: f64) -> Option<CapHint> {
        let (label, capped, detail) = match (water, upgrade) {
            (0, 10) => {
                let spaces = self.spaces();
                if spaces >= 16 {
                    ("Capped", true, "Spaces maxed at 16".to_string())
                } else {
                    ("Near cap", false, format!("{spaces}/16 spaces"))
                }
            }
            (0, 11) => {
                let stack = self.max_stack();
                if stack >= 50.0 {
                    ("Capped", true, "Stack size maxed at 50".to_string())
                } else {
                    ("Near cap", false, format!("{}/50 stack", plain(stack)))
                }
            }
            (0, 12) if bonus >= 1.0 => {
                let per_level = data().upgrades[0][12].per_level / 100.0;
                (
                    "Soft cap",
                    false,
                    format!("4x term maxed. Extra levels give +{per_level}/lv only"),
                )
            }
            (0, 0) if self.waters_owned() >= WATERS => (
                "Max water",
                false,
                format!("{}/{WATERS} waters", self.waters_owned()),
            ),
            _ => return None,
        };
        Some(CapHint {
            label,
            capped,
            detail,
        })
    }

    /// The cost of each of the next `steps + 1` levels of one upgrade, starting
    /// at the saved level, as the cost curve plots it.
    pub fn cost_curve(&self, water: usize, upgrade: usize, steps: usize) -> Vec<f64> {
        let level = self.upgrade_level(water, upgrade);
        (0..=steps)
            .map(|step| Self::upgrade_cost(water, upgrade, level + step as f64))
            .collect()
    }

    /// `opalCost`: the opals the next fountain level costs.
    pub fn opal_cost(level: f64) -> f64 {
        100.0 * (1.0 + level).powf(1.5) * 4.8_f64.powf(level)
    }

    /// The next `count` fountain levels, as the overview's planner lists them.
    pub fn level_plan(&self, count: usize) -> Vec<LevelStep> {
        let level = self.level().max(0.0) as usize;
        let mut cumulative: [f64; CURRENCIES] = [0.0; CURRENCIES];
        (level..level + count)
            .map(|level| {
                let cost = Self::opal_cost(level as f64);
                let currency = level_currency(level as f64);
                cumulative[currency] += cost;
                LevelStep {
                    level,
                    cost,
                    currency,
                    cumulative: cumulative[currency],
                    affordable: self.level_currency_balance(currency) >= cumulative[currency],
                }
            })
            .collect()
    }

    // ----- cross-system bonuses -----

    /// The eighteen fountain upgrades that boost other systems, in upstream order.
    pub fn cross_bonuses(&self) -> Vec<CrossBonus> {
        const BONUSES: [(usize, usize, &str, &str); 18] = [
            (0, 13, "Bravery Monument", "Monumental Boost"),
            (1, 13, "Justice Monument", "Judicial Boost"),
            (2, 13, "Wisdom Monument", "Wisdom Boost"),
            (0, 14, "Cosmo EXP", "Cosmological Boost"),
            (0, 15, "Golden Bucket", "Golden Bucket Boost"),
            (0, 16, "Class EXP", "Class EXP Boost"),
            (0, 17, "Sailing Treasure", "Treasure Boost"),
            (0, 18, "Orion Feathers", "Orion Boost"),
            (1, 16, "All Damage", "Swingy O' Sword"),
            (1, 14, "Bolaia Study", "Skim Reading"),
            (1, 15, "Harp EXP", "Fine Tuning"),
            (1, 17, "Summoning Essence", "Summoneer"),
            (1, 18, "Poppy Fish", "Better Poppy"),
            (2, 14, "Minau's Costs", "Minau's Costs"),
            (2, 15, "Jar Enchant", "Enchanterest"),
            (2, 16, "Research EXP", "Pen N Paper"),
            (2, 17, "Cooking Mastery", "Cook Maxxing"),
            (2, 18, "Bubba Meat", "Bubba Forever"),
        ];
        BONUSES
            .iter()
            .map(|(water, upgrade, label, source)| CrossBonus {
                label,
                percent: self.bon_tot(*water, *upgrade),
                source,
            })
            .collect()
    }

    /// The currency-value breakdown the shared formula tree renders.
    pub fn multi_tree(&self) -> FormulaNode {
        let total = self.currency_all_multi();
        let external = self.external_terms();
        let fountain = FormulaNode::new("Fountain upgrades", self.currency_fountain_multi())
            .with_format(ValueFormat::Multiplier)
            .with_input("Water unlocks × currency boosters")
            .with_source("fountain.js · currencyFountainMulti")
            .with_children([
                FormulaNode::leaf(
                    "Blue Water",
                    1.0 + self.bon_tot(0, 0) / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_source(format!("Yellow Water Lv {}", self.upgrade_level(0, 0))),
                FormulaNode::leaf(
                    "Currency boosters",
                    1.0 + self.boosters() / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_input("Booster I + II + III + Turn and Push"),
                FormulaNode::leaf(
                    "Yellow Water",
                    1.0 + self.bon_tot(1, 0) / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_source(format!("Green Water Lv {}", self.upgrade_level(1, 0))),
                FormulaNode::leaf(
                    "Green Water",
                    1.0 + self.bon_tot(2, 0) / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_source(format!("Red Water Lv {}", self.upgrade_level(2, 0))),
            ]);
        let external = external.into_iter().map(|(label, value, input)| {
            FormulaNode::leaf(label, value, ValueFormat::Multiplier).with_input(input)
        });
        FormulaNode::new("Currency value", total)
            .with_format(ValueFormat::Multiplier)
            .with_input("Fountain bonuses × external bonuses")
            .with_multiplier(total)
            .with_source("fountain.js · currencyAllMulti")
            .with_note("What every coin is worth before its own currency and stack bonuses.")
            .with_children(std::iter::once(fountain).chain(external))
    }
}

/// A number as JS `String` prints it: whole numbers lose the fraction.
fn plain(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}
