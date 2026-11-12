//! `planKitchenForecast`: the scenarios the Kitchens tab compares.
//!
//! Upstream plans the same horizon three ways, so the section can show what the
//! optimizer's assignment and the Blue Jellyfish donation policy each add: the
//! saved assignments with no new donations, the optimized assignments with no new
//! donations, and the optimized assignments with the selected policy. Only the
//! assignment search and the donations differ; every scenario spends the same
//! guaranteed income directly.

use super::layout::{LayoutChoice, LayoutMode};
use super::plan::{Inputs, Plan, Policy, plan};
use super::{Objective, Problem, Reset};

/// The scenarios the comparison shows, in the order upstream renders them.
pub const SCENARIOS: [Scenario; 3] = [Scenario::Current, Scenario::Layout, Scenario::Recommended];

/// One way to plan the same horizon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scenario {
    /// `currentPlan`: the saved kitchen assignments, with no new donations.
    Current,
    /// `baselinePlan`: the optimized assignments, with no new donations.
    Layout,
    /// `plan`: the optimized assignments plus the selected donation policy.
    #[default]
    Recommended,
}

impl Scenario {
    /// The stable id the section's select carries.
    pub fn id(self) -> &'static str {
        match self {
            Scenario::Current => "current",
            Scenario::Layout => "layout",
            Scenario::Recommended => "recommended",
        }
    }

    /// The name upstream gives the scenario.
    pub fn label(self) -> &'static str {
        match self {
            Scenario::Current => "Saved reset assignments, no new donations",
            Scenario::Layout => "Optimized reset assignments, no new donations",
            Scenario::Recommended => "Optimized assignments + selected donation policy",
        }
    }

    /// The scenario a select value names, if any.
    pub fn from_id(id: &str) -> Option<Self> {
        SCENARIOS.into_iter().find(|scenario| scenario.id() == id)
    }
}

/// One horizon planned all three ways.
#[derive(Clone, Debug, PartialEq)]
pub struct Forecast {
    pub current: Plan,
    pub layout: Plan,
    pub recommended: Plan,
}

impl Forecast {
    /// The scenario's plan.
    pub fn plan(&self, scenario: Scenario) -> &Plan {
        match scenario {
            Scenario::Current => &self.current,
            Scenario::Layout => &self.layout,
            Scenario::Recommended => &self.recommended,
        }
    }

    /// `rankSum`: the meal levels the scenario gained over the horizon.
    pub fn ranks(&self, scenario: Scenario) -> f64 {
        self.plan(scenario).gained.iter().sum()
    }
}

/// `planKitchenForecast` with a fixed donation schedule: the same horizon under
/// the saved layout, the optimized layout and the policy.
///
/// The donation search that picks the policy itself is the NMLB planner's job;
/// here the policy is whatever the section's controls already say.
pub fn forecast(reset: &Reset, inputs: &Inputs) -> Result<Forecast, Problem> {
    if inputs.objective == Objective::Targets && !inputs.targets.iter().any(|level| *level > 0.0) {
        return Err(Problem::Targets);
    }
    let without_donations = |mode| Inputs {
        policy: Policy::None,
        owned_fraction: 0.0,
        layout: LayoutChoice {
            mode,
            ..inputs.layout.clone()
        },
        ..inputs.clone()
    };
    Ok(Forecast {
        current: plan(reset, &without_donations(LayoutMode::Saved))?,
        layout: plan(reset, &without_donations(LayoutMode::Optimized))?,
        recommended: plan(reset, inputs)?,
    })
}
