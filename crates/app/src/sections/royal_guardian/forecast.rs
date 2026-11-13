//! The Forecast tab: the projection over native collects, the agenda of what
//! each collect would commit, and the currency track.
//!
//! The engine is `idleon_royal_guardian::forecast`; this pane places collects and
//! menu-open windows, shows the summary, the agenda and the currency balances.
//! The upstream timeline chart is not drawn: the agenda lists the same events
//! with their hours.

use super::overview::{figure_text, kpi};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::{Choice, Input, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Styled, Subscription, Window, div,
};
use idleon_royal_guardian::{Forecast, ForecastOptions, HORIZON, RoyalGuardian};
use idleon_ui::format;

/// The horizons the selector offers, in hours, with their labels.
const HORIZONS: [(u32, &str); 5] = [
    (12, "12h"),
    (24, "24h"),
    (72, "3d"),
    (168, "7d"),
    (336, "14d"),
];

/// The forecast tab's live fields.
pub(crate) struct Inputs {
    pub(crate) collect: Entity<TextInput>,
    pub(crate) window_start: Entity<TextInput>,
    pub(crate) window_end: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

impl Inputs {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let mut field = |placeholder, label, cx: &mut Context<AppState>| {
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .filter(|ch: char| ch.is_ascii_digit() || ch == '.')
                    .placeholder(placeholder)
                    .label(label)
            })
        };
        let collect = field("e.g. 6", "Collect at hour", cx);
        let window_start = field("e.g. 20", "Menu opens at hour", cx);
        let window_end = field("e.g. 22", "Menu closes at hour", cx);
        let subscriptions = vec![
            crate::state::redraw_on_edit(cx, &collect),
            crate::state::redraw_on_edit(cx, &window_start),
            crate::state::redraw_on_edit(cx, &window_end),
        ];
        Self {
            collect,
            window_start,
            window_end,
            _subscriptions: subscriptions,
        }
    }
}

/// A read of a field as an hour, or `None` when it is blank or invalid.
pub(crate) fn hours(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// The Forecast pane.
pub(crate) fn forecast(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
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
    div()
        .id("royal-forecast")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, cx))
        .child(summary(result, cx))
        .child(agenda(royal, result, cx))
        .child(currency(result))
        .child(resets(result))
        .into_any_element()
}

/// The horizon and the collect and window fields.
fn controls(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let choices: Vec<Choice> = HORIZONS
        .iter()
        .map(|(hours, label)| Choice::new(hours.to_string(), *label))
        .collect();
    let selected = state.royal_state.forecast_hours.round() as u32;
    let horizon = Select::new("royal-forecast-horizon", choices)
        .label("Horizon")
        .selected(selected.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(hours) = value.parse::<f64>() {
                entity.update(cx, |state, cx| state.set_royal_forecast_hours(hours, cx));
            }
        });
    let inputs = &state.royal_forecast_inputs;
    let field = |id: &'static str, input: &Entity<TextInput>| {
        let focus = input.read(cx).focus().clone();
        div()
            .id(id)
            .w_24()
            .on_click(move |_event, window, cx| window.focus(&focus, cx))
            .child(Input::new(input).size(ControlSize::Sm))
            .into_any_element()
    };
    let add_collect = cx.entity();
    let add_window = cx.entity();
    div()
        .id("royal-forecast-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_2()
        .child(horizon)
        .child(field("royal-forecast-collect", &inputs.collect))
        .child(
            Button::new("royal-forecast-add-collect", "Add collect")
                .icon(IconName::Plus)
                .on_click(move |_event, _window, cx| {
                    add_collect.update(cx, |state, cx| state.add_royal_collect(cx));
                }),
        )
        .child(field("royal-forecast-window-start", &inputs.window_start))
        .child(field("royal-forecast-window-end", &inputs.window_end))
        .child(
            Button::new("royal-forecast-add-window", "Add menu window")
                .icon(IconName::Plus)
                .on_click(move |_event, _window, cx| {
                    add_window.update(cx, |state, cx| state.add_royal_window(cx));
                }),
        )
        .into_any_element()
}

/// The summary cards: what the horizon commits.
fn summary(result: &Forecast, cx: &mut Context<AppState>) -> AnyElement {
    let summary = &result.summary;
    let muted = cx.theme().colors.fg_muted;
    let horizon = format!(
        "{} · {}",
        if result.banks {
            "time banks while the menu is closed"
        } else {
            "only menu-open windows collect"
        },
        if result.resets.is_empty() {
            "no reset inside the horizon"
        } else {
            "the daily reset is projected"
        }
    );
    div()
        .id("royal-forecast-summary")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
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
                    div()
                        .id("royal-forecast-horizon-note")
                        .role(gpui::Role::Label)
                        .aria_label(horizon.clone())
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new(format!(
                            "Horizon {}h",
                            figure_text(result.horizon_hours)
                        )))
                        .child(Caption::new(horizon).text_color(muted)),
                ),
        )
        .into_any_element()
}

/// What each collect would commit, and what the segments expect next.
fn agenda(royal: &RoyalGuardian, result: &Forecast, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut list = div().id("royal-forecast-agenda").flex().flex_col().gap_2();
    for (index, commit) in result.collects.iter().enumerate() {
        let remove = cx.entity();
        let mut chips = div().flex().flex_wrap().items_center().gap_2();
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
            chips = chips.child(Caption::new("Nothing banked").text_color(muted));
        }
        list = list.child(
            div()
                .id(format!("royal-forecast-collect-{index}"))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Collect at {}h: banked {}h",
                    figure_text(commit.at),
                    figure_text(commit.banked)
                ))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(format!(
                            "Collect at {}h · banked {}h",
                            figure_text(commit.at),
                            figure_text(commit.banked)
                        ))
                        .child(
                            Button::new(format!("royal-forecast-remove-collect-{index}"), "Remove")
                                .size(ControlSize::Sm)
                                .on_click(move |_event, _window, cx| {
                                    remove.update(cx, |state, cx| {
                                        state.remove_royal_collect(index, cx)
                                    });
                                }),
                        ),
                )
                .child(chips),
        );
    }
    for (index, window) in result.windows.iter().enumerate() {
        let remove = cx.entity();
        list = list.child(
            div()
                .id(format!("royal-forecast-window-{index}"))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Menu open from {}h to {}h in {} steps",
                    figure_text(window.start),
                    figure_text(window.end),
                    window.steps
                ))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(format!(
                            "Menu open {}h–{}h · {} steps",
                            figure_text(window.start),
                            figure_text(window.end),
                            window.steps
                        ))
                        .child(
                            Button::new(format!("royal-forecast-remove-window-{index}"), "Remove")
                                .size(ControlSize::Sm)
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
        list =
            list.child(div().child(Caption::new("Next events if collected: ").text_color(muted)));
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
fn currency(result: &Forecast) -> AnyElement {
    let mut table = Table::new(["Currency", "Start", "At the horizon", "Collected"]);
    for (slot, value) in &result.summary.currency {
        // The projected balance across every segment: the first point is the
        // saved balance and the last is where the horizon leaves it.
        let points: Vec<(f64, f64)> = result
            .segments
            .iter()
            .filter_map(|segment| segment.currency.get(slot))
            .flatten()
            .copied()
            .collect();
        let start = points.first().map(|(_, value)| *value).unwrap_or(0.0);
        let end = points.last().map(|(_, value)| *value).unwrap_or(0.0);
        table = table.row([
            div()
                .id(format!("royal-forecast-currency-{slot}"))
                .child(format!("Currency slot {slot}"))
                .into_any_element(),
            div().child(format::value(start)).into_any_element(),
            div().child(format::value(end)).into_any_element(),
            div().child(format::value(*value)).into_any_element(),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Currency").description(
            "The balance at the start, the last projected point and what the collects credit",
        ))
        .child(div().id("royal-forecast-currency").child(table))
        .into_any_element()
}

/// The resets and what they refilled.
fn resets(result: &Forecast) -> AnyElement {
    let mut table = Table::new(["Reset", "Refills", "Refilled", "Grades", "Missed"]);
    for event in &result.reset_events {
        table = table.row([
            div().child(format!("{:.0}h", event.at)).into_any_element(),
            div()
                .child(if event.refills { "Yes" } else { "No" })
                .into_any_element(),
            div()
                .child(format::count(event.refilled.len() as u64))
                .into_any_element(),
            div()
                .child(format::count(event.grade_gains as u64))
                .into_any_element(),
            div()
                .child(format::count(event.missed.len() as u64))
                .into_any_element(),
        ]);
    }
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

/// The first field's text, or an empty string when the input is unavailable.
pub(crate) fn text(input: &Entity<TextInput>, cx: &Context<AppState>) -> String {
    input.read(cx).text().to_string()
}
