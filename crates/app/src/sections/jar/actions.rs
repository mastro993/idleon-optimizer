//! What the window does when the jar section is edited or run.

use crate::sections::jar::State as JarState;
use crate::sections::jar::Tab as JarTab;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_jar::{COLLECTIBLES, EnchantSnapshot, Jar, SimInputs, SimResult, record_snapshot};
use idleon_ui::runner::TaskState;

impl AppState {
    pub(crate) fn set_jar_tab(&mut self, tab: JarTab, cx: &mut Context<Self>) {
        self.jar_tab = tab;
        cx.notify();
    }

    /// Switch the tier the rupie value tab projects, clamped to the six tiers.
    pub(crate) fn set_jar_tier(&mut self, tier: u32, cx: &mut Context<Self>) {
        self.jar_tier = tier.min(idleon_jar::INCOME_TIERS - 1);
        cx.notify();
    }

    /// Change the enchant simulator's controls.
    pub(crate) fn edit_jar_state(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut JarState),
    ) {
        edit(&mut self.jar_state);
        cx.notify();
    }

    /// Run the enchant simulation off the UI thread through the shared runner.
    pub(crate) fn run_jar_simulation(&mut self, cx: &mut Context<Self>) {
        let Some(jar) = self.jar.as_ref() else {
            self.set_status("Enchant simulation: import a save first.", cx);
            return;
        };
        let inputs = self.jar_state.inputs(jar.collectible_levels());
        if inputs.found().is_empty() {
            self.set_status(
                "Enchant simulation: the cavern has not found a collectible yet.",
                cx,
            );
            return;
        }
        let line = format!(
            "Simulating {} enchants over {} runs with seed {}…",
            inputs.enchants, inputs.sims, inputs.seed
        );
        self.start_run(
            self.jar_sim.clone(),
            line,
            inputs,
            move |inputs, report| idleon_jar::simulate(&inputs, report),
            cx,
        );
    }

    /// Ask a running simulation to stop; the previous spread stays.
    pub(crate) fn cancel_jar_simulation(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.jar_sim.clone(),
            "Cancelling the enchant simulation…",
            cx,
        );
    }

    /// The stored spread and whether its inputs changed since the run.
    pub(crate) fn jar_simulation(&self, cx: &App) -> TaskState<SimResult> {
        self.jar_sim.read(cx).state(&self.jar_sim_inputs())
    }

    /// The inputs a simulation started now would read.
    fn jar_sim_inputs(&self) -> SimInputs {
        let levels = self
            .jar
            .as_ref()
            .map(Jar::collectible_levels)
            .unwrap_or([0; COLLECTIBLES]);
        self.jar_state.inputs(levels)
    }

    /// `_saveSnapshot`: record the applied save's spread once.
    pub(crate) fn record_jar_snapshot(&mut self) {
        let Some(jar) = self.jar.as_ref() else {
            return;
        };
        let at = jiff::Timestamp::now().as_second();
        record_snapshot(&mut self.jar_state.history, EnchantSnapshot::of(jar, at));
    }

    /// `esim-clear-history`: drop every saved spread.
    pub(crate) fn clear_jar_history(&mut self, cx: &mut Context<Self>) {
        let cleared = self.jar_state.history.len();
        self.jar_state.history.clear();
        self.set_status(format!("Cleared {cleared} enchant snapshot(s)."), cx);
    }
}
