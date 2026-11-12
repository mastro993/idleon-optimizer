//! The AFK claim tab's own state: the character, its combat stand-ins and the
//! hours away.
//!
//! A claim is a closed-form calculation, so nothing here runs off the UI thread:
//! the pane asks [`State::inputs`] for the current entry and the claim answers at
//! once. The hours are the one typed value, because the upstream page rejects an
//! entry outside the credited range instead of clamping it.

use crate::state::AppState;
use ely_gpui_component::forms::TextInput;
use gpui::{App, AppContext as _, Context, Entity, Window};
use idleon_clamworks::afk::{ClaimError, Compare, Inputs, Phase};
use std::collections::BTreeMap;

/// One character's combat stand-ins, which the combat simulator will supply.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct StandIn {
    /// Monsters killed per hour on Clamworks.
    pub(crate) kills_per_hour: f64,
    /// The character's drop-rate multiplier.
    pub(crate) drop_rate: f64,
}

impl Default for StandIn {
    fn default() -> Self {
        Self {
            kills_per_hour: 0.0,
            drop_rate: 1.0,
        }
    }
}

/// The claim tab's state, kept across dashboard visits.
pub(crate) struct State {
    /// The save character the claim and the comparison run for.
    pub(crate) character: usize,
    /// Each character's stand-ins, keyed by its save index; switching character
    /// switches the numbers, which is what makes a comparison that character's.
    pub(crate) stand_ins: BTreeMap<usize, StandIn>,
    /// The hours away the last entry committed, valid or not.
    pub(crate) hours: f64,
    pub(crate) phase: Phase,
    /// Why the claim refused the committed entry, or `None` when it accepted it.
    pub(crate) error: Option<ClaimError>,
    /// The hours field, read when the claim is calculated.
    pub(crate) hours_field: Entity<TextInput>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let default = Inputs::default();
        let hours_field = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .placeholder("Hours away")
                .label("Hours away");
            input.set_text(default.hours.to_string(), cx);
            input
        });
        Self {
            character: 0,
            stand_ins: BTreeMap::new(),
            hours: default.hours,
            phase: default.phase,
            error: None,
            hours_field,
        }
    }

    /// The claim's inputs for the selected character.
    pub(crate) fn inputs(&self) -> Inputs {
        let stand_in = self.stand_in(self.character);
        Inputs {
            hours: self.hours,
            kills_per_hour: stand_in.kills_per_hour,
            drop_rate: stand_in.drop_rate,
            phase: self.phase,
        }
    }

    /// One character's stand-ins; a character without any reads the defaults.
    pub(crate) fn stand_in(&self, character: usize) -> StandIn {
        self.stand_ins.get(&character).copied().unwrap_or_default()
    }

    /// The claimed comparison for the saved state, or why the entry is refused.
    pub(crate) fn compare(
        &self,
        clamworks: &idleon_clamworks::Clamworks,
        multikill: f64,
    ) -> Result<Compare, ClaimError> {
        match self.error {
            Some(error) => Err(error),
            None => clamworks.afk_compare(self.inputs(), multikill),
        }
    }

    /// The hours field's text.
    pub(crate) fn hours_text(&self, cx: &App) -> String {
        self.hours_field.read(cx).text().trim().to_string()
    }

    /// Commit the typed hours. Text that is not a number is the one entry the
    /// claim cannot read at all; a number outside the credited range is the
    /// claim's own refusal, so it stays here until a later entry is accepted.
    pub(crate) fn commit_hours(&mut self, text: &str) {
        match text.trim().parse::<f64>() {
            Ok(hours) => {
                self.hours = hours;
                self.error = None;
            }
            Err(_) => self.error = Some(ClaimError::Hours),
        }
    }

    /// Pick the character the claim and the comparison run for.
    pub(crate) fn set_character(&mut self, character: usize, count: usize) {
        self.character = if count == 0 {
            0
        } else {
            character.min(count - 1)
        };
    }

    pub(crate) fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
    }

    /// A stand-in number, kept non-negative and finite.
    pub(crate) fn set_stand_in(&mut self, character: usize, update: impl FnOnce(&mut StandIn)) {
        let mut stand_in = self.stand_in(character);
        update(&mut stand_in);
        stand_in.kills_per_hour = non_negative(stand_in.kills_per_hour, 0.0);
        stand_in.drop_rate = non_negative(stand_in.drop_rate, 1.0);
        self.stand_ins.insert(character, stand_in);
    }

    /// Replace the entries from a project config and reseed the field.
    pub(crate) fn adopt(
        &mut self,
        character: usize,
        stand_ins: BTreeMap<usize, StandIn>,
        hours: f64,
        phase: Phase,
        count: usize,
        cx: &mut Context<AppState>,
    ) {
        self.stand_ins = stand_ins
            .into_iter()
            .map(|(index, stand_in)| {
                (
                    index,
                    StandIn {
                        kills_per_hour: non_negative(stand_in.kills_per_hour, 0.0),
                        drop_rate: non_negative(stand_in.drop_rate, 1.0),
                    },
                )
            })
            .collect();
        self.hours = non_negative(hours, Inputs::default().hours);
        self.phase = phase;
        self.set_character(character, count);
        self.seed_hours(cx);
        self.error = None;
    }

    /// After a save import the selected character may be gone.
    pub(crate) fn load(&mut self, count: usize) {
        self.set_character(self.character, count);
    }

    /// Put the committed hours back in the field, as a project import does.
    fn seed_hours(&self, cx: &mut Context<AppState>) {
        let text = self.hours.to_string();
        self.hours_field
            .update(cx, |input, cx| input.set_text(text, cx));
    }
}

fn non_negative(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        fallback
    }
}
