//! The Fountain section's visible state: its tab, the water the upgrade table
//! lists, the simulated desired currency and the upgrade the cost curve plots.
//!
//! The save supplies every number; these are only the section's selections, so
//! they live in the project document like every other section's.

use idleon_fountain::Fountain;
use idleon_fountain::{self as fountain, Goal, MarbleRequest, PlanRequest, SimRequest, WATERS};
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
    /// The marble planner and the greedy marble optimizer.
    Marble,
    /// The simulated run.
    Sim,
}

impl Tab {
    pub(crate) const ALL: [Tab; 6] = [
        Tab::Overview,
        Tab::Currency,
        Tab::Upgrades,
        Tab::Planner,
        Tab::Marble,
        Tab::Sim,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Currency => "currency",
            Tab::Upgrades => "upgrades",
            Tab::Planner => "planner",
            Tab::Marble => "marble",
            Tab::Sim => "sim",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Currency => "Currency",
            Tab::Upgrades => "Upgrades",
            Tab::Planner => "Planner",
            Tab::Marble => "Marble",
            Tab::Sim => "Sim",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The optimizer's step caps, as the page's selector lists them.
pub(crate) const STEP_CHOICES: [usize; 5] = [10, 25, 50, 100, 250];

/// The simulation's time limit bounds, in hours. The page's input has no
/// maximum; the section caps a run so a typo cannot start an endless one.
pub(crate) const SIM_MIN_HOURS: f64 = 1.0;
pub(crate) const SIM_MAX_HOURS: f64 = 720.0;
pub(crate) const SIM_DEFAULT_HOURS: f64 = 24.0;

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
    /// The marble planner's goal.
    pub(crate) marble_goal: Goal,
    /// The desired currency the `optimal` marble goal simulates.
    pub(crate) marble_desired: Option<usize>,
    /// Only rank marble levels the saved balance can pay for.
    pub(crate) marble_affordable: bool,
    /// The marble optimizer's goal.
    pub(crate) marble_optimizer_goal: Goal,
    /// The marble optimizer's step cap.
    pub(crate) marble_optimizer_steps: usize,
    /// Only buy marble levels the saved balance can pay for.
    pub(crate) marble_optimizer_affordable: bool,
    /// The simulated run's time limit, in hours.
    pub(crate) sim_hours: f64,
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
            marble_goal: Goal::Currency,
            marble_desired: None,
            marble_affordable: false,
            marble_optimizer_goal: Goal::Currency,
            marble_optimizer_steps: 50,
            marble_optimizer_affordable: true,
            sim_hours: SIM_DEFAULT_HOURS,
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
        self.marble_desired = self.marble_desired.filter(|currency| {
            *currency < fountain::CURRENCIES && fountain.currency_unlocked(*currency)
        });
        if !STEP_CHOICES.contains(&self.optimizer_steps) {
            self.optimizer_steps = 50;
        }
        if !STEP_CHOICES.contains(&self.marble_optimizer_steps) {
            self.marble_optimizer_steps = 50;
        }
        if !self.sim_hours.is_finite() {
            self.sim_hours = SIM_DEFAULT_HOURS;
        }
        self.sim_hours = self.sim_hours.clamp(SIM_MIN_HOURS, SIM_MAX_HOURS);
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

    /// The marble planner's request: its own goal, desired currency and
    /// affordability, with one step per candidate.
    pub(crate) fn marble_planner_request(&self) -> MarbleRequest {
        MarbleRequest {
            goal: self.marble_goal,
            desired: self.marble_desired,
            max_steps: 1,
            affordable: self.marble_affordable,
        }
    }

    /// The marble optimizer's request, including its step cap.
    pub(crate) fn marble_plan_request(&self) -> MarbleRequest {
        MarbleRequest {
            goal: self.marble_optimizer_goal,
            desired: None,
            max_steps: self.marble_optimizer_steps,
            affordable: self.marble_optimizer_affordable,
        }
    }

    /// The simulated run's request.
    pub(crate) fn sim_request(&self) -> SimRequest {
        SimRequest {
            hours: self.sim_hours,
            desired: None,
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
