//! The Fleet Simulation tab: the run controls, progress and the pile results.
//!
//! The pile simulation runs through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, cancel action,
//! cancelled notice and stale marker are the ones every other section uses.
//! Every trial starts with an empty pile and all boats freshly departed, and
//! the tier filter applies only until the pile reaches its near-full target,
//! matching the game's order. A stored run is formatted once, when it lands
//! ([`View::of`]); the tables are bounded by the pile's slots and tiers.

use super::table;
use super::{choice_button, field, islands};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_sailing::{AssignmentMode, Goal, SimResult, SimSummary};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Statistic, TEXT_BODY, TableColumn, show_number,
};

/// What a simulation result was produced from; a change marks it stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) island: usize,
    pub(crate) goal: Goal,
    pub(crate) mode: AssignmentMode,
    pub(crate) minimum_tier: u32,
    pub(crate) hours: f64,
    pub(crate) trials: usize,
    /// The account's talent, so a re-import with a different talent is stale.
    pub(crate) unending_loot_search: f64,
    /// The pile capacity the account carries.
    pub(crate) capacity: usize,
    /// The save this run read, so a new import is stale.
    pub(crate) revision: u64,
}

/// A stored run, formatted once.
pub(crate) struct View {
    description: SharedString,
    kpis: [(&'static str, &'static str, SharedString, SharedString); 7],
    currencies: Vec<(&'static str, &'static str, [SharedString; 5])>,
    tiers: Vec<(usize, [SharedString; 5])>,
    slots: Vec<(usize, [SharedString; 10])>,
}

impl View {
    pub(crate) fn of(result: &SimResult) -> Self {
        let pile = result.pile;
        let events = result.events;
        let assignment = match result.setup.mode {
            AssignmentMode::Optimized => format!("{} optimized", result.setup.goal.id()),
            AssignmentMode::Saved => "current save".into(),
        };
        let currencies = &result.currencies;
        let total = pile.count.mean;
        Self {
            description: format!(
                "{}h · {} trials · seed {} · Island {} · {assignment} assignment · empty/fresh start",
                result.setup.hours,
                result.setup.trials,
                result.setup.seed,
                result.setup.island + 1
            )
            .into(),
            kpis: [
                (
                    "sailing-sim-pile-count",
                    "Average final pile",
                    format!("{:.2} chests", pile.count.mean).into(),
                    format!("{} near-full target", result.setup.target_count).into(),
                ),
                (
                    "sailing-sim-pile-value",
                    "Final opened value",
                    format::value(pile.value.mean).into(),
                    summary_line(&pile.value).into(),
                ),
                (
                    "sailing-sim-average",
                    "Average / banked chest",
                    format::value(pile.average_value_per_chest).into(),
                    "Total opened value ÷ average chest count".into(),
                ),
                (
                    "sailing-sim-retained",
                    "Retained vs active",
                    format!("{:.1}%", result.retained_fraction * 100.0).into(),
                    "Final pile value ÷ collect-everything value".into(),
                ),
                (
                    "sailing-sim-generated",
                    "Generated",
                    format::value(events.generated).into(),
                    format!(
                        "{} accepted before near-full",
                        format::value(events.accepted)
                    )
                    .into(),
                ),
                (
                    "sailing-sim-improvements",
                    "Pile improvements",
                    format!("{} upgrades", format::value(events.upgrades)).into(),
                    format!(
                        "{} higher-tier replacements",
                        format::value(events.replacements)
                    )
                    .into(),
                ),
                (
                    "sailing-sim-discards",
                    "Discarded / blocked",
                    format::value(events.filter_discards + events.overflow_discards + events.blocked)
                        .into(),
                    format!(
                        "{} filter · {} overflow · {} full",
                        format::value(events.filter_discards),
                        format::value(events.overflow_discards),
                        format::value(events.blocked)
                    )
                    .into(),
                ),
            ],
            currencies: [
                ("gold", "Gold Bars", currencies.gold),
                (
                    "primary",
                    "Primary treasure (chest island)",
                    currencies.primary,
                ),
                ("rare", "Rare treasure (chest island)", currencies.rare),
            ]
            .into_iter()
            .map(|(key, label, row)| {
                (
                    key,
                    label,
                    [
                        format::value(row.value.mean).into(),
                        format::value(row.value.p10).into(),
                        format::value(row.value.median).into(),
                        format::value(row.value.p90).into(),
                        format!("{:.2}", row.payouts.mean).into(),
                    ],
                )
            })
            .collect(),
            tiers: result
                .tiers
                .iter()
                .map(|row| {
                    let share = if total > 0.0 {
                        row.average_count / total
                    } else {
                        0.0
                    };
                    (
                        row.tier,
                        [
                            format!("{:.3}", row.average_count).into(),
                            format!("{:.1}%", share * 100.0).into(),
                            format::value(row.average_value_per_payout).into(),
                            format::value(row.average_opened_value).into(),
                            format::value(row.opened_value_contribution).into(),
                        ],
                    )
                })
                .collect(),
            slots: result
                .slots
                .iter()
                .filter(|slot| slot.occupancy > 0.0)
                .map(|slot| {
                    (
                        slot.index,
                        [
                            format!("{:.1}%", slot.occupancy * 100.0).into(),
                            format!("{:.2}", slot.average_tier).into(),
                            slot.most_likely_tier
                                .map_or_else(|| "—".into(), |tier| format!("Tier {tier}"))
                                .into(),
                            format::value(slot.average_value_per_payout).into(),
                            format!("{:.2}", slot.average_payouts).into(),
                            format::value(slot.average_opened_value).into(),
                            format!("{}x", format::value(slot.average_artifact_multiplier)).into(),
                            format::value(slot.average_currency_value[0]).into(),
                            format::value(slot.average_currency_value[1]).into(),
                            format::value(slot.average_currency_value[2]).into(),
                        ],
                    )
                })
                .collect(),
        }
    }
}

/// The simulation tab: controls, the run in flight and the stored results.
pub(crate) fn simulation_tab(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let runner = state.sailing_sim.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = runner.is_stale(&state.sailing_sim_inputs());
    // The stored result drawn is the one the runner still holds.
    let result = runner.result().and(state.sailing_controls.sim.clone());
    let muted = cx.theme().muted_foreground;
    let mut body = v_flex()
        .id("sailing-sim-panel")
        .gap_3()
        .child(controls(state, running, window, cx))
        .child(
            Caption::new(
                "Every trial starts with an empty pile and all boats freshly departed, so the first \
                 chest from each boat arrives after one full trip. All active pairings target the \
                 selected island and use the selected assignment. The minimum tier applies only \
                 before the pile reaches its near-full target, matching the game's order.",
            )
            .text_color(muted),
        );
    if running && let Some(progress) = progress {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "sailing-sim",
            "Simulating the fleet pile",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_sailing_simulation(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "sailing-sim-cancelled",
            result.is_some(),
        ));
    }
    if let Some(result) = result {
        body = body.child(results_card(&result, stale, muted));
    }
    body.into_any_element()
}

/// The interval presets, the two number fields, the destination and the actions.
fn controls(
    state: &mut AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let hours = state.sailing_state.sim_hours;
    let assignment = match state.sailing_state.mode {
        AssignmentMode::Optimized => format!("optimized {}", state.sailing_state.goal.id()),
        AssignmentMode::Saved => "current save".into(),
    };
    let islands = islands(state, cx);
    let controls = &state.sailing_controls;
    controls.sim_island.show(
        (*islands).clone(),
        Some(&state.sailing_state.sim_island.min(islands.len() - 1)),
        window,
        cx,
    );
    show_number(&controls.sim_hours, hours, 2, window, cx);
    show_number(
        &controls.sim_trials,
        state.sailing_state.sim_trials as f64,
        0,
        window,
        cx,
    );

    let presets =
        h_flex()
            .id("sailing-sim-presets")
            .gap_1()
            .children([8.0_f64, 12.0, 24.0, 48.0].map(|preset| {
                let entity = cx.entity();
                choice_button(
                    SharedString::from(format!("sailing-sim-hours-{}", preset as u32)),
                    &format!("{preset:.0}h"),
                    (hours - preset).abs() < f64::EPSILON,
                )
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.set_sailing_sim_hours(preset, cx));
                })
            }));

    let run_entity = cx.entity();
    let run = Button::new("sailing-sim-run")
        .label(if running {
            "Running…"
        } else {
            "Run simulation"
        })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_sailing_simulation(cx));
        });
    let cancel_entity = cx.entity();
    let cancel = Button::new("sailing-sim-cancel")
        .label("Cancel")
        .secondary()
        .small()
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_sailing_simulation(cx));
        });

    Card::new()
        .header(CardHeader::new("Fleet pile simulation").description(
            "Runs a fresh seeded Monte Carlo over the active fleet; the same inputs and seed reproduce the result.",
        ))
        .child(
            v_flex()
                .id("sailing-sim-controls")
                .gap_3()
                .child(
                    h_flex()
                        .items_end()
                        .gap_4()
                        .child(field("Interval presets", presets))
                        .child(field(
                            "Hours",
                            number_field("sailing-sim-hours", &controls.sim_hours, "Hours"),
                        ))
                        .child(field(
                            "Destination",
                            div().w(px(260.0)).child(
                                Select::new(controls.sim_island.state())
                                    .id("sailing-sim-island")
                                    .accessibility_label("Destination")
                                    .small(),
                            ),
                        ))
                        .child(field(
                            "Trials",
                            number_field("sailing-sim-trials", &controls.sim_trials, "Trials"),
                        )),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .child(field(
                            "Assignment",
                            Caption::new(assignment).text_color(muted),
                        ))
                        .child(div().flex_1())
                        .child(h_flex().gap_2().child(run).child(cancel)),
                ),
        )
        .into_any_element()
}

/// The stored run: the summary figures, the currency spread, the tier mix and
/// each pile position's expected chest.
fn results_card(view: &View, stale: bool, muted: gpui::Hsla) -> AnyElement {
    let mut header = CardHeader::new("Simulation results").description(view.description.clone());
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("sailing-sim"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("sailing-sim"));
    }
    let kpis = div()
        .id("sailing-sim-kpis")
        .child(
            KpiStrip::new().children(view.kpis.iter().map(|(id, label, value, caption)| {
                Statistic::new(*id, *label, value.clone())
                    .caption(caption.clone())
                    .small()
            })),
        );
    let currencies = section(
        "sailing-sim-currency-table",
        "Currency outcome distribution",
        table::table(
            "sailing-sim-currency-scroll",
            &[
                TableColumn::text("Currency"),
                TableColumn::number("Mean value"),
                TableColumn::number("P10"),
                TableColumn::number("P50"),
                TableColumn::number("P90"),
                TableColumn::number("Avg payouts"),
            ],
            view.currencies.iter().map(|(key, label, cells)| {
                table::Row::new(SharedString::from(format!(
                    "sailing-sim-currency-row-{key}"
                )))
                .child(table::cell(
                    div()
                        .id(SharedString::from(format!("sailing-sim-currency-{key}")))
                        .child(*label),
                ))
                .children(cells.iter().cloned().map(table::cell))
            }),
        ),
    );
    let tiers = section(
        "sailing-sim-tier-table",
        "Final pile tier composition",
        table::table(
            "sailing-sim-tier-scroll",
            &[
                TableColumn::text("Tier"),
                TableColumn::number("Average count"),
                TableColumn::number("Pile share"),
                TableColumn::number("Average value / payout"),
                TableColumn::number("Average opened value / chest"),
                TableColumn::number("Opened value contribution"),
            ],
            view.tiers.iter().map(|(tier, cells)| {
                table::Row::new(("sailing-sim-tier-row", *tier))
                    .child(table::cell(
                        div()
                            .id(SharedString::from(format!("sailing-sim-tier-{tier}")))
                            .child(format!("Tier {tier}")),
                    ))
                    .children(cells.iter().cloned().map(table::cell))
            }),
        ),
    );
    let slots = section(
        "sailing-sim-slot-table",
        "Average banked chest by pile position",
        table::table(
            "sailing-sim-slot-scroll",
            &[
                TableColumn::text("Position"),
                TableColumn::number("Occupied"),
                TableColumn::number("Average tier"),
                TableColumn::number("Most likely tier"),
                TableColumn::number("Value / payout"),
                TableColumn::number("Payouts"),
                TableColumn::number("Opened value"),
                TableColumn::number("Artifact x"),
                TableColumn::number("Gold"),
                TableColumn::number("Primary"),
                TableColumn::number("Rare"),
            ],
            view.slots.iter().map(|(index, cells)| {
                table::Row::new(("sailing-sim-slot-row", *index))
                    .child(table::cell(
                        div()
                            .id(SharedString::from(format!("sailing-sim-slot-{index}")))
                            .child(format!("#{}", index + 1)),
                    ))
                    .children(cells.iter().cloned().map(table::cell))
            }),
        ),
    )
    .child(
        Caption::new(
            "Position is opening order: #1 opens next and is the first banked chest the \
             near-full scan reads. Averages are conditional on occupancy except the currency \
             columns, which are the expected contribution per trial.",
        )
        .text_color(muted),
    );
    card.child(kpis)
        .child(currencies)
        .child(tiers)
        .child(slots)
        .child(
            Caption::new(
                "Final pile values include every guaranteed payout. Each payout picks its currency \
                 independently. P10/P50/P90 describe the spread across trials, not confidence \
                 intervals around the mean. Active collection opens every generated chest \
                 immediately and has no capacity loss.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

/// A titled table of the results card.
fn section(
    id: &'static str,
    title: &'static str,
    table: impl IntoElement,
) -> gpui::Stateful<gpui::Div> {
    v_flex()
        .id(id)
        .gap(PANEL_GAP)
        .child(
            div()
                .text_size(TEXT_BODY)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(table)
}

fn summary_line(summary: &SimSummary) -> String {
    format!(
        "P10 {} · P50 {} · P90 {}",
        format::value(summary.p10),
        format::value(summary.median),
        format::value(summary.p90)
    )
}
