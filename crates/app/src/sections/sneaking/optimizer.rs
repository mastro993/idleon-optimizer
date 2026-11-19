//! SN-03: the Optimizer workspace.
//!
//! The pane picks the objective, runs the search for it off the UI thread
//! through the shared task runner, and prints the saved placement against the
//! recommendation: the score, the per-twin action and try rate, and the charm
//! pair each twin should wear.
//!
//! The run only reads the loadout, so cancelling leaves the editor exactly as it
//! was; writing a recommendation into the loadout is the explicit Apply action.

use super::controls::objective_choices;
use super::loadout::tag;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_sneaking::{Item, Layout, Outcome};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The Optimizer pane: the controls, the run chrome and the stored outcome.
pub(super) fn pane(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.sneak_optimizer_task(cx);
    let objective = state.sneaking_state.objective;
    let running = task.running;
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let picker = &state.sneaking_controls.objective;
    picker.show(objective_choices(), Some(&objective), window, cx);
    let picker = div().flex_none().w(px(220.0)).h_6().child(
        Select::new(picker.state())
            .id("sneak-optimize-objective")
            .accessibility_label("Objective")
            .small(),
    );
    let run = Button::new("sneak-optimize-run")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_sneak_optimize(cx));
        });
    let cancel = Button::new("sneak-optimize-cancel")
        .label("Cancel")
        .small()
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_sneak_optimize(cx));
        });
    let mut body = div()
        .id("sneaking-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            div()
                .id("sneaking-optimizer-controls")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(picker)
                .child(run)
                .child(cancel),
        )
        .child(Caption::new(
            "The search moves charms only: the saved floors, hats and weapons stay put, so the \
             recommendation is a loadout your own floors can run. Every pair it tries comes from \
             the charms the account owns, and Scroll of Power still only fits once.",
        ));
    if running && let Some(progress) = task.progress.clone() {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "sneak-optimize",
            "Searching charm placements",
            "Runs off the UI thread; cancelling keeps the recommendation this run would have replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_sneak_optimize(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "sneak-optimize-cancelled",
            task.result.is_some(),
        ));
    }
    match &task.result {
        Some(outcome) => body = body.child(result(outcome, task.stale, cx)),
        None if !running => body = body.child(empty()),
        None => {}
    }
    Card::new()
        .header(
            CardHeader::new("Charm Optimizer")
                .description(
                    "Every legal charm pair the owned pool can cover, ranked for one objective.",
                )
                .action(tag(objective.unit(), Tone::Info)),
        )
        .child(body)
        .into_any_element()
}

/// Before the first run: what the search does, and what it needs.
fn empty() -> AnyElement {
    div()
        .id("sneaking-optimizer-empty")
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(
            "Pick an objective and run the search. It scores the saved placement first — that is the \
             baseline every result is compared against — then offers each untied twin every legal \
             pair the pool can supply.",
        ))
        .child(Caption::new(
            "Door clears need a twin holding a Nunchaku on the one unfinished door; untie progress \
             needs a Kunai and a captive waiting on the floor.",
        ))
        .into_any_element()
}

/// The stored outcome: its score, the twins it moves and how each twin scores.
fn result(outcome: &Outcome, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let objective = outcome.request.objective;
    let summary = format!(
        "{}: {} → {} ({:+.1}%)",
        objective.label(),
        format::value(outcome.baseline.score),
        format::value(outcome.winner.score),
        outcome.gain_percent()
    );
    let mut header = CardHeader::new("Recommendation").description(format!(
        "{} placements measured in {} sweep(s) · {}",
        outcome.evaluations,
        outcome.passes,
        objective.unit()
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("sneaking-optimizer"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("sneaking-optimizer"));
    }
    let changes = changes(outcome);
    let mut controls = div()
        .id("sneaking-optimizer-score")
        .role(Role::Label)
        .aria_label(summary.clone())
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(if outcome.improved() {
            tag(
                format!("{:+.1}% over the saved placement", outcome.gain_percent()),
                Tone::Success,
            )
        } else {
            tag("The saved placement already scores best", Tone::Warning)
        })
        .child(div().child(summary));
    if outcome.improved() {
        let apply = cx.entity();
        controls = controls.child(
            Button::new("sneak-optimize-apply")
                .label("Apply to loadout")
                .icon(IconName::Check)
                .small()
                .on_click(move |_event, _window, cx| {
                    apply.update(cx, |state, cx| state.sneak_optimize_apply(cx));
                }),
        );
    }
    let mut body = div()
        .id("sneaking-optimizer-result")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls);
    if outcome.winner.score <= 0.0 {
        body = body.child(Caption::new(format!(
            "{} scores zero with the saved floors: no twin is on that action.",
            objective.label()
        )));
    } else if changes.is_empty() {
        body = body.child(Caption::new(
            "No twin gains from a different pair; every slot stays as the save has it.",
        ));
    } else {
        body = body.child(charm_table(&changes));
    }
    card.child(body)
        .child(
            div()
                .id("sneaking-optimizer-twins")
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .pb_3()
                .child(Caption::new("Recommendation by twin"))
                .child(twin_table(&outcome.winner)),
        )
        .into_any_element()
}

/// One twin whose pair the recommendation changes.
struct Change {
    twin: usize,
    name: String,
    floor: usize,
    now: [Item; 2],
    recommended: [Item; 2],
}

/// The twins the recommendation moves, in roster order.
fn changes(outcome: &Outcome) -> Vec<Change> {
    outcome
        .winner
        .charms
        .iter()
        .filter_map(|(twin, pair)| {
            let before = outcome
                .baseline
                .charms
                .iter()
                .find(|(index, _)| index == twin)
                .map_or_else(|| pair.clone(), |(_, pair)| pair.clone());
            (before != *pair).then(|| Change {
                twin: *twin,
                name: twin_name(&outcome.winner, *twin),
                floor: twin_floor(&outcome.winner, *twin),
                now: before,
                recommended: pair.clone(),
            })
        })
        .collect()
}

fn twin_name(layout: &Layout, twin: usize) -> String {
    layout
        .twins
        .iter()
        .find(|row| row.twin == twin)
        .map_or_else(|| format!("Twin {}", twin + 1), |row| row.name.clone())
}

fn twin_floor(layout: &Layout, twin: usize) -> usize {
    layout
        .twins
        .iter()
        .find(|row| row.twin == twin)
        .map_or(0, |row| row.floor)
}

/// One charm as `Name +123`, or the empty slot.
fn charm(item: &Item) -> String {
    if item.is_blank() {
        "empty".into()
    } else {
        format!("{} +{}", item.name(), item.level)
    }
}

/// A kit table with `heads`; columns flagged in `numeric` align right. The
/// rows are one per twin, so they never grow past the roster.
fn table(heads: &[(&'static str, bool)], rows: Vec<TableRow>) -> Table {
    Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(heads.iter().map(
                |(head, numeric)| {
                    TableHead::new()
                        .when(*numeric, |head| head.text_right())
                        .child(*head)
                },
            ))),
        )
        .child(TableBody::new().children(rows))
}

/// The pairs the recommendation moves, with what the save has now.
fn charm_table(changes: &[Change]) -> AnyElement {
    let rows = changes
        .iter()
        .map(|change| {
            TableRow::new()
                .id(("sneak-optimize-change", change.twin))
                .child(
                    TableCell::new().child(
                        div()
                            .id(format!("sneak-optimize-twin-{}", change.twin))
                            .child(change.name.clone()),
                    ),
                )
                .child(
                    TableCell::new()
                        .text_right()
                        .child(change.floor.to_string()),
                )
                .child(TableCell::new().child(format!(
                    "{} · {}",
                    charm(&change.now[0]),
                    charm(&change.now[1])
                )))
                .child(TableCell::new().child(format!(
                    "{} · {}",
                    charm(&change.recommended[0]),
                    charm(&change.recommended[1])
                )))
        })
        .collect();
    div()
        .id("sneaking-optimizer-changes")
        .flex()
        .flex_col()
        .gap_2()
        .child(table(
            &[
                ("Twin", false),
                ("Floor", true),
                ("Now", false),
                ("Recommended", false),
            ],
            rows,
        ))
        .into_any_element()
}

/// Each twin's action, detection and share of the recommended layout's score.
fn twin_table(layout: &Layout) -> AnyElement {
    let rows = layout
        .twins
        .iter()
        .enumerate()
        .map(|(index, row)| {
            TableRow::new()
                .id(("sneak-optimize-twin-row", index))
                .child(TableCell::new().child(row.name.clone()))
                .child(TableCell::new().text_right().child(row.floor.to_string()))
                .child(TableCell::new().child(row.mode.label()))
                .child(
                    TableCell::new()
                        .text_right()
                        .child(format!("{:.1}%", row.detection * 100.0)),
                )
                .child(
                    TableCell::new()
                        .text_right()
                        .child(format::value(row.hits_per_hour)),
                )
                .child(
                    TableCell::new()
                        .text_right()
                        .child(format::value(row.contribution)),
                )
        })
        .collect();
    div()
        .id("sneaking-optimizer-twin-table")
        .flex()
        .flex_col()
        .gap_2()
        .child(table(
            &[
                ("Twin", false),
                ("Floor", true),
                ("Action", false),
                ("Detection", true),
                ("Actions / hour", true),
                ("Share", true),
            ],
            rows,
        ))
        .into_any_element()
}
