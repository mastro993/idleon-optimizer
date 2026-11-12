//! The Mastery EXP tab's distribution controls: the goal and the budget.
//!
//! The page keeps both in module-level variables (`_masteryBudgetOverride` and
//! the input's value), so a tab switch keeps them. This struct does the same for
//! the app and is what a project document carries, like the other sections'.

use idleon_core::cooking_mastery::{Goal, Mastery, Plan};

/// The daily distribution's goal and budget, kept across dashboard visits.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub goal: Goal,
    /// The points to spend; `None` spends every point the rank has granted, the
    /// page's default before its input is touched.
    pub budget: Option<f64>,
}

impl State {
    /// The budget the plan is asked for: the override, or every point earned.
    pub fn requested(&self, mastery: &Mastery) -> f64 {
        self.budget
            .unwrap_or_else(|| mastery.points_earned())
            .max(0.0)
    }

    /// The plan for this state's goal and budget.
    pub fn plan(&self, mastery: &Mastery) -> Plan {
        mastery.distribution(self.goal, self.requested(mastery))
    }

    /// Switch the goal from a select value; an unknown id keeps the current one.
    pub fn set_goal(&mut self, id: &str) {
        if let Some(goal) = Goal::from_id(id) {
            self.goal = goal;
        }
    }

    /// Spend `budget` points instead of every point the rank granted.
    pub fn set_budget(&mut self, budget: f64) {
        self.budget = Some(if budget.is_finite() {
            budget.max(0.0)
        } else {
            0.0
        });
    }

    /// Follow a new save: its own point total is the new default.
    pub fn adopt(&mut self) {
        self.budget = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mastery() -> Mastery {
        Mastery {
            level: 18.0,
            upgrades: [7.0, 3.0, 0.0, 0.0, 0.0, 0.0],
            ..Mastery::default()
        }
    }

    #[test]
    fn the_default_budget_is_every_point_the_rank_grants() {
        let state = State::default();
        let mastery = mastery();
        assert_eq!(state.requested(&mastery), mastery.points_earned());
        assert_eq!(state.plan(&mastery).budget, 19.0);
        assert_eq!(state.plan(&mastery).goal, Goal::Now);
    }

    #[test]
    fn a_budget_override_is_kept_within_the_points_earned() {
        let mut state = State {
            budget: Some(6.0),
            ..State::default()
        };
        let mastery = mastery();
        assert_eq!(state.plan(&mastery).budget, 6.0);
        // A negative override is the page's zero, and the plan says so.
        state.set_budget(-4.0);
        assert_eq!(state.plan(&mastery).budget, 0.0);
        assert_eq!(
            state.plan(&mastery).levels.iter().sum::<f64>(),
            0.0,
            "nothing is spent"
        );
        // An override above the total is the total, not an overspend.
        state.set_budget(500.0);
        assert_eq!(state.plan(&mastery).budget, 19.0);
        // A non-finite entry is rejected as zero rather than becoming a NaN plan.
        state.set_budget(f64::NAN);
        assert_eq!(state.requested(&mastery), 0.0);
    }

    #[test]
    fn the_goal_switch_keeps_an_unknown_id_out() {
        let mut state = State::default();
        state.set_goal("next-unlock");
        assert_eq!(state.goal, Goal::NextUnlock);
        state.set_goal("chocolate");
        assert_eq!(
            state.goal,
            Goal::NextUnlock,
            "an unknown id changes nothing"
        );
        state.set_goal("now");
        assert_eq!(state.goal, Goal::Now);
    }

    #[test]
    fn a_new_save_gets_its_own_default_budget() {
        let mut state = State {
            budget: Some(3.0),
            goal: Goal::NextUnlock,
        };
        state.adopt();
        assert_eq!(state.budget, None);
        assert_eq!(state.goal, Goal::NextUnlock, "the goal survives an import");
    }
}
