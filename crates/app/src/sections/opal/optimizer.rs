//! The optimizer tab: the target and total, the plan run and its table.
//!
//! The plan comes off the shared task runner, so progress, cancel and the stale
//! marker read like every other section. The card prints the distribution, the
//! rate each villager earns and the copyable text the section writes to the
//! clipboard.

use super::pane;
use super::state::target_choices;
use super::table::{Col, figure, note, table, tag, text};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_opal::Opal;
use idleon_opal::{NAMES, Plan, PlanTarget, VILLAGERS};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Tone, labeled, number_width, show_number,
};

/// The optimizer pane: the run controls, the progress chrome and the plan.
pub(crate) fn optimizer_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.save.clone() else {
        return div().into_any_element();
    };
    let task = state.opal.task(cx);
    let target = plan_target(state, &opal);
    let mut body = v_flex()
        .id("opal-optimizer")
        .gap(PANEL_GAP)
        .child(controls(state, &opal, task.running, window, cx))
        .child(note(
            format!(
                "Maximizes {} for the selected total. The search checks every lowest count and \
                 both placements a villager can profitably take, so the plan is never worse than \
                 the saved split.",
                objective_label(target)
            ),
            cx,
        ));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "opal-plan",
            "Optimizing the opal distribution",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_opal_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "opal-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.as_ref() {
        body = body.child(plan_card(&opal, plan, task.stale, cx));
    }
    pane("opal-optimizer-scroll", body)
}

/// Which rate the current selection maximizes.
fn plan_target(state: &AppState, opal: &Opal) -> PlanTarget {
    match state.opal.target {
        Some(villager) if opal.unlocked(villager) => PlanTarget::Villager(villager),
        _ => PlanTarget::Total,
    }
}

fn objective_label(target: PlanTarget) -> String {
    match target {
        PlanTarget::Total => "the total EXP/hr across every unlocked villager".into(),
        PlanTarget::Villager(villager) => format!(
            "{}'s own EXP/hr",
            NAMES.get(villager).copied().unwrap_or("the villager")
        ),
    }
}

/// A whole opal count, grouped, with a leading `+` for a gain.
fn signed(value: f64) -> String {
    if value > 0.0 {
        format!("+{}", format::count(value as u64))
    } else if value < 0.0 {
        format!("-{}", format::count((-value) as u64))
    } else {
        "0".to_owned()
    }
}

/// The target picker, the total, the saved-total reset and the run action.
fn controls(
    state: &mut AppState,
    opal: &Opal,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let total = state.opal.total;
    let saved = opal.total_opals();
    let hint = if opal.unlocked_count() == 0 {
        "no villager is unlocked: import a save first"
    } else if total <= 0.0 {
        "raise the total above zero to plan a split"
    } else {
        "the plan never spends more than this total"
    };
    let target = match plan_target(state, opal) {
        PlanTarget::Villager(villager) => Some(villager),
        PlanTarget::Total => None,
    };
    // Values flow section value -> control; edits come back through the
    // subscriptions in `State::new`.
    let section = &state.opal;
    section
        .target_select
        .show(target_choices(Some(opal)), Some(&target), window, cx);
    show_number(&section.total_field, total, 0, window, cx);

    let reset_entity = cx.entity();
    let reset = Button::new("opal-total-saved")
        .label("Use saved total")
        .icon(IconName::RotateCcw)
        .small()
        .disabled(total == saved)
        .on_click(move |_event, window, cx| {
            // The reset replaces what was typed, so it ends the edit: a focused
            // field would keep its typed number (`show_number`).
            window.blur(cx);
            reset_entity.update(cx, |state, cx| state.opal.use_saved_total(cx));
        });

    let run_entity = cx.entity();
    let run = Button::new("opal-run")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .icon(IconName::Play)
        .disabled(running || total <= 0.0 || opal.unlocked_count() == 0)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_opal_plan(cx));
        });

    Card::new()
        .header(CardHeader::new("Plan a run").description(
            "Picks which rate to grow and how many opals to split across the unlocked villagers.",
        ))
        .child(
            h_flex()
                .id("opal-plan-controls")
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(labeled(
                    "Target",
                    div()
                        .flex_none()
                        .max_w_full()
                        .w(section.target_select.width(window, cx))
                        .child(
                            Select::new(section.target_select.state())
                                .id("opal-target")
                                .accessibility_label("Target")
                                .small(),
                        ),
                ))
                .child(labeled(
                    "Total opals",
                    ui::number_field("opal-total", &section.total_field, "Total opals")
                        .w(number_width(&section.total_field, window, cx)),
                ))
                .child(reset)
                .child(
                    Caption::new(format!(
                        "Saved total {} · {hint}",
                        format::count(saved as u64)
                    ))
                    .text_color(muted),
                ),
        )
        .child(div().child(run))
        .into_any_element()
}

/// The stored plan: what it earns against the save, the distribution and the copy
/// action.
fn plan_card(opal: &Opal, plan: &Plan, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let improvement = plan.improvement(opal);
    let improvement_text = improvement.map_or_else(|| "—".to_owned(), format::percent);
    let summary = format!(
        "{} · {} opals · {} → {} EXP/hr · {} improvement",
        plan.target.label(),
        format::count(plan.total as u64),
        format::rate(plan.saved_rate(opal)),
        format::rate(plan.rate(opal)),
        improvement_text
    );
    let mut header = CardHeader::new("Plan").description(format!(
        "{} · {} opals across {} villagers",
        plan.target.label(),
        format::count(plan.total as u64),
        opal.unlocked_count()
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("opal-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("opal-plan"));
    }

    let rows = (0..VILLAGERS)
        .filter(|villager| opal.unlocked(*villager))
        .map(|villager| {
            let label = h_flex()
                .gap_2()
                .child(NAMES[villager])
                .when(plan.target == PlanTarget::Villager(villager), |label| {
                    label.child(tag("Target", Tone::Info))
                });
            [
                label.into_any_element(),
                text(format!("★{}", opal.villagers[villager].stars as i64)),
                text(format::count(plan.current[villager] as u64)),
                text(format::count(plan.opals[villager] as u64)),
                text(signed(plan.change(villager))),
                text(format::rate(opal.exp_rate(villager, &plan.current))),
                text(format::rate(opal.exp_rate(villager, &plan.opals))),
            ]
        });
    let table = table(
        "opal-plan",
        &[
            Col::Text("Villager"),
            Col::Number("Stars"),
            Col::Number("Current Opals"),
            Col::Number("Planned"),
            Col::Number("Change"),
            Col::Number("Current EXP/hr"),
            Col::Number("Planned EXP/hr"),
        ],
        rows,
        cx,
    );

    let mut totals = format!(
        "The plan spends {} of {} opals; the save holds {}.",
        format::count(plan.spent() as u64),
        format::count(plan.total as u64),
        format::count(plan.current.iter().sum::<f64>() as u64)
    );
    if plan.total != opal.total_opals() {
        totals.push_str(
            " The total differs from the save, so the comparison reads the same split for both.",
        );
    }
    let copy = cx.entity();
    card.child(
        v_flex()
            .id("opal-plan-result")
            .gap_3()
            .child(
                div()
                    .id("opal-plan-summary")
                    .role(Role::Label)
                    .aria_label(summary),
            )
            .child(
                div().id("opal-plan-figures").child(
                    KpiStrip::new()
                        .child(figure(
                            "opal-plan-current",
                            "Current EXP/hr",
                            format::rate(plan.saved_rate(opal)),
                            "The saved split, for the same target",
                        ))
                        .child(figure(
                            "opal-plan-rate",
                            "Planned EXP/hr",
                            format::rate(plan.rate(opal)),
                            "The best split of the total",
                        ))
                        .child(figure(
                            "opal-plan-improvement",
                            "Improvement",
                            improvement_text,
                            "Planned over the saved split",
                        ))
                        .child(figure(
                            "opal-plan-lowest",
                            "Lowest Villager",
                            format::count(opal.min_opals(&plan.opals) as u64),
                            "What Equal Spread scales with",
                        )),
                ),
            )
            .child(table)
            .child(note(totals, cx))
            .child(
                div().child(
                    Button::new("opal-plan-copy")
                        .label("Copy plan")
                        .icon(IconName::Copy)
                        .on_click(move |_event, _window, cx| {
                            copy.update(cx, |state, cx| state.copy_opal_plan(cx));
                        }),
                ),
            ),
    )
    .into_any_element()
}
