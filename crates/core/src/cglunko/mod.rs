//! Cglunko (The Cove, W5): the overview metrics and the drop-rate breakdown.
//!
//! Ports the Overview tab of `cglunko-calc.html` and
//! `js/stats/systems/w5/cglunko.js`. Save fields: `OptionsListAccount` (or
//! `OptLacc`), where 630..=653 are the 24 upgrade levels over two pages,
//! 654..=659 the blue shape counts, 660..=665 the purple ones, 668 the Glunko
//! kills and 200 the account's highest drop rate the formula reads.
//!
//! The upgrade list, the optimizer and the stone focus are the later CL-02 and
//! CL-03 features; this module carries what the overview and its formula tree
//! need. `Option`-less decoding matches the upstream page: a missing `OptLacc`
//! row reads as zero and is reported once by [`Cglunko::from_save`].

mod data;

use crate::formula::{FormulaNode, ValueFormat};
use crate::save::{game_log, json};
use serde_json::{Map, Value};

pub use data::name as upgrade_name;

/// The upgrades across both pages.
pub const UPGRADE_COUNT: usize = 24;
/// The stones on one page.
pub const STONE_COUNT: usize = 6;

/// `OptLacc[630]`: upgrade level 0.
const UPGRADE_BASE: usize = 630;
/// `OptLacc[654]`: blue shape count 0.
const BLUE_BASE: usize = 654;
/// `OptLacc[660]`: purple shape count 0.
const PURPLE_BASE: usize = 660;
/// `OptLacc[668]`: Glunko kills, the `POW 10` count of upgrade 4.
const KILLS: usize = 668;
/// `OptLacc[200]`: the account's highest drop rate, the `POW 10` count of 18.
const NORMAL_DR: usize = 200;

/// The Cglunko state a save carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cglunko {
    /// The 24 upgrade levels, `OptLacc[630..654]`.
    pub upgrades: [u32; UPGRADE_COUNT],
    /// The six blue shape counts, `OptLacc[654..660]`.
    pub blue_stones: [f64; STONE_COUNT],
    /// The six purple shape counts, `OptLacc[660..666]`.
    pub purple_stones: [f64; STONE_COUNT],
    /// The Glunko kill count, `OptLacc[668]`.
    pub kills: f64,
    /// The normal drop rate the digit bonus reads, `OptLacc[200]`.
    pub normal_dr: f64,
    /// Whether the save carried any of the Cglunko rows at all.
    pub saved: bool,
}

/// The seven factors of [`Cglunko::dr`], one per upstream breakdown node.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DrFactors {
    /// `1 + (upgrade 1 + 17 + 21) / 100`.
    pub additive_base: f64,
    /// `1 + upgrade 3 / 100`.
    pub droplier: f64,
    /// `1 + upgrade 12 / 100`.
    pub droplier_plus: f64,
    /// `1 + upgrade 10 × blue digits / 100`.
    pub blue_digits: f64,
    /// `1 + upgrade 22 × purple digits / 100`.
    pub purple_digits: f64,
    /// `1 + upgrade 4 × Glunko kill digits / 100`.
    pub kills: f64,
    /// `1 + upgrade 18 × normal DR digits / 100`.
    pub normal_dr: f64,
}

impl DrFactors {
    /// The drop-rate multiplier the factors multiply to.
    fn product(&self) -> f64 {
        self.additive_base
            * self.droplier
            * self.droplier_plus
            * self.blue_digits
            * self.purple_digits
            * self.kills
            * self.normal_dr
    }
}

impl Cglunko {
    /// Read the Cglunko slice of a decoded save.
    ///
    /// A save without `OptLacc` keeps the zero state and says so once, the way
    /// the upstream page leaves a fresh account at zero.
    pub fn from_save(save: &Map<String, Value>, warnings: &mut Vec<String>) -> Self {
        let Some(options) = ["OptionsListAccount", "OptLacc"]
            .into_iter()
            .find_map(|key| json::parsed_field(save, key).filter(Value::is_array))
        else {
            warnings.push("OptLacc: not in the save; Cglunko values kept at zero".into());
            return Self::default();
        };
        let number = |index: usize| json::index(&options, &[index]).and_then(json::number);
        let level = |index: usize| {
            number(index)
                .map(|level| level.max(0.0).round() as u32)
                .unwrap_or(0)
        };
        // A saved account has the row; an account that never reached W5 does not.
        let saved = (UPGRADE_BASE..=KILLS).any(|index| number(index).is_some());
        if !saved {
            warnings.push(
                "OptLacc[630..669]: not in the save; this account has no Cglunko upgrades, stones or kills"
                    .into(),
            );
        }
        Self {
            upgrades: std::array::from_fn(|index| level(UPGRADE_BASE + index)),
            blue_stones: std::array::from_fn(|index| number(BLUE_BASE + index).unwrap_or(0.0)),
            purple_stones: std::array::from_fn(|index| number(PURPLE_BASE + index).unwrap_or(0.0)),
            kills: number(KILLS).unwrap_or(0.0),
            normal_dr: number(NORMAL_DR).unwrap_or(0.0),
            saved,
        }
    }

    /// One upgrade's saved level.
    pub fn level(&self, index: usize) -> u32 {
        self.upgrades.get(index).copied().unwrap_or(0)
    }

    /// `Cglunko_upgBon(i)`: the saved level times the game's per-level value.
    pub fn bonus(&self, index: usize) -> f64 {
        f64::from(self.level(index)) * data::per_lv(index)
    }

    /// `Bdig`: the sum of `ceil(log10)` over the six blue shape counts.
    pub fn blue_digits(&self) -> u32 {
        self.blue_stones.iter().map(|stone| digits(*stone)).sum()
    }

    /// `Pdig`: the sum of `ceil(log10)` over the six purple shape counts.
    pub fn purple_digits(&self) -> u32 {
        self.purple_stones.iter().map(|stone| digits(*stone)).sum()
    }

    /// The `POW 10` count of the Glunko kills.
    pub fn kill_digits(&self) -> u32 {
        digits(self.kills)
    }

    /// The `POW 10` count of the normal drop rate.
    pub fn normal_dr_digits(&self) -> u32 {
        digits(self.normal_dr)
    }

    /// The seven drop-rate factors in upstream display order.
    fn factors(&self) -> DrFactors {
        DrFactors {
            additive_base: 1.0 + (self.bonus(1) + self.bonus(17) + self.bonus(21)) / 100.0,
            droplier: 1.0 + self.bonus(3) / 100.0,
            droplier_plus: 1.0 + self.bonus(12) / 100.0,
            blue_digits: 1.0 + self.bonus(10) * f64::from(self.blue_digits()) / 100.0,
            purple_digits: 1.0 + self.bonus(22) * f64::from(self.purple_digits()) / 100.0,
            kills: 1.0 + self.bonus(4) * f64::from(self.kill_digits()) / 100.0,
            normal_dr: 1.0 + self.bonus(18) * f64::from(self.normal_dr_digits()) / 100.0,
        }
    }

    /// `Cglunko_DR`: the Cove's drop-rate multiplier.
    pub fn dr(&self) -> f64 {
        self.factors().product()
    }

    /// `Cglunko_AFKgains`: `(10 + upgrades 8 + 13) / 100`, as a fraction.
    pub fn afk_gains(&self) -> f64 {
        (10.0 + self.bonus(8) + self.bonus(13)) / 100.0
    }

    /// `Cglunko_MKtier`: the multikill per damage tier.
    pub fn mk_tier(&self) -> f64 {
        self.bonus(6)
    }

    /// `Cglunko_MKbase`: the base multikill percentage.
    pub fn mk_base(&self) -> f64 {
        self.bonus(15)
    }

    /// `Cglunko_Respawn`: the crystal respawn rate percentage.
    pub fn respawn(&self) -> f64 {
        self.bonus(2)
    }

    /// The cavern resource multiplier percentage.
    pub fn cavern_currency(&self) -> f64 {
        self.bonus(14)
    }

    /// The research EXP multiplier percentage.
    pub fn research_exp(&self) -> f64 {
        self.bonus(11)
    }

    /// The total damage multiplier percentage.
    pub fn total_dmg(&self) -> f64 {
        self.bonus(19)
    }

    /// The Grand Discovery chance percentage.
    pub fn grand_discovery(&self) -> f64 {
        self.bonus(20)
    }

    /// The drop-rate formula tree: the seven factors under their product.
    pub fn breakdown(&self) -> FormulaNode {
        let factors = self.factors();
        let additive = FormulaNode::group(
            "Additive DR (+%)",
            ValueFormat::Percent,
            [self.dr_upgrade(1), self.dr_upgrade(17), self.dr_upgrade(21)],
        )
        .with_note("Slop Drop, Slop Drop+ and Slop Drop++ add together");
        let additive_base = FormulaNode::leaf(
            "Base + Additive DR",
            factors.additive_base,
            ValueFormat::Multiplier,
        )
        .with_input("1 + additive% / 100")
        .with_children([additive]);
        let droplier = FormulaNode::leaf(data::name(3), factors.droplier, ValueFormat::Multiplier)
            .with_input("1 + bonus% / 100")
            .with_note(format!(
                "+{} at Lv {}",
                ValueFormat::Raw.format(self.bonus(3)),
                self.level(3)
            ));
        let droplier_plus = FormulaNode::leaf(
            data::name(12),
            factors.droplier_plus,
            ValueFormat::Multiplier,
        )
        .with_input("1 + bonus% / 100")
        .with_note(format!(
            "+{} at Lv {}",
            ValueFormat::Raw.format(self.bonus(12)),
            self.level(12)
        ));
        let blue = FormulaNode::leaf(
            format!("{} × Blue-shape Digits", data::name(10)),
            factors.blue_digits,
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{}% × {} digits",
            self.bonus(10),
            self.blue_digits()
        ))
        .with_note("Blue shape counts, each ceil(log10)");
        let purple = FormulaNode::leaf(
            format!("{} × Purple-shape Digits", data::name(22)),
            factors.purple_digits,
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{}% × {} digits",
            self.bonus(22),
            self.purple_digits()
        ))
        .with_note("Purple shape counts, each ceil(log10)");
        let kills = FormulaNode::leaf(
            format!("{} × Glunko Kill Digits", data::name(4)),
            factors.kills,
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{}% × {} digits",
            self.bonus(4),
            self.kill_digits()
        ))
        .with_note(format!(
            "{} Glunko kills",
            ValueFormat::Raw.format(self.kills)
        ));
        let normal_dr = FormulaNode::leaf(
            format!("{} × Normal DR Digits", data::name(18)),
            factors.normal_dr,
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{}% × {} digits",
            self.bonus(18),
            self.normal_dr_digits()
        ))
        .with_note(format!(
            "{} normal DR",
            ValueFormat::Raw.format(self.normal_dr)
        ));
        FormulaNode::new("Cglunko DR", factors.product())
            .with_format(ValueFormat::Multiplier)
            .with_input("Product of the seven factors")
            .with_source("cglunko.js · drBonusAt")
            .with_note("Multiplies out to the Cove's drop rate.")
            .with_children([
                additive_base,
                droplier,
                droplier_plus,
                blue,
                purple,
                kills,
                normal_dr,
            ])
    }

    /// One additive drop-rate upgrade as a percentage leaf.
    fn dr_upgrade(&self, index: usize) -> FormulaNode {
        FormulaNode::leaf(data::name(index), self.bonus(index), ValueFormat::Percent)
            .with_input(format!(
                "Lv {} × {}%",
                self.level(index),
                data::per_lv(index)
            ))
            .with_source(format!("OptLacc[{}]", UPGRADE_BASE + index))
    }
}

/// `ceil(getLOG(value))`, the digit a stone or drop rate contributes.
fn digits(value: f64) -> u32 {
    game_log(value).ceil().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// An account with every drop-rate upgrade and a mixed set of stones.
    fn sample() -> Cglunko {
        let mut upgrades = [0_u32; UPGRADE_COUNT];
        upgrades[1] = 2; // Slop Drop, 15 per level.
        upgrades[3] = 10; // Droplier, 8 per level.
        upgrades[4] = 1; // Killionaire, 3 per level.
        upgrades[10] = 2; // Blue Collector, 1 per level.
        upgrades[17] = 1; // Slop Drop+, 25 per level.
        upgrades[18] = 1; // Rich Get Rich, 2 per level.
        upgrades[21] = 4; // Slop Drop++, 150 per level.
        upgrades[22] = 1; // Purp Collector, 2 per level.
        Cglunko {
            upgrades,
            blue_stones: [1000.0, 10.0, 0.0, 0.0, 0.0, 0.0],
            purple_stones: [100.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            kills: 1000.0,
            normal_dr: 100.0,
            saved: true,
        }
    }

    #[test]
    fn the_upgrade_bonus_is_level_times_the_per_level_value() {
        let cglunko = sample();
        assert_eq!(cglunko.bonus(1), 30.0);
        assert_eq!(cglunko.bonus(21), 600.0);
        assert_eq!(cglunko.bonus(0), 0.0);
    }

    #[test]
    fn stone_and_drop_rate_digits_are_ceil_log10() {
        let cglunko = sample();
        // 1000 -> 3, 10 -> 1, 0 stays 0.
        assert_eq!(cglunko.blue_digits(), 4);
        assert_eq!(cglunko.purple_digits(), 2);
        assert_eq!(cglunko.kill_digits(), 3);
        assert_eq!(cglunko.normal_dr_digits(), 2);
        assert_eq!(Cglunko::default().blue_digits(), 0);
        assert_eq!(Cglunko::default().kill_digits(), 0);
    }

    #[test]
    fn the_factors_multiply_to_the_drop_rate() {
        let cglunko = sample();
        let factors = cglunko.factors();
        // 1 + (30 + 25 + 600) / 100; 1.8; 1; 1.08; 1.02; 1.09; 1.04.
        assert_eq!(factors.additive_base, 7.55);
        assert_eq!(factors.droplier, 1.8);
        assert_eq!(factors.droplier_plus, 1.0);
        assert_eq!(factors.blue_digits, 1.08);
        assert_eq!(factors.purple_digits, 1.04);
        assert_eq!(factors.kills, 1.09);
        assert_eq!(factors.normal_dr, 1.04);
        assert!((cglunko.dr() - 17.303_596_876_8).abs() < 1e-9);
    }

    #[test]
    fn the_breakdown_tree_multiplies_out_to_the_total() {
        let cglunko = sample();
        let tree = cglunko.breakdown();
        assert_eq!(tree.label, "Cglunko DR");
        assert_eq!(tree.children.len(), 7);
        let product: f64 = tree.children.iter().map(|child| child.value).product();
        assert!((tree.value - product).abs() < 1e-12);
        assert!((tree.value - cglunko.dr()).abs() < 1e-12);
        // The first child is the additive group plus its base.
        let additive = &tree.children[0];
        assert_eq!(additive.label, "Base + Additive DR");
        assert_eq!(additive.children.len(), 1);
        assert!((additive.children[0].value - 655.0).abs() < 1e-12);
        assert!((additive.value - (1.0 + 655.0 / 100.0)).abs() < 1e-12);
        // The additive group sums its three upgrades.
        let group = &additive.children[0];
        assert_eq!(group.label, "Additive DR (+%)");
        let sum: f64 = group.children.iter().map(|child| child.value).sum();
        assert!((group.value - sum).abs() < 1e-12);
        assert_eq!(group.children.len(), 3);
    }

    #[test]
    fn the_overview_bonuses_follow_the_upgrade_table() {
        let mut cglunko = Cglunko {
            saved: true,
            ..Cglunko::default()
        };
        cglunko.upgrades[2] = 5; // Reslime, 2 per level.
        cglunko.upgrades[6] = 3; // Big Hit Multkill, 1 per level.
        cglunko.upgrades[8] = 2; // Sleepy Drops, 1 per level.
        cglunko.upgrades[11] = 4; // Researchy, 1 per level.
        cglunko.upgrades[13] = 1; // Sleepy Drops+, 3 per level.
        cglunko.upgrades[14] = 2; // Cavernicus, 10 per level.
        cglunko.upgrades[15] = 1; // Multikill, 100 per level.
        cglunko.upgrades[19] = 3; // DeEmGe, 1 per level.
        cglunko.upgrades[20] = 2; // Grandioso, 5 per level.
        assert_eq!(cglunko.respawn(), 10.0);
        assert_eq!(cglunko.mk_tier(), 3.0);
        assert_eq!(cglunko.mk_base(), 100.0);
        assert_eq!(cglunko.afk_gains(), 0.15);
        assert_eq!(cglunko.cavern_currency(), 20.0);
        assert_eq!(cglunko.research_exp(), 4.0);
        assert_eq!(cglunko.total_dmg(), 3.0);
        assert_eq!(cglunko.grand_discovery(), 10.0);
    }

    #[test]
    fn a_save_without_optlacc_keeps_zero_and_warns_once() {
        let mut warnings = Vec::new();
        let cglunko = Cglunko::from_save(&Map::new(), &mut warnings);
        assert!(!cglunko.saved);
        assert_eq!(cglunko.dr(), 1.0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("OptLacc: not in the save"));
    }

    #[test]
    fn a_save_without_the_cove_rows_is_warned() {
        let save = json!({ "OptLacc": [0, 0, 1] });
        let mut warnings = Vec::new();
        let cglunko = Cglunko::from_save(save.as_object().unwrap(), &mut warnings);
        assert!(!cglunko.saved);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("OptLacc[630..669]"));
    }

    #[test]
    fn the_cove_rows_decode_from_a_saved_account() {
        // The game writes the option list as a JSON string holding an array.
        let mut options: Vec<f64> = vec![0.0; 669];
        options[630] = 5.0;
        options[631] = 26.0;
        options[654] = 3400.1;
        options[659] = 1.1;
        options[668] = 320_986_464.0;
        options[200] = 5933.67663835006;
        let save = json!({ "OptLacc": serde_json::to_string(&options).unwrap() });
        let mut warnings = Vec::new();
        let cglunko = Cglunko::from_save(save.as_object().unwrap(), &mut warnings);
        assert!(warnings.is_empty());
        assert!(cglunko.saved);
        assert_eq!(cglunko.level(0), 5);
        assert_eq!(cglunko.level(1), 26);
        assert_eq!(cglunko.blue_stones[0], 3400.1);
        assert_eq!(cglunko.blue_stones[5], 1.1);
        assert_eq!(cglunko.kills, 320_986_464.0);
        assert_eq!(cglunko.blue_digits(), 5);
        assert_eq!(cglunko.normal_dr_digits(), 4);
    }
}
