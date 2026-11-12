//! The Fleet Simulation tab: the run controls, progress and the pile results.
//!
//! Upstream runs the pile in a module worker. This section runs the same
//! simulation through the shared [`crate::task::TaskRunner`], so the progress
//! card, cancel action, cancelled notice and stale marker are the ones every
//! other section uses. Every trial starts with an empty pile and all boats
//! freshly departed, and the tier filter applies only until the pile reaches its
//! near-full target, matching the game's order.

use super::text_kpi;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, div};
use idleon_core::sailing::{
    AssignmentMode, Goal, MAX_HOURS, MAX_TRIALS, MIN_HOURS, MIN_TRIALS, SimCurrencies, SimResult,
    SimSummary, island_artifact_count, island_distance,
};

/// What a simulation result was produced from; a change marks it stale.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInputs {
    pub island: usize,
    pub goal: Goal,
    pub mode: AssignmentMode,
    pub minimum_tier: u32,
    pub hours: f64,
    pub trials: usize,
    /// The account's talent, so a re-import with a different talent is stale.
    pub unending_loot_search: f64,
    /// The pile capacity the account carries.
    pub capacity: usize,
    /// The save this run read, so a new import is stale.
    pub revision: u64,
}

/// The simulation tab: controls, the run in flight and the stored results.
pub fn simulation_tab(
    state: &mut AppState,
    _table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.sailing_simulation(cx);
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id("sailing-sim-panel")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls(state, cx))
        .child(
            Caption::new(
                "Every trial starts with an empty pile and all boats freshly departed, so the first \
                 chest from each boat arrives after one full trip. All active pairings target the \
                 selected island and use the selected assignment. The minimum tier applies only \
                 before the pile reaches its near-full target, matching the game's order.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(crate::ui::task::progress_card(
            "sailing-sim",
            "Simulating the fleet pile",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_sailing_simulation(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(crate::ui::task::cancelled_banner(
            "sailing-sim-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(result) = task.result.clone() {
        body = body.child(results_card(&result, task.stale, cx));
    }
    ScrollArea::new("sailing-sim-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The interval presets, the two steppers, the destination and the actions.
fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let running = state.sailing_simulation(cx).running;
    let hours = state.sailing_state.sim_hours;
    let trials = state.sailing_state.sim_trials;
    let island = state.sailing_state.sim_island;
    let available = state.fleet.available_islands().max(1);
    let assignment = match state.sailing_state.mode {
        AssignmentMode::Optimized => format!("optimized {}", state.sailing_state.goal.id()),
        AssignmentMode::Saved => "current save".into(),
    };

    let mut presets = div()
        .id("sailing-sim-presets")
        .flex()
        .items_center()
        .gap_1();
    for preset in [8.0_f64, 12.0, 24.0, 48.0] {
        let entity = cx.entity();
        presets = presets.child(
            Button::new(
                format!("sailing-sim-hours-{}", preset as u32),
                format!("{preset:.0}h"),
            )
            .variant(if (hours - preset).abs() < f64::EPSILON {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Subtle
            })
            .size(ControlSize::Sm)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.set_sailing_sim_hours(preset, cx));
            }),
        );
    }

    let hours_entity = cx.entity();
    let hours_control = Stepper::new("sailing-sim-hours", hours)
        .range(MIN_HOURS, MAX_HOURS)
        .step(0.25)
        .precision(2)
        .on_change(move |value, _window, cx| {
            hours_entity.update(cx, |state, cx| state.set_sailing_sim_hours(value, cx));
        });

    let island_entity = cx.entity();
    let islands: Vec<Choice> = (0..available)
        .map(|island| {
            Choice::new(
                island.to_string(),
                format!("Island {} · {} dist", island + 1, island_distance(island)),
            )
            .note(format!("{} artifacts", island_artifact_count(island)))
        })
        .collect();
    let island_select = Select::new("sailing-sim-island", islands)
        .label("Destination")
        .selected(island.min(available - 1).to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(island) = value.parse::<usize>() {
                island_entity.update(cx, |state, cx| state.set_sailing_sim_island(island, cx));
            }
        });

    let trials_entity = cx.entity();
    let trials_control = Stepper::new("sailing-sim-trials", trials as f64)
        .range(MIN_TRIALS as f64, MAX_TRIALS as f64)
        .step(100.0)
        .precision(0)
        .on_change(move |value, _window, cx| {
            trials_entity.update(cx, |state, cx| {
                state.set_sailing_sim_trials(value.round().max(0.0) as usize, cx);
            });
        });

    let run_entity = cx.entity();
    let run = Button::new(
        "sailing-sim-run",
        if running {
            "Running…"
        } else {
            "Run simulation"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_sailing_simulation(cx));
    });

    let cancel_entity = cx.entity();
    let cancel = Button::new("sailing-sim-cancel", "Cancel")
        .variant(ButtonVariant::Subtle)
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
            div()
                .id("sailing-sim-controls")
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
                        .child(Caption::new("Interval presets").text_color(muted))
                        .child(presets),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Hours").text_color(muted))
                        .child(hours_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(gpui::px(220.0))
                        .child(Caption::new("Destination").text_color(muted))
                        .child(island_select),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Trials").text_color(muted))
                        .child(trials_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Assignment").text_color(muted))
                        .child(Caption::new(assignment).text_color(muted)),
                )
                .child(div().flex().items_center().gap_2().child(run).child(cancel)),
        )
        .into_any_element()
}

/// The stored run: the summary cards, the currency spread, the tier mix and
/// each pile position's expected chest.
fn results_card(result: &SimResult, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let pile = result.pile;
    let events = result.events;
    let assignment = match result.setup.mode {
        AssignmentMode::Optimized => format!("{} optimized", result.setup.goal.id()),
        AssignmentMode::Saved => "current save".into(),
    };
    let mut header = CardHeader::new("Simulation results").description(format!(
        "{}h · {} trials · seed {} · Island {} · {assignment} assignment · empty/fresh start",
        result.setup.hours,
        result.setup.trials,
        result.setup.seed,
        result.setup.island + 1
    ));
    if stale {
        header = header.action(crate::ui::task::stale_badge("sailing-sim"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(crate::ui::task::stale_banner("sailing-sim"));
    }
    let cards = div()
        .id("sailing-sim-kpis")
        .flex()
        .flex_wrap()
        .items_stretch()
        .gap_2()
        .child(text_kpi(
            "sailing-sim-pile-count",
            "Average final pile",
            &format!("{:.2} chests", pile.count.mean),
            &format!("{} near-full target", result.setup.target_count),
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-pile-value",
            "Final opened value",
            &format::value(pile.value.mean),
            &summary_line(&pile.value),
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-average",
            "Average / banked chest",
            &format::value(pile.average_value_per_chest),
            "Total opened value ÷ average chest count",
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-retained",
            "Retained vs active",
            &format!("{:.1}%", result.retained_fraction * 100.0),
            "Final pile value ÷ collect-everything value",
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-generated",
            "Generated",
            &format::value(events.generated),
            &format!(
                "{} accepted before near-full",
                format::value(events.accepted)
            ),
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-improvements",
            "Pile improvements",
            &format!("{} upgrades", format::value(events.upgrades)),
            &format!(
                "{} higher-tier replacements",
                format::value(events.replacements)
            ),
            muted,
        ))
        .child(text_kpi(
            "sailing-sim-discards",
            "Discarded / blocked",
            &format::value(events.filter_discards + events.overflow_discards + events.blocked),
            &format!(
                "{} filter · {} overflow · {} full",
                format::value(events.filter_discards),
                format::value(events.overflow_discards),
                format::value(events.blocked)
            ),
            muted,
        ));
    card.child(
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_3()
            .child(cards)
            .child(currency_table(&result.currencies))
            .child(tier_table(result))
            .child(slot_table(result, muted))
            .child(Caption::new(
                "Final pile values include every guaranteed payout. Each payout picks its currency \
                 independently. P10/P50/P90 describe the spread across trials, not confidence \
                 intervals around the mean. Active collection opens every generated chest \
                 immediately and has no capacity loss.",
            )
            .text_color(muted)),
    )
    .into_any_element()
}

fn summary_line(summary: &SimSummary) -> String {
    format!(
        "P10 {} · P50 {} · P90 {}",
        format::value(summary.p10),
        format::value(summary.median),
        format::value(summary.p90)
    )
}

fn currency_table(currencies: &SimCurrencies) -> AnyElement {
    let mut table = Table::new(["Currency", "Mean value", "P10", "P50", "P90", "Avg payouts"]);
    for (key, label, row) in [
        ("gold", "Gold Bars", currencies.gold),
        (
            "primary",
            "Primary treasure (chest island)",
            currencies.primary,
        ),
        ("rare", "Rare treasure (chest island)", currencies.rare),
    ] {
        table = table.row([
            div()
                .id(format!("sailing-sim-currency-{key}"))
                .child(label)
                .into_any_element(),
            div()
                .child(format::value(row.value.mean))
                .into_any_element(),
            div().child(format::value(row.value.p10)).into_any_element(),
            div()
                .child(format::value(row.value.median))
                .into_any_element(),
            div().child(format::value(row.value.p90)).into_any_element(),
            div()
                .child(format!("{:.2}", row.payouts.mean))
                .into_any_element(),
        ]);
    }
    div()
        .id("sailing-sim-currency-table")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(gpui::px(14.0))
                .child("Currency outcome distribution"),
        )
        .child(table)
        .into_any_element()
}

fn tier_table(result: &SimResult) -> AnyElement {
    let total = result.pile.count.mean;
    let mut table = Table::new([
        "Tier",
        "Average count",
        "Pile share",
        "Average value / payout",
        "Average opened value / chest",
        "Opened value contribution",
    ]);
    for row in &result.tiers {
        let share = if total > 0.0 {
            row.average_count / total
        } else {
            0.0
        };
        table = table.row([
            div()
                .id(format!("sailing-sim-tier-{}", row.tier))
                .child(format!("Tier {}", row.tier))
                .into_any_element(),
            div()
                .child(format!("{:.3}", row.average_count))
                .into_any_element(),
            div()
                .child(format!("{:.1}%", share * 100.0))
                .into_any_element(),
            div()
                .child(format::value(row.average_value_per_payout))
                .into_any_element(),
            div()
                .child(format::value(row.average_opened_value))
                .into_any_element(),
            div()
                .child(format::value(row.opened_value_contribution))
                .into_any_element(),
        ]);
    }
    div()
        .id("sailing-sim-tier-table")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(gpui::px(14.0))
                .child("Final pile tier composition"),
        )
        .child(table)
        .into_any_element()
}

fn slot_table(result: &SimResult, muted: gpui::Hsla) -> AnyElement {
    let mut table = Table::new([
        "Position",
        "Occupied",
        "Average tier",
        "Most likely tier",
        "Value / payout",
        "Payouts",
        "Opened value",
        "Artifact x",
        "Gold",
        "Primary",
        "Rare",
    ]);
    for slot in result.slots.iter().filter(|slot| slot.occupancy > 0.0) {
        table = table.row([
            div()
                .id(format!("sailing-sim-slot-{}", slot.index))
                .child(format!("#{}", slot.index + 1))
                .into_any_element(),
            div()
                .child(format!("{:.1}%", slot.occupancy * 100.0))
                .into_any_element(),
            div()
                .child(format!("{:.2}", slot.average_tier))
                .into_any_element(),
            div()
                .child(
                    slot.most_likely_tier
                        .map_or_else(|| "—".into(), |tier| format!("Tier {tier}")),
                )
                .into_any_element(),
            div()
                .child(format::value(slot.average_value_per_payout))
                .into_any_element(),
            div()
                .child(format!("{:.2}", slot.average_payouts))
                .into_any_element(),
            div()
                .child(format::value(slot.average_opened_value))
                .into_any_element(),
            div()
                .child(format!(
                    "{}x",
                    format::value(slot.average_artifact_multiplier)
                ))
                .into_any_element(),
            div()
                .child(format::value(slot.average_currency_value[0]))
                .into_any_element(),
            div()
                .child(format::value(slot.average_currency_value[1]))
                .into_any_element(),
            div()
                .child(format::value(slot.average_currency_value[2]))
                .into_any_element(),
        ]);
    }
    div()
        .id("sailing-sim-slot-table")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(gpui::px(14.0))
                .child("Average banked chest by pile position"),
        )
        .child(table)
        .child(
            Caption::new(
                "Position is opening order: #1 opens next and is the first banked chest the \
                 near-full scan reads. Averages are conditional on occupancy except the currency \
                 columns, which are the expected contribution per trial.",
            )
            .text_color(muted),
        )
        .into_any_element()
}
