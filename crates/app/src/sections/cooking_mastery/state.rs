//! The section's controls: the Mastery EXP tab's distribution goal and budget,
//! and the yellow point allocator's tab, goal, budget and character.
//!
//! The page keeps the budget in module-level variables
//! (`_masteryBudgetOverride`, `_selectedGoal`, `_yellowBudgetOverride`,
//! `_yellowCharIdx`), so a tab switch keeps them; this struct does the same for
//! the app and is what a project document carries.

use super::Tab;
use idleon_cooking_mastery::{Allocation, Goal, Mastery, Plan, Yellow};

/// The section's controls, kept across dashboard visits.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// The pane the strip shows.
    pub(crate) tab: Tab,
    /// The daily distribution's goal.
    pub(crate) goal: Goal,
    /// The points to spend; `None` spends every point the rank has granted, the
    /// page's default before its input is touched.
    pub(crate) budget: Option<f64>,
    /// The allocator's goal id; `None` takes the first goal the save offers.
    pub(crate) yellow_goal: Option<String>,
    /// The yellow points to spend; `None` spends every point the account holds.
    pub(crate) yellow_budget: Option<f64>,
    /// The character the allocator's stat goals scale with.
    pub(crate) character: usize,
}

impl State {
    /// The budget the plan is asked for: the override, or every point earned.
    pub(crate) fn requested(&self, mastery: &Mastery) -> f64 {
        self.budget
            .unwrap_or_else(|| mastery.points_earned())
            .max(0.0)
    }

    /// The plan for this state's goal and budget.
    pub(crate) fn plan(&self, mastery: &Mastery) -> Plan {
        mastery.distribution(self.goal, self.requested(mastery))
    }

    /// The allocator's goal: the chosen one while the save still offers it, the
    /// page's fallback being the first offered goal.
    pub(crate) fn yellow_goal(
        &self,
        yellow: &Yellow,
    ) -> Option<&'static idleon_cooking_mastery::YellowGoal> {
        self.yellow_goal
            .as_deref()
            .and_then(idleon_cooking_mastery::goal)
            .filter(|goal| yellow.offers(goal))
            .or_else(|| yellow.default_goal())
    }

    /// The yellow points to spend: the override, or every point the account
    /// holds. An override outside `0..=held` is clamped by the allocator.
    pub(crate) fn yellow_requested(&self, yellow: &Yellow) -> f64 {
        self.yellow_budget.unwrap_or(yellow.budget)
    }

    /// The allocator's plan for this state's goal, budget and character.
    pub(crate) fn yellow_plan(&self, yellow: &Yellow) -> Option<Allocation> {
        let goal = self.yellow_goal(yellow)?;
        yellow.allocation(goal.id, self.yellow_requested(yellow), self.character)
    }

    /// Switch the goal from a select value; an unknown id keeps the current one.
    pub(crate) fn set_goal(&mut self, id: &str) {
        if let Some(goal) = Goal::from_id(id) {
            self.goal = goal;
        }
    }

    /// Spend `budget` points instead of every point the rank granted.
    pub(crate) fn set_budget(&mut self, budget: f64) {
        self.budget = Some(if budget.is_finite() {
            budget.max(0.0)
        } else {
            0.0
        });
    }

    /// Switch the allocator's goal; an unknown id keeps the current one.
    pub(crate) fn set_yellow_goal(&mut self, id: &str) {
        if idleon_cooking_mastery::goal(id).is_some() {
            self.yellow_goal = Some(id.to_string());
        }
    }

    /// Spend `budget` yellow points; a non-finite entry is rejected as zero.
    pub(crate) fn set_yellow_budget(&mut self, budget: f64) {
        self.yellow_budget = Some(if budget.is_finite() { budget } else { 0.0 });
    }

    /// Scale the allocator's stat goals with another character.
    pub(crate) fn set_character(&mut self, character: usize) {
        self.character = character;
    }

    /// Follow a new save: its own point totals are the new defaults.
    pub(crate) fn adopt(&mut self) {
        self.budget = None;
        self.yellow_budget = None;
        self.character = 0;
    }
}

#[cfg(test)]
mod tests {
    use idleon_cooking_mastery::{bonus_multi_cook, meals};

    use super::*;

    fn mastery() -> Mastery {
        Mastery {
            level: 18.0,
            upgrades: [7.0, 3.0, 0.0, 0.0, 0.0, 0.0],
            ..Mastery::default()
        }
    }

    /// Two TotDmg meals and two Seff meals, as the allocator needs to offer a
    /// goal.
    fn yellow() -> Yellow {
        let mut levels = vec![0.0; meals().len()];
        levels[0] = 100.0;
        levels[42] = 100.0;
        levels[6] = 50.0;
        levels[27] = 50.0;
        Yellow {
            mastery: vec![0.0; meals().len()],
            levels,
            ribbons: vec![1.0; meals().len()],
            cook_multi: 1.0,
            budget: 12.0,
            characters: vec![
                idleon_cooking_mastery::MealCharacter {
                    name: "Ana".into(),
                    farming: 0.0,
                    summoning: 0.0,
                },
                idleon_cooking_mastery::MealCharacter {
                    name: "Bo".into(),
                    farming: 99.0,
                    summoning: 49.0,
                },
            ],
            kitchens: Vec::new(),
            event_53: false,
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
    fn a_new_save_gets_its_own_default_budgets() {
        let mut state = State {
            budget: Some(3.0),
            goal: Goal::NextUnlock,
            tab: Tab::Yellow,
            yellow_goal: Some("totdmg".into()),
            yellow_budget: Some(4.0),
            character: 3,
        };
        state.adopt();
        assert_eq!(state.budget, None);
        assert_eq!(state.yellow_budget, None);
        assert_eq!(state.character, 0);
        assert_eq!(state.goal, Goal::NextUnlock, "the goal survives an import");
        assert_eq!(state.tab, Tab::Yellow, "the tab survives an import");
        assert_eq!(state.yellow_goal.as_deref(), Some("totdmg"));
    }

    #[test]
    fn the_yellow_budget_and_goal_follow_the_save() {
        let yellow = yellow();
        let state = State::default();
        // The page spends every point the account holds, on its first offered
        // goal, scaled by the first character.
        assert_eq!(state.yellow_requested(&yellow), 12.0);
        let plan = state.yellow_plan(&yellow).expect("a goal is offered");
        assert_eq!(plan.goal.id, "seff");
        assert_eq!(plan.spent, 12.0);
        assert_eq!(plan.character, 0);
        // An override above the total is clamped, and the plan says so.
        let over = State {
            yellow_budget: Some(40.0),
            ..State::default()
        };
        let plan = over.yellow_plan(&yellow).expect("a goal is offered");
        assert_eq!(plan.spent, 12.0);
        assert!(plan.rejected());
        // A non-finite override never becomes a NaN plan.
        let nan = State {
            yellow_budget: Some(f64::NAN),
            ..State::default()
        };
        let plan = nan.yellow_plan(&yellow).expect("a goal is offered");
        assert_eq!(plan.spent, 0.0);
        assert!(plan.rejected());
        // An unknown goal falls back to the first one offered, and a stored goal
        // the save no longer offers does too.
        let mut state = State {
            yellow_goal: Some("chocolate".into()),
            ..State::default()
        };
        assert_eq!(
            state
                .yellow_plan(&yellow)
                .expect("a goal is offered")
                .goal
                .id,
            "seff"
        );
        state.set_yellow_goal("mcook");
        assert_eq!(state.yellow_goal.as_deref(), Some("mcook"));
        state.set_yellow_goal("chocolate");
        assert_eq!(
            state.yellow_goal.as_deref(),
            Some("mcook"),
            "an unknown id changes nothing"
        );
        state.set_yellow_goal("totdmg");
        let plan = state.yellow_plan(&yellow).expect("a goal is offered");
        assert_eq!(plan.goal.id, "totdmg");
        assert_eq!(plan.rows.iter().map(|row| row.allocated).sum::<f64>(), 12.0);
        assert!(plan.rows[0].mastery > bonus_multi_cook(0.0));
    }

    #[test]
    fn the_character_switch_moves_the_stat_goals() {
        let yellow = yellow();
        let mut state = State {
            yellow_goal: Some("seff".into()),
            character: 1,
            ..State::default()
        };
        assert_eq!(
            state
                .yellow_plan(&yellow)
                .expect("a goal is offered")
                .character_name(&yellow),
            "Bo"
        );
        state.set_character(0);
        assert_eq!(
            state
                .yellow_plan(&yellow)
                .expect("a goal is offered")
                .character_name(&yellow),
            "Ana"
        );
    }
}
