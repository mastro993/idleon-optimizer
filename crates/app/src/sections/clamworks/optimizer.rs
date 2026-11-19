//! The Clamworks optimizer: the budgeted upgrade plan and the priority table.
//!
//! `clamworks.plan` repeats the priority ranking at the levels each purchase
//! reaches, so every step pays the next-level cost the game would charge there.
//! The plan runs through the shared [`idleon_ui::runner::TaskRunner`]: its
//! progress and cancel chrome come from `idleon_ui::run_chrome`, and a result
//! whose budget or multikill changed is marked stale instead of dropped.

use super::table::{Col, note, table, tag, text};
use super::upgrades::{effect, money};
use super::{kpi, pane};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, h_flex, v_flex};
use idleon_clamworks::Clamworks;
use idleon_clamworks::{Category, Plan, PlanStep, UpgradeRank};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Tone, show_number};

pub(crate) fn optimizer_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let clamworks = state.clamworks_shown();
    let multikill = state.clamworks_multikill;
    let task = state.clamworks_task(cx);
    let mut body = v_flex()
        .id("clamworks-optimizer")
        .gap(PANEL_GAP)
        .child(plan_controls(state, task.running, window, cx))
        .child(note(
            "Each purchase is the best marginal pearls-per-kill gain the budget can afford at the levels the plan has reached, so cost reductions from earlier purchases are already priced in.",
            cx,
        ));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
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
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "clamworks-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.as_ref() {
        body = body.child(plan_card(plan, task.stale, cx));
    }
    let body = body.child(priority_card(&clamworks, multikill, cx));
    pane("clamworks-optimizer-scroll", body)
}

/// The budget, how to run the plan, and what running does.
fn plan_controls(
    state: &mut AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let budget = state.clamworks_budget;
    let balance = state.clamworks.pearl_balance;
    let hint = if budget <= 0.0 {
        "no budget yet: import a save or raise it above zero"
    } else {
        "the plan never spends more than the budget"
    };
    let field = &state.clamworks_controls.budget;
    show_number(field, budget, 0, window, cx);
    let run_entity = cx.entity();
    let run = Button::new("run-clamworks-plan")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .icon(IconName::Play)
        .disabled(running || budget <= 0.0)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_clamworks_plan(cx));
        });
    let reset_entity = cx.entity();
    let reset = Button::new("reset-clamworks-budget")
        .label("Use saved balance")
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
            h_flex()
                .id("clamworks-budget-row")
                .gap_3()
                .child(Caption::new("Budget").text_color(muted))
                .child(ui::number_field("clamworks-budget", field, "Budget").w(gpui::px(160.0)))
                .child(Caption::new("pearls").text_color(muted))
                .child(reset)
                .child(
                    Caption::new(format!("Saved balance {} · {hint}", format::rate(balance)))
                        .text_color(muted),
                ),
        )
        .child(div().child(run))
        .into_any_element()
}

/// The stored plan: what it spends, the purchases it makes and the actions.
fn plan_card(plan: &Plan, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let spent = format::scale(plan.spent);
    let remaining = format::scale(plan.remaining());
    let value_after = plan.pearl_value_after();
    let bought = purchases(&plan.steps);
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
        header = header.action(idleon_ui::run_chrome::stale_badge("clamworks-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("clamworks-plan"));
    }
    let summary = div().id("clamworks-plan-result").child(
        KpiStrip::new()
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
            )),
    );
    let rows = bought.iter().map(|row| {
        let name = idleon_clamworks::data::clamworks().upgrades[row.index]
            .name
            .clone();
        [
            div()
                .id(("clamworks-plan-upgrade", row.index))
                .child(name)
                .into_any_element(),
            text(row.count.to_string()),
            text(format!("{} → {}", row.first_level, row.last_level)),
            text(money(row.spent)),
            text(format!("+{:.2}%", row.gain * 100.0)),
        ]
    });
    let table = table(
        "clamworks-plan-purchases",
        &[
            Col::Text("Upgrade"),
            Col::Number("Purchases"),
            Col::Number("Levels"),
            Col::Number("Pearls"),
            Col::Number("Pearls / Kill"),
        ],
        rows,
        cx,
    );
    card.child(summary)
        .child(table)
        .child(note(plan.stop_reason(), cx))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("apply-clamworks-plan")
                        .label("Apply plan")
                        .primary()
                        .disabled(stale)
                        .on_click(move |_event, _window, cx| {
                            apply.update(cx, |state, cx| state.apply_clamworks_plan(cx));
                        }),
                )
                .child(
                    Button::new("dismiss-clamworks-plan")
                        .label("Dismiss")
                        .on_click(move |_event, _window, cx| {
                            dismiss.update(cx, |state, cx| state.dismiss_clamworks_plan(cx));
                        }),
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

/// The rank table: pearl upgrades first, then the utility upgrades.
fn priority_card(clamworks: &Clamworks, multikill: f64, cx: &App) -> AnyElement {
    let ranks = clamworks.priority(multikill);
    let mut rows: Vec<[AnyElement; 6]> = ranks
        .iter()
        .filter(|rank| rank.is_pearl_upgrade)
        .enumerate()
        .map(|(index, rank)| priority_row(clamworks, multikill, rank, Some(index + 1), false))
        .collect();
    let utility: Vec<&UpgradeRank> = ranks.iter().filter(|rank| !rank.is_pearl_upgrade).collect();
    if !utility.is_empty() {
        rows.push([
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
        rows.extend(
            utility
                .into_iter()
                .map(|rank| priority_row(clamworks, multikill, rank, None, true)),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade priority").description(
                "Ranked by marginal pearls-per-kill improvement per unit cost at the levels above. Utility upgrades are listed separately.",
            ),
        )
        .child(table(
            "clamworks-priority",
            &[
                Col::Number("#"),
                Col::Text("Upgrade"),
                Col::Number("Lv"),
                Col::Number("Next Cost"),
                Col::Number("Pearls/Kill Gain"),
                Col::Number("Per Cost"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// One ranked upgrade; `position` 1 is the best pearl upgrade.
fn priority_row(
    clamworks: &Clamworks,
    multikill: f64,
    rank: &UpgradeRank,
    position: Option<usize>,
    utility: bool,
) -> [AnyElement; 6] {
    let category = match rank.category {
        Category::Cost => Some(("Cost", Tone::Info)),
        Category::Clams => Some(("Clams", Tone::Accent)),
        Category::Black => Some(("Pure Pearl", Tone::Neutral)),
        Category::Pearl => None,
    };
    let name = h_flex()
        .id(("clamworks-priority", rank.index))
        .gap_2()
        .child(
            div()
                .min_w_0()
                .child(effect(clamworks, rank.index, multikill)),
        )
        .when(position == Some(1), |name| {
            name.child(tag("Best", Tone::Success))
        })
        .when_some(category.filter(|_| utility), |name, (label, tone)| {
            name.child(tag(label, tone))
        });
    let gain = if rank.cost_save > 0.0 {
        format!("-{:.2}% cost", rank.cost_save * 100.0)
    } else if rank.ppk_gain > 0.0 {
        format!("+{:.4}%", rank.ppk_gain * 100.0)
    } else {
        "—".to_string()
    };
    [
        text(position.map_or_else(String::new, |position| position.to_string())),
        name.into_any_element(),
        text(rank.level.to_string()),
        text(money(rank.next_cost)),
        text(gain),
        text(if utility {
            "—".to_string()
        } else {
            format!("{:.2e}", rank.efficiency)
        }),
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
