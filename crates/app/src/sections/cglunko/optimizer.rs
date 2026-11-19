//! The Cglunko optimizer: the priority table and the plan the saved
//! stones buy.
//!
//! `cglunko::Cglunko::priority` ranks the upgrades per page: the marginal
//! drop-rate gain per unit cost at the saved levels. `plan` repeats that ranking
//! at the levels and stone balances each purchase reaches, pays every step's
//! next-level cost from the upgrade's own stone, and refuses a purchase that would
//! lose more DR through a stone's digit than the level adds. The plan runs
//! through the shared [`idleon_ui::runner::TaskRunner`]: its progress and cancel chrome
//! come from `idleon_ui::run_chrome`, and a result whose page changed is marked stale instead
//! of dropped.

use super::table::{table, text};
use super::upgrades::effect;
use super::{grouped, pane, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, Styled, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use idleon_cglunko::Cglunko;
use idleon_cglunko::{Page, Plan, Rank, Step};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, TableColumn, Tone};

pub(crate) fn optimizer_pane(state: &AppState, entity: Entity<AppState>, cx: &App) -> AnyElement {
    let page = state.cglunko_page;
    let task = state.cglunko_task(cx);
    let mut body = div()
        .id("cglunko-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .py_3()
        .child(controls(state, task.running, &entity))
        .child(Caption::new(
            "Each purchase is the best marginal DR gain per stone the page's saved balances can afford at the levels the plan has reached, so every step pays the cost at that level, never an earlier cheaper price.",
        ));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = entity.clone();
        body = body.child(idleon_ui::run_chrome::progress_card(
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
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "cglunko-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.as_ref() {
        body = body.child(plan_card(plan, task.stale, cx));
    }
    body = body.child(priority_card(
        &state.cglunko,
        page,
        &state.cglunko_figures.ranks,
        cx,
    ));
    pane("cglunko-optimizer-scroll", body)
}

/// The run and cancel actions for the selected page.
fn controls(state: &AppState, running: bool, entity: &Entity<AppState>) -> AnyElement {
    let page = state.cglunko_page;
    let run_entity = entity.clone();
    let run = Button::new("run-cglunko-plan")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .icon(IconName::Play)
        .disabled(running || !state.save_loaded)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_cglunko_plan(cx));
        });
    let cancel_entity = entity.clone();
    let cancel = Button::new("cancel-cglunko-plan")
        .label("Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_cglunko_plan(cx));
        });
    div()
        .id("cglunko-optimizer-controls")
        .flex()
        .items_center()
        .gap_3()
        .child(run)
        .child(cancel)
        .child(Caption::new(format!(
            "Spending the saved {} stones on page {}.",
            page.label(),
            page.index() + 1
        )))
        .into_any_element()
}

/// The stored plan: what it spends, the purchases it makes and the DR it buys.
fn plan_card(plan: &Plan, stale: bool, cx: &App) -> AnyElement {
    let rows = purchases(&plan.steps);
    let mut header = CardHeader::new("Plan").description(format!(
        "{} purchases · {} {} stones",
        plan.steps.len(),
        format::value(plan.spent),
        plan.page.label()
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("cglunko-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("cglunko-plan"));
    }
    let summary = KpiStrip::new()
        .child(
            Statistic::new(
                "cglunko-plan-purchases",
                "Purchases",
                format::count(plan.steps.len() as u64),
            )
            .caption("One level each"),
        )
        .child(
            Statistic::new("cglunko-plan-spent", "Spent", grouped(plan.spent, 0))
                .caption("Sum of every purchase"),
        )
        .child(
            Statistic::new(
                "cglunko-plan-dr-before",
                "DR Before",
                grouped(plan.dr_before(), 2),
            )
            .suffix("×")
            .caption("At the saved levels and stones"),
        )
        .child(
            Statistic::new(
                "cglunko-plan-dr-after",
                "DR After",
                grouped(plan.dr_after(), 2),
            )
            .suffix("×")
            .caption(format!("+{:.2}% across the plan", plan.gain() * 100.0)),
        )
        .child(
            Statistic::new(
                "cglunko-plan-stones-left",
                "Stones Left",
                grouped(plan.end.page_balance(plan.page), 2),
            )
            .caption("On the planned page"),
        );
    let table = table(
        "cglunko-plan-table",
        &[
            TableColumn::text("Upgrade"),
            TableColumn::number("Purchases").width(96.),
            TableColumn::number("Levels").width(80.),
            TableColumn::number("Stones").width(96.),
            TableColumn::number("Stones Left").width(96.),
        ],
        rows.iter().map(|row| {
            vec![
                div()
                    .id(("cglunko-plan-upgrade", row.index))
                    .child(idleon_cglunko::upgrade_name(row.index))
                    .into_any_element(),
                text(row.count.to_string()),
                text(format!("{} → {}", row.first_level, row.last_level)),
                text(format::value(row.spent)),
                text(format::value(row.balance_after)),
            ]
        }),
        cx,
    );
    card.child(div().id("cglunko-plan-result").child(summary))
        .child(table)
        .child(Caption::new(plan.stop_reason()))
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

/// The rank table for the selected page: DR upgrades first, then the
/// page's other upgrades.
fn priority_card(cglunko: &Cglunko, page: Page, ranks: &[Rank], cx: &App) -> AnyElement {
    let dr = ranks.iter().filter(|rank| rank.is_dr).enumerate();
    let mut rows: Vec<Vec<AnyElement>> = dr
        .map(|(index, rank)| priority_row(cglunko, rank, Some(index + 1), index == 0, false))
        .collect();
    let other: Vec<&Rank> = ranks.iter().filter(|rank| !rank.is_dr).collect();
    if !other.is_empty() {
        rows.push(vec![
            text(""),
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Non-DR upgrades")
                .into_any_element(),
            text(""),
            text(""),
            text(""),
            text(""),
        ]);
        rows.extend(
            other
                .into_iter()
                .map(|rank| priority_row(cglunko, rank, None, false, true)),
        );
    }
    Card::new()
        .header(CardHeader::new("Upgrade priority").description(format!(
            "Ranked by marginal DR gain per unit cost at the saved levels on the {} page. Non-DR upgrades are listed separately.",
            page.label()
        )))
        .child(table(
            "cglunko-priority-table",
            &[
                TableColumn::number("#").width(40.),
                TableColumn::text("Upgrade"),
                TableColumn::number("Lv").width(56.),
                TableColumn::number("Next Cost").width(136.),
                TableColumn::number("DR Gain").width(88.),
                TableColumn::number("DR/Cost").width(88.),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

fn priority_row(
    cglunko: &Cglunko,
    rank: &Rank,
    position: Option<usize>,
    best: bool,
    utility: bool,
) -> Vec<AnyElement> {
    let name = div()
        .id(("cglunko-priority", rank.index))
        .flex()
        .items_center()
        .gap_2()
        .child(effect(cglunko, rank.index))
        .when(best, |name| name.child(tag("Best", Tone::Success)))
        .when(!utility && !rank.affordable(), |name| {
            name.child(tag("Short", Tone::Warning))
        });
    vec![
        text(position.map_or_else(String::new, |position| position.to_string())),
        name.into_any_element(),
        text(rank.level.to_string()),
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap_1()
            .child(if rank.next_cost.is_finite() {
                format::value(rank.next_cost)
            } else {
                "—".into()
            })
            .child(Caption::new(format!(
                "{} #{} · {} held",
                rank.page.label(),
                idleon_cglunko::stone(rank.index) % 6 + 1,
                format::rate(rank.balance)
            )))
            .into_any_element(),
        text(if utility {
            "—".to_string()
        } else {
            format!("+{:.4}%", rank.dr_gain * 100.0)
        }),
        text(if utility {
            "—".to_string()
        } else {
            format!("{:.2e}", rank.dr_efficiency)
        }),
    ]
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
