//! The Forecast tab: the projection over native collects, the agenda of what
//! each collect would commit, and the currency track.
//!
//! The engine is `idleon_royal_guardian::forecast`; this pane places collects
//! and menu-open windows, shows the summary, the agenda and the currency
//! balances. There is no timeline chart: the agenda lists the same events with
//! their hours.

use super::overview::{figure_text, kpi};
use super::table::{table, text, text_id};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use idleon_royal_guardian::{Forecast, ForecastOptions, HORIZON, RoyalGuardian};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, TableColumn};

/// The horizons the selector offers, in hours, with their labels.
const HORIZONS: [(u32, &str); 5] = [
    (12, "12h"),
    (24, "24h"),
    (72, "3d"),
    (168, "7d"),
    (336, "14d"),
];

/// The horizon selector's options.
pub(super) fn horizon_choices() -> Vec<Choice<u32>> {
    HORIZONS
        .iter()
        .map(|(hours, label)| Choice::new(*hours, *label))
        .collect()
}

/// A read of a field as an hour, or `None` when it is blank or invalid.
pub(crate) fn hours(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// The Forecast pane.
pub(crate) fn forecast(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let Some(result) = state.royal_forecast.as_ref() else {
        return div()
            .id("royal-forecast-empty")
            .child(Caption::new("The forecast could not run on this save."))
            .into_any_element();
    };
    v_flex()
        .id("royal-forecast")
        .gap(PANEL_GAP)
        .child(controls(state, window, cx))
        .child(summary(result))
        .child(agenda(royal, result, cx))
        .child(currency(result, cx))
        .child(resets(result, cx))
        .into_any_element()
}

/// The horizon and the collect and window fields.
fn controls(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let controls = &state.royal_controls;
    let selected = state.royal_state.forecast_hours.round() as u32;
    controls
        .horizon
        .show(horizon_choices(), Some(&selected), window, cx);
    // A kit select grows in a row; the box keeps it to its width.
    let horizon = div().flex_none().w(px(96.0)).child(
        Select::new(controls.horizon.state())
            .id("royal-forecast-horizon")
            .accessibility_label("Horizon")
            .small(),
    );
    let add_collect = cx.entity();
    let add_window = cx.entity();
    h_flex()
        .id("royal-forecast-controls")
        .gap_2()
        .child(horizon)
        .child(hours_field(
            "royal-forecast-collect",
            &controls.collect,
            "Collect at hour",
        ))
        .child(
            Button::new("royal-forecast-add-collect")
                .label("Add collect")
                .icon(IconName::Plus)
                .small()
                .on_click(move |_event, _window, cx| {
                    add_collect.update(cx, |state, cx| state.add_royal_collect(cx));
                }),
        )
        .child(hours_field(
            "royal-forecast-window-start",
            &controls.window_start,
            "Menu opens at hour",
        ))
        .child(hours_field(
            "royal-forecast-window-end",
            &controls.window_end,
            "Menu closes at hour",
        ))
        .child(
            Button::new("royal-forecast-add-window")
                .label("Add menu window")
                .icon(IconName::Plus)
                .small()
                .on_click(move |_event, _window, cx| {
                    add_window.update(cx, |state, cx| state.add_royal_window(cx));
                }),
        )
        .into_any_element()
}

/// An hour field under the element id the harness presses; a press focuses it.
fn hours_field(id: &'static str, field: &Entity<InputState>, label: &'static str) -> AnyElement {
    let focus = field.clone();
    div()
        .id(id)
        .flex_none()
        .w(px(110.0))
        .on_click(move |_event, window, cx| {
            focus.update(cx, |field, cx| field.focus(window, cx));
        })
        .child(Input::new(field).aria_label(label).small())
        .into_any_element()
}

/// The summary cards: what the horizon commits.
fn summary(result: &Forecast) -> AnyElement {
    let summary = &result.summary;
    let horizon = format!(
        "{} · {}",
        if result.banks {
            "time banks while the menu is closed"
        } else {
            "only menu-open windows collect"
        },
        if result.partial {
            "the reset countdown is missing"
        } else if result.resets.is_empty() {
            "no reset inside the horizon"
        } else {
            "the daily reset is projected"
        }
    );
    div()
        .id("royal-forecast-summary")
        .child(
            KpiStrip::new()
                .child(kpi(
                    "royal-kpi-forecast-collects",
                    "Collects",
                    summary.collects as f64,
                    "",
                    format!(
                        "{} menu-open {}",
                        summary.windows,
                        if summary.windows == 1 {
                            "window"
                        } else {
                            "windows"
                        }
                    ),
                ))
                .child(kpi(
                    "royal-kpi-forecast-refills",
                    "Refills",
                    summary.refilled as f64,
                    "",
                    format!(
                        "{} missed · +{} grades",
                        format::count(summary.missed as u64),
                        format::count(summary.grade_gains as u64)
                    ),
                ))
                .child(kpi(
                    "royal-kpi-forecast-rankups",
                    "Rank-ups",
                    summary.rank_ups as f64,
                    "",
                    format!("+{} outpost points", format::value(summary.points)),
                ))
                .child(kpi(
                    "royal-kpi-forecast-banked",
                    "Banked at the horizon",
                    summary.banked_at_horizon,
                    "h",
                    "Uncollected time",
                ))
                .child(
                    v_flex()
                        .id("royal-forecast-horizon-note")
                        .role(gpui::Role::Label)
                        .aria_label(horizon.clone())
                        .gap_1()
                        .child(Caption::new(format!(
                            "Horizon {}h",
                            figure_text(result.horizon_hours)
                        )))
                        .child(Caption::new(horizon)),
                ),
        )
        .into_any_element()
}

/// What each collect would commit, and what the segments expect next.
fn agenda(royal: &RoyalGuardian, result: &Forecast, cx: &mut Context<AppState>) -> AnyElement {
    let mut list = v_flex().id("royal-forecast-agenda").gap_2();
    for (index, commit) in result.collects.iter().enumerate() {
        let remove = cx.entity();
        let mut chips = h_flex().flex_wrap().gap_2();
        for (slot, value) in &commit.currency {
            chips = chips.child(Caption::new(format!(
                "slot {slot} +{}",
                format::value(*value)
            )));
        }
        if !commit.drained.is_empty() {
            chips = chips.child(Caption::new(format!(
                "{} nodes drained",
                commit.drained.len()
            )));
        }
        if !commit.rank_ups.is_empty() {
            chips = chips.child(Caption::new(format!("{} rank-ups", commit.rank_ups.len())));
        }
        if !commit.secured.is_empty() {
            chips = chips.child(Caption::new(format!("{} cleared", commit.secured.len())));
        }
        if !commit.affordable.is_empty() {
            chips = chips.child(Caption::new(format!(
                "{} upgrades affordable",
                commit.affordable.len()
            )));
        }
        if commit.banked == 0.0 {
            chips = chips.child(Caption::new("Nothing banked"));
        }
        list = list.child(
            v_flex()
                .id(SharedString::from(format!(
                    "royal-forecast-collect-{index}"
                )))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Collect at {}h: banked {}h",
                    figure_text(commit.at),
                    figure_text(commit.banked)
                ))
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .child(format!(
                            "Collect at {}h · banked {}h",
                            figure_text(commit.at),
                            figure_text(commit.banked)
                        ))
                        .child(
                            Button::new(SharedString::from(format!(
                                "royal-forecast-remove-collect-{index}"
                            )))
                            .label("Remove")
                            .small()
                            .on_click(move |_event, _window, cx| {
                                remove
                                    .update(cx, |state, cx| state.remove_royal_collect(index, cx));
                            }),
                        ),
                )
                .child(chips),
        );
    }
    for (index, window) in result.windows.iter().enumerate() {
        let remove = cx.entity();
        list = list.child(
            v_flex()
                .id(SharedString::from(format!("royal-forecast-window-{index}")))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Menu open from {}h to {}h in {} steps",
                    figure_text(window.start),
                    figure_text(window.end),
                    window.steps
                ))
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .child(format!(
                            "Menu open {}h–{}h · {} steps",
                            figure_text(window.start),
                            figure_text(window.end),
                            window.steps
                        ))
                        .child(
                            Button::new(SharedString::from(format!(
                                "royal-forecast-remove-window-{index}"
                            )))
                            .label("Remove")
                            .small()
                            .on_click(move |_event, _window, cx| {
                                remove.update(cx, |state, cx| {
                                    state.remove_royal_window(index, cx);
                                });
                            }),
                        ),
                )
                .child(Caption::new(format!(
                    "{} fills · {} rank-ups · {} cleared · {} upgrades affordable{}",
                    window.summary.drained.len(),
                    window.summary.rank_ups.len(),
                    window.summary.secured.len(),
                    window.summary.affordable.len(),
                    if window.truncated {
                        " · step limit reached"
                    } else {
                        ""
                    }
                ))),
        );
    }
    if result.collects.is_empty() && result.windows.is_empty() {
        list = list.child(Caption::new(
            "Nothing scheduled yet. Add a collect hour or a menu-open window.",
        ));
    }
    let next = next_ghosts(royal, result);
    if !next.is_empty() {
        list = list.child(Caption::new("Next events if collected:"));
        for (label, at) in next {
            list = list.child(Caption::new(format!("{label} at {}h", figure_text(at))));
        }
    }
    Card::new()
        .header(
            CardHeader::new("Agenda")
                .description("Each collect and menu-open window, in horizon order"),
        )
        .child(list)
        .into_any_element()
}

/// The first fork events the segments expect, one of each kind.
fn next_ghosts(royal: &RoyalGuardian, result: &Forecast) -> Vec<(String, f64)> {
    let mut next: Vec<(String, f64)> = Vec::new();
    let mut push = |label: String, at: f64| {
        if !next.iter().any(|(_, hour)| *hour == at) && next.len() < 5 {
            next.push((label, at));
        }
    };
    for segment in &result.segments {
        for fill in &segment.ghosts.fills {
            push(format!("Resource {} fills", fill.resource + 1), fill.at);
        }
        for ghost in &segment.ghosts.ranks {
            if ghost.ready {
                continue;
            }
            push(
                format!(
                    "{} rank {} at {}",
                    idleon_royal_guardian::RANK_BARS[ghost.bar],
                    figure_text(ghost.rank),
                    map_name(royal, ghost.map)
                ),
                ghost.at,
            );
        }
        for clear in &segment.ghosts.clears {
            push(format!("{} clears", map_name(royal, clear.map)), clear.at);
        }
        for item in &segment.ghosts.affordable {
            push(format!("{} affordable", item.name), item.at);
        }
    }
    next.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    next.truncate(6);
    next
}

/// A map's display name.
fn map_name(royal: &RoyalGuardian, map: usize) -> String {
    royal.outpost(map).name.replace('_', " ").to_string()
}

/// The currency balance track: start, at the horizon and what was collected.
fn currency(result: &Forecast, cx: &App) -> AnyElement {
    let rows = result.summary.currency.iter().map(|(slot, value)| {
        // The projected balance across every segment: the first point is the
        // saved balance and the last is where the horizon leaves it.
        let mut points = result
            .segments
            .iter()
            .filter_map(|segment| segment.currency.get(slot))
            .flatten();
        let start = points.next().map(|(_, value)| *value).unwrap_or(0.0);
        let end = points.last().map(|(_, value)| *value).unwrap_or(start);
        vec![
            text_id(
                SharedString::from(format!("royal-forecast-currency-{slot}")),
                format!("Currency slot {slot}"),
            ),
            text(format::value(start)),
            text(format::value(end)),
            text(format::value(*value)),
        ]
    });
    let table = table(
        "royal-forecast-currency",
        &[
            TableColumn::text("Currency"),
            TableColumn::number("Start"),
            TableColumn::number("At the horizon"),
            TableColumn::number("Collected"),
        ],
        rows,
        cx,
    );
    Card::new()
        .header(CardHeader::new("Currency").description(
            "The balance at the start, the last projected point and what the collects credit",
        ))
        .child(div().id("royal-forecast-currency").child(table))
        .into_any_element()
}

/// The resets and what they refilled.
fn resets(result: &Forecast, cx: &App) -> AnyElement {
    let rows = result.reset_events.iter().map(|event| {
        vec![
            text(format!("{:.0}h", event.at)),
            text(if event.refills { "Yes" } else { "No" }),
            text(format::count(event.refilled.len() as u64)),
            text(format::count(event.grade_gains as u64)),
            text(format::count(event.missed.len() as u64)),
        ]
    });
    let table = table(
        "royal-forecast-reset",
        &[
            TableColumn::text("Reset"),
            TableColumn::text("Refills"),
            TableColumn::number("Refilled"),
            TableColumn::number("Grades"),
            TableColumn::number("Missed"),
        ],
        rows,
        cx,
    );
    Card::new()
        .header(CardHeader::new("Daily resets"))
        .child(div().id("royal-forecast-resets").child(table))
        .into_any_element()
}

/// The options the section state asks for.
pub(crate) fn options(hours: f64, collects: &[f64], windows: &[(f64, f64)]) -> ForecastOptions {
    ForecastOptions {
        horizon_hours: hours.clamp(HORIZON.0, HORIZON.1),
        collects: collects.to_vec(),
        windows: windows.to_vec(),
    }
}

/// What an hour field holds.
pub(crate) fn field_text(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}
