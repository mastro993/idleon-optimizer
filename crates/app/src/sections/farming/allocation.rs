//! The Land Rank allocation: the recommended levels for the objective.
//!
//! The run goes through the shared task runner, so it reports progress,
//! cancels without dropping the previous result, and a later objective, mode or
//! save edit marks the stored allocation stale.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_farming::{self as farming, Allocation, Mode, Objective, RANK_UPGRADES};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// The recommended allocation, its scope buttons and the twenty upgrade rows.
pub(super) fn allocation_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.farming_task(cx);
    let running = task.running;
    let result = task.result.clone();
    let mut body = div()
        .id("farming-allocation")
        .role(gpui::Role::Group)
        .aria_label("Land Rank allocation")
        .flex()
        .flex_col()
        .gap_3()
        .px_4()
        .pb_4()
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
            body = body.child(summary(allocation, muted));
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
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(format!(
                            "{} point(s) stay unused: {}",
                            format::number(f64::from(allocation.points_unused())),
                            unused_reason(allocation)
                        )),
                );
            }
            body = body.child(rows_table(state, allocation, muted));
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
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let mut modes = div().id("farming-mode").flex().items_center().gap_1();
    for mode in Mode::ALL {
        let clicks = entity.clone();
        let selected = state.farming.mode == mode;
        modes = modes.child(
            Button::new(format!("farming-mode-{}", mode.id()), mode.label())
                .variant(if selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    clicks.update(cx, |state, cx| state.set_farming_mode(mode, cx));
                }),
        );
    }
    let run_entity = entity.clone();
    let run = Button::new("farming-run", "Allocate")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .disabled(!state.save_loaded || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_farming(cx));
        });
    let cancel_entity = entity.clone();
    let cancel = Button::new("farming-cancel", "Cancel")
        .size(ControlSize::Sm)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_farming(cx));
        });
    div()
        .id("farming-allocation-toolbar")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(modes)
        .child(div().flex().items_center().gap_2().child(run).child(cancel))
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
fn summary(allocation: &Allocation, muted: gpui::Hsla) -> AnyElement {
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
    div()
        .id("farming-allocation-summary")
        .role(gpui::Role::Label)
        .aria_label(aria.clone())
        .flex()
        .flex_wrap()
        .gap_4()
        .child(figure(
            "Points available",
            format::number(f64::from(allocation.points_available)),
            muted,
        ))
        .child(figure(
            "Points assigned",
            format::number(f64::from(allocation.points_spent)),
            muted,
        ))
        .child(figure(
            "Points unused",
            format::number(f64::from(allocation.points_unused())),
            muted,
        ))
        .child(
            div()
                .id("farming-allocation-gain")
                .role(gpui::Role::Label)
                .aria_label(format!("Projected gain: {gain}"))
                .flex()
                .flex_col()
                .child(Caption::new("Projected gain").text_color(muted))
                .child(div().text_size(px(15.0)).child(gain)),
        )
        .into_any_element()
}

/// One labelled figure in the summary.
fn figure(label: &'static str, value: String, muted: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(Caption::new(label).text_color(muted))
        .child(div().text_size(px(15.0)).child(value))
        .into_any_element()
}

/// The twenty upgrade rows: saved levels against the recommended ones.
fn rows_table(state: &AppState, allocation: &Allocation, muted: gpui::Hsla) -> AnyElement {
    let data = farming::data();
    let total = state.farm.total_land_rank();
    let fifth_max = state.farm.fifth_column_max();
    let mut table = Table::new([
        "Upgrade",
        "Unlock rank",
        "Current Lv",
        "Target Lv",
        "Move",
        "Current bonus",
        "Target bonus",
        "Status",
    ]);
    for (index, &after) in allocation.levels.iter().enumerate().take(RANK_UPGRADES) {
        // The saved levels, not the allocation's baseline: a reallocation can
        // take a row below what the save carries.
        let before = state.farm.rank_levels.get(index).copied().unwrap_or(0);
        let delta = i64::from(after) - i64::from(before);
        let unlocked = total >= data.rank_unlocks.get(index).copied().unwrap_or(0);
        let capped = index % 5 == 4 && after >= fifth_max;
        let status = if !unlocked {
            Badge::new("Locked").tone(Tone::Danger)
        } else if capped {
            Badge::new("Capped").tone(Tone::Warning)
        } else {
            Badge::new("Open").tone(Tone::Success)
        };
        let name = data
            .rank_names
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("Row {index}"));
        table = table.row([
            div()
                .id(format!("farming-rank-row-{index}"))
                .child(name)
                .into_any_element(),
            div()
                .child(format::number(f64::from(
                    data.rank_unlocks.get(index).copied().unwrap_or(0),
                )))
                .into_any_element(),
            div().child(before.to_string()).into_any_element(),
            div().child(after.to_string()).into_any_element(),
            div()
                .child(if delta > 0 {
                    format!("+{delta}")
                } else {
                    delta.to_string()
                })
                .into_any_element(),
            div()
                .child(format::number(state.farm.rank_bonus_at(index, before)))
                .into_any_element(),
            div()
                .child(format::number(state.farm.rank_bonus_at(index, after)))
                .into_any_element(),
            div().child(status).into_any_element(),
        ]);
    }
    div()
        .id("farming-allocation-rows")
        .flex()
        .flex_col()
        .gap_2()
        .child(table)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Every fifth row is linear and capped by the fifth Land Rank column; the others decay toward their base.",
                ),
        )
        .into_any_element()
}

/// Why the allocator left points unspent, as the upstream callout phrases it.
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
