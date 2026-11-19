//! The Fishing Calculator section: AFK catches, the fish split and the real
//! versus panel EXP of every pool, the all-combination lure/line ranking and the
//! Trench depth projection.
//!
//! FI-01 builds the toolbar, the character context and the per-pool outcome;
//! FI-02 adds the lure/line ranking over all 225 combinations; FI-03 adds the
//! Trench depth projection, the fastest lure + line per projected depth and the
//! other characters fishing alongside. The account values that come from the
//! skill systems are entered in the account card until those systems are
//! decoded.

mod actions;

mod controls;
pub(crate) mod detail;
pub(crate) mod inputs;
pub(crate) mod pools;
pub(crate) mod ranking;
mod table;
pub(crate) mod toolbar;
pub(crate) mod trench;

pub(crate) use controls::Controls;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    px,
};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_fishing::{self as fishing, AccountInputs, FishingSave, HelperMode, Objective};
use idleon_ui::widgets::{Caption, Tone};

/// The section's live state: the character, toolkit, location, hours and the
/// account values the skill systems would supply.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) character: usize,
    pub(crate) lure: usize,
    pub(crate) line: usize,
    /// The location id, `map:spot` or `x:pool`.
    pub(crate) location: String,
    /// Hours away; the AFK claim length.
    pub(crate) hours: f64,
    /// What the lure/line ranking sorts by.
    pub(crate) objective: Objective,
    pub(crate) account: AccountInputs,
    /// How many Trench depths the depth projection covers.
    pub(crate) trench_depths: usize,
    /// How the other Trench characters choose their lure and line.
    pub(crate) trench_helper_mode: HelperMode,
    /// One tick per character: the helpers assumed to fish the Trench. `None`
    /// until a save seeds it from the characters' saved targets.
    pub(crate) trench_helpers: Option<Vec<bool>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            character: 0,
            lure: 0,
            line: 0,
            location: fishing::data().default_location().id.clone(),
            hours: 1.0,
            objective: Objective::default(),
            account: AccountInputs::default(),
            trench_depths: 5,
            trench_helper_mode: HelperMode::Saved,
            trench_helpers: None,
        }
    }
}

impl State {
    /// Take a character's saved toolkit, spot and level after an import, and
    /// tick the characters already left on the Trench.
    pub(crate) fn adopt(&mut self, save: &FishingSave) {
        self.character = save.default_character();
        self.character_defaults(save);
        self.trench_helpers = Some(
            save.characters
                .iter()
                .map(|character| character.afk_target == fishing::TRENCH_POOL)
                .collect(),
        );
    }

    /// Tick or untick one helper, keeping the list aligned with the roster.
    pub(crate) fn set_helper(&mut self, index: usize, on: bool, characters: usize) {
        let helpers = self
            .trench_helpers
            .get_or_insert_with(|| vec![false; characters]);
        if helpers.len() < characters {
            helpers.resize(characters, false);
        }
        if let Some(slot) = helpers.get_mut(index) {
            *slot = on;
        }
    }

    /// Reset the toolkit and location to the selected character's saved ones.
    pub(crate) fn character_defaults(&mut self, save: &FishingSave) {
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

    /// Apply a ranked lure/line pair to the toolbar, clamped like the selectors.
    pub(crate) fn use_toolkit(&mut self, lure: usize, line: usize) {
        let data = fishing::data();
        self.lure = lure.min(data.lures.len().saturating_sub(1));
        self.line = line.min(data.lines.len().saturating_sub(1));
    }

    /// Normalize everything an imported config could carry out of range.
    pub(crate) fn clamp(&mut self, save: &FishingSave) {
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
        self.trench_depths = self.trench_depths.clamp(1, 25);
        if let Some(helpers) = self.trench_helpers.as_mut() {
            helpers.resize(save.characters.len(), false);
        }
    }
}

/// What the page shows for one [`controls::Key`]: every pool, the selected
/// pool's split and the ranking's summary, formatted once.
pub(crate) struct Page {
    summary: SharedString,
    /// The selected character's index and whether the save carried its
    /// toolkit and spot.
    character: (usize, bool, bool),
    /// The rod's item key and its `name · speed` line.
    rod: Option<(String, SharedString, SharedString)>,
    kpis: pools::Kpis,
    locations: pools::Locations,
    detail: detail::View,
    ranking: ranking::Summary,
}

/// Evaluate the selected pool, every other pool and all 225 combinations once,
/// fill the ranking table and hand the formula tree its root.
fn build(state: &AppState, cx: &mut App) -> Page {
    let save = &state.fishing_save;
    let config = &state.fishing;
    let evaluation = fishing::evaluate(
        save,
        &config.account,
        config.character,
        config.lure,
        config.line,
        &config.location,
        config.hours,
    );
    let rows: Vec<fishing::Evaluation> = fishing::data()
        .locations
        .iter()
        .map(|location| {
            fishing::evaluate(
                save,
                &config.account,
                config.character,
                config.lure,
                config.line,
                &location.id,
                config.hours,
            )
        })
        .collect();
    let ranked = fishing::rank(
        save,
        &config.account,
        config.character,
        &config.location,
        config.objective,
        config.hours,
    );
    let root = fishing::breakdown(&evaluation);
    state
        .fishing_breakdown
        .update(cx, |tree, cx| tree.set_root(root, cx));
    let ranking = ranking::fill(state, &ranked, &evaluation, cx);

    let character = &evaluation.character;
    let summary = format!(
        "{} · Lv {} · {} · {} + {} · {} · {}h",
        character.name,
        character.level,
        character.rod_name,
        evaluation.lure.name,
        evaluation.line.name,
        evaluation.location.label,
        config.hours
    );
    let rod = save.character(config.character).map(|character| {
        (
            character.rod_key.clone(),
            SharedString::from(character.rod_name.clone()),
            SharedString::from(format!(
                "{} · speed {}",
                character.rod_name, character.rod_speed
            )),
        )
    });
    Page {
        summary: summary.into(),
        character: (
            character.index,
            character.toolkit_available,
            character.spot_available,
        ),
        rod,
        kpis: pools::kpis(&evaluation, &ranked, config.objective, config.hours),
        locations: pools::locations(&rows, &evaluation.location.id, config.hours),
        detail: detail::view(&evaluation, config.hours),
        ranking,
    }
}

/// The section: the character toolbar, the account inputs, the pool table and
/// the selected pool's split, with the shared formula tree underneath.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let breakpoint = Breakpoint::of(window, cx);
    let save = std::mem::take(&mut state.fishing_save);
    state.fishing.clamp(&save);
    state.fishing_save = save;
    let key = state.fishing_key();
    let page = controls::cached(state, |controls| &mut controls.page, key, cx, build);
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let muted = cx.theme().muted_foreground;

    let header = h_flex()
        .id("fishing-header")
        .flex_none()
        .gap_3()
        .child(table::tag("AFK fishing", Tone::Accent))
        .child(
            div()
                .id("fishing-summary")
                .role(Role::Label)
                .aria_label(page.summary.clone())
                .min_w_0()
                .child(Caption::new(page.summary.clone()).text_color(muted)),
        )
        .child(div().flex_1())
        .child(toolbar::reset(cx));

    let warnings = warnings(state, &page);
    let toolbar = toolbar::pane(state, &page, window, cx);
    let account = inputs::card(state, window, cx);
    let detail = detail::card(&page.detail, cx);
    let pool_table = pools::location_table(&page.locations, cx);
    let ranking = ranking::card(state, &page.ranking, table_height, window, cx);
    let trench = trench::card(state, window, cx);
    let formula = toolbar::formula_card(&state.fishing_breakdown);

    let panes = if breakpoint.is_compact() {
        v_flex().gap_3()
    } else {
        h_flex().items_start().gap_3()
    };
    let panes = panes
        .child(div().flex_1().min_w_0().child(pool_table))
        .child(
            div()
                .flex_none()
                .when(!breakpoint.is_compact(), |pane| pane.w(px(420.0)))
                .child(detail),
        );

    v_flex()
        .id("fishing-section")
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(toolbar)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(
                    v_flex()
                        .gap_3()
                        .pb_4()
                        .when_some(warnings, |page, warnings| page.child(warnings))
                        .child(pools::kpis_view(&page.kpis))
                        .child(panes)
                        .child(ranking)
                        .child(trench)
                        .child(account)
                        .child(formula),
                )
                .overflow_y_scrollbar()
                .id("fishing-scroll"),
        )
        .into_any_element()
}

/// The missing-toolkit and missing-spot flags.
fn warnings(state: &AppState, page: &Page) -> Option<AnyElement> {
    if !state.save_loaded {
        return None;
    }
    let (index, toolkit, spot) = page.character;
    let mut flags = Vec::new();
    if !toolkit {
        flags.push(
            Alert::new(
                "fishing-toolkit-missing",
                format!(
                    "This save has no PVFishingToolkit_{index}; the saved lure and line default to Fly + Fishing Twine."
                ),
            )
            .with_variant(Tone::Warning.into())
            .banner()
            .into_any_element(),
        );
    }
    if !spot {
        flags.push(
            Alert::new(
                "fishing-spot-missing",
                format!(
                    "This save has no PVFishingSpotIndex_{index}; the saved fishing spot is unknown."
                ),
            )
            .with_variant(Tone::Info.into())
            .banner()
            .into_any_element(),
        );
    }
    (!flags.is_empty()).then(|| v_flex().gap_2().children(flags).into_any_element())
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
        let save = idleon_save::Save::new(serde_json::json!({
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
    fn a_ranked_toolkit_lands_in_the_toolbar() {
        let mut state = State::default();
        state.use_toolkit(9, 4);
        assert_eq!((state.lure, state.line), (9, 4));
        state.use_toolkit(99, 99);
        assert_eq!((state.lure, state.line), (14, 14));
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

    #[test]
    fn adopting_a_save_ticks_the_characters_left_on_the_trench() {
        let save = idleon_save::Save::new(serde_json::json!({
            "charNames": ["Ana", "Bo", "Cy"],
            "Lv0_0": [0, 0, 0, 0, 100],
            "Lv0_1": [0, 0, 0, 0, 80],
            "Lv0_2": [0, 0, 0, 0, 60],
            "AFKtarget_0": "FishBig",
            "AFKtarget_1": "MotherlodeFISH",
            "AFKtarget_2": "FishSmall",
        }));
        let fishing = FishingSave::from_save(&save);
        let mut state = State::default();
        state.adopt(&fishing);
        assert_eq!(state.trench_helpers, Some(vec![false, true, false]));
        assert_eq!(state.trench_depths, 5);
    }

    #[test]
    fn a_helper_tick_and_the_projection_length_stay_in_range() {
        let mut state = State {
            trench_depths: 99,
            ..State::default()
        };
        state.trench_helpers = Some(vec![false; 2]);
        state.set_helper(1, true, 2);
        let save = FishingSave::default();
        state.clamp(&save);
        assert_eq!(
            state.trench_helpers,
            Some(Vec::new()),
            "an empty roster resizes"
        );
        assert_eq!(state.trench_depths, 25);
        state.trench_depths = 0;
        state.clamp(&save);
        assert_eq!(state.trench_depths, 1);
        // Ticking past the roster does not grow it, and the default is Saved.
        state.set_helper(3, true, 2);
        assert!(!state.trench_helpers.as_ref().unwrap().contains(&true));
        assert_eq!(state.trench_helper_mode, HelperMode::Saved);
    }
}
