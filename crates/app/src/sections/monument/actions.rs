//! What the window does when the monument section is edited or run.

use crate::sections::monument::Tab as MonumentTab;
use crate::sections::monument::justice::{
    Action as JusticeAction, Effect as JusticeEffect, Field as JusticeField,
};
use crate::state::AppState;
use gpui::Context;
use idleon_monument as monument;

impl AppState {
    /// Switch the character whose Monument charge and warnings are read.
    pub(crate) fn set_monument_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let count = self
            .save
            .as_ref()
            .map_or(0, idleon_save::Save::character_count);
        self.monument.character = if count == 0 {
            0
        } else {
            character.min(count - 1)
        };
        cx.notify();
    }

    /// Switch the monument tab.
    pub(crate) fn set_monument_tab(&mut self, tab: MonumentTab, cx: &mut Context<Self>) {
        self.monument.tab = tab;
        cx.notify();
    }

    /// The enemy number the odds are for, clamped to the upstream 1-1001 range.
    pub(crate) fn set_monument_wave(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.wave = value.round().clamp(1.0, 1001.0) as u32;
        cx.notify();
    }

    /// The damage-boost stacks already earned this story.
    pub(crate) fn set_monument_boosts(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.boosts = value.round().max(0.0) as u32;
        cx.notify();
    }

    /// Rethrows left; `None` means the saved maximum.
    pub(crate) fn set_monument_rerolls(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.rerolls = Some(value.round().max(0.0) as u32);
        cx.notify();
    }

    /// Go back to the saved maximum rethrows.
    pub(crate) fn reset_monument_rerolls(&mut self, cx: &mut Context<Self>) {
        self.monument.rerolls = None;
        cx.notify();
    }

    /// The HP before the last sword, or `None` when unknown.
    pub(crate) fn set_monument_hp(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.remaining_hp = Some(value.max(f64::MIN_POSITIVE));
        cx.notify();
    }

    /// Clear the optional last-sword HP.
    pub(crate) fn clear_monument_hp(&mut self, cx: &mut Context<Self>) {
        self.monument.remaining_hp = None;
        cx.notify();
    }

    /// The Bravery snapshot for the imported save, if any.
    pub(crate) fn monument_snapshot(&self) -> Option<monument::Snapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::snapshot(save, self.monument.character))
    }

    /// The permanent rewards, next-run charge and training summary, if a save
    /// with the Monument rows is loaded.
    pub(crate) fn monument_summary(&self) -> Option<monument::Summary> {
        self.monument_save
            .as_ref()
            .map(|save| monument::summary(save, self.monument.character))
    }

    /// The Wisdom snapshot for the imported save, if any.
    pub(crate) fn monument_wisdom(&self) -> Option<monument::WisdomSnapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::wisdom_snapshot(save, self.monument.character))
    }

    /// The Justice snapshot for the imported save, if any.
    pub(crate) fn monument_justice(&self) -> Option<monument::JusticeSnapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::justice_snapshot(save, self.monument.character))
    }

    /// Write one Justice stat, clamped like its input, as one undoable step.
    pub(crate) fn set_monument_justice_field(
        &mut self,
        field: JusticeField,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let mut next = self.monument.worksheet.state.clone();
        field.set(&mut next, value);
        self.edit_monument_justice(next, cx);
    }

    /// Add or remove one deal, promise or investment as one undoable step.
    pub(crate) fn toggle_monument_justice_effect(
        &mut self,
        effect: JusticeEffect,
        on: bool,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.monument.worksheet.state.clone();
        if on {
            next.effects.entry(effect).or_insert(0);
        } else {
            next.effects.remove(&effect);
        }
        self.edit_monument_justice(next, cx);
    }

    /// Change an active effect's value as one undoable step.
    pub(crate) fn set_monument_justice_effect(
        &mut self,
        effect: JusticeEffect,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let mut next = self.monument.worksheet.state.clone();
        if let Some(slot) = next.effects.get_mut(&effect) {
            *slot = value.round().max(0.0) as i64;
            self.edit_monument_justice(next, cx);
        }
    }

    /// Record `next` as one undoable worksheet step.
    fn edit_monument_justice(&mut self, next: monument::justice::State, cx: &mut Context<Self>) {
        self.monument.worksheet.edit(next);
        self.monument.notice.clear();
        cx.notify();
    }

    /// Choose the case the player actually received; not an undoable step.
    pub(crate) fn select_monument_justice_case(&mut self, id: usize, cx: &mut Context<Self>) {
        self.monument.worksheet.select(id);
        self.monument.notice.clear();
        cx.notify();
    }

    /// Record the response and outcome that happened, and wait for the next case.
    pub(crate) fn record_monument_justice(
        &mut self,
        action: JusticeAction,
        outcome: usize,
        cx: &mut Context<Self>,
    ) {
        let worksheet = &self.monument.worksheet;
        let Ok(report) = monument::justice::responses(&worksheet.state, worksheet.case_id) else {
            return;
        };
        let Some(chosen) = report
            .responses
            .iter()
            .find(|response| response.action == action)
            .and_then(|response| response.outcomes.get(outcome))
        else {
            return;
        };
        let next = chosen.state.clone();
        let ended = next.health <= 0;
        self.monument.worksheet.record(next);
        self.monument.notice = if ended {
            "Story ended. Fresh story starts another worksheet."
        } else {
            "Response recorded. Select the next case, then open it to settle delayed effects."
        }
        .into();
        cx.notify();
    }

    /// Open the selected case once, settling the delayed effects it triggers.
    pub(crate) fn open_monument_justice(&mut self, cx: &mut Context<Self>) {
        self.monument.notice = match self.monument.worksheet.open_next() {
            Ok(notes) if notes.is_empty() => {
                "Next case opened; no delayed payments this turn.".to_string()
            }
            Ok(notes) => notes.join(" "),
            Err(error) => error.to_string(),
        };
        cx.notify();
    }

    /// Walk one worksheet step back.
    pub(crate) fn undo_monument_justice(&mut self, cx: &mut Context<Self>) {
        if self.monument.worksheet.undo() {
            self.monument.notice = "Worksheet step undone.".into();
            cx.notify();
        }
    }

    /// Start the worksheet over from the loaded save's fresh-story stats.
    pub(crate) fn reset_monument_justice(&mut self, cx: &mut Context<Self>) {
        let Some(save) = self.monument_save.as_ref() else {
            return;
        };
        let initial = monument::justice::initial_state(save);
        self.monument.worksheet.reset(initial);
        self.monument.notice =
            "Fresh-story stats loaded. Select the first visitor you actually received.".into();
        cx.notify();
    }

    /// The sword odds for the current inputs.
    pub(crate) fn monument_odds(&self) -> Option<Result<monument::Odds, monument::Error>> {
        let save = self.monument_save.as_ref()?;
        Some(monument::bravery_odds(
            save,
            self.monument.wave.saturating_sub(1),
            monument::Runtime {
                rerolls: self.monument.rerolls,
                boosts: self.monument.boosts,
            },
            self.monument.remaining_hp,
        ))
    }
}
