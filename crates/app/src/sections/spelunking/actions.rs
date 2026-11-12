//! What the Spelunking controls do to the window state.
//!
//! Each action fits the request to the save (capacity, unlocked speeds) and
//! notifies the window; the arithmetic lives in [`State`] and the core crate.

use crate::state::AppState;
use gpui::Context;
use idleon_core::spelunking::{Inputs, Policy, Preset, ReferenceTool, StaminaMode};

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
}
