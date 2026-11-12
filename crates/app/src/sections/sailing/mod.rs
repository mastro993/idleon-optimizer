//! The Sailing Calculator: the upstream fleet, ship, captain, chest, artifact and
//! breakdown tabs.
//!
//! The upstream `sailing-calc.html` has fleet, levels, captains, odds, simulation,
//! artifacts and formula tabs. SL-01 built the fleet assignment, SL-02 the ship
//! breakpoints, captain quality and replacement matrix, SL-03 the chest
//! outcomes, the artifact ranking and the four breakdowns, and SL-04 the fleet
//! pile simulation.
//!
//! The assignment controls and the header stay above the strip because every tab
//! reads the same island, goal and fleet mode, as upstream's own layout does.

mod actions;

mod artifacts;
mod breakdowns;
mod captains;
mod fleet;
mod odds;
mod replacements;
mod ships;
pub(crate) mod simulation;

use crate::state::AppState;
use crate::ui::{formula_tree, layout};
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use idleon_sailing::{
    AssignmentMode, DEFAULT_HOURS, DEFAULT_TRIALS, Goal, captain_bonus_name, island_artifact_count,
    island_distance,
};
use idleon_ui::formula::FormulaTree;

/// The section's own state, kept across dashboard visits.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) tab: Tab,
    pub(crate) mode: AssignmentMode,
    pub(crate) island: usize,
    pub(crate) goal: Goal,
    /// Sort the ship tab by the lowest breakpoint ETA instead of by boat.
    pub(crate) sort_by_eta: bool,
    /// The boat whose breakdowns the formula tab draws.
    pub(crate) breakdown_boat: usize,
    /// The destination the pile simulation runs on.
    pub(crate) sim_island: usize,
    /// The interval each simulation trial covers.
    pub(crate) sim_hours: f64,
    /// How many simulation trials to run.
    pub(crate) sim_trials: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Fleet,
            mode: AssignmentMode::Optimized,
            island: 0,
            goal: Goal::Treasure,
            sort_by_eta: false,
            breakdown_boat: 0,
            sim_island: 0,
            sim_hours: DEFAULT_HOURS,
            sim_trials: DEFAULT_TRIALS,
        }
    }
}

/// The four formula trees the breakdowns tab owns.
pub(crate) struct Trees {
    pub(crate) loot: Entity<FormulaTree>,
    pub(crate) speed: Entity<FormulaTree>,
    pub(crate) artifact: Entity<FormulaTree>,
    pub(crate) chest: Entity<FormulaTree>,
}

impl Trees {
    /// Create the trees; the tab replaces their roots each render.
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            loot: formula_tree("sailing-loot-formula", window, cx),
            speed: formula_tree("sailing-speed-formula", window, cx),
            artifact: formula_tree("sailing-artifact-formula", window, cx),
            chest: formula_tree("sailing-chest-formula", window, cx),
        }
    }
}

/// The sailing page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// Current versus optimized captain-to-boat pairings.
    #[default]
    Fleet,
    /// Ship loot and speed breakpoints, costs and ETAs.
    Ships,
    /// Captain roll quality, EXP and leveling.
    Captains,
    /// The strongest-source replacement matrix.
    Replacements,
    /// Fleet chest outcomes, per-boat tier chances and the near-full pile.
    #[serde(rename = "odds")]
    ChestOdds,
    /// The fleet pile simulation with progress and results.
    Simulation,
    /// The artifact route ranking and the selected island's artifacts.
    Artifacts,
    /// The loot, trip, artifact and chest formula trees.
    #[serde(rename = "formula")]
    Breakdowns,
}

impl Tab {
    pub(crate) const ALL: [Tab; 8] = [
        Tab::Fleet,
        Tab::Ships,
        Tab::Captains,
        Tab::Replacements,
        Tab::ChestOdds,
        Tab::Simulation,
        Tab::Artifacts,
        Tab::Breakdowns,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Fleet => "fleet",
            Tab::Ships => "ships",
            Tab::Captains => "captains",
            Tab::Replacements => "replacements",
            Tab::ChestOdds => "odds",
            Tab::Simulation => "simulation",
            Tab::Artifacts => "artifacts",
            Tab::Breakdowns => "formula",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Fleet => "Fleet",
            Tab::Ships => "Ships",
            Tab::Captains => "Captains",
            Tab::Replacements => "Replacements",
            Tab::ChestOdds => "Chest Odds",
            Tab::Simulation => "Fleet Simulation",
            Tab::Artifacts => "Artifacts",
            Tab::Breakdowns => "Breakdowns",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The sailing section: header, fleet controls, the tab strip and the active tab.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if state.fleet.boats.is_empty() {
        return empty(cx);
    }
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let tab = state.sailing_state.tab;
    let body = match tab {
        Tab::Fleet => fleet::fleet_tab(state, table_height, cx),
        Tab::Ships => ships::ships_tab(state, table_height, cx),
        Tab::Captains => captains::captains_tab(state, table_height, cx),
        Tab::Replacements => replacements::replacements_tab(state, table_height, cx),
        Tab::ChestOdds => odds::odds_tab(state, table_height, cx),
        Tab::Simulation => simulation::simulation_tab(state, table_height, cx),
        Tab::Artifacts => artifacts::artifacts_tab(state, table_height, cx),
        Tab::Breakdowns => breakdowns::breakdowns_tab(state, table_height, cx),
    };
    div()
        .id("sailing-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header(state, cx))
        .child(controls(state, cx))
        .child(tabs(state, cx))
        .child(
            div()
                .id(("sailing-pane", tab as usize))
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

/// The upstream tab strip, driven by the state's own tab.
fn tabs(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs =
        TabBar::new("sailing-tabs").selected(SharedString::from(state.sailing_state.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_sailing_tab(tab, cx));
        }
    })
}

fn header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let island = state.sailing_state.island;
    let summary = format!(
        "Island {} · {} boats · {} captains",
        island + 1,
        state.fleet.boats.len(),
        state.fleet.captains.len()
    );
    div()
        .id("sailing-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(Badge::new("Sailing").tone(Tone::Accent))
        .child(
            Caption::new("Chest outcomes, artifacts and breakdowns from the save fleet.")
                .text_color(muted),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("sailing-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .child(Caption::new(summary).text_color(muted)),
        )
        .into_any_element()
}

/// The shared fleet controls: everything below reads them.
fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mode = state.sailing_state.mode;
    let optimized = entity.clone();
    let saved = entity.clone();
    let mode_control = div()
        .id("fleet-mode-control")
        .flex()
        .items_center()
        .gap_1()
        .child(
            Button::new("fleet-mode-optimized", "Optimized")
                .variant(if mode == AssignmentMode::Optimized {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    optimized.update(cx, |state, cx| {
                        state.set_fleet_mode(AssignmentMode::Optimized, cx);
                    });
                }),
        )
        .child(
            Button::new("fleet-mode-saved", "Current save")
                .variant(if mode == AssignmentMode::Saved {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    saved.update(cx, |state, cx| {
                        state.set_fleet_mode(AssignmentMode::Saved, cx);
                    });
                }),
        );

    let island_entity = entity.clone();
    let available = state.fleet.available_islands().max(1);
    let islands: Vec<Choice> = (0..available)
        .map(|island| {
            Choice::new(
                island.to_string(),
                format!("Island {} · {} dist", island + 1, island_distance(island)),
            )
            .note(format!("{} artifacts", island_artifact_count(island)))
        })
        .collect();
    let island_select = Select::new("fleet-island", islands)
        .label("Fleet destination")
        .selected(state.sailing_state.island.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(island) = value.parse::<usize>() {
                island_entity.update(cx, |state, cx| state.set_fleet_island(island, cx));
            }
        });

    let goal_entity = entity.clone();
    let goal_control = SegmentedControl::new("fleet-goal", state.sailing_state.goal.id())
        .size(ControlSize::Sm)
        .segment("treasure", "Loot value", None)
        .segment("artifact", "Artifacts", None)
        .segment("chests", "Chest rate", None)
        .on_change(move |value, _window, cx| {
            if let Some(goal) = Goal::from_id(value.as_ref()) {
                goal_entity.update(cx, |state, cx| state.set_fleet_goal(goal, cx));
            }
        });

    let tier_entity = entity.clone();
    let tiers: Vec<Choice> = (0..5)
        .map(|tier| Choice::new(tier.to_string(), format!("Tier {tier}+")))
        .collect();
    let tier_select = Select::new("fleet-min-tier", tiers)
        .label("Minimum chest")
        .selected(state.sailing.minimum_tier.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(tier) = value.parse::<u32>() {
                tier_entity.update(cx, |state, cx| state.set_fleet_minimum_tier(tier, cx));
            }
        });

    Card::new()
        .header(CardHeader::new("Fleet controls").description(
            "The selected island applies to every pairing. Current save keeps each boat's captain, not its stored destination.",
        ))
        .child(
            div()
                .id("fleet-controls-row")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .p_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Fleet assignment"))
                        .child(mode_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(gpui::px(220.0))
                        .child(Caption::new("Fleet destination"))
                        .child(island_select),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Optimize for"))
                        .child(goal_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(gpui::px(140.0))
                        .child(Caption::new("Minimum chest"))
                        .child(tier_select),
                ),
        )
        .into_any_element()
}

/// A save without boats has no fleet: say so and offer the shared import.
fn empty(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(CardHeader::new("Fleet assignment"))
        .child(
            EmptyState::new("fleet-empty", IconName::Map, "No sailing fleet loaded")
                .body(
                    "Import a save with boats and captains to compare the current pairings with \
                     the optimizer.",
                )
                .action(
                    Button::new("fleet-import", "Import a save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            entity.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

/// One card holding a string value beside its label and caption.
pub(super) fn text_kpi(
    id: &'static str,
    label: &'static str,
    value: &str,
    caption: &str,
    muted: gpui::Hsla,
) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(gpui::Role::Label)
                .aria_label(value.to_string())
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .child(Caption::new(label).text_color(muted))
                .child(div().text_size(gpui::px(20.0)).child(value.to_string()))
                .child(Caption::new(caption.to_string()).text_color(muted)),
        )
        .into_any_element()
}

/// A, B, … Z, AA, AB, … as upstream labels captains.
pub(super) fn captain_code(index: usize) -> String {
    let mut value = index + 1;
    let mut code = String::new();
    while value > 0 {
        value -= 1;
        code.insert(0, (b'A' + (value % 26) as u8) as char);
        value /= 26;
    }
    code
}

/// `Captain B · Lv20`, the fleet table's short label.
pub(super) fn captain_short(state: &AppState, captain: usize) -> String {
    let Some(entry) = state.fleet.captains.get(captain) else {
        return "—".into();
    };
    format!("{} · Lv{}", captain_code(captain), entry.level)
}

/// The tooltip text under a captain label.
pub(super) fn captain_details(state: &AppState, captain: usize) -> String {
    let Some(entry) = state.fleet.captains.get(captain) else {
        return "No captain".into();
    };
    let bonuses: Vec<String> = entry
        .bonus_types
        .iter()
        .zip(entry.rolls)
        .filter(|(kind, _)| **kind >= 0)
        .map(|(kind, roll)| format!("{} +{}%", captain_bonus_name(*kind), roll))
        .collect();
    let code = captain_code(captain);
    if bonuses.is_empty() {
        format!("Captain {code} · Lv{}", entry.level)
    } else {
        format!(
            "Captain {code} · Lv{} · {}",
            entry.level,
            bonuses.join(" / ")
        )
    }
}

/// A percentage as the upstream `pct` prints it, three decimals.
pub(super) fn percent(value: f64) -> String {
    format!("{:.3}%", 100.0 * value)
}

/// The upstream `chestOdds`: never, a probability at its own precision, or a
/// reciprocal when it is tiny.
pub(super) fn chest_odds(probability: f64) -> String {
    if probability <= 0.0 {
        return "Never".into();
    }
    if probability < 0.00001 {
        return format!("1 in {}", idleon_ui::format::fmt_val(1.0 / probability));
    }
    let digits = if probability >= 0.1 {
        1
    } else if probability >= 0.01 {
        2
    } else if probability >= 0.0001 {
        3
    } else {
        5
    };
    format!("{:.*}%", digits, 100.0 * probability)
}

/// The upstream `chance`: a percentage, or a reciprocal below a tenth of a percent.
pub(super) fn chance(probability: f64) -> String {
    if probability <= 0.0 {
        return "—".into();
    }
    if probability >= 1.0 {
        return "100%".into();
    }
    if probability >= 0.001 {
        return percent(probability);
    }
    format!("1 in {}", idleon_ui::format::fmt_val(1.0 / probability))
}

/// A trip time as seconds, minutes or hours.
pub(super) fn trip(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "—".into();
    }
    if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.2}h", seconds / 3600.0)
    }
}

/// An artifact find rate, reading as a reciprocal once it is tiny.
pub(super) fn artifacts(value: f64) -> String {
    if value <= 0.0 {
        "—".into()
    } else if value >= 0.01 {
        format!("{value:.2}")
    } else if value >= 1e-4 {
        format!("{value:.4}")
    } else {
        format!("1 in {}", idleon_ui::format::rate(1.0 / value))
    }
}

/// A signed percentage gain.
pub(super) fn gain(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value.abs() < 0.005 {
        "0.00%".into()
    } else {
        format!("{value:+.2}%")
    }
}

/// An hours-to-event as a projection, `Ready` once it is already paid for.
pub(super) fn hours_or_ready(hours: f64) -> String {
    if hours <= 0.0 {
        "Ready".into()
    } else if hours.is_finite() {
        idleon_ui::format::projection(hours)
    } else {
        "—".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captain_codes_match_the_upstream_sequence() {
        assert_eq!(captain_code(0), "A");
        assert_eq!(captain_code(25), "Z");
        assert_eq!(captain_code(26), "AA");
        assert_eq!(captain_code(27), "AB");
    }

    #[test]
    fn trip_times_read_in_seconds_minutes_and_hours() {
        assert_eq!(trip(45.0), "45s");
        assert_eq!(trip(90.0), "1.5m");
        assert_eq!(trip(7200.0), "2.00h");
        assert_eq!(trip(f64::INFINITY), "—");
    }

    #[test]
    fn chances_read_as_the_upstream_prints_them() {
        assert_eq!(chance(0.0), "—");
        assert_eq!(chance(1.0), "100%");
        assert_eq!(chance(0.433), "43.300%");
        assert_eq!(chance(8e-6), "1 in 125.0K");
        assert_eq!(chest_odds(0.0), "Never");
        assert_eq!(chest_odds(0.675), "67.5%");
        assert_eq!(chest_odds(1.0), "100.0%");
        assert_eq!(chest_odds(0.05525), "5.53%");
        assert_eq!(chest_odds(0.0003737), "0.037%");
        assert_eq!(chest_odds(0.00000001625), "1 in 61.54M");
    }

    #[test]
    fn tabs_round_trip_through_their_ids() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
    }

    #[test]
    fn gains_and_etas_read_without_noise() {
        assert_eq!(gain(21.8007531667), "+21.80%");
        assert_eq!(gain(0.0), "0.00%");
        assert_eq!(gain(-3.0), "-3.00%");
        assert_eq!(hours_or_ready(0.0), "Ready");
        assert_eq!(hours_or_ready(f64::INFINITY), "—");
    }
}
