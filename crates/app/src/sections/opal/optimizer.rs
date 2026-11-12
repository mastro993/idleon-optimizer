//! The optimizer tab: the target and total, the plan run and its table.
//!
//! The plan comes off the shared task runner, so progress, cancel and the stale
//! marker read like every other section. The card prints the distribution, the
//! rate each villager earns and the copyable text the section writes to the
//! clipboard.

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_opal::Opal;
use idleon_opal::{NAMES, Plan, PlanTarget, VILLAGERS};
use idleon_ui::format;

/// The largest total the stepper offers; a save above it raises the ceiling.
const MAX_TOTAL: f64 = 1_000_000.0;

/// The optimizer pane: the run controls, the progress chrome and the plan.
pub(crate) fn optimizer_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.clone() else {
        return div().into_any_element();
    };
    let task = state.opal_task(cx);
    let muted = cx.theme().colors.fg_muted;
    let target = plan_target(state, &opal);
    let mut body = div()
        .id("opal-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls(state, &opal, cx))
        .child(
            Caption::new(format!(
                "Maximizes {} for the selected total. The search checks every lowest count and \
                 both placements a villager can profitably take, so the plan is never worse than \
                 the saved split.",
                objective_label(target)
            ))
            .text_color(muted),
        );
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
    if let Some(plan) = task.result.clone() {
        body = body.child(plan_card(&opal, &plan, task.stale, cx));
    }
    ScrollArea::new("opal-optimizer-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// Which rate the current selection maximizes.
fn plan_target(state: &AppState, opal: &Opal) -> PlanTarget {
    match state.opal_target {
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
fn controls(state: &mut AppState, opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let total = state.opal_total;
    let saved = opal.total_opals();
    let running = state.opal_task(cx).running;
    let hint = if opal.unlocked_count() == 0 {
        "no villager is unlocked: import a save first"
    } else if total <= 0.0 {
        "raise the total above zero to plan a split"
    } else {
        "the plan never spends more than this total"
    };

    let selected = match plan_target(state, opal) {
        PlanTarget::Villager(villager) => villager.to_string(),
        PlanTarget::Total => "-1".to_owned(),
    };
    let choices = std::iter::once(Choice::new("-1", "All villagers (total EXP)")).chain(
        (0..VILLAGERS)
            .filter(|villager| opal.unlocked(*villager))
            .map(|villager| {
                Choice::new(
                    villager.to_string(),
                    format!(
                        "{} (★{})",
                        NAMES[villager], opal.villagers[villager].stars as i64
                    ),
                )
            }),
    );
    let target_entity = cx.entity();
    let select = Select::new("opal-target", choices)
        .label("Target")
        .selected(selected)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let target = value
                .parse::<usize>()
                .ok()
                .filter(|villager| *villager < VILLAGERS);
            target_entity.update(cx, |state, cx| state.set_opal_target(target, cx));
        });

    let total_entity = cx.entity();
    let stepper = Stepper::new("opal-total", total)
        .range(0.0, MAX_TOTAL.max(total))
        .step(1.0)
        .precision(0)
        .on_change(move |value, _window, cx| {
            total_entity.update(cx, |state, cx| state.set_opal_total(value, cx));
        });

    let reset_entity = cx.entity();
    let reset = Button::new("opal-total-saved", "Use saved total")
        .icon(IconName::RotateCcw)
        .size(ControlSize::Sm)
        .disabled(total == saved)
        .on_click(move |_event, _window, cx| {
            reset_entity.update(cx, |state, cx| state.use_saved_opal_total(cx));
        });

    let run_entity = cx.entity();
    let run = Button::new("opal-run", if running { "Optimizing…" } else { "Optimize" })
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
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id("opal-plan-controls")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_3()
                        .child(select)
                        .child(Caption::new("Total").text_color(muted))
                        .child(stepper)
                        .child(Caption::new("opals").text_color(muted))
                        .child(reset)
                        .child(
                            Caption::new(format!(
                                "Saved total {} · {hint}",
                                format::count(saved as u64)
                            ))
                            .text_color(muted),
                        ),
                )
                .child(run),
        )
        .into_any_element()
}

/// The stored plan: what it earns against the save, the distribution and the copy
/// action.
fn plan_card(opal: &Opal, plan: &Plan, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
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

    let mut table = Table::new([
        "Villager",
        "Stars",
        "Current Opals",
        "Planned",
        "Change",
        "Current EXP/hr",
        "Planned EXP/hr",
    ]);
    for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
        let mut label = div().flex().items_center().gap_2().child(NAMES[villager]);
        if plan.target == PlanTarget::Villager(villager) {
            label = label.child(Badge::new("Target").tone(Tone::Info));
        }
        let change = signed(plan.change(villager));
        table = table.row([
            label.into_any_element(),
            div()
                .child(format!("★{}", opal.villagers[villager].stars as i64))
                .into_any_element(),
            div()
                .child(format::count(plan.current[villager] as u64))
                .into_any_element(),
            div()
                .child(format::count(plan.opals[villager] as u64))
                .into_any_element(),
            div().child(change).into_any_element(),
            div()
                .child(format::rate(opal.exp_rate(villager, &plan.current)))
                .into_any_element(),
            div()
                .child(format::rate(opal.exp_rate(villager, &plan.opals)))
                .into_any_element(),
        ]);
    }

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
        div()
            .id("opal-plan-result")
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(
                div()
                    .id("opal-plan-summary")
                    .role(Role::Label)
                    .aria_label(summary),
            )
            .child(
                div()
                    .id("opal-plan-figures")
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(crate::ui::layout::figure(
                        "opal-plan-current",
                        "Current EXP/hr",
                        format::rate(plan.saved_rate(opal)),
                        "The saved split, for the same target",
                        cx,
                    ))
                    .child(crate::ui::layout::figure(
                        "opal-plan-rate",
                        "Planned EXP/hr",
                        format::rate(plan.rate(opal)),
                        "The best split of the total",
                        cx,
                    ))
                    .child(crate::ui::layout::figure(
                        "opal-plan-improvement",
                        "Improvement",
                        improvement_text.clone(),
                        "Planned over the saved split",
                        cx,
                    ))
                    .child(crate::ui::layout::figure(
                        "opal-plan-lowest",
                        "Lowest Villager",
                        format::count(opal.min_opals(&plan.opals) as u64),
                        "What Equal Spread scales with",
                        cx,
                    )),
            )
            .child(div().id("opal-plan-table").min_w_0().child(table))
            .child(Caption::new(totals).text_color(muted))
            .child(
                Button::new("opal-plan-copy", "Copy plan")
                    .icon(IconName::Copy)
                    .on_click(move |_event, _window, cx| {
                        copy.update(cx, |state, cx| state.copy_opal_plan(cx));
                    }),
            ),
    )
    .into_any_element()
}
