//! FN-04: the Sim tab, the timeline of a simulated fountain run.
//!
//! Upstream runs `js/fountain-sim-worker.js` in a module worker. This section
//! runs the same simulation through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, cancel action,
//! cancelled notice and stale marker are the ones every other section uses.
//! The run works on a copy of the cavern: the exported save and every control
//! on this tab are unchanged when a run is cancelled.

use super::overview::num;
use super::state::{SIM_MAX_HOURS, SIM_MIN_HOURS};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_fountain as fountain;
use idleon_fountain::{SimEvent, SimEventKind, SimResult};

/// How many timeline rows the tab renders; the count and the summary pin the
/// rest, and a pathological run can buy thousands of levels.
const TIMELINE_ROWS: usize = 200;

/// The Sim tab: the run controls, the run in flight and the stored timeline.
pub(super) fn sim_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.fountain_sim_task(cx);
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id("fountain-sim")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls(state, cx))
        .child(
            Caption::new(
                "Simulates active fountain play forward in time. It picks regular and marble \
                 upgrades, changes Desired when unlocked, focuses generation through Turn and \
                 Push, and drains the marble bar as it fills. Currency income is the current \
                 steady-state rate, so the saved ground stacks do not seed the run.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
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
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-sim-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(result) = task.result.clone() {
        body = body.child(result_card(&result, task.stale));
    }
    body.into_any_element()
}

/// The time limit and the two actions.
fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let running = state.fountain_sim_task(cx).running;
    let hours = state.fountain_state.sim_hours;
    let entity = cx.entity();
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let run = Button::new(
        "run-fountain-sim",
        if running { "Simulating…" } else { "Run Sim" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || !state.save_loaded)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_fountain_sim(cx));
    });
    let cancel = Button::new("cancel-fountain-sim", "Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_sim(cx));
        });
    let stepper = Stepper::new("fountain-sim-hours", hours)
        .range(SIM_MIN_HOURS, SIM_MAX_HOURS)
        .step(1.0)
        .precision(0)
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| state.set_fountain_sim_hours(value, cx));
        });
    div()
        .id("fountain-sim-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(stepper)
        .child(run)
        .child(cancel)
        .into_any_element()
}

/// The stored run: its summary, final balances and timeline.
fn result_card(result: &SimResult, stale: bool) -> AnyElement {
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
    let summary = div()
        .id("fountain-sim-result")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(Badge::new(format!("{} steps", result.steps)).tone(Tone::Info))
        .child(Badge::new(format!("{} events", result.events.len())).tone(Tone::Neutral))
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
    let rows = timeline(result);
    let mut table = Table::new([
        "Time",
        "Event",
        "Details",
        "Marble /hr",
        "Base Currency /hr",
    ]);
    for row in rows.iter().take(TIMELINE_ROWS) {
        table = table.row([
            div().child(clock(row.time)).into_any_element(),
            div().child(row.label).into_any_element(),
            div()
                .child(match &row.claim {
                    Some(claim) => format!("{} ⬆ {claim}", row.detail),
                    None => row.detail.clone(),
                })
                .into_any_element(),
            div().child(num(row.marble_rate)).into_any_element(),
            div().child(num(row.base_rate)).into_any_element(),
        ]);
    }
    let mut body = div()
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(summary)
        .child(Caption::new(format!("Final: {}", balances.join(" • "))));
    if rows.is_empty() {
        body = body.child(div().id("fountain-sim-empty").child(Caption::new(
            "No events — nothing to buy or no beneficial upgrades found.",
        )));
    } else {
        let mut caption = "The timeline, in simulated order. The marble-fill rows are hidden: \
             the marble those fills created is in the marble purchases and the final balance."
            .to_string();
        if rows.len() > TIMELINE_ROWS {
            caption.push_str(&format!(
                " Showing the first {TIMELINE_ROWS} of {} events.",
                rows.len()
            ));
        }
        body = body
            .child(
                div()
                    .id("fountain-sim-timeline")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(table),
            )
            .child(Caption::new(caption));
    }
    card.child(body).into_any_element()
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
