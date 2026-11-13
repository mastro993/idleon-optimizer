//! What the Spelunking controls do to the window state.
//!
//! Each action fits the request to the save (capacity, unlocked speeds) and
//! notifies the window; the arithmetic lives in [`State`] and `idleon_spelunking`.

use crate::sections::spelunking::OptimizerInputs as SpelunkOptimizerInputs;
use crate::sections::spelunking::OptimizerMode;
use crate::sections::spelunking::RunInputs as SpelunkRunInputs;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_spelunking::{
    AmberPolicy, Entry, Inputs, Policy, Preset, ReferenceTool, RunOptions,
    RunResult as SpelunkRunResult, StaminaMode, ToolChoice, normalize, simulate,
};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The Spelunking inputs for the loaded save and the account inputs on screen.
    pub(crate) fn spelunk_inputs(&self) -> Option<Inputs> {
        self.spelunking_save
            .as_ref()
            .map(|save| save.inputs(&self.spelunking.account))
    }

    /// Select the logged-in character.
    pub(crate) fn set_spelunk_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.character = character.min(inputs.characters.len().saturating_sub(1));
        cx.notify();
    }

    /// Change caves, fitting the party to the new capacity and re-anchoring depths.
    pub(crate) fn set_spelunk_cave(&mut self, cave: usize, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_cave(cave, &inputs);
        cx.notify();
    }

    /// Switch Normal/Manic mode.
    pub(crate) fn set_spelunk_manic(&mut self, manic: bool, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_manic(manic, &inputs);
        cx.notify();
    }

    /// Change the expedition policy.
    pub(crate) fn set_spelunk_policy(&mut self, policy: Policy, cx: &mut Context<Self>) {
        self.spelunking.set_policy(policy);
        cx.notify();
    }

    /// Change the phased Amber policy's limits.
    pub(crate) fn set_spelunk_amber_policy(&mut self, amber: AmberPolicy, cx: &mut Context<Self>) {
        self.spelunking.set_amber_policy(amber);
        cx.notify();
    }

    /// Set the Auto full-clear flag.
    pub(crate) fn set_spelunk_full_clear(&mut self, full_clear: bool, cx: &mut Context<Self>) {
        self.spelunking.full_clear = full_clear;
        cx.notify();
    }

    /// Set the Auto speed, clamped to the save's unlocked maximum.
    pub(crate) fn set_spelunk_speed(&mut self, speed: u32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.speed = speed.min(inputs.speed_max);
        cx.notify();
    }

    /// Choose saved or fully recovered starting stamina.
    pub(crate) fn set_spelunk_starting(&mut self, mode: StaminaMode, cx: &mut Context<Self>) {
        self.spelunking.starting = mode;
        cx.notify();
    }

    /// Fill one party slot.
    pub(crate) fn set_spelunk_party(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_party_slot(slot, id, &inputs);
        cx.notify();
    }

    /// Fill one belt slot.
    pub(crate) fn set_spelunk_elixir(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_elixir(slot, id, &inputs);
        cx.notify();
    }

    /// Fill one memorized slot.
    pub(crate) fn set_spelunk_memorized(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_memorized(slot, id, &inputs);
        cx.notify();
    }

    /// Apply a built-in belt or memory preset.
    pub(crate) fn apply_spelunk_preset(&mut self, preset: Preset, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.apply_preset(&inputs, &preset);
        cx.notify();
    }

    /// Set the manual Spelunking Power.
    pub(crate) fn set_spelunk_power(&mut self, power: f64, cx: &mut Context<Self>) {
        self.spelunking.set_power(power);
        cx.notify();
    }

    /// Set the manual extra-exit bonus.
    pub(crate) fn set_spelunk_holes_bonus(&mut self, bonus: f64, cx: &mut Context<Self>) {
        self.spelunking.set_holes_bonus(bonus);
        cx.notify();
    }

    /// Set the manual Amber multiplier.
    pub(crate) fn set_spelunk_amber(&mut self, amber: f64, cx: &mut Context<Self>) {
        self.spelunking.set_amber(amber);
        cx.notify();
    }

    /// Set the manual stamina regeneration per hour.
    pub(crate) fn set_spelunk_regen(&mut self, regen: f64, cx: &mut Context<Self>) {
        self.spelunking.set_regen(regen);
        cx.notify();
    }

    /// Set the Grand Discovery chance, entered as a percentage.
    pub(crate) fn set_spelunk_grand_pct(&mut self, percent: f64, cx: &mut Context<Self>) {
        self.spelunking.set_grand_pct(percent);
        cx.notify();
    }

    /// Override one character's maximum stamina.
    pub(crate) fn set_spelunk_max_stamina(
        &mut self,
        character: usize,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.set_max_stamina(character, value);
        cx.notify();
    }

    /// Set the first depth the estimate covers.
    pub(crate) fn set_spelunk_depth_from(&mut self, from: usize, cx: &mut Context<Self>) {
        self.spelunking.depth_from = from;
        self.spelunking.clamp_range();
        cx.notify();
    }

    /// Set the last depth the estimate covers.
    pub(crate) fn set_spelunk_depth_to(&mut self, to: usize, cx: &mut Context<Self>) {
        self.spelunking.depth_to = to;
        self.spelunking.clamp_range();
        cx.notify();
    }

    /// Choose the reference tool the estimate isolates objects with.
    pub(crate) fn set_spelunk_depth_tool(&mut self, tool: ReferenceTool, cx: &mut Context<Self>) {
        self.spelunking.depth_tool = tool;
        cx.notify();
    }

    /// Choose the depth the inspector shows.
    pub(crate) fn set_spelunk_inspect(&mut self, depth: usize, cx: &mut Context<Self>) {
        self.spelunking.inspect = depth;
        self.spelunking.clamp_inspect();
        cx.notify();
    }

    /// Set the seed the next run uses.
    pub(crate) fn set_spelunk_sim_seed(&mut self, seed: u32, cx: &mut Context<Self>) {
        self.spelunking.sim_seed = seed;
        cx.notify();
    }

    /// Set the manual reaction time the run waits between actions.
    pub(crate) fn set_spelunk_sim_reaction(&mut self, reaction_ms: f64, cx: &mut Context<Self>) {
        if reaction_ms.is_finite() {
            self.spelunking.sim_reaction_ms = reaction_ms.clamp(100.0, 5_000.0);
        }
        cx.notify();
    }

    /// Choose the manual tool: adaptive, or one unlocked tool.
    pub(crate) fn set_spelunk_sim_tool(&mut self, tool: ToolChoice, cx: &mut Context<Self>) {
        self.spelunking.sim_tool = tool;
        cx.notify();
    }

    /// Choose where the run starts.
    pub(crate) fn set_spelunk_sim_entry(&mut self, entry: Entry, cx: &mut Context<Self>) {
        self.spelunking.sim_entry = entry;
        cx.notify();
    }

    /// Run the expedition off the UI thread through the shared task runner.
    pub(crate) fn run_spelunk_simulation(&mut self, cx: &mut Context<Self>) {
        let Some(spelunk_inputs) = self.spelunk_inputs() else {
            return;
        };
        let requested = self.spelunking.run_requested();
        let config = match normalize(&spelunk_inputs, &requested) {
            Ok(config) => config,
            Err(error) => {
                self.set_status(format!("This expedition cannot run: {error}"), cx);
                return;
            }
        };
        let run_inputs = SpelunkRunInputs {
            requested,
            account: self.spelunking.account.clone(),
            seed: self.spelunking.sim_seed,
            revision: self.save_revision,
        };
        let seed = run_inputs.seed;
        self.start_run(
            self.spelunk_sim.clone(),
            format!("Running the expedition with seed {seed}…"),
            run_inputs,
            move |_, report| {
                simulate(&spelunk_inputs, config, RunOptions { seed }, report)
                    .ok()
                    .flatten()
            },
            cx,
        );
    }

    /// Ask a running expedition to stop; the previous result stays.
    pub(crate) fn cancel_spelunk_simulation(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.spelunk_sim.clone(), "Cancelling the expedition…", cx);
    }

    /// The stored expedition run and whether its inputs changed since the run.
    pub(crate) fn spelunk_simulation(&self, cx: &App) -> TaskState<SpelunkRunResult> {
        self.spelunk_sim.read(cx).state(&self.spelunk_sim_inputs())
    }

    /// The inputs a run started now would read.
    pub(crate) fn spelunk_sim_inputs(&self) -> SpelunkRunInputs {
        SpelunkRunInputs {
            requested: self.spelunking.run_requested(),
            account: self.spelunking.account.clone(),
            seed: self.spelunking.sim_seed,
            revision: self.save_revision,
        }
    }
}

impl AppState {
    /// The optimizer inputs for the loaded save and the controls on screen.
    pub(crate) fn spelunk_optimizer_inputs(&self) -> Option<SpelunkOptimizerInputs> {
        let spelunking = &self.spelunking;
        Some(SpelunkOptimizerInputs {
            requested: spelunking.run_requested(),
            account: spelunking.account.clone(),
            mode: spelunking.optimizer_mode,
            objective: spelunking.optimizer_objective,
            screening: spelunking.optimizer_screening,
            trials: spelunking.optimizer_trials,
            seed: spelunking.optimizer_seed,
            scope: spelunking.optimizer_requested(),
            amber_from: spelunking.optimizer_from,
            amber_to: spelunking.optimizer_to,
            amber_min_rope: spelunking.optimizer_min_rope,
            amber_max_trials: spelunking.optimizer_max_trials,
            revision: self.save_revision,
        })
    }

    /// Switch between the configuration search and the Amber phase search.
    pub(crate) fn set_spelunk_optimizer_mode(
        &mut self,
        mode: OptimizerMode,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.optimizer_mode = mode;
        if mode == OptimizerMode::Amber && !self.spelunking.optimizer_objective.amber() {
            self.spelunking.optimizer_objective = idleon_spelunking::Objective::Amber;
        }
        cx.notify();
    }

    /// Choose the objective the optimizer maximises.
    pub(crate) fn set_spelunk_optimizer_objective(
        &mut self,
        objective: idleon_spelunking::Objective,
        cx: &mut Context<Self>,
    ) {
        if self.spelunking.optimizer_mode == OptimizerMode::Amber && !objective.amber() {
            return;
        }
        self.spelunking.optimizer_objective = objective;
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_screening(
        &mut self,
        screening: usize,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.optimizer_screening = screening.clamp(1, 8);
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_trials(&mut self, trials: usize, cx: &mut Context<Self>) {
        self.spelunking.optimizer_trials = trials.clamp(2, 64);
        self.spelunking.optimizer_max_trials = self
            .spelunking
            .optimizer_max_trials
            .max(self.spelunking.optimizer_trials);
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_seed(&mut self, seed: u32, cx: &mut Context<Self>) {
        self.spelunking.optimizer_seed = seed;
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_scope(
        &mut self,
        scope: idleon_spelunking::Enabled,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.optimizer_scope = scope;
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_from(&mut self, from: usize, cx: &mut Context<Self>) {
        self.spelunking.optimizer_from = from.clamp(1, 10_000);
        self.spelunking.optimizer_to = self
            .spelunking
            .optimizer_to
            .clamp(self.spelunking.optimizer_from, 10_000);
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_to(&mut self, to: usize, cx: &mut Context<Self>) {
        self.spelunking.optimizer_to = to.clamp(self.spelunking.optimizer_from, 10_000);
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_min_rope(
        &mut self,
        percent: usize,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.optimizer_min_rope = percent.min(100);
        cx.notify();
    }

    pub(crate) fn set_spelunk_optimizer_max_trials(
        &mut self,
        trials: usize,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.optimizer_max_trials = trials.clamp(self.spelunking.optimizer_trials, 256);
        cx.notify();
    }

    /// Run the selected optimizer off the UI thread through the shared runner.
    pub(crate) fn run_spelunk_optimizer(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        let Some(requested) = self.spelunk_optimizer_inputs() else {
            return;
        };
        if let Err(error) = normalize(&inputs, &requested.requested) {
            self.set_status(format!("This expedition cannot run: {error}"), cx);
            return;
        }
        if requested.mode == OptimizerMode::Configuration {
            match idleon_spelunking::search_space(&inputs, &requested.requested, &requested.scope) {
                Ok((count, _)) => {
                    if count == 0 {
                        self.set_status("The search scope has no allowed configuration.", cx);
                        return;
                    }
                }
                Err(error) => {
                    self.set_status(format!("The search scope is invalid: {error}"), cx);
                    return;
                }
            }
        } else {
            let config = match normalize(&inputs, &requested.requested) {
                Ok(config) => config,
                Err(error) => {
                    self.set_status(format!("This expedition cannot run: {error}"), cx);
                    return;
                }
            };
            if !config.elixirs.contains(&3) {
                self.set_status(
                    "Amber farming needs at least one equipped orange elixir.",
                    cx,
                );
                return;
            }
            if !requested.objective.amber() {
                self.set_status("Choose a collected-Amber objective for Amber farming.", cx);
                return;
            }
        }
        let options = idleon_spelunking::SearchOptions {
            objective: requested.objective,
            screening: requested.screening,
            trials: requested.trials,
            seed: requested.seed,
            scope: requested.scope.clone(),
        };
        let amber = idleon_spelunking::AmberSearchOptions {
            objective: requested.objective,
            screening: requested.screening,
            trials: requested.trials,
            max_trials: requested.amber_max_trials,
            min_rope_pct: requested.amber_min_rope,
            from: requested.amber_from,
            to: Some(requested.amber_to),
            seed: requested.seed,
        };
        let mode = requested.mode;
        let run_inputs = requested.clone();
        self.start_run(
            self.spelunk_opt.clone(),
            "Optimizing the expedition…",
            run_inputs,
            move |input, report| {
                let result = match mode {
                    OptimizerMode::Configuration => {
                        idleon_spelunking::search(&inputs, &input.requested, &options, report)
                    }
                    OptimizerMode::Amber => {
                        idleon_spelunking::amber_search(&inputs, &input.requested, &amber, report)
                    }
                };
                result.ok().flatten()
            },
            cx,
        );
    }

    /// Ask a running optimizer to stop; the previous result stays.
    pub(crate) fn cancel_spelunk_optimizer(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.spelunk_opt.clone(),
            "Cancelling the expedition optimization…",
            cx,
        );
    }

    /// The stored optimizer result and whether its inputs changed since the run.
    pub(crate) fn spelunk_optimizer(
        &self,
        cx: &gpui::App,
    ) -> TaskState<idleon_spelunking::SearchResult> {
        self.spelunk_opt.read(cx).state(
            &self
                .spelunk_optimizer_inputs()
                .expect("the runner only starts with a save"),
        )
    }

    /// Apply the stored recommendation to the section's controls.
    pub(crate) fn use_spelunk_optimizer_winner(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.spelunk_opt.read(cx).result().cloned() else {
            self.set_status("Run the optimizer before applying a recommendation.", cx);
            return;
        };
        let Some(winner) = result.winner else {
            self.set_status("The optimizer found no recommendation to apply.", cx);
            return;
        };
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking
            .apply_config(&winner.summary.config, &inputs);
        if let Some(inputs) = self.spelunk_optimizer_inputs() {
            self.spelunk_opt
                .update(cx, |runner, _| runner.commit_inputs(inputs));
        }
        self.set_status("Applied the optimizer recommendation.", cx);
    }

    /// Copy the last recommendation's plan to the clipboard.
    pub(crate) fn copy_spelunk_optimizer_plan(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.spelunk_opt.read(cx).result().cloned() else {
            self.set_status("Run the optimizer before copying the plan.", cx);
            return;
        };
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        let text = idleon_spelunking::result_text(&inputs, &result);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        self.set_status(
            format!(
                "Copied the expedition plan to the clipboard ({} bytes).",
                text.len()
            ),
            cx,
        );
    }
}
