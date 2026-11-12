//! What the window does when the jelly operator section is edited or run.

use crate::sections::jelly_operator::{Simulator as JellySimulator, Tab as JellyTab};
use crate::state::AppState;
use gpui::{App, Context};
use idleon_common::{Control, Progress};
use idleon_jelly::operation::{RevivePolicy, simulate as simulate_jelly_operation};
use idleon_jelly::{Estimate as JellyEstimate, Jelly, Trial as JellyTrial};
use idleon_ui::runner::TaskState;

impl AppState {
    /// What the shared Jelly estimate runner shows against the current inputs.
    pub(crate) fn jelly_task(&self, cx: &App) -> TaskState<JellyEstimate> {
        let inputs = self.jelly_state.inputs(self.jelly.as_ref());
        self.jelly_run.read(cx).state(&inputs)
    }

    /// Switch the Jelly Operator tab.
    pub(crate) fn set_jelly_tab(&mut self, tab: JellyTab, cx: &mut Context<Self>) {
        if self.jelly_state.tab != tab {
            self.jelly_state.tab = tab;
            cx.notify();
        }
    }

    /// Pick the palette cell type the next board click places.
    pub(crate) fn jelly_select(&mut self, cell_type: usize, cx: &mut Context<Self>) {
        self.jelly_state.select(cell_type);
        let message = self.jelly_state.message.clone();
        self.set_status(
            if message.is_empty() {
                format!(
                    "Jelly palette: {}.",
                    idleon_jelly::data().cell_name(cell_type)
                )
            } else {
                message
            },
            cx,
        );
    }

    /// Place the picked cell on the slot, or remove the cell that covers it.
    pub(crate) fn jelly_click_slot(&mut self, anchor: usize, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        let layout = self.jelly_state.layout.clone();
        if idleon_jelly::board::occupancy(&layout).contains_key(&anchor) {
            self.jelly_state.remove_at(&layout, anchor);
        } else {
            self.jelly_state.place(&jelly, anchor);
        }
        self.set_status(self.jelly_state.message.clone(), cx);
    }

    /// Put the board back to the save's own layout.
    pub(crate) fn jelly_reset_layout(&mut self, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        self.jelly_state.reset(&jelly);
        self.set_status(self.jelly_state.message.clone(), cx);
    }

    /// How many seeded trials an estimate runs.
    pub(crate) fn set_jelly_trials(&mut self, value: f64, cx: &mut Context<Self>) {
        let trials = value.clamp(1.0, 1024.0).round() as u32;
        if self.jelly_state.trials != trials {
            self.jelly_state.trials = trials;
            cx.notify();
        }
    }

    /// The seed the trials are derived from.
    pub(crate) fn set_jelly_seed(&mut self, value: f64, cx: &mut Context<Self>) {
        let seed = if value.is_finite() {
            value.clamp(0.0, 4_294_967_295.0) as u32
        } else {
            1
        };
        if self.jelly_state.seed != seed {
            self.jelly_state.seed = seed;
            cx.notify();
        }
    }

    /// Run the operation estimate off the UI thread through the shared runner.
    pub(crate) fn run_jelly_estimate(&mut self, cx: &mut Context<Self>) {
        let inputs = self.jelly_state.inputs(self.jelly.as_ref());
        let Some(jelly) = inputs.jelly.clone() else {
            self.set_status("Jelly estimate: import a save with a board first.", cx);
            return;
        };
        let trials = inputs.trials;
        let layout = inputs.layout.clone();
        let seed = inputs.seed;
        self.start_run(
            self.jelly_run.clone(),
            format!("Estimating the Jelly operation over {trials} trials…"),
            inputs,
            move |_inputs, report| {
                JellyEstimate::run_with(&jelly, &layout, trials, seed, |done| {
                    report(Progress {
                        fraction: done as f32 / trials as f32,
                        message: format!("Trial {done} of {trials}"),
                    }) == Control::Continue
                })
            },
            cx,
        );
    }

    /// Ask a running estimate to stop at its next trial boundary.
    pub(crate) fn cancel_jelly_estimate(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.jelly_run.clone(), "Cancelling the Jelly estimate…", cx);
    }

    /// What the shared Jelly simulator runner shows against the sandbox.
    pub(crate) fn jelly_sim_task(&self, cx: &App) -> TaskState<JellyTrial> {
        let inputs = self.jelly_state.simulator.inputs(self.jelly.as_ref());
        self.jelly_sim_run.read(cx).state(&inputs)
    }

    /// Apply one sandbox edit, publish its message, and let the runner notice
    /// the new inputs.
    pub(crate) fn jelly_sim_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut JellySimulator, &Jelly),
    ) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        edit(&mut self.jelly_state.simulator, &jelly);
        self.set_status(self.jelly_state.simulator.message.clone(), cx);
    }

    /// The simulator's stage select.
    pub(crate) fn set_jelly_sim_stage(&mut self, stage: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_stage(jelly, stage));
    }

    /// The simulator's Fever select.
    pub(crate) fn set_jelly_sim_fever(&mut self, fever: u32, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_fever(jelly, fever));
    }

    /// Stronkroid's activation second; a negative value disables it.
    pub(crate) fn set_jelly_sim_roid(&mut self, seconds: f64, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_roid(seconds));
    }

    /// The simulator's automatic revive policy.
    pub(crate) fn set_jelly_sim_revive(&mut self, revive: RevivePolicy, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_revive(revive));
    }

    /// The seed one simulator attempt is drawn from.
    pub(crate) fn set_jelly_sim_seed(&mut self, seed: f64, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_seed(seed));
    }

    /// Pick the cell the next sandbox board click places.
    pub(crate) fn jelly_sim_select(&mut self, cell_type: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.select(cell_type));
    }

    /// Remove mode: the next sandbox board click removes what it covers.
    pub(crate) fn jelly_sim_remove_mode(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.remove_mode());
    }

    /// Place the picked cell on the sandbox slot, or remove the cell there.
    pub(crate) fn jelly_sim_click_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.click(jelly, slot));
    }

    /// Empty the sandbox board.
    pub(crate) fn clear_jelly_simulator(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.clear_board());
    }

    /// Put the sandbox back to the save's own board and knobs.
    pub(crate) fn reset_jelly_simulator(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.reset(jelly));
    }

    /// Restore the save's purchased board sections in the sandbox.
    pub(crate) fn jelly_sim_use_save_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.use_save_sections(jelly));
    }

    /// Lock every purchased board section in the sandbox.
    pub(crate) fn jelly_sim_lock_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.lock_sections(jelly));
    }

    /// Unlock every board section in the sandbox.
    pub(crate) fn jelly_sim_unlock_all_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.unlock_all_sections(jelly));
    }

    /// Apply the whole board-section selection the sandbox toggle group reports.
    pub(crate) fn jelly_sim_set_plots(&mut self, plots: &[usize], cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_plots(jelly, plots));
    }

    /// Run one seeded operation of the sandbox board off the UI thread.
    pub(crate) fn run_jelly_simulation(&mut self, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            self.set_status("Jelly simulation: import a save with a board first.", cx);
            return;
        };
        let simulator = &self.jelly_state.simulator;
        let sandbox = simulator.sandbox(&jelly);
        let layout = simulator.layout.clone();
        if let Err(reason) = idleon_jelly::board::validate(&sandbox, &layout) {
            let message = format!("The sandbox board is not valid: {reason}");
            let simulator = &mut self.jelly_state.simulator;
            simulator.message = message.clone();
            simulator.message_bad = true;
            self.set_status(message, cx);
            return;
        }
        let inputs = simulator.inputs(Some(&jelly));
        let options = simulator.options();
        self.start_run(
            self.jelly_sim_run.clone(),
            "Simulating one Jelly operation…",
            inputs,
            move |_inputs, report| {
                let _ = report(Progress {
                    fraction: 0.0,
                    message: "One seeded 60 FPS operation…".into(),
                });
                Some(simulate_jelly_operation(&sandbox, &layout, options))
            },
            cx,
        );
    }

    /// Ask a running simulation to stop at its next report.
    pub(crate) fn cancel_jelly_simulation(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.jelly_sim_run.clone(),
            "Cancelling the Jelly simulation…",
            cx,
        );
    }
}
