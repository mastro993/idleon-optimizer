//! What the Spelunking controls do to the window state.
//!
//! Each action fits the request to the save (capacity, unlocked speeds) and
//! notifies the window; the arithmetic lives in [`State`] and the core crate.

use crate::sections::spelunking::RunInputs as SpelunkRunInputs;
use crate::state::AppState;
use gpui::Context;
use idleon_core::spelunking::{
    Entry, Inputs, Policy, Preset, ReferenceTool, RunOptions, StaminaMode, ToolChoice, normalize,
    simulate,
};

impl AppState {
    /// The Spelunking inputs for the loaded save and the account inputs on screen.
    pub fn spelunk_inputs(&self) -> Option<Inputs> {
        self.spelunking_save
            .as_ref()
            .map(|save| save.inputs(&self.spelunking.account))
    }

    /// Select the logged-in character.
    pub fn set_spelunk_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.character = character.min(inputs.characters.len().saturating_sub(1));
        cx.notify();
    }

    /// Change caves, fitting the party to the new capacity and re-anchoring depths.
    pub fn set_spelunk_cave(&mut self, cave: usize, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_cave(cave, &inputs);
        cx.notify();
    }

    /// Switch Normal/Manic mode.
    pub fn set_spelunk_manic(&mut self, manic: bool, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_manic(manic, &inputs);
        cx.notify();
    }

    /// Change the expedition policy.
    pub fn set_spelunk_policy(&mut self, policy: Policy, cx: &mut Context<Self>) {
        self.spelunking.set_policy(policy);
        cx.notify();
    }

    /// Set the Auto full-clear flag.
    pub fn set_spelunk_full_clear(&mut self, full_clear: bool, cx: &mut Context<Self>) {
        self.spelunking.full_clear = full_clear;
        cx.notify();
    }

    /// Set the Auto speed, clamped to the save's unlocked maximum.
    pub fn set_spelunk_speed(&mut self, speed: u32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.speed = speed.min(inputs.speed_max);
        cx.notify();
    }

    /// Choose saved or fully recovered starting stamina.
    pub fn set_spelunk_starting(&mut self, mode: StaminaMode, cx: &mut Context<Self>) {
        self.spelunking.starting = mode;
        cx.notify();
    }

    /// Fill one party slot.
    pub fn set_spelunk_party(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_party_slot(slot, id, &inputs);
        cx.notify();
    }

    /// Fill one belt slot.
    pub fn set_spelunk_elixir(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_elixir(slot, id, &inputs);
        cx.notify();
    }

    /// Fill one memorized slot.
    pub fn set_spelunk_memorized(&mut self, slot: usize, id: i32, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.set_memorized(slot, id, &inputs);
        cx.notify();
    }

    /// Apply a built-in belt or memory preset.
    pub fn apply_spelunk_preset(&mut self, preset: Preset, cx: &mut Context<Self>) {
        let Some(inputs) = self.spelunk_inputs() else {
            return;
        };
        self.spelunking.apply_preset(&inputs, &preset);
        cx.notify();
    }

    /// Set the manual Spelunking Power.
    pub fn set_spelunk_power(&mut self, power: f64, cx: &mut Context<Self>) {
        self.spelunking.set_power(power);
        cx.notify();
    }

    /// Set the manual extra-exit bonus.
    pub fn set_spelunk_holes_bonus(&mut self, bonus: f64, cx: &mut Context<Self>) {
        self.spelunking.set_holes_bonus(bonus);
        cx.notify();
    }

    /// Set the manual Amber multiplier.
    pub fn set_spelunk_amber(&mut self, amber: f64, cx: &mut Context<Self>) {
        self.spelunking.set_amber(amber);
        cx.notify();
    }

    /// Set the manual stamina regeneration per hour.
    pub fn set_spelunk_regen(&mut self, regen: f64, cx: &mut Context<Self>) {
        self.spelunking.set_regen(regen);
        cx.notify();
    }

    /// Set the Grand Discovery chance, entered as a percentage.
    pub fn set_spelunk_grand_pct(&mut self, percent: f64, cx: &mut Context<Self>) {
        self.spelunking.set_grand_pct(percent);
        cx.notify();
    }

    /// Override one character's maximum stamina.
    pub fn set_spelunk_max_stamina(
        &mut self,
        character: usize,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        self.spelunking.set_max_stamina(character, value);
        cx.notify();
    }

    /// Set the first depth the estimate covers.
    pub fn set_spelunk_depth_from(&mut self, from: usize, cx: &mut Context<Self>) {
        self.spelunking.depth_from = from;
        self.spelunking.clamp_range();
        cx.notify();
    }

    /// Set the last depth the estimate covers.
    pub fn set_spelunk_depth_to(&mut self, to: usize, cx: &mut Context<Self>) {
        self.spelunking.depth_to = to;
        self.spelunking.clamp_range();
        cx.notify();
    }

    /// Choose the reference tool the estimate isolates objects with.
    pub fn set_spelunk_depth_tool(&mut self, tool: ReferenceTool, cx: &mut Context<Self>) {
        self.spelunking.depth_tool = tool;
        cx.notify();
    }

    /// Choose the depth the inspector shows.
    pub fn set_spelunk_inspect(&mut self, depth: usize, cx: &mut Context<Self>) {
        self.spelunking.inspect = depth;
        self.spelunking.clamp_inspect();
        cx.notify();
    }

    /// Set the seed the next run uses.
    pub fn set_spelunk_sim_seed(&mut self, seed: u32, cx: &mut Context<Self>) {
        self.spelunking.sim_seed = seed;
        cx.notify();
    }

    /// Set the manual reaction time the run waits between actions.
    pub fn set_spelunk_sim_reaction(&mut self, reaction_ms: f64, cx: &mut Context<Self>) {
        if reaction_ms.is_finite() {
            self.spelunking.sim_reaction_ms = reaction_ms.clamp(100.0, 5_000.0);
        }
        cx.notify();
    }

    /// Choose the manual tool: adaptive, or one unlocked tool.
    pub fn set_spelunk_sim_tool(&mut self, tool: ToolChoice, cx: &mut Context<Self>) {
        self.spelunking.sim_tool = tool;
        cx.notify();
    }

    /// Choose where the run starts.
    pub fn set_spelunk_sim_entry(&mut self, entry: Entry, cx: &mut Context<Self>) {
        self.spelunking.sim_entry = entry;
        cx.notify();
    }

    /// Run the expedition off the UI thread through the shared task runner.
    pub fn run_spelunk_simulation(&mut self, cx: &mut Context<Self>) {
        let Some(spelunk_inputs) = self.spelunk_inputs() else {
            return;
        };
        let requested = self.spelunking.run_requested();
        let config = match normalize(&spelunk_inputs, &requested) {
            Ok(config) => config,
            Err(error) => {
                self.status = format!("This expedition cannot run: {error}");
                cx.notify();
                return;
            }
        };
        if config.policy == Policy::AmberFarm {
            self.status = "Phased Amber farming arrives with SP-03.".into();
            cx.notify();
            return;
        }
        let run_inputs = SpelunkRunInputs {
            requested,
            account: self.spelunking.account.clone(),
            seed: self.spelunking.sim_seed,
            revision: self.save_revision,
        };
        let seed = run_inputs.seed;
        let started = self.spelunk_sim.update(cx, |runner, cx| {
            runner.start(cx, run_inputs, move |_, report| {
                simulate(&spelunk_inputs, config, RunOptions { seed }, report)
                    .ok()
                    .flatten()
            })
        });
        if started {
            self.status = format!("Running the expedition with seed {seed}…");
            cx.notify();
        }
    }

    /// Ask a running expedition to stop; the previous result stays.
    pub fn cancel_spelunk_simulation(&mut self, cx: &mut Context<Self>) {
        if self.spelunk_sim.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the expedition…".into();
            cx.notify();
        }
    }
}
