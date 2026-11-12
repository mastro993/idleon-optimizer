//! What the window does when the sailing section is edited or run.

use crate::sections::sailing::Tab as SailingTab;
use crate::sections::sailing::simulation::RunInputs as SailingSimInputs;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_sailing::{
    AssignmentMode, DEFAULT_HOURS, DEFAULT_SEED, FleetComparison, FleetPlan, Goal, MAX_HOURS,
    MAX_TRIALS, MIN_HOURS, MIN_TRIALS, SimConfig, SimOptions as SailingSimOptions, SimResult,
    compare, optimize, saved, simulate,
};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The sailing fleet comparison for the section's current island and goal.
    pub(crate) fn fleet_comparison(&self) -> FleetComparison {
        compare(
            &self.fleet,
            self.sailing_state.island,
            self.sailing_state.goal,
            &self.sailing,
        )
    }

    /// One island's plan under the selected assignment mode.
    pub(crate) fn fleet_plan(&self, island: usize, goal: Goal) -> FleetPlan {
        match self.sailing_state.mode {
            AssignmentMode::Optimized => optimize(&self.fleet, island, goal, &self.sailing),
            AssignmentMode::Saved => saved(&self.fleet, island, goal, &self.sailing),
        }
    }

    pub(crate) fn set_sailing_tab(&mut self, tab: SailingTab, cx: &mut Context<Self>) {
        self.sailing_state.tab = tab;
        cx.notify();
    }

    /// The boat the artifact ranking and the breakdowns read.
    pub(crate) fn set_sailing_breakdown_boat(&mut self, boat: usize, cx: &mut Context<Self>) {
        self.sailing_state.breakdown_boat = boat.min(self.fleet.boats.len().saturating_sub(1));
        cx.notify();
    }

    pub(crate) fn set_sailing_sort_by_eta(&mut self, sorted: bool, cx: &mut Context<Self>) {
        self.sailing_state.sort_by_eta = sorted;
        cx.notify();
    }

    /// The Unending Loot Search talent percentage the replacement matrix reads.
    pub(crate) fn set_unending_loot_search(&mut self, percent: f64, cx: &mut Context<Self>) {
        self.sailing.unending_loot_search = percent.clamp(0.0, 100.0);
        cx.notify();
    }

    pub(crate) fn set_fleet_mode(&mut self, mode: AssignmentMode, cx: &mut Context<Self>) {
        self.sailing_state.mode = mode;
        cx.notify();
    }

    pub(crate) fn set_fleet_island(&mut self, island: usize, cx: &mut Context<Self>) {
        let total = self.fleet.available_islands().max(1);
        self.sailing_state.island = island.min(total - 1);
        cx.notify();
    }

    pub(crate) fn set_fleet_goal(&mut self, goal: Goal, cx: &mut Context<Self>) {
        self.sailing_state.goal = goal;
        cx.notify();
    }

    pub(crate) fn set_fleet_minimum_tier(&mut self, tier: u32, cx: &mut Context<Self>) {
        self.sailing.minimum_tier = tier.min(5);
        cx.notify();
    }

    /// The stored pile simulation and whether its inputs changed since the run.
    pub(crate) fn sailing_simulation(&self, cx: &App) -> TaskState<SimResult> {
        self.sailing_sim.read(cx).state(&self.sailing_sim_inputs())
    }

    /// The inputs a simulation started now would read.
    pub(crate) fn sailing_sim_inputs(&self) -> SailingSimInputs {
        SailingSimInputs {
            island: self.sailing_state.sim_island,
            goal: self.sailing_state.goal,
            mode: self.sailing_state.mode,
            minimum_tier: self.sailing.minimum_tier,
            hours: self.sailing_state.sim_hours,
            trials: self.sailing_state.sim_trials,
            unending_loot_search: self.sailing.unending_loot_search,
            capacity: self.sailing.chest_capacity,
            revision: self.save_revision,
        }
    }

    /// Select the destination the pile simulation runs on.
    pub(crate) fn set_sailing_sim_island(&mut self, island: usize, cx: &mut Context<Self>) {
        let total = self.fleet.available_islands().max(1);
        self.sailing_state.sim_island = island.min(total - 1);
        cx.notify();
    }

    /// Set the interval one simulation trial covers.
    pub(crate) fn set_sailing_sim_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        self.sailing_state.sim_hours = if hours.is_finite() {
            hours.clamp(MIN_HOURS, MAX_HOURS)
        } else {
            DEFAULT_HOURS
        };
        cx.notify();
    }

    /// Set how many independent trials a run makes.
    pub(crate) fn set_sailing_sim_trials(&mut self, trials: usize, cx: &mut Context<Self>) {
        self.sailing_state.sim_trials = trials.clamp(MIN_TRIALS, MAX_TRIALS);
        cx.notify();
    }

    /// Run the pile simulation off the UI thread through the shared runner.
    pub(crate) fn run_sailing_simulation(&mut self, cx: &mut Context<Self>) {
        let inputs = self.sailing_sim_inputs();
        let plan = self.fleet_plan(inputs.island, inputs.goal);
        if plan.routes.is_empty() {
            self.set_status("No active boats are available for the pile simulation.", cx);
            return;
        }
        let config = SimConfig::from_plan(
            &plan,
            &self.sailing,
            SailingSimOptions {
                hours: inputs.hours,
                trials: inputs.trials,
                seed: DEFAULT_SEED,
            },
        );
        self.start_run(
            self.sailing_sim.clone(),
            format!(
                "Simulating the fleet pile for {}h over {} trials…",
                self.sailing_state.sim_hours, self.sailing_state.sim_trials
            ),
            inputs,
            move |_, report| simulate(&config, report),
            cx,
        );
    }

    /// Ask a running pile simulation to stop; the previous result stays.
    pub(crate) fn cancel_sailing_simulation(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.sailing_sim.clone(),
            "Cancelling the fleet pile simulation…",
            cx,
        );
    }
}
