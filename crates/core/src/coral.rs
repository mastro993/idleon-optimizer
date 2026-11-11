//! Coral Kid upgrades and the God Rank offering planner's inputs and state.
//!
//! Ported from `js/stats/systems/w5/coral-god-rank.js` in
//! <https://github.com/Corgan/idleon-research-optimizer>. This module owns the
//! formulas and the planner inputs; the app renders them and the beam search in
//! a later feature issue consumes them.

/// Total Divinity LV across all players required per Coral Kid upgrade,
/// `CORAL_KID_UNLOCK_LEVELS` upstream. The first upgrade of each page is always
/// available.
pub const CORAL_KID_UNLOCK_LEVELS: [f64; 6] = [0.0, 600.0, 900.0, 1200.0, 1500.0, 1750.0];

/// Minehead floor-reward bonus at slot 9, from `Research[20][9]` upstream.
const MINEHEAD_BONUS_QTY_9: f64 = 80.0;

/// The planner objective, `GOALS` upstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    #[default]
    Balanced,
    DivinityPoints,
    DivinityExp,
    ClassExp,
    MinorLinks,
    BlessingCap,
    DailyReefCoral,
}

impl Goal {
    /// Every goal in the upstream select's order.
    pub const ALL: [Goal; 7] = [
        Goal::Balanced,
        Goal::DivinityPoints,
        Goal::DivinityExp,
        Goal::ClassExp,
        Goal::MinorLinks,
        Goal::BlessingCap,
        Goal::DailyReefCoral,
    ];

    /// The value the select reports and the planner receives.
    pub fn id(self) -> &'static str {
        match self {
            Goal::Balanced => "balanced",
            Goal::DivinityPoints => "divinity-points",
            Goal::DivinityExp => "divinity-exp",
            Goal::ClassExp => "class-exp",
            Goal::MinorLinks => "minor-links",
            Goal::BlessingCap => "blessing-cap",
            Goal::DailyReefCoral => "daily-reef-coral",
        }
    }

    /// The row the select shows.
    pub fn label(self) -> &'static str {
        match self {
            Goal::Balanced => "Balanced",
            Goal::DivinityPoints => "Divinity Points",
            Goal::DivinityExp => "Divinity EXP",
            Goal::ClassExp => "Class EXP",
            Goal::MinorLinks => "Minor Links",
            Goal::BlessingCap => "Blessing Cap",
            Goal::DailyReefCoral => "Daily Reef Coral",
        }
    }

    /// Parse a select value; unknown values fall back to `Balanced`, as upstream.
    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|goal| goal.id() == id)
            .unwrap_or_default()
    }
}

/// The save fields the planner reads, before the user edits them.
///
/// `None` means the save did not carry that value; [`Inputs::from_save`] then
/// uses the upstream fallback and the section warns about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveInputs {
    pub budget: Option<f64>,
    pub god_rank: Option<f64>,
    pub tier: Option<f64>,
    pub levels: [Option<u32>; 6],
    pub singularity_levels: [Option<u32>; 6],
    pub reef_upgrade_count: Option<f64>,
    pub mine_floor: Option<f64>,
    pub vault76_bonus: Option<f64>,
    pub y4: Option<f64>,
    pub div_cost_after3: Option<f64>,
    pub total_divinity_levels: Option<u32>,
    /// Save fields that were absent or could not be derived, named for the UI.
    pub missing: Vec<String>,
}

/// Every upstream planner input plus the save's current Coral Kid level.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Inputs {
    pub goal: Goal,
    pub budget: f64,
    pub max_levels: u32,
    pub beam_size: u32,
    pub god_rank: u32,
    pub y4: f64,
    pub div_cost_after3: f64,
    pub reef_upgrade_count: f64,
    pub mine_floor: f64,
    pub vault76_bonus: f64,
    pub tier: u32,
    pub planning_confidence: f64,
    pub assume_unlocked: bool,
    /// Current Coral Kid levels 0-5, from `OptLacc[427+i]`.
    pub levels: [u32; 6],
    /// Current Singularity upgrade levels 6-11, from `OptLacc[616+i]`.
    pub singularity_levels: [u32; 6],
    /// Total Divinity LV across every character (`Lv0_<n>[14]`), when known.
    pub total_divinity_levels: Option<u32>,
    pub blessing_base: f64,
}

impl Default for Inputs {
    fn default() -> Self {
        Self::from_save(&SaveInputs::default())
    }
}

impl Inputs {
    /// The upstream `getDefaultInputs` values, taking what the save carries.
    pub fn from_save(save: &SaveInputs) -> Self {
        Self {
            goal: Goal::Balanced,
            budget: save.budget.unwrap_or(0.0),
            max_levels: 25,
            beam_size: 100,
            god_rank: save.god_rank.unwrap_or(0.0).max(0.0).round() as u32,
            y4: save.y4.unwrap_or(0.0),
            div_cost_after3: save.div_cost_after3.unwrap_or(0.0),
            reef_upgrade_count: save.reef_upgrade_count.unwrap_or(0.0),
            mine_floor: save.mine_floor.unwrap_or(0.0),
            vault76_bonus: save.vault76_bonus.unwrap_or(0.0),
            tier: save.tier.unwrap_or(0.0).max(0.0).round() as u32,
            planning_confidence: 0.9,
            assume_unlocked: true,
            levels: std::array::from_fn(|index| save.levels[index].unwrap_or(0)),
            singularity_levels: std::array::from_fn(|index| {
                save.singularity_levels[index].unwrap_or(0)
            }),
            total_divinity_levels: save.total_divinity_levels,
            blessing_base: 100.0,
        }
    }

    /// Whether an upgrade may be planned. `Assume unlocked` wins; otherwise the
    /// save's total Divinity levels decide, and an unknown total locks it.
    pub fn upgrade_unlocked(&self, index: usize) -> bool {
        if self.assume_unlocked {
            return true;
        }
        let Some(required) = CORAL_KID_UNLOCK_LEVELS.get(index) else {
            return false;
        };
        match self.total_divinity_levels {
            Some(total) => f64::from(total) >= *required,
            None => false,
        }
    }

    /// How many of the six upgrades are unlocked under the current inputs.
    pub fn unlocked_count(&self) -> usize {
        (0..6).filter(|index| self.upgrade_unlocked(*index)).count()
    }

    /// The current coral multipliers and account bonuses.
    pub fn metrics(&self) -> Metrics {
        let levels: [f64; 6] = std::array::from_fn(|index| f64::from(self.levels[index]));
        let singularity: [f64; 6] =
            std::array::from_fn(|index| f64::from(self.singularity_levels[index]));
        metrics(
            &levels,
            &singularity,
            f64::from(self.god_rank),
            self.reef_upgrade_count,
            self.mine_floor,
            self.vault76_bonus,
            self.blessing_base,
        )
    }

    /// One row per Coral Kid upgrade: its level, current effect and next cost.
    pub fn level_rows(&self) -> [LevelRow; 6] {
        std::array::from_fn(|index| {
            let level = self.levels[index];
            LevelRow {
                index,
                per_level: upgrade_effect(index, 1.0),
                level,
                effect: upgrade_effect(index, f64::from(level)),
                next_cost: upgrade_cost(index, f64::from(level)),
            }
        })
    }

    /// Whether the planner inputs can produce a plan.
    pub fn validation(&self) -> Validation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        non_negative("Budget", self.budget, &mut errors);
        non_negative("Y4", self.y4, &mut errors);
        non_negative("DivCostAfter3", self.div_cost_after3, &mut errors);
        non_negative("Reef upgrades", self.reef_upgrade_count, &mut errors);
        non_negative("Minehead floor", self.mine_floor, &mut errors);
        non_negative("Vault 76 bonus", self.vault76_bonus, &mut errors);
        if !self.planning_confidence.is_finite() {
            errors.push("Confidence must be a number between 0.1 and 0.99.".into());
        } else if !(0.1..=0.99).contains(&self.planning_confidence) {
            warnings.push("Confidence outside 0.1-0.99 is clamped by the planner.".into());
        }
        if self.beam_size == 0 {
            errors.push("Beam size must be at least 1.".into());
        }
        if self.max_levels == 0 {
            warnings.push("Max levels is 0, so the plan can only hold the current state.".into());
        }
        if self.tier > 5 {
            warnings.push("Offer tier above 5 is capped by the offering odds table.".into());
        }
        if self.y4 > 100.0 {
            warnings.push("Y4 above 100% is capped at a 35% free offering chance.".into());
        }
        if self.god_rank > 100 {
            warnings.push("God rank above 100 stops the offering planner at 100.".into());
        }
        if !self.assume_unlocked && self.total_divinity_levels.is_none() {
            errors.push(
                "Coral Kid unlock state is unknown; tick Assume unlocked or import a save with character Divinity levels.".into(),
            );
        } else if self.unlocked_count() == 0 {
            errors.push(
                "No Coral Kid upgrade is unlocked; tick Assume unlocked or raise total Divinity levels.".into(),
            );
        } else if let Some(cheapest) = self.cheapest_unlocked_upgrade()
            && self.budget < cheapest
        {
            warnings.push(format!(
                "Budget buys no unlocked upgrade; the cheapest costs {cheapest:.0}."
            ));
        }

        Validation { errors, warnings }
    }

    /// The cheapest next level among the unlocked upgrades.
    fn cheapest_unlocked_upgrade(&self) -> Option<f64> {
        (0..6)
            .filter(|index| self.upgrade_unlocked(*index))
            .map(|index| upgrade_cost(index, f64::from(self.levels[index])))
            .filter(|cost| cost.is_finite())
            .min_by(f64::total_cmp)
    }
}

/// Planner input problems: `errors` block a plan, `warnings` do not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Validation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Validation {
    /// Whether a plan may run.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

fn non_negative(name: &str, value: f64, errors: &mut Vec<String>) {
    if !value.is_finite() || value < 0.0 {
        errors.push(format!("{name} must be a finite, non-negative number."));
    }
}

/// One Coral Kid upgrade row, as the current-levels table shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRow {
    pub index: usize,
    pub per_level: f64,
    pub level: u32,
    pub effect: f64,
    pub next_cost: f64,
}

/// The coral state `coralKidMetrics` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub divinity_exp: f64,
    pub class_exp: f64,
    pub minor_links: f64,
    pub divinity_points: f64,
    pub blessing_cap: f64,
    pub daily_reef_coral: f64,
}

/// `coralKidLevelSlot`: upgrades 0-5 use `OLA[427+i]`, 6-11 use `OLA[616+(i-6)]`.
pub fn upgrade_slot(index: usize) -> usize {
    if index >= 6 {
        616 + index - 6
    } else {
        427 + index
    }
}

/// `coralKidUpgradeCost`: Divinity points for 0-5, Singularity for 6-11.
pub fn upgrade_cost(index: usize, level: f64) -> f64 {
    let lv = level.max(0.0);
    let index = index as f64;
    if index == 1.0 {
        return 1e7 * 6f64.powf(index) * 1.25f64.powf(lv);
    }
    if index < 6.0 {
        return 1e7 * 6f64.powf(index) * 1.1f64.powf(lv);
    }
    if index == 10.0 || index == 11.0 {
        return (5.0 + 2.0 * (index - 6.0)).powf(index - 6.0) * 1.85f64.powf(lv);
    }
    (7.0 + 2.0 * (index - 6.0)).powf(index - 6.0) * 1.13f64.powf(lv)
}

/// `coralKidUpgradeEffect`: the bonus the upgrade grants at `level`.
pub fn upgrade_effect(index: usize, level: f64) -> f64 {
    let lv = level.max(0.0);
    match index {
        0 => 10.0 * lv,
        1 => (2.0 * lv).round(),
        2 => lv / (25.0 + lv) * 20.0,
        3 => lv.round(),
        4 => (2.0 * lv).round(),
        5 => lv / (40.0 + lv) * 100.0,
        6 => (100.0 * lv).round(),
        7 => (10.0 * lv).round(),
        8 => (50.0 * lv).round(),
        9 => (125.0 * lv / (lv + 100.0)).round(),
        10 => (5.0 * lv).round(),
        11 => lv.round(),
        _ => 0.0,
    }
}

/// `MINEHEAD_BONUS_QTY(9, floor)`: the blessing-cap bonus from the Minehead.
fn minehead_bonus(mine_floor: f64) -> f64 {
    if mine_floor > 9.0 {
        MINEHEAD_BONUS_QTY_9
    } else {
        0.0
    }
}

/// `GIFTS_ABOUND` (Y4) at a bubble level: `AlchemyDescription[3][23]` uses the
/// decay formula with x1 = 40 and x2 = 60.
pub fn y4_base(bubble_level: f64) -> f64 {
    let lv = bubble_level.max(0.0);
    40.0 * lv / (lv + 60.0)
}

/// `coralKidMetrics`: the current/projected coral multipliers.
pub fn metrics(
    levels: &[f64; 6],
    singularity_levels: &[f64; 6],
    god_rank: f64,
    reef_upgrade_count: f64,
    mine_floor: f64,
    vault76_bonus: f64,
    blessing_base: f64,
) -> Metrics {
    let minor = upgrade_effect(3, levels[3]);
    let points_bonus = upgrade_effect(4, levels[4]);
    let class_bonus = upgrade_effect(2, levels[2]);
    let singularity_points = 1.0 + upgrade_effect(6, singularity_levels[0]) / 100.0;
    let singularity_exp = 1.0 + upgrade_effect(8, singularity_levels[2]) / 100.0;
    Metrics {
        divinity_exp: (1.0 + upgrade_effect(0, levels[0]) / 100.0) * singularity_exp,
        class_exp: (1.0 + class_bonus / 100.0).powf((god_rank - 10.0).max(0.0)),
        minor_links: 1.0 + minor / 100.0,
        divinity_points: (1.0 + reef_upgrade_count * points_bonus / 100.0) * singularity_points,
        blessing_cap: (blessing_base
            + upgrade_effect(1, levels[1])
            + minehead_bonus(mine_floor)
            + vault76_bonus)
            .round(),
        daily_reef_coral: 1.0 + upgrade_effect(5, levels[5]) / 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_inputs() -> Inputs {
        Inputs {
            budget: 2.909_587_276_919_98e18,
            god_rank: 31,
            reef_upgrade_count: 53.0,
            mine_floor: 23.0,
            vault76_bonus: 40.0,
            levels: [154, 64, 124, 98, 100, 90],
            singularity_levels: [49, 31, 0, 0, 0, 0],
            ..Inputs::default()
        }
    }

    #[test]
    fn upgrade_values_match_upstream_at_the_fixture_levels() {
        let expected = [
            (1540.0, 23685006833599.492),
            (128.0, 95585514667947.14),
            (16.644_295_302_013_422, 48864725156227.086),
            (98.0, 24600101366955.652),
            (200.0, 178596735924098.06),
            (69.230_769_230_769_23, 413140638297324.75),
        ];
        for (index, (effect, cost)) in expected.into_iter().enumerate() {
            let level = f64::from(fixture_inputs().levels[index]);
            assert!((upgrade_effect(index, level) - effect).abs() < 1e-9);
            assert!((upgrade_cost(index, level) - cost).abs() < 1.0);
        }
    }

    #[test]
    fn metrics_match_upstream_for_the_fixture_levels() {
        let metrics = fixture_inputs().metrics();
        assert_eq!(metrics.divinity_exp, 16.4);
        assert_eq!(metrics.divinity_points, 5350.0);
        assert_eq!(metrics.blessing_cap, 348.0);
        assert_eq!(metrics.minor_links, 1.98);
        assert!((metrics.class_exp - 25.359_084_901_759_285).abs() < 1e-12);
        assert!((metrics.daily_reef_coral - 1.692_307_692_307_692).abs() < 1e-12);
    }

    #[test]
    fn the_singularity_upgrades_scale_points_and_exp() {
        let inputs = Inputs {
            singularity_levels: [0, 0, 0, 0, 0, 0],
            ..fixture_inputs()
        };
        let metrics = inputs.metrics();
        assert_eq!(metrics.divinity_points, 107.0);
        assert_eq!(metrics.divinity_exp, 16.4);
    }

    #[test]
    fn slots_follow_the_upgrade_page() {
        assert_eq!(upgrade_slot(0), 427);
        assert_eq!(upgrade_slot(5), 432);
        assert_eq!(upgrade_slot(6), 616);
        assert_eq!(upgrade_slot(11), 621);
    }

    #[test]
    fn unlock_levels_decide_when_the_save_knows_them() {
        let inputs = Inputs {
            assume_unlocked: false,
            total_divinity_levels: Some(1600),
            ..Inputs::default()
        };
        assert_eq!(inputs.unlocked_count(), 5);
        assert!(!inputs.upgrade_unlocked(5));
        assert!(inputs.upgrade_unlocked(4));

        let all = Inputs {
            total_divinity_levels: Some(4727),
            ..inputs
        };
        assert_eq!(all.unlocked_count(), 6);
    }

    #[test]
    fn assume_unlocked_wins_over_the_save() {
        let inputs = Inputs {
            assume_unlocked: true,
            total_divinity_levels: Some(0),
            ..Inputs::default()
        };
        assert_eq!(inputs.unlocked_count(), 6);
    }

    #[test]
    fn validation_rejects_impossible_inputs() {
        let inputs = Inputs {
            budget: -1.0,
            beam_size: 0,
            assume_unlocked: false,
            total_divinity_levels: None,
            ..Inputs::default()
        };
        let validation = inputs.validation();
        assert!(!validation.ok());
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("Budget"))
        );
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("Beam size"))
        );
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("unlock state"))
        );
    }

    #[test]
    fn validation_accepts_the_fixture_defaults() {
        let validation = fixture_inputs().validation();
        assert!(validation.ok(), "{validation:?}");
    }

    #[test]
    fn a_budget_below_the_cheapest_upgrade_is_a_warning() {
        let inputs = Inputs {
            budget: 1.0,
            ..fixture_inputs()
        };
        let validation = inputs.validation();
        assert!(validation.ok());
        assert!(
            validation
                .warnings
                .iter()
                .any(|warning| warning.contains("Budget buys no"))
        );
    }
}
