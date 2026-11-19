//! The Land Rank allocation: the recommended levels for the objective.
//!
//! The run goes through the shared task runner, so it reports progress,
//! cancels without dropping the previous result, and a later objective, mode or
//! save edit marks the stored allocation stale.

use super::table::{Col, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_farming::{self as farming, Allocation, Mode, Objective, RANK_UPGRADES};
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone};

/// The recommended allocation, its scope buttons and the twenty upgrade rows.
pub(super) fn allocation_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let task = state.farming_task(cx);
    let running = task.running;
    let result = task.result.clone();
    let mut body = v_flex()
        .id("farming-allocation")
        .role(gpui::Role::Group)
        .aria_label("Land Rank allocation")
        .gap_3()
        .child(toolbar(state, running, cx));

    if running {
        if let Some(progress) = task.progress.clone() {
            let cancel = cx.entity();
            body = body.child(run_chrome::progress_card(
                "farming",
                "Allocating Land Rank",
                "Runs off the UI thread. Cancelling keeps the result this run replaced.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_farming(cx));
                },
            ));
        }
    } else if task.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "farming-cancelled",
            result.is_some(),
        ));
    }
    if task.stale {
        body = body.child(run_chrome::stale_banner("farming-stale"));
    }

    match &result {
        Some(allocation) => {
            body = body.child(summary(allocation));
            if allocation.points_unused() > 0 {
                body = body.child(
                    div()
                        .id("farming-allocation-unused")
                        .role(gpui::Role::Label)
                        .aria_label(format!(
                            "{} point(s) unused: {}",
                            allocation.points_unused(),
                            unused_reason(allocation)
                        ))
                        .child(note(
                            format!(
                                "{} point(s) stay unused: {}",
                                format::number(f64::from(allocation.points_unused())),
                                unused_reason(allocation)
                            ),
                            cx,
                        )),
                );
            }
            body = body.child(rows_table(state, allocation, cx));
        }
        None => {
            body = body.child(
                div()
                    .id("farming-allocation-empty")
                    .role(gpui::Role::Label)
                    .aria_label("No allocation yet")
                    .child(
                        Caption::new(if state.save_loaded {
                            "Run the allocator to spend the account's Land Rank points on this objective."
                        } else {
                            "Import a save to read its Land Rank points, plot ranks and saved levels."
                        })
                        .text_color(muted),
                    ),
            );
        }
    }
    Card::new()
        .header(
            CardHeader::new("Land Rank allocation").description(if state.save_loaded {
                format!(
                    "{} point(s) unspent · fifth column allows {}",
                    format::number(state.farm.land_rank_points_left().max(0) as f64),
                    state.farm.fifth_column_max()
                )
            } else {
                "Import a save to allocate its Land Rank".into()
            }),
        )
        .child(body)
        .into_any_element()
}

/// The scope buttons and the run.
fn toolbar(state: &mut AppState, running: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let mut modes = h_flex().id("farming-mode").gap_1();
    for mode in Mode::ALL {
        let clicks = entity.clone();
        let selected = state.farming.mode == mode;
        modes = modes.child(
            Button::new(format!("farming-mode-{}", mode.id()))
                .label(mode.label())
                .when(selected, |button| button.primary())
                .when(!selected, |button| button.secondary())
                .small()
                .on_click(move |_event, _window, cx| {
                    clicks.update(cx, |state, cx| state.set_farming_mode(mode, cx));
                }),
        );
    }
    let run_entity = entity.clone();
    let run = Button::new("farming-run")
        .label("Allocate")
        .primary()
        .small()
        .disabled(!state.save_loaded || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_farming(cx));
        });
    let cancel_entity = entity.clone();
    let cancel = Button::new("farming-cancel")
        .label("Cancel")
        .small()
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_farming(cx));
        });
    h_flex()
        .id("farming-allocation-toolbar")
        .gap_3()
        .child(modes)
        .child(h_flex().gap_2().child(run).child(cancel))
        .child(
            div()
                .id("farming-allocation-objective")
                .role(gpui::Role::Label)
                .aria_label(format!("Objective: {}", state.farming.objective.label()))
                .child(
                    Caption::new(format!(
                        "{} · {}",
                        state.farming.objective.label(),
                        state.farming.mode.label()
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// Points available, spent, unused and the projected gain.
fn summary(allocation: &Allocation) -> AnyElement {
    let gain = if allocation.objective == Objective::Evolution {
        format!(
            "{} → {}",
            format::chance(allocation.initial_score),
            format::chance(allocation.final_score)
        )
    } else {
        format::percent(allocation.relative_gain())
    };
    let aria = format!(
        "Projected gain: {gain} · {} of {} points assigned",
        allocation.points_spent, allocation.points_available
    );
    let points = |value: u32| format::number(f64::from(value));
    div()
        .id("farming-allocation-summary")
        .role(gpui::Role::Label)
        .aria_label(aria)
        .child(
            KpiStrip::new()
                .child(Statistic::new(
                    "farming-allocation-available",
                    "Points available",
                    points(allocation.points_available),
                ))
                .child(Statistic::new(
                    "farming-allocation-spent",
                    "Points assigned",
                    points(allocation.points_spent),
                ))
                .child(Statistic::new(
                    "farming-allocation-unused-points",
                    "Points unused",
                    points(allocation.points_unused()),
                ))
                .child(
                    div()
                        .id("farming-allocation-gain")
                        .role(gpui::Role::Label)
                        .aria_label(format!("Projected gain: {gain}"))
                        .child(Statistic::new(
                            "farming-allocation-gain-value",
                            "Projected gain",
                            gain,
                        )),
                ),
        )
        .into_any_element()
}

/// The twenty upgrade rows: saved levels against the recommended ones.
fn rows_table(state: &AppState, allocation: &Allocation, cx: &App) -> AnyElement {
    let data = farming::data();
    let total = state.farm.total_land_rank();
    let fifth_max = state.farm.fifth_column_max();
    let rows = allocation
        .levels
        .iter()
        .enumerate()
        .take(RANK_UPGRADES)
        .map(|(index, &after)| {
            // The saved levels, not the allocation's baseline: a reallocation
            // can take a row below what the save carries.
            let before = state.farm.rank_levels.get(index).copied().unwrap_or(0);
            let delta = i64::from(after) - i64::from(before);
            let unlock = data.rank_unlocks.get(index).copied().unwrap_or(0);
            let capped = index % 5 == 4 && after >= fifth_max;
            let status = if total < unlock {
                tag("Locked", Tone::Danger)
            } else if capped {
                tag("Capped", Tone::Warning)
            } else {
                tag("Open", Tone::Success)
            };
            let name = data
                .rank_names
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("Row {index}"));
            [
                div()
                    .id(format!("farming-rank-row-{index}"))
                    .child(name)
                    .into_any_element(),
                text(format::number(f64::from(unlock))),
                text(before.to_string()),
                text(after.to_string()),
                text(if delta > 0 {
                    format!("+{delta}")
                } else {
                    delta.to_string()
                }),
                text(format::number(state.farm.rank_bonus_at(index, before))),
                text(format::number(state.farm.rank_bonus_at(index, after))),
                status.into_any_element(),
            ]
        });
    let table = table(
        "farming-allocation-rows",
        &[
            Col::Text("Upgrade"),
            Col::Number("Unlock rank"),
            Col::Number("Current Lv"),
            Col::Number("Target Lv"),
            Col::Number("Move"),
            Col::Number("Current bonus"),
            Col::Number("Target bonus"),
            Col::Text("Status"),
        ],
        rows,
        cx,
    );
    v_flex()
        .id("farming-allocation-rows")
        .gap_2()
        .child(table)
        .child(note(
            "Every fifth row is linear and capped by the fifth Land Rank column; the others decay toward their base.",
            cx,
        ))
        .into_any_element()
}

/// Why the allocator left points unspent, as the callout phrases it.
fn unused_reason(allocation: &Allocation) -> &'static str {
    match allocation.objective {
        Objective::CropValue | Objective::Beans => {
            "the current crop-value cap makes further production bonuses ineffective"
        }
        Objective::Growth => "Land Rank has no growth-speed upgrade channel",
        Objective::Evolution if allocation.final_score >= 1.0 => {
            "the selected evolution roll is already guaranteed"
        }
        _ => "no remaining unlocked upgrade raises this objective",
    }
}
