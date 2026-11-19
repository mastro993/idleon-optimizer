//! FN-04: the Sim tab, the timeline of a simulated fountain run.
//!
//! The simulation runs through the shared [`idleon_ui::runner::TaskRunner`], so
//! the progress card, cancel action, cancelled notice and stale marker are the
//! ones every other section uses. The run works on a copy of the cavern: the
//! exported save and every control on this tab are unchanged when a run is
//! cancelled.

use super::overview::num;
use super::table::{data_height, tag};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{SimEvent, SimEventKind, SimResult};
use idleon_ui::widgets::{Caption, Card, CardHeader, RowColumn, RowsDelegate, Tone, show_number};

/// The Sim tab: the run controls, the run in flight and the stored timeline.
pub(super) fn sim_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let runner = state.fountain_sim.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = runner.is_stale(&state.fountain_sim_inputs());
    // The stored result drawn is the one the runner still holds.
    let result = runner.result().and(state.fountain_controls.sim.clone());
    let muted = cx.theme().muted_foreground;
    let mut body = v_flex()
        .id("fountain-sim")
        .gap_3()
        .child(controls(state, running, window, cx))
        .child(
            Caption::new(
                "Simulates active fountain play forward in time. It picks regular and marble \
                 upgrades, changes Desired when unlocked, focuses generation through Turn and \
                 Push, and drains the marble bar as it fills. Currency income is the current \
                 steady-state rate, so the saved ground stacks do not seed the run.",
            )
            .text_color(muted),
        );
    if running && let Some(progress) = progress {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "fountain-sim",
            "Simulating the fountain",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_fountain_sim(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-sim-cancelled",
            result.is_some(),
        ));
    }
    if let Some(result) = result {
        body = body.child(result_card(state, &result, stale));
    }
    body.into_any_element()
}

/// The time limit and the two actions.
fn controls(
    state: &AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let field = &state.fountain_controls.sim_hours;
    show_number(field, state.fountain_state.sim_hours, 0, window, cx);
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let run = Button::new("run-fountain-sim")
        .label(if running { "Simulating…" } else { "Run Sim" })
        .primary()
        .icon(IconName::Play)
        .disabled(running || !state.save_loaded)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_fountain_sim(cx));
        });
    let cancel = Button::new("cancel-fountain-sim")
        .label("Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_sim(cx));
        });
    h_flex()
        .id("fountain-sim-controls")
        .flex_wrap()
        .gap_3()
        .child(Caption::new("Time limit (hours)"))
        .child(ui::number_field(
            "fountain-sim-hours",
            field,
            "Time limit (hours)",
        ))
        .child(run)
        .child(cancel)
        .into_any_element()
}

/// The stored run: its summary, final balances and timeline.
fn result_card(state: &AppState, result: &SimResult, stale: bool) -> AnyElement {
    let mut header = CardHeader::new("Simulation")
        .description(format!("{}h limit", num(result.clock_seconds / 3600.0)));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("fountain-sim"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("fountain-sim"));
    }
    let marble_pct = percent(result.start_marble_rate, result.end_marble_rate);
    let currency_pct = percent(result.start_base_currency, result.end_base_currency);
    let summary = h_flex()
        .id("fountain-sim-result")
        .flex_wrap()
        .gap_3()
        .child(tag(format!("{} steps", result.steps), Tone::Info))
        .child(tag(
            format!("{} events", result.events.len()),
            Tone::Neutral,
        ))
        .child(Caption::new(format!(
            "Marble {} → {}/hr ({:+.1}%) · Base currency {} → {}/hr ({:+.1}%)",
            num(result.start_marble_rate),
            num(result.end_marble_rate),
            marble_pct,
            num(result.start_base_currency),
            num(result.end_base_currency),
            currency_pct
        )));
    let balances: Vec<String> = result
        .currencies
        .iter()
        .enumerate()
        .filter(|(_, balance)| **balance > 0.0)
        .map(|(currency, balance)| {
            format!("{}: {}", fountain::currency_label(currency), num(*balance))
        })
        .chain(std::iter::once(format!("Marble: {}", num(result.marbles))))
        .collect();
    let mut body = v_flex()
        .gap_3()
        .child(summary)
        .child(Caption::new(format!("Final: {}", balances.join(" • "))));
    let rows = state.fountain_controls.timeline_len;
    if rows == 0 {
        body = body.child(div().id("fountain-sim-empty").child(Caption::new(
            "No events — nothing to buy or no beneficial upgrades found.",
        )));
    } else {
        body = body
            .child(
                div()
                    .id("fountain-sim-timeline")
                    .h(data_height(rows))
                    .child(DataTable::new(&state.fountain_controls.timeline)),
            )
            .child(Caption::new(
                "The timeline, in simulated order. The marble-fill rows are hidden: the marble \
                 those fills created is in the marble purchases and the final balance.",
            ));
    }
    card.child(body).into_any_element()
}

/// One timeline row, formatted once per stored run.
pub(crate) struct TimelineRow {
    time: SharedString,
    event: SharedString,
    details: SharedString,
    marble_rate: SharedString,
    base_rate: SharedString,
}

/// The timeline's columns.
pub(crate) fn timeline_table() -> RowsDelegate<TimelineRow> {
    RowsDelegate::new(vec![
        RowColumn::text("time", "Time", |row: &TimelineRow| row.time.clone())
            .column(|column| column.width(px(110.0))),
        RowColumn::text("event", "Event", |row: &TimelineRow| row.event.clone())
            .column(|column| column.width(px(120.0))),
        RowColumn::text("details", "Details", |row: &TimelineRow| {
            row.details.clone()
        })
        .column(|column| column.width(px(520.0))),
        RowColumn::number("marble", "Marble /hr", |row: &TimelineRow| {
            row.marble_rate.clone()
        })
        .column(|column| column.width(px(130.0))),
        RowColumn::number("base", "Base Currency /hr", |row: &TimelineRow| {
            row.base_rate.clone()
        })
        .column(|column| column.width(px(170.0))),
    ])
}

/// The run's timeline as table rows.
pub(crate) fn timeline_rows(result: &SimResult) -> Vec<TimelineRow> {
    timeline(result)
        .into_iter()
        .map(|row| TimelineRow {
            time: clock(row.time).into(),
            event: row.label.into(),
            details: match &row.claim {
                Some(claim) => format!("{} ⬆ {claim}", row.detail),
                None => row.detail,
            }
            .into(),
            marble_rate: num(row.marble_rate).into(),
            base_rate: num(row.base_rate).into(),
        })
        .collect()
}

/// One rendered timeline row.
struct Row {
    time: f64,
    label: &'static str,
    detail: String,
    /// The claim a purchase consumed, as the page folds it into the buy row.
    claim: Option<String>,
    marble_rate: f64,
    base_rate: f64,
    /// The upgrade a buy row groups, so repeated purchases at one time merge.
    key: Option<(usize, usize, bool)>,
}

/// The page's timeline: hide the marble fills, merge purchases of one upgrade
/// at the same second, and fold a claim into the purchase that follows it.
fn timeline(result: &SimResult) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for event in &result.events {
        let SimEvent {
            time,
            kind,
            marble_rate,
            base_currency_rate,
            ..
        } = event;
        match kind {
            SimEventKind::MarbleFill { .. } => continue,
            SimEventKind::Switch { desired, focus } => rows.push(Row {
                time: *time,
                label: "⇄ Switch",
                detail: match focus {
                    Some(focus) if *desired == *focus => {
                        format!("Desired → {}", fountain::currency_label(*desired))
                    }
                    Some(focus) => format!(
                        "Desired → {} • Focus → {}",
                        fountain::currency_label(*desired),
                        fountain::currency_label(*focus)
                    ),
                    None => format!("Desired → {}", fountain::currency_label(*desired)),
                },
                claim: None,
                marble_rate: *marble_rate,
                base_rate: *base_currency_rate,
                key: None,
            }),
            SimEventKind::Claim {
                currency,
                amount,
                seconds,
            } => rows.push(Row {
                time: *time,
                label: "↑ Earned",
                detail: format!(
                    "+{} {} ({})",
                    num(*amount),
                    fountain::currency_label(*currency),
                    clock(*seconds)
                ),
                claim: None,
                marble_rate: *marble_rate,
                base_rate: *base_currency_rate,
                key: None,
            }),
            SimEventKind::BuyUpgrade {
                water,
                upgrade,
                name,
                cost,
                currency,
                level,
            } => add_buy(
                &mut rows,
                event,
                (*water, *upgrade, false),
                "↑ Upgrade",
                name,
                *cost,
                Some(*currency),
                *level,
            ),
            SimEventKind::BuyMarble {
                water,
                upgrade,
                name,
                cost,
                marble_level,
            } => add_buy(
                &mut rows,
                event,
                (*water, *upgrade, true),
                "● Marble",
                name,
                *cost,
                None,
                *marble_level,
            ),
        }
    }
    // Fold a claim into the purchase that follows it at the same second.
    let mut slots: Vec<Option<Row>> = rows.into_iter().map(Some).collect();
    for index in 0..slots.len() {
        let is_claim = slots[index]
            .as_ref()
            .is_some_and(|row| row.key.is_none() && row.label == "↑ Earned");
        if !is_claim {
            continue;
        }
        let Some(next_index) = (index + 1..slots.len()).find(|next| slots[*next].is_some()) else {
            continue;
        };
        let claim_time = slots[index].as_ref().map(|row| row.time);
        let merges = slots[next_index]
            .as_ref()
            .is_some_and(|next| next.time == claim_time.unwrap_or(f64::NAN) && next.key.is_some());
        if !merges {
            continue;
        }
        let claim = slots[index].take().map(|row| row.detail);
        if let Some(next) = slots[next_index].as_mut() {
            next.claim = claim;
        }
    }
    slots.into_iter().flatten().collect()
}

/// Merge one purchase into the same upgrade's row at the same second.
#[allow(clippy::too_many_arguments)]
fn add_buy(
    rows: &mut Vec<Row>,
    event: &SimEvent,
    key: (usize, usize, bool),
    label: &'static str,
    name: &'static str,
    cost: f64,
    currency: Option<usize>,
    level: f64,
) {
    let same = rows
        .iter()
        .rposition(|row| row.time == event.time && row.key == Some(key));
    if let Some(index) = same {
        let row = &mut rows[index];
        row.detail
            .push_str(&format!(", {}", cost_label(cost, currency)));
        row.marble_rate = event.marble_rate;
        row.base_rate = event.base_currency_rate;
        return;
    }
    let range = if currency.is_some() {
        format!("Lv {} → {level}", fountain::short_number(level - 1.0))
    } else {
        format!(
            "M{} → M{}",
            fountain::short_number(level - 1.0),
            fountain::short_number(level)
        )
    };
    rows.push(Row {
        time: event.time,
        label,
        detail: format!(
            "{} {name} {range} ({})",
            fountain::water_names()[key.0].clone(),
            cost_label(cost, currency)
        ),
        claim: None,
        marble_rate: event.marble_rate,
        base_rate: event.base_currency_rate,
        key: Some(key),
    });
}

/// A cost with its currency, or marble.
fn cost_label(cost: f64, currency: Option<usize>) -> String {
    match currency {
        Some(currency) => format!("{} {}", num(cost), fountain::currency_label(currency)),
        None => format!("{} marble", num(cost)),
    }
}

/// The page's timeline clock: hours, minutes and seconds, no milliseconds.
fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0s".into();
    }
    let whole = seconds as u64;
    let (hours, minutes, secs) = (whole / 3600, whole % 3600 / 60, whole % 60);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if secs > 0 || parts.is_empty() {
        parts.push(format!("{secs}s"));
    }
    parts.join(" ")
}

/// A relative change, as the page's summary cards read it.
fn percent(start: f64, end: f64) -> f64 {
    if start > 0.0 {
        (end / start - 1.0) * 100.0
    } else {
        0.0
    }
}
