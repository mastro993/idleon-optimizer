//! The Cglunko optimizer: the upstream priority table and the plan the saved
//! stones buy.
//!
//! `cglunko::Cglunko::priority` ports `rankUpgrades` per page: the marginal
//! drop-rate gain per unit cost at the saved levels. `plan` repeats that ranking
//! at the levels and stone balances each purchase reaches, pays every step's
//! `upgNextCost` from the upgrade's own stone, and refuses a purchase that would
//! lose more DR through a stone's digit than the level adds. The plan runs
//! through the shared [`crate::task::TaskRunner`]: its progress and cancel chrome
//! come from `ui::task`, and a result whose page changed is marked stale instead
//! of dropped.

use super::upgrades::effect;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div};
use idleon_core::Cglunko;
use idleon_core::cglunko::{Page, Plan, Rank, Step};

pub fn optimizer_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let cglunko = state.cglunko.clone();
    let page = state.cglunko_page;
    let task = state.cglunko_task(cx);
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id("cglunko-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls(state, cx))
        .child(
            Caption::new(
                "Each purchase is the best marginal DR gain per stone the page's saved balances can afford at the levels the plan has reached, so every step pays the cost at that level, never an earlier cheaper price.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(crate::ui::task::progress_card(
            "cglunko-plan",
            "Planning the purchases",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_cglunko_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(crate::ui::task::cancelled_banner(
            "cglunko-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.clone() {
        body = body.child(plan_card(&plan, task.stale));
    }
    body = body.child(priority_card(&cglunko, page));
    ScrollArea::new("cglunko-optimizer-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The run and cancel actions for the selected page.
fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let page = state.cglunko_page;
    let running = state.cglunko_task(cx).running;
    let run_entity = cx.entity();
    let run = Button::new(
        "run-cglunko-plan",
        if running { "Optimizing…" } else { "Optimize" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || !state.save_loaded)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_cglunko_plan(cx));
    });
    let cancel_entity = cx.entity();
    let cancel = Button::new("cancel-cglunko-plan", "Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_cglunko_plan(cx));
        });
    div()
        .id("cglunko-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(run)
        .child(cancel)
        .child(
            Caption::new(format!(
                "Spending the saved {} stones on page {}.",
                page.label(),
                page.index() + 1
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// The stored plan: what it spends, the purchases it makes and the DR it buys.
fn plan_card(plan: &Plan, stale: bool) -> AnyElement {
    let rows = purchases(&plan.steps);
    let mut header = CardHeader::new("Plan").description(format!(
        "{} purchases · {} {} stones",
        plan.steps.len(),
        format::value(plan.spent),
        plan.page.label()
    ));
    if stale {
        header = header.action(crate::ui::task::stale_badge("cglunko-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(crate::ui::task::stale_banner("cglunko-plan"));
    }
    let summary = div()
        .id("cglunko-plan-result")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "cglunko-plan-purchases",
            "Purchases",
            plan.steps.len() as f64,
            0,
            None,
            "One level each",
        ))
        .child(kpi(
            "cglunko-plan-spent",
            "Spent",
            plan.spent,
            0,
            None,
            "Sum of every purchase",
        ))
        .child(kpi(
            "cglunko-plan-dr-before",
            "DR Before",
            plan.dr_before(),
            2,
            Some("×"),
            "At the saved levels and stones",
        ))
        .child(kpi(
            "cglunko-plan-dr-after",
            "DR After",
            plan.dr_after(),
            2,
            Some("×"),
            &format!("+{:.2}% across the plan", plan.gain() * 100.0),
        ))
        .child(kpi(
            "cglunko-plan-stones-left",
            "Stones Left",
            plan.end.page_balance(plan.page),
            2,
            None,
            "On the planned page",
        ));
    let mut table = Table::new(["Upgrade", "Purchases", "Levels", "Stones", "Stones Left"]);
    for row in &rows {
        table = table.row([
            div()
                .id(("cglunko-plan-upgrade", row.index))
                .child(idleon_core::cglunko::upgrade_name(row.index))
                .into_any_element(),
            div().child(row.count.to_string()).into_any_element(),
            div()
                .child(format!("{} → {}", row.first_level, row.last_level))
                .into_any_element(),
            div().child(format::value(row.spent)).into_any_element(),
            div()
                .child(format::value(row.balance_after))
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
            .child(div().min_w_0().child(table))
            .child(Caption::new(plan.stop_reason())),
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
    balance_after: f64,
}

fn purchases(steps: &[Step]) -> Vec<Bought> {
    let mut rows: Vec<Bought> = Vec::new();
    for step in steps {
        match rows.iter_mut().find(|row| row.index == step.index) {
            Some(row) => {
                row.count += 1;
                row.last_level = step.to_level();
                row.spent += step.cost;
                row.balance_after = step.balance_after;
            }
            None => rows.push(Bought {
                index: step.index,
                count: 1,
                first_level: step.from_level,
                last_level: step.to_level(),
                spent: step.cost,
                balance_after: step.balance_after,
            }),
        }
    }
    rows
}

/// The upstream rank table for the selected page: DR upgrades first, then the
/// page's other upgrades.
fn priority_card(cglunko: &Cglunko, page: Page) -> AnyElement {
    let ranks = cglunko.priority(page);
    let mut table = Table::new(["#", "Upgrade", "Lv", "Next Cost", "DR Gain", "DR/Cost"]);
    let mut position = 0;
    for rank in ranks.iter().filter(|rank| rank.is_dr) {
        position += 1;
        table = table.row(priority_row(
            cglunko,
            rank,
            Some(position),
            position == 1,
            false,
        ));
    }
    let other: Vec<&Rank> = ranks.iter().filter(|rank| !rank.is_dr).collect();
    if !other.is_empty() {
        table = table.row([
            div().into_any_element(),
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Non-DR upgrades")
                .into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
        ]);
        for rank in other {
            table = table.row(priority_row(cglunko, rank, None, false, true));
        }
    }
    Card::new()
        .header(CardHeader::new("Upgrade priority").description(format!(
            "Ranked by marginal DR gain per unit cost at the saved levels on the {} page. Non-DR upgrades are listed separately.",
            page.label()
        )))
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}

fn priority_row(
    cglunko: &Cglunko,
    rank: &Rank,
    position: Option<usize>,
    best: bool,
    utility: bool,
) -> [AnyElement; 6] {
    let mut name = div()
        .id(("cglunko-priority", rank.index))
        .flex()
        .items_center()
        .gap_2()
        .child(effect(cglunko, rank.index));
    if best {
        name = name.child(Badge::new("Best").tone(Tone::Success));
    }
    if !utility && !rank.affordable() {
        name = name.child(Badge::new("Short").tone(Tone::Warning));
    }
    let gain = if utility {
        div().child("—").into_any_element()
    } else {
        div()
            .child(format!("+{:.4}%", rank.dr_gain * 100.0))
            .into_any_element()
    };
    [
        div()
            .child(position.map_or_else(String::new, |position| position.to_string()))
            .into_any_element(),
        name.into_any_element(),
        div().child(rank.level.to_string()).into_any_element(),
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(if rank.next_cost.is_finite() {
                format::value(rank.next_cost)
            } else {
                "—".into()
            })
            .child(Caption::new(format!(
                "{} #{} · {} held",
                rank.page.label(),
                idleon_core::cglunko::stone(rank.index) % 6 + 1,
                format::rate(rank.balance)
            )))
            .into_any_element(),
        gain,
        div()
            .child(if utility {
                "—".to_string()
            } else {
                format!("{:.2e}", rank.dr_efficiency)
            })
            .into_any_element(),
    ]
}

/// One summary card: the figure and a line of context.
fn kpi(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    suffix: Option<&'static str>,
    caption: &str,
) -> AnyElement {
    let mut statistic = Statistic::new(id, label, value)
        .decimals(decimals)
        .size(TextSize::Xl);
    if let Some(suffix) = suffix {
        statistic = statistic.suffix(suffix);
    }
    KpiCard::new(statistic).caption(caption).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purchases_group_the_steps_by_upgrade_in_first_buy_order() {
        let steps = [
            Step {
                index: 3,
                from_level: 10,
                cost: 10.0,
                dr_gain: 0.1,
                balance_before: 100.0,
                balance_after: 90.0,
            },
            Step {
                index: 10,
                from_level: 1,
                cost: 5.0,
                dr_gain: 0.05,
                balance_before: 50.0,
                balance_after: 45.0,
            },
            Step {
                index: 3,
                from_level: 11,
                cost: 20.0,
                dr_gain: 0.2,
                balance_before: 90.0,
                balance_after: 70.0,
            },
        ];
        let rows = purchases(&steps);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 3);
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].first_level, 10);
        assert_eq!(rows[0].last_level, 12);
        assert_eq!(rows[0].spent, 30.0);
        assert_eq!(rows[0].balance_after, 70.0);
        assert_eq!(rows[1].index, 10);
        assert_eq!(rows[1].count, 1);
    }
}
