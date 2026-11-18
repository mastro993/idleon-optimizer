//! The Optimizer tab: the bounded search over the kingdom's legal moves, its
//! recommendation and the moves that reach it.
//!
//! The search is `idleon_royal_guardian::Request`; it runs through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, the cancel action and
//! the stale marker are the ones every other section uses. A cancelled run keeps
//! the recommendation the section already showed.

use super::overview::kpi;
use super::table::{Col, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_royal_guardian::{Candidate, Objective, Outcome};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP};

/// The objective picker's options.
pub(super) fn objective_choices() -> Vec<Choice<Objective>> {
    Objective::ALL
        .into_iter()
        .map(|objective| Choice::new(objective, objective.label()))
        .collect()
}

/// The Optimizer pane.
pub(crate) fn optimizer(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.royal_optimizer_task(cx);
    let mut body = v_flex()
        .id("royal-optimizer")
        .gap(PANEL_GAP)
        .child(controls(state, task.running, window, cx))
        .child(Caption::new(
            "Each round applies every legal move to every layout on the beam and keeps the best \
             few, so the recommendation is a layout the game accepts, reached by moves a player \
             can make.",
        ));
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
fn controls(
    state: &AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let select = &state.royal_controls.objective;
    select.show(
        objective_choices(),
        Some(&state.royal_state.optimizer),
        window,
        cx,
    );
    // A kit select grows in a row; the box keeps it to its width.
    let objective = div().flex_none().w(px(180.0)).child(
        Select::new(select.state())
            .id("royal-optimizer-objective")
            .accessibility_label("Objective")
            .small(),
    );
    let run_entity = cx.entity();
    let run = Button::new("royal-optimizer-run")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_royal_optimizer(cx));
        });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-optimizer-cancel")
        .label("Cancel")
        .small()
        .icon(IconName::Square)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_optimizer(cx));
        });
    Card::new()
        .header(
            CardHeader::new("Plan a kingdom layout")
                .description("Moves units, links nodes, switches outpost types and buys upgrades."),
        )
        .child(
            h_flex()
                .gap_3()
                .child(objective)
                .child(run)
                .child(cancel)
                .child(Caption::new(
                    "The search is bounded; it reports after every layout it scores.",
                )),
        )
        .into_any_element()
}

/// The stored recommendation and the moves that reach it.
fn result(outcome: &Outcome, stale: bool, cx: &App) -> AnyElement {
    let Some(best) = outcome.best() else {
        return Card::new()
            .header(CardHeader::new("Recommendation").description(
                "The saved layout is the best this search found; no move improves it.",
            ))
            .child(Caption::new(format!(
                "{} layouts scored. Raise the search budget or change the objective.",
                outcome.evaluated
            )))
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
        v_flex()
            .id("royal-optimizer-result")
            .gap_3()
            .child(
                div().id("royal-optimizer-figures").child(
                    KpiStrip::new()
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
                ),
            )
            .child(moves(best, cx))
            .child(Caption::new(
                "Apply the moves in the listed order: a type switch clears its links, so the link \
                 moves come after it.",
            )),
    )
    .into_any_element()
}

/// The moves the recommendation applies, in order.
fn moves(best: &Candidate, cx: &App) -> AnyElement {
    let rows = best
        .moves
        .iter()
        .enumerate()
        .map(|(index, change)| vec![text(format!("{}", index + 1)), text(change.label())]);
    div()
        .id("royal-optimizer-moves")
        .child(table(
            "royal-optimizer-move",
            &[Col::Text("#"), Col::Text("Move")],
            rows,
            cx,
        ))
        .into_any_element()
}
