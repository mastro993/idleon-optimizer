//! The Clamworks optimizer: the budgeted upgrade plan and the upstream priority table.
//!
//! `clamworks.plan` repeats the upstream ranking (`rankUpgrades`) at the levels
//! each purchase reaches, so every step pays the `upgNextCost` the game would
//! charge there. The plan runs through the shared [`crate::task::TaskRunner`]:
//! its progress and cancel chrome come from `ui::task`, and a result whose
//! budget or multikill changed is marked stale instead of dropped.

use super::kpi;
use super::upgrades::{effect, money};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div};
use idleon_core::Clamworks;
use idleon_core::clamworks::{Category, Plan, PlanStep, UpgradeRank};

/// The largest budget the control offers, so the stepper stays finite.
const MAX_BUDGET: f64 = 1e18;

pub fn optimizer_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let clamworks = state.clamworks_view();
    let multikill = state.clamworks_multikill;
    let task = state.clamworks_task(cx);
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id("clamworks-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(plan_controls(state, cx))
        .child(
            Caption::new(
                "Each purchase is the best marginal pearls-per-kill gain the budget can afford at the levels the plan has reached, so cost reductions from earlier purchases are already priced in.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(crate::ui::task::progress_card(
            "clamworks-plan",
            "Planning the purchases",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_clamworks_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(crate::ui::task::cancelled_banner(
            "clamworks-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.clone() {
        body = body.child(plan_card(&plan, task.stale, cx));
    }
    let body = body.child(priority_card(&clamworks, multikill));
    ScrollArea::new("clamworks-optimizer-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The budget, how to run the plan, and what running does.
fn plan_controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let budget = state.clamworks_budget;
    let balance = state.clamworks.pearl_balance;
    let running = state.clamworks_task(cx).running;
    let hint = if budget <= 0.0 {
        "no budget yet: import a save or raise it above zero"
    } else {
        "the plan never spends more than the budget"
    };
    let budget_entity = cx.entity();
    let budget_control = Stepper::new("clamworks-budget", budget)
        .range(0.0, MAX_BUDGET)
        .step(1_000_000.0)
        .precision(0)
        .on_change(move |value, _window, cx| {
            budget_entity.update(cx, |state, cx| state.set_clamworks_budget(value, cx));
        });
    let run_entity = cx.entity();
    let run = Button::new(
        "run-clamworks-plan",
        if running { "Optimizing…" } else { "Optimize" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || budget <= 0.0)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_clamworks_plan(cx));
    });
    let reset_entity = cx.entity();
    let reset = Button::new("reset-clamworks-budget", "Use saved balance")
        .icon(IconName::RotateCcw)
        .disabled(budget == balance)
        .on_click(move |_event, _window, cx| {
            reset_entity.update(cx, |state, cx| state.set_clamworks_budget(balance, cx));
        });
    Card::new()
        .header(
            CardHeader::new("Plan a run").description(
                "Spends a pearl budget on the upgrades that move pearls per kill, highest gain per pearl first.",
            ),
        )
        .child(
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id("clamworks-budget-row")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_3()
                        .child(Caption::new("Budget").text_color(muted))
                        .child(budget_control)
                        .child(Caption::new("pearls").text_color(muted))
                        .child(reset)
                        .child(
                            Caption::new(format!("Saved balance {} · {hint}", format::rate(balance)))
                                .text_color(muted),
                        ),
                )
                .child(run),
        )
        .into_any_element()
}

/// The stored plan: what it spends, the purchases it makes and the actions.
fn plan_card(plan: &Plan, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let spent = format::scale(plan.spent);
    let remaining = format::scale(plan.remaining());
    let value_after = plan.pearl_value_after();
    let rows = purchases(&plan.steps);
    let entity = cx.entity();
    let apply = entity.clone();
    let dismiss = entity;

    let mut header = CardHeader::new("Plan").description(format!(
        "{} purchases · {} of {} pearls",
        plan.steps.len(),
        format::rate(plan.spent),
        format::rate(plan.budget)
    ));
    if stale {
        header = header.action(crate::ui::task::stale_badge("clamworks-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(crate::ui::task::stale_banner("clamworks-plan"));
    }
    let summary = div()
        .id("clamworks-plan-result")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-plan-purchases",
            "Purchases",
            plan.steps.len() as f64,
            0,
            None,
            "One level each",
        ))
        .child(kpi(
            "clamworks-plan-spent",
            "Spent",
            spent.0,
            2,
            Some(spent.1),
            "Sum of every purchase",
        ))
        .child(kpi(
            "clamworks-plan-remaining",
            "Remaining",
            remaining.0,
            2,
            Some(remaining.1),
            "Left after the last purchase",
        ))
        .child(kpi(
            "clamworks-plan-ppk",
            "Pearls / Kill",
            plan.pearls_per_kill_after(),
            4,
            None,
            &format!(
                "+{:.2}% from {}",
                plan.gain() * 100.0,
                format::rate(plan.pearls_per_kill_before())
            ),
        ))
        .child(kpi(
            "clamworks-plan-value",
            "Pearl Value",
            value_after,
            2,
            Some("×"),
            &format!("Was ×{:.2}", plan.pearl_value_before()),
        ));
    let mut table = Table::new(["Upgrade", "Purchases", "Levels", "Pearls", "Pearls / Kill"]);
    for row in &rows {
        let name = idleon_core::data::clamworks().upgrades[row.index]
            .name
            .clone();
        table = table.row([
            div()
                .id(("clamworks-plan-upgrade", row.index))
                .child(name)
                .into_any_element(),
            div().child(row.count.to_string()).into_any_element(),
            div()
                .child(format!("{} → {}", row.first_level, row.last_level))
                .into_any_element(),
            div().child(money(row.spent)).into_any_element(),
            div()
                .child(format!("+{:.2}%", row.gain * 100.0))
                .into_any_element(),
        ]);
    }
    card.child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(summary)
            .child(
                div()
                    .id("clamworks-plan-purchases-table")
                    .min_w_0()
                    .child(table),
            )
            .child(Caption::new(plan.stop_reason()).text_color(muted))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("apply-clamworks-plan", "Apply plan")
                            .primary()
                            .disabled(stale)
                            .on_click(move |_event, _window, cx| {
                                apply.update(cx, |state, cx| state.apply_clamworks_plan(cx));
                            }),
                    )
                    .child(Button::new("dismiss-clamworks-plan", "Dismiss").on_click(
                        move |_event, _window, cx| {
                            dismiss.update(cx, |state, cx| state.dismiss_clamworks_plan(cx));
                        },
                    )),
            ),
    )
    .into_any_element()
}

/// One row per upgrade the plan buys, in the order it first buys it.
struct Bought {
    index: usize,
    count: usize,
    first_level: u32,
    last_level: u32,
    spent: f64,
    gain: f64,
}

fn purchases(steps: &[PlanStep]) -> Vec<Bought> {
    let mut rows: Vec<Bought> = Vec::new();
    for step in steps {
        match rows.iter_mut().find(|row| row.index == step.index) {
            Some(row) => {
                row.count += 1;
                row.last_level = step.to_level();
                row.spent += step.cost;
                row.gain = (1.0 + row.gain) * (1.0 + step.gain) - 1.0;
            }
            None => rows.push(Bought {
                index: step.index,
                count: 1,
                first_level: step.from_level,
                last_level: step.to_level(),
                spent: step.cost,
                gain: step.gain,
            }),
        }
    }
    rows
}

/// The upstream rank table: pearl upgrades first, then the utility upgrades.
fn priority_card(clamworks: &Clamworks, multikill: f64) -> AnyElement {
    let ranks = clamworks.priority(multikill);
    let mut table = Table::new([
        "#",
        "Upgrade",
        "Lv",
        "Next Cost",
        "Pearls/Kill Gain",
        "Per Cost",
    ]);
    let mut position = 0;
    for rank in ranks.iter().filter(|rank| rank.is_pearl_upgrade) {
        position += 1;
        table = table.row(priority_row(
            clamworks,
            multikill,
            rank,
            Some(position),
            position == 1,
            false,
        ));
    }
    let utility: Vec<&UpgradeRank> = ranks.iter().filter(|rank| !rank.is_pearl_upgrade).collect();
    if !utility.is_empty() {
        table = table.row([
            div().into_any_element(),
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Utility upgrades")
                .into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
        ]);
        for rank in utility {
            table = table.row(priority_row(clamworks, multikill, rank, None, false, true));
        }
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade priority").description(
                "Ranked by marginal pearls-per-kill improvement per unit cost at the levels above. Utility upgrades are listed separately.",
            ),
        )
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}

fn priority_row(
    clamworks: &Clamworks,
    multikill: f64,
    rank: &UpgradeRank,
    position: Option<usize>,
    best: bool,
    utility: bool,
) -> [AnyElement; 6] {
    let mut name = div()
        .id(("clamworks-priority", rank.index))
        .flex()
        .items_center()
        .gap_2()
        .child(effect(clamworks, rank.index, multikill));
    if best {
        name = name.child(Badge::new("Best").tone(Tone::Success));
    }
    if utility {
        let category = match rank.category {
            Category::Cost => Some(("Cost", Tone::Info)),
            Category::Clams => Some(("Clams", Tone::Accent)),
            Category::Black => Some(("Pure Pearl", Tone::Neutral)),
            Category::Pearl => None,
        };
        if let Some((label, tone)) = category {
            name = name.child(Badge::new(label).tone(tone));
        }
    }
    let gain = if rank.cost_save > 0.0 {
        div()
            .child(format!("-{:.2}% cost", rank.cost_save * 100.0))
            .into_any_element()
    } else if rank.ppk_gain > 0.0 {
        div()
            .child(format!("+{:.4}%", rank.ppk_gain * 100.0))
            .into_any_element()
    } else {
        div().child("—").into_any_element()
    };
    [
        div()
            .child(position.map_or_else(String::new, |position| position.to_string()))
            .into_any_element(),
        name.into_any_element(),
        div().child(rank.level.to_string()).into_any_element(),
        div().child(money(rank.next_cost)).into_any_element(),
        gain,
        div()
            .child(if utility {
                "—".to_string()
            } else {
                format!("{:.2e}", rank.efficiency)
            })
            .into_any_element(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purchases_group_the_steps_by_upgrade_in_first_buy_order() {
        let steps = [
            PlanStep {
                index: 2,
                from_level: 30,
                cost: 10.0,
                gain: 0.1,
            },
            PlanStep {
                index: 0,
                from_level: 123,
                cost: 20.0,
                gain: 0.05,
            },
            PlanStep {
                index: 2,
                from_level: 31,
                cost: 30.0,
                gain: 0.2,
            },
        ];
        let rows = purchases(&steps);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 2);
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].first_level, 30);
        assert_eq!(rows[0].last_level, 32);
        assert_eq!(rows[0].spent, 40.0);
        assert!((rows[0].gain - 0.32).abs() < 1e-12, "gains compound");
        assert_eq!(rows[1].index, 0);
        assert_eq!(rows[1].count, 1);
    }
}
