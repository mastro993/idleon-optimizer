//! The Optimizer tab: the bounded search over the kingdom's legal moves, its
//! recommendation and the moves that reach it.
//!
//! The search is `idleon_royal_guardian::Request`; it runs through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, the cancel action and
//! the stale marker are the ones every other section uses. A cancelled run keeps
//! the recommendation the section already showed.

use super::overview::kpi;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div};
use idleon_royal_guardian::{Candidate, Objective, Outcome};
use idleon_ui::format;

/// The Optimizer pane.
pub(crate) fn optimizer(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.royal_optimizer_task(cx);
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id("royal-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, &task.running, cx))
        .child(
            Caption::new(
                "Each round applies every legal move to every layout on the beam and keeps the \
                 best few, so the recommendation is a layout the game accepts, reached by moves a \
                 player can make.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "royal-optimizer-progress",
            "Optimizing the kingdom",
            "Runs off the UI thread; cancelling keeps the recommendation this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_royal_optimizer(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "royal-optimizer-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(outcome) = task.result.clone() {
        body = body.child(result(&outcome, task.stale, cx));
    }
    body.into_any_element()
}

/// The objective picker and the run action.
fn controls(state: &AppState, running: &bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let choices: Vec<Choice> = Objective::ALL
        .iter()
        .map(|objective| Choice::new(objective.id(), objective.label()))
        .collect();
    let objective = Select::new("royal-optimizer-objective", choices)
        .label("Objective")
        .selected(state.royal_state.optimizer.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(objective) = Objective::from_id(value.as_ref()) {
                entity.update(cx, |state, cx| {
                    state.set_royal_optimizer_objective(objective, cx);
                });
            }
        });
    let run_entity = cx.entity();
    let run = Button::new(
        "royal-optimizer-run",
        if *running {
            "Optimizing…"
        } else {
            "Optimize"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(*running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_royal_optimizer(cx));
    });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-optimizer-cancel", "Cancel")
        .icon(IconName::Square)
        .disabled(!*running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_optimizer(cx));
        });
    Card::new()
        .header(
            CardHeader::new("Plan a kingdom layout")
                .description("Moves units, links nodes, switches outpost types and buys upgrades."),
        )
        .child(
            div()
                .p_3()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(objective)
                .child(run)
                .child(cancel)
                .child(
                    Caption::new("The search is bounded; it reports after every layout it scores.")
                        .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The stored recommendation and the moves that reach it.
fn result(outcome: &Outcome, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(best) = outcome.best() else {
        return Card::new()
            .header(CardHeader::new("Recommendation").description(
                "The saved layout is the best this search found; no move improves it.",
            ))
            .child(
                div().p_3().child(
                    Caption::new(format!(
                        "{} layouts scored. Raise the search budget or change the objective.",
                        outcome.evaluated
                    ))
                    .text_color(muted),
                ),
            )
            .into_any_element();
    };
    let mut header = CardHeader::new("Recommendation").description(format!(
        "{} moves · {} layouts scored{}",
        best.moves.len(),
        outcome.evaluated,
        if outcome.truncated {
            " · the budget cut the search short"
        } else {
            ""
        }
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("royal-optimizer"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("royal-optimizer"));
    }
    card.child(
        div()
            .id("royal-optimizer-result")
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("royal-optimizer-figures")
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(kpi(
                        "royal-kpi-optimizer-resources",
                        "Currency / hour",
                        best.metrics.resources_per_hour,
                        "",
                        format!("from {}", format::rate(outcome.start.resources_per_hour)),
                    ))
                    .child(kpi(
                        "royal-kpi-optimizer-rank",
                        "Rank EXP / hour",
                        best.metrics.rank_exp_per_hour,
                        "",
                        format!("from {}", format::rate(outcome.start.rank_exp_per_hour)),
                    ))
                    .child(kpi(
                        "royal-kpi-optimizer-range",
                        "Reach",
                        best.metrics.range,
                        "range",
                        format!("from {}", format::count(outcome.start.range as u64)),
                    ))
                    .child(kpi(
                        "royal-kpi-optimizer-moves",
                        "Moves",
                        best.moves.len() as f64,
                        "",
                        outcome.objective.label().to_owned(),
                    )),
            )
            .child(moves(best))
            .child(
                Caption::new(
                    "Apply the moves in the listed order: a type switch clears its links, so the \
                     link moves come after it.",
                )
                .text_color(muted),
            ),
    )
    .into_any_element()
}

/// The moves the recommendation applies, in order.
fn moves(best: &Candidate) -> AnyElement {
    let mut table = Table::new(["#", "Move"]);
    for (index, change) in best.moves.iter().enumerate() {
        table = table.row([
            div().child(format!("{}", index + 1)).into_any_element(),
            div().child(change.label()).into_any_element(),
        ]);
    }
    div()
        .id("royal-optimizer-moves")
        .child(table)
        .into_any_element()
}
