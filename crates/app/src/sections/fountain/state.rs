//! The Fountain section's visible state: its tab, the water the upgrade table
//! lists, the simulated desired currency and the upgrade the cost curve plots.
//!
//! The save supplies every number; these are only the section's selections, so
//! they live in the project document like every other section's.

use idleon_fountain::Fountain;
use idleon_fountain::{self as fountain, Goal, PlanRequest, WATERS};
use serde::{Deserialize, Serialize};

/// The fountain tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// Summary, income, bonuses, stacks and the level plan.
    #[default]
    Overview,
    /// Currency values, per-water multipliers and the desired simulator.
    Currency,
    /// Every upgrade, the water switch and the cost curve.
    Upgrades,
    /// The upgrade planner and the greedy optimizer.
    Planner,
}

impl Tab {
    pub(crate) const ALL: [Tab; 4] = [Tab::Overview, Tab::Currency, Tab::Upgrades, Tab::Planner];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Currency => "currency",
            Tab::Upgrades => "upgrades",
            Tab::Planner => "planner",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Currency => "Currency",
            Tab::Upgrades => "Upgrades",
            Tab::Planner => "Planner",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The optimizer's step caps, as the page's selector lists them.
pub(crate) const STEP_CHOICES: [usize; 5] = [10, 25, 50, 100, 250];

/// The section's selections, kept across tab switches and dashboard visits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The water the upgrade table lists.
    pub(crate) water: usize,
    /// The desired currency the currency tab simulates; `None` reads the save's.
    pub(crate) desired: Option<usize>,
    /// The upgrade the cost curve plots.
    pub(crate) upgrade: Option<usize>,
    /// The planner's goal.
    pub(crate) planner_goal: Goal,
    /// The desired currency the `optimal` planner goal simulates.
    pub(crate) planner_desired: Option<usize>,
    /// Only rank upgrades the saved balance can pay for.
    pub(crate) planner_affordable: bool,
    /// Divide the cost by the currency's value before ranking.
    pub(crate) planner_normalize: bool,
    /// Rank by the hours needed to earn the cost.
    pub(crate) planner_per_hr: bool,
    /// The optimizer's goal.
    pub(crate) optimizer_goal: Goal,
    /// The optimizer's step cap.
    pub(crate) optimizer_steps: usize,
    /// Only buy steps the saved balance can pay for.
    pub(crate) optimizer_affordable: bool,
    /// Divide the cost by the currency's value before ranking.
    pub(crate) optimizer_normalize: bool,
    /// Rank by the hours needed to earn the cost.
    pub(crate) optimizer_per_hr: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            water: 0,
            desired: None,
            upgrade: None,
            planner_goal: Goal::Currency,
            planner_desired: None,
            planner_affordable: false,
            planner_normalize: false,
            planner_per_hr: false,
            optimizer_goal: Goal::Currency,
            optimizer_steps: 50,
            optimizer_affordable: true,
            optimizer_normalize: false,
            optimizer_per_hr: false,
        }
    }
}

impl State {
    /// Keep the selections the current cavern can still resolve.
    pub(crate) fn adopt(&mut self, fountain: Option<&Fountain>) {
        let Some(fountain) = fountain else {
            return;
        };
        self.water = self.water.min(WATERS - 1);
        self.desired = self.desired.filter(|currency| {
            *currency < fountain::CURRENCIES && fountain.currency_unlocked(*currency)
        });
        self.upgrade = self
            .upgrade
            .filter(|upgrade| fountain::upgrade(self.water, *upgrade).is_some());
        self.planner_desired = self.planner_desired.filter(|currency| {
            *currency < fountain::CURRENCIES && fountain.currency_unlocked(*currency)
        });
        if !STEP_CHOICES.contains(&self.optimizer_steps) {
            self.optimizer_steps = 50;
        }
    }

    /// The planner's request: its own goal, desired currency and ranking.
    pub(crate) fn planner_request(&self) -> PlanRequest {
        PlanRequest {
            goal: self.planner_goal,
            desired: self.planner_desired,
            max_steps: 1,
            affordable: self.planner_affordable,
            normalize: self.planner_normalize,
            per_hr: self.planner_per_hr,
        }
    }

    /// The optimizer's request, including its step cap.
    pub(crate) fn plan_request(&self) -> PlanRequest {
        PlanRequest {
            goal: self.optimizer_goal,
            desired: None,
            max_steps: self.optimizer_steps,
            affordable: self.optimizer_affordable,
            normalize: self.optimizer_normalize,
            per_hr: self.optimizer_per_hr,
        }
    }
}
