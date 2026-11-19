//! What the tabs show that costs a calculation, worked out when its inputs
//! change instead of on every frame.
//!
//! Each figure keeps the key it was built from, so a frame only compares keys:
//! the character rates follow the save and the panel, the breakdown trees the
//! rates and the chosen character, the Cooking EXP table the rates and the
//! balloon controls, and the reset forecast the save and the plan inputs. The
//! Kitchens and Cooking EXP figures are only built while their tab is open.

use super::outcomes::OutcomeSort;
use super::planner::{LadleDonation, LadlePlanRequest};
use super::{State, Tab, exp, kitchens, outcomes, planner, summary};
use crate::state::AppState;
use gpui::{Context, Entity, Window};
use idleon_ladle::kitchen::{self, Scenario, Schedule};
use idleon_ladle::{Ladle, Panel, Stats};
use idleon_ui::runner::TaskRunner;

#[derive(Default)]
pub(super) struct Derived {
    stats_key: Option<(u64, Panel, f64)>,
    /// Bumped whenever [`Self::stats`] is rebuilt, so later keys can name it.
    stats_generation: u64,
    /// Every character's rates for the panel the section holds.
    pub(super) stats: Vec<Stats>,
    trees_key: Option<(u64, usize)>,
    exp_key: Option<(u64, usize, usize, u32, bool)>,
    pub(super) exp: exp::Figures,
    kitchen_key: Option<(u64, kitchen::Inputs)>,
    kitchen_generation: u64,
    /// The reset forecast and what it feeds, while the save has kitchens.
    pub(super) kitchen: Option<kitchens::Figures>,
    outcomes_key: Option<(u64, Scenario, OutcomeSort)>,
    days_key: Option<(u64, Scenario)>,
    schedule_key: Option<Schedule>,
}

impl State {
    /// Bring the figures the open tab reads in line with the save and the
    /// inputs, and give the save's kitchens and meals their controls.
    pub(super) fn sync(
        &mut self,
        revision: u64,
        ladle: &Ladle,
        plan: &Entity<TaskRunner<LadleDonation, LadlePlanRequest>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        let key = (revision, self.panel, self.claim_hours);
        if self.derived.stats_key != Some(key) {
            self.derived.stats = summary::all(ladle, self.panel, self.claim_hours);
            self.derived.stats_key = Some(key);
            self.derived.stats_generation += 1;
        }
        match self.tab {
            Tab::Ladles => self.sync_trees(cx),
            Tab::Kitchens => self.sync_kitchens(revision, ladle, plan, window, cx),
            Tab::Exp => self.sync_exp(ladle, cx),
        }
    }

    /// The chosen character among the rates, clamped to the roster.
    pub(super) fn chosen(&self) -> usize {
        self.breakdown_character
            .min(self.derived.stats.len().saturating_sub(1))
    }

    fn sync_trees(&mut self, cx: &mut Context<AppState>) {
        let key = (self.derived.stats_generation, self.chosen());
        if self.derived.trees_key == Some(key) {
            return;
        }
        self.derived.trees_key = Some(key);
        if let Some(row) = self.derived.stats.get(key.1) {
            let (ladles, claims) = (row.ladles_tree(), row.claims_tree());
            self.ladles_tree
                .update(cx, |tree, cx| tree.set_root(ladles, cx));
            self.claims_tree
                .update(cx, |tree, cx| tree.set_root(claims, cx));
        }
    }

    fn sync_exp(&mut self, ladle: &Ladle, cx: &mut Context<AppState>) {
        let key = (
            self.derived.stats_generation,
            self.chosen(),
            self.balloon,
            self.target_level,
            self.split,
        );
        if self.derived.exp_key == Some(key) {
            return;
        }
        self.derived.exp_key = Some(key);
        let (figures, tree) = exp::figures(self, ladle);
        self.derived.exp = figures;
        if let Some(root) = tree {
            self.balloons_tree
                .update(cx, |tree, cx| tree.set_root(root, cx));
        }
    }

    fn sync_kitchens(
        &mut self,
        revision: u64,
        ladle: &Ladle,
        plan: &Entity<TaskRunner<LadleDonation, LadlePlanRequest>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        let Some(reset) = ladle.kitchen.as_ref() else {
            self.derived.kitchen = None;
            self.derived.kitchen_key = None;
            return;
        };
        let inputs = kitchens::inputs(
            &self.kitchen,
            &self.assigned,
            self.panel,
            self.use_owned,
            ladle,
            &self.derived.stats,
        );
        if self
            .derived
            .kitchen_key
            .as_ref()
            .is_none_or(|(known, known_inputs)| *known != revision || *known_inputs != inputs)
        {
            self.derived.kitchen = Some(kitchens::Figures::new(reset, &inputs));
            self.derived.kitchen_key = Some((revision, inputs));
            self.derived.kitchen_generation += 1;
        }
        self.controls.ensure_pins(reset.kitchens.len(), window, cx);
        self.controls
            .ensure_targets(reset.meals.levels.len(), window, cx);
        let Some(figures) = self.derived.kitchen.as_ref() else {
            return;
        };
        let generation = self.derived.kitchen_generation;
        let scenario_plan = figures
            .forecast
            .as_ref()
            .as_ref()
            .ok()
            .map(|forecast| forecast.plan(self.scenario));

        let outcomes_key = (generation, self.scenario, self.outcome_sort);
        if self.derived.outcomes_key != Some(outcomes_key) {
            self.derived.outcomes_key = Some(outcomes_key);
            let rows = outcomes::rows(
                &reset.meals.levels,
                scenario_plan,
                self.outcome_sort,
                &self.kitchen.targets,
            );
            self.controls.outcomes.update(cx, |table, cx| {
                table.delegate_mut().rows = rows;
                cx.notify();
            });
        }

        let days_key = (generation, self.scenario);
        if self.derived.days_key != Some(days_key) {
            self.derived.days_key = Some(days_key);
            let count = scenario_plan.map_or(0, |plan| plan.timeline.len());
            self.controls.days_list.reset(count);
        }

        let stored = plan.read(cx).result().map(|donation| &donation.schedule);
        if stored != self.derived.schedule_key.as_ref() {
            let stored = stored.cloned();
            let rows = stored
                .as_ref()
                .map(planner::schedule_rows)
                .unwrap_or_default();
            self.derived.schedule_key = stored;
            self.controls.schedule.update(cx, |table, cx| {
                table.delegate_mut().set_rows(rows);
                cx.notify();
            });
        }
    }
}
