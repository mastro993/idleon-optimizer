//! The Fishing Calculator section: AFK catches, the fish split and the real
//! versus panel EXP of every pool.
//!
//! FI-01 ports the upstream `fishing-calc.html` toolbar, the character context
//! and the per-pool outcome from `js/stats/systems/w2/fishing.js`. The lure/line
//! ranking (FI-02) and the Trench depth projection (FI-03) are later features in
//! the same milestone: the toolbar and the location rows already switch pools,
//! and the account values the upstream derives from the skill systems are
//! entered in the account card until those systems are decoded.

pub mod detail;
pub mod inputs;
pub mod pools;
pub mod toolbar;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use idleon_core::fishing::{self, AccountInputs, Evaluation, FishingSave};

/// The section's live state: the character, toolkit, location, hours and the
/// account values the upstream derives.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub character: usize,
    pub lure: usize,
    pub line: usize,
    /// The upstream location id, `map:spot` or `x:pool`.
    pub location: String,
    /// Hours away; the AFK claim length.
    pub hours: f64,
    pub account: AccountInputs,
}

impl Default for State {
    fn default() -> Self {
        Self {
            character: 0,
            lure: 0,
            line: 0,
            location: fishing::data().default_location().id.clone(),
            hours: 1.0,
            account: AccountInputs::default(),
        }
    }
}

impl State {
    /// Take a character's saved toolkit, spot and level after an import.
    pub fn adopt(&mut self, save: &FishingSave) {
        self.character = save.default_character();
        self.character_defaults(save);
    }

    /// Reset the toolkit and location to the selected character's saved ones.
    pub fn character_defaults(&mut self, save: &FishingSave) {
        let Some(character) = save.character(self.character) else {
            self.lure = 0;
            self.line = 0;
            self.location = fishing::data().default_location().id.clone();
            return;
        };
        let data = fishing::data();
        self.lure = character.saved_lure.min(data.lures.len().saturating_sub(1));
        self.line = character.saved_line.min(data.lines.len().saturating_sub(1));
        if let Some(location) = character.saved_location() {
            self.location = location.id.clone();
        }
    }

    /// Normalize everything an imported config could carry out of range.
    pub fn clamp(&mut self, save: &FishingSave) {
        let data = fishing::data();
        self.character = self.character.min(save.characters.len().saturating_sub(1));
        self.lure = self.lure.min(data.lures.len().saturating_sub(1));
        self.line = self.line.min(data.lines.len().saturating_sub(1));
        if data.location(&self.location).is_none() {
            self.location = data.default_location().id.clone();
        }
        self.hours = if self.hours.is_finite() {
            self.hours.clamp(0.04, 10_000.0)
        } else {
            1.0
        };
        let account = &mut self.account;
        for value in [
            &mut account.efficiency,
            &mut account.exp_multi,
            &mut account.afk_rate,
            &mut account.multi_fish,
            &mut account.speed_bonus,
            &mut account.attack_factor,
            &mut account.prowess,
        ] {
            if !value.is_finite() {
                *value = 0.0;
            }
        }
        for depth in &mut account.equipment_depth {
            if !depth.is_finite() {
                *depth = 0.0;
            }
            *depth = depth.max(0.0);
        }
        account.efficiency = account.efficiency.max(0.0);
        account.exp_multi = account.exp_multi.max(0.0);
        account.afk_rate = account.afk_rate.max(0.0);
        account.multi_fish = account.multi_fish.max(0.0);
        account.speed_bonus = account.speed_bonus.max(0.0);
        account.prowess = account.prowess.max(0.0);
        account.attack_factor = account.attack_factor.clamp(1.0, 3.0);
    }
}

/// The section: the character toolbar, the account inputs, the pool table and
/// the selected pool's split, with the shared formula tree underneath.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let breakpoint = Breakpoint::of(window);
    let save = state.fishing_save.clone();
    state.fishing.clamp(&save);
    let config = state.fishing.clone();
    let evaluation = fishing::evaluate(
        &save,
        &config.account,
        config.character,
        config.lure,
        config.line,
        &config.location,
        config.hours,
    );
    let rows: Vec<Evaluation> = fishing::data()
        .locations
        .iter()
        .map(|location| {
            fishing::evaluate(
                &save,
                &config.account,
                config.character,
                config.lure,
                config.line,
                &location.id,
                config.hours,
            )
        })
        .collect();

    // The shared tree owns its search and its open branches; the section only
    // hands it the tree for the current location.
    let tree = state.fishing_breakdown.clone();
    let root = fishing::breakdown(&evaluation);
    tree.update(cx, |tree, cx| tree.set_root(root, cx));

    let character = evaluation.character.clone();
    let toolkit = format!("{} + {}", evaluation.lure.name, evaluation.line.name);
    let summary = format!(
        "{} · Lv {} · {} · {} · {} · {}h",
        character.name,
        character.level,
        character.rod_name,
        toolkit,
        evaluation.location.label,
        config.hours
    );
    let header = div()
        .id("fishing-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(Badge::new("AFK fishing").tone(Tone::Accent))
        .child(
            div()
                .id("fishing-summary")
                .role(Role::Label)
                .aria_label(summary.clone())
                .min_w_0()
                .child(Caption::new(summary).text_color(cx.theme().colors.fg_muted)),
        )
        .child(div().flex_1())
        .child(toolbar::reset(cx))
        .into_any_element();

    let warnings = warnings(state, &character);
    let toolbar = toolbar::pane(state, &save, breakpoint, cx);
    let account = inputs::card(state, cx);
    let kpis = pools::kpis(&evaluation, config.hours, cx);
    let pool_table = pools::location_table(
        &rows,
        &evaluation.location.id,
        config.hours,
        layout::table_height(f32::from(window.viewport_size().height)),
        cx,
    );
    let detail = detail::card(&evaluation, config.hours, cx);
    let formula = toolbar::formula_card(&state.fishing_breakdown);

    let panes = if breakpoint.is_compact() {
        div().flex().flex_col().gap_3()
    } else {
        div().flex().flex_row().items_start().gap_3()
    };
    let panes = panes
        .child(div().flex_1().min_w_0().child(pool_table))
        .child(
            div()
                .flex_none()
                .when(!breakpoint.is_compact(), |pane| pane.w(px(420.0)))
                .child(detail),
        );

    div()
        .id("fishing-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(toolbar)
        .child(
            ScrollArea::new("fishing-scroll")
                .vertical()
                .shadows()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .pb_4()
                        .when_some(warnings, |page, warnings| page.child(warnings))
                        .child(kpis)
                        .child(panes)
                        .child(account)
                        .child(formula),
                ),
        )
        .into_any_element()
}

/// The missing-toolkit and missing-spot flags the upstream page shows.
fn warnings(state: &AppState, character: &fishing::Character) -> Option<AnyElement> {
    if !state.save_loaded {
        return None;
    }
    let mut flags = Vec::new();
    if !character.toolkit_available {
        flags.push(
            Banner::new(
                "fishing-toolkit-missing",
                Severity::Warning,
                format!(
                    "This save has no PVFishingToolkit_{}; the saved lure and line default to Fly + Fishing Twine.",
                    character.index
                ),
            )
            .into_any_element(),
        );
    }
    if !character.spot_available {
        flags.push(
            Banner::new(
                "fishing-spot-missing",
                Severity::Info,
                format!(
                    "This save has no PVFishingSpotIndex_{}; the saved fishing spot is unknown.",
                    character.index
                ),
            )
            .into_any_element(),
        );
    }
    (!flags.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(flags)
            .into_any_element()
    })
}

/// One location's spot depth bonus as `D1 +25, D2 +5`.
pub(super) fn spot_text(bonus: &[f64; 4]) -> String {
    let parts: Vec<String> = bonus
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0.0)
        .map(|(tier, value)| format!("D{tier} +{value}"))
        .collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adopting_a_save_takes_its_character_toolkit_and_spot() {
        let save = idleon_core::save::Save::new(serde_json::json!({
            "charNames": ["Ana"],
            "Lv0_0": [0, 0, 0, 0, 100],
            "EquipOrder_0": [[], { "2": "FishingRod13" }],
            "PVFishingToolkit_0": [8, 4],
            "PVFishingSpotIndex_0": 1,
            "CurrentMap_0": 61,
            "AFKtarget_0": "FishMed",
        }));
        let fishing = FishingSave::from_save(&save);
        let mut state = State::default();
        state.adopt(&fishing);
        assert_eq!(state.character, 0);
        assert_eq!((state.lure, state.line), (8, 4));
        assert_eq!(state.location, "61:1");
    }

    #[test]
    fn the_claim_length_and_indices_stay_in_range() {
        let mut state = State {
            hours: f64::NAN,
            lure: 99,
            line: 99,
            location: "nope".into(),
            account: AccountInputs {
                attack_factor: 99.0,
                ..AccountInputs::default()
            },
            ..State::default()
        };
        state.clamp(&FishingSave::default());
        assert_eq!(state.hours, 1.0);
        assert_eq!(state.lure, 14);
        assert_eq!(state.line, 14);
        assert_eq!(state.location, fishing::data().default_location().id);
        assert_eq!(state.account.attack_factor, 3.0);
    }
}
