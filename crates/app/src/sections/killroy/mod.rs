//! The Killroy Calculator: the weekly forecast, the `+1` upgrade comparison and
//! the run's results.
//!
//! Ported from `js/ui/killroy.js` and
//! `js/stats/systems/common/killroy-planner.js`. KL-01 ported the forecast; KL-02
//! added the Baseline and Upgrade comparisons tabs, the upgrade list, the
//! selection preview, the trials and the comparison that highlights the best
//! option; KL-03 adds the run options (saved buffs and unlimited inventory), the
//! saved-buff table, the results export and the assumptions the numbers rest on.
//! The shared [`idleon_ui::runner::TaskRunner`] runs the trials off the UI thread.

mod comparison;
mod forecast_view;
mod state;

pub(crate) use state::KillroyOutcome;

use crate::state::AppState;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};
use idleon_killroy::{self as killroy, Selection};

/// The section's two halves, as the upstream tab strip splits them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Mode {
    /// The forecast and the baseline selection.
    #[default]
    Baseline,
    /// Every upgrade compared against its baseline.
    Upgrades,
}

impl Mode {
    pub(crate) const ALL: [Mode; 2] = [Mode::Baseline, Mode::Upgrades];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Mode::Baseline => "baseline",
            Mode::Upgrades => "upgrades",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Baseline => "Baseline",
            Mode::Upgrades => "Upgrade comparisons",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }
}

/// The section's settings, saved with the project.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// Upcoming weeks the forecast covers, 1-52.
    pub(crate) weeks: usize,
    pub(crate) mode: Mode,
    /// Trials per configuration, 1-20.
    pub(crate) trials: usize,
    /// The base seed the trial seeds step from.
    pub(crate) seed: u64,
    /// The combat simulator stand-in: how many monsters the character kills a
    /// second.
    pub(crate) kills_per_second: f64,
    /// Start with the save's `BuffsActive` timers instead of a fresh map entry.
    pub(crate) keep_buffs: bool,
    /// Lift the bag-space limit on the run's pickups.
    pub(crate) unlimited_inventory: bool,
    /// The weeks the run covers; this week is checked by default.
    pub(crate) checked_weeks: Vec<usize>,
    /// The encounters the run covers.
    pub(crate) selection: Vec<Selection>,
    /// The upgrade ids the comparison tests; every upgrade by default.
    pub(crate) upgrades: Vec<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            weeks: 12,
            mode: Mode::Baseline,
            trials: 3,
            seed: 1,
            kills_per_second: 3.0,
            keep_buffs: false,
            unlimited_inventory: false,
            checked_weeks: vec![0],
            selection: Vec::new(),
            upgrades: (0..killroy::UPGRADES).collect(),
        }
    }
}

impl State {
    /// The settings with every field inside the range the plan accepts.
    pub(crate) fn normalized(mut self) -> Self {
        self.weeks = self.weeks.clamp(1, 52);
        self.trials = self.trials.clamp(1, 20);
        self.seed = self.seed.max(1);
        self.kills_per_second = if self.kills_per_second.is_finite() {
            self.kills_per_second.clamp(0.0, 100.0)
        } else {
            State::default().kills_per_second
        };
        self.checked_weeks.retain(|week| *week <= self.weeks);
        self.checked_weeks.sort_unstable();
        self.checked_weeks.dedup();
        if self.checked_weeks.is_empty() {
            self.checked_weeks.push(0);
        }
        self.selection.sort_unstable();
        self.selection.dedup();
        self.upgrades.retain(|id| *id < killroy::UPGRADES);
        self.upgrades.sort_unstable();
        self.upgrades.dedup();
        self
    }

    /// Whether the run covers a selection of this week and slot.
    pub(crate) fn selected(&self, week_offset: usize, ci: usize, slot: usize) -> bool {
        let key = Selection {
            week_offset,
            ci,
            slot,
        };
        self.selection.contains(&key)
    }

    /// Whether a week is checked for the run.
    pub(crate) fn week_checked(&self, week_offset: usize) -> bool {
        self.checked_weeks.contains(&week_offset)
    }

    /// Whether the comparison tests an upgrade.
    pub(crate) fn compares(&self, id: usize) -> bool {
        self.mode == Mode::Upgrades && self.upgrades.contains(&id)
    }
}

/// The section as the registry opens it.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mode = state.killroy.mode;
    let entity = cx.entity();
    let tabs = TabBar::new("killroy-tabs")
        .selected(SharedString::from(mode.id()))
        .tab(WindowTab::new(Mode::Baseline.id(), Mode::Baseline.label()))
        .tab(WindowTab::new(Mode::Upgrades.id(), Mode::Upgrades.label()))
        .on_select(move |id, _window, cx| {
            if let Some(mode) = Mode::from_id(id.as_ref()) {
                entity.update(cx, |state, cx| state.set_killroy_mode(mode, cx));
            }
        });

    let header = div()
        .id("killroy-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(div().flex_1())
        .child(
            Caption::new(if mode == Mode::Baseline {
                "Imported save only. Check characters to queue their week."
            } else {
                "Imported save only. Choose characters and +1 upgrades to compare."
            })
            .text_color(cx.theme().colors.fg_muted),
        );

    let save = state.save.clone();
    let body = match save.as_ref() {
        None => forecast_view::empty_state(window, cx),
        Some(save) => match killroy::forecast(save, state.killroy.weeks) {
            Err(error) => forecast_view::error_state(error.to_string(), window, cx),
            Ok(forecast) => {
                let primary = match mode {
                    Mode::Baseline => forecast_view::forecast_card(state, &forecast, window, cx),
                    Mode::Upgrades => comparison::upgrade_card(state, save, cx),
                };
                let primary = div()
                    .id("killroy-primary")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(primary)
                    .child(comparison::results_card(state, window, cx))
                    .child(comparison::buffs_card(state, save, cx));
                let mut side = div()
                    .id("killroy-rail")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(comparison::run_card(state, cx));
                if mode == Mode::Baseline {
                    side = side.child(forecast_view::rotation_card(
                        &forecast,
                        crate::ui::layout::table_height(f32::from(window.viewport_size().height)),
                        cx,
                    ));
                }
                div()
                    .id("killroy-pane")
                    .flex()
                    .size_full()
                    .min_h_0()
                    .child(crate::ui::layout::split_pane(
                        "killroy-body",
                        crate::ui::layout::Breakpoint::of(window),
                        primary,
                        side,
                        gpui::px(360.0),
                    ))
                    .into_any_element()
            }
        },
    };

    div()
        .id("killroy-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(body)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_ids_match_the_tabs() {
        for mode in Mode::ALL {
            assert_eq!(Mode::from_id(mode.id()), Some(mode));
        }
        assert_eq!(Mode::from_id("saved"), None);
    }

    #[test]
    fn normalization_keeps_every_setting_in_range() {
        let state = State {
            weeks: 3,
            trials: 99,
            seed: 0,
            kills_per_second: f64::NAN,
            checked_weeks: vec![9, 3, 3],
            selection: vec![],
            upgrades: vec![9, 1, 1],
            ..State::default()
        }
        .normalized();
        assert_eq!(state.weeks, 3);
        assert_eq!(state.trials, 20);
        assert_eq!(state.seed, 1);
        assert_eq!(state.kills_per_second, 3.0);
        assert_eq!(state.checked_weeks, vec![3]);
        assert_eq!(state.upgrades, vec![1]);
    }

    #[test]
    fn the_horizon_and_the_checked_weeks_clamp() {
        let state = State {
            weeks: 0,
            checked_weeks: vec![0, 7],
            ..State::default()
        }
        .normalized();
        assert_eq!(state.weeks, 1);
        assert_eq!(state.checked_weeks, vec![0]);
        let state = State {
            weeks: 99,
            checked_weeks: Vec::new(),
            ..State::default()
        }
        .normalized();
        assert_eq!(state.weeks, 52);
        assert_eq!(
            state.checked_weeks,
            vec![0],
            "no checked week keeps this one"
        );
    }

    #[test]
    fn the_run_options_default_to_a_fresh_map_entry() {
        let state = State::default();
        assert!(!state.keep_buffs);
        assert!(!state.unlimited_inventory);
        // The options survive a project round trip with the rest of the config.
        let text = serde_json::to_string(&State {
            keep_buffs: true,
            unlimited_inventory: true,
            ..State::default()
        })
        .expect("the state serializes");
        let back: State = serde_json::from_str(&text).expect("the state deserializes");
        assert!(back.keep_buffs);
        assert!(back.unlimited_inventory);
        let normalized = back.normalized();
        assert!(normalized.keep_buffs);
        assert!(normalized.unlimited_inventory);
    }

    #[test]
    fn the_comparison_only_tests_upgrades_on_its_own_tab() {
        let mut state = State {
            upgrades: vec![2],
            ..State::default()
        };
        assert!(!state.compares(2));
        state.mode = Mode::Upgrades;
        assert!(state.compares(2));
        assert!(!state.compares(0));
    }
}
