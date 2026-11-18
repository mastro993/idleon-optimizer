//! The Unit EXP tab: the trader/surveyor split that reaches the rank targets,
//! and the steps toward the next Armory shelf.
//!
//! Both plans run through the shared [`idleon_ui::runner::TaskRunner`], so the
//! progress card, the cancel action and the stale marker are the ones every
//! other section uses. A cancelled plan keeps the result the section showed.

use super::overview::{kpi, tag};
use super::table::{Col, table, text};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, SharedString, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_royal_guardian::{ShelfAction, ShelfPlan, Status, Step, Target, UnitExpPlan};
use idleon_ui::format;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, Tone, show_number,
};

/// The Unit EXP pane: the unit plan and the shelf steps.
pub(crate) fn unit_exp(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    v_flex()
        .id("royal-unit-exp")
        .gap(PANEL_GAP)
        .child(super::world_rail(state.royal_state.world, cx))
        .child(unit_card(state, window, cx))
        .child(shelf_card(state, window, cx))
        .into_any_element()
}

/// The unit EXP controls and their stored plan.
fn unit_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.royal_unit_exp_task(cx);
    let running = task.as_ref().is_some_and(|task| task.running);
    let mut body = v_flex()
        .id("royal-unit-exp-plan")
        .gap_3()
        .child(unit_controls(state, running, window, cx))
        .child(Caption::new(
            "Trading EXP comes from Traders and Intel EXP from Surveyors, so the plan is a split. \
             Each step retargets a slot the outpost already fields, and the moves apply in order.",
        ));
    if let Some(task) = task {
        body = body.children(run_chrome(
            task,
            (
                "royal-unit-exp-progress",
                "Planning the unit EXP",
                "royal-unit-exp-cancelled",
            ),
            AppState::cancel_royal_unit_exp,
            |plan, stale, cx| unit_result(&plan, stale, cx),
            cx,
        ));
    }
    Card::new()
        .header(
            CardHeader::new("Unit EXP moves")
                .description("The split that reaches the target ranks fastest."),
        )
        .child(body)
        .into_any_element()
}

/// A run's progress card, its cancelled banner and its stored result.
fn run_chrome<R>(
    task: TaskState<R>,
    (progress_id, title, cancelled_id): (&'static str, &'static str, &'static str),
    cancel: fn(&mut AppState, &mut Context<AppState>),
    result: impl FnOnce(R, bool, &App) -> AnyElement,
    cx: &mut Context<AppState>,
) -> Vec<AnyElement> {
    let mut parts = Vec::new();
    if task.running
        && let Some(progress) = task.progress
    {
        let entity = cx.entity();
        parts.push(idleon_ui::run_chrome::progress_card(
            progress_id,
            title,
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| entity.update(cx, cancel),
        ));
    }
    if task.cancelled {
        parts.push(
            idleon_ui::run_chrome::cancelled_banner(cancelled_id, task.result.is_some())
                .into_any_element(),
        );
    }
    if let Some(plan) = task.result {
        parts.push(result(plan, task.stale, cx));
    }
    parts
}

/// The outpost, the three targets and the run action.
fn unit_controls(
    state: &AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let controls = &state.royal_controls;
    let choices: Vec<Choice<usize>> = royal
        .world_outposts(state.royal_state.world)
        .into_iter()
        .filter(|outpost| outpost.built)
        .map(|outpost| {
            let kind = royal.outpost_type(outpost.map).round() as usize;
            Choice::new(
                outpost.map,
                format!(
                    "Map {} · {}",
                    outpost.map,
                    idleon_royal_guardian::sandbox::outpost_type_name(kind)
                ),
            )
        })
        .collect();
    let selected = state.royal_unit_exp_map();
    controls
        .unit_exp_outpost
        .show(choices, selected.as_ref(), window, cx);
    // A kit select grows in a row; the box keeps it to its width.
    let outpost = div().flex_none().w(px(200.0)).child(
        Select::new(controls.unit_exp_outpost.state())
            .id("royal-unit-exp-outpost")
            .accessibility_label("Outpost")
            .small(),
    );
    let targets = state.royal_state.unit_exp_targets;
    let mut fields = Vec::new();
    for (target, field) in Target::ALL.into_iter().zip(&controls.targets) {
        show_number(
            field,
            f64::from(targets.get(target).unwrap_or(0)),
            0,
            window,
            cx,
        );
        let label = target.label();
        fields.push(
            v_flex()
                .gap_1()
                .child(Caption::new(label))
                .child(number_field(
                    SharedString::from(format!("royal-unit-exp-target-{}", target.id())),
                    field,
                    label,
                )),
        );
    }
    let move_entity = cx.entity();
    let move_transients = Checkbox::new("royal-unit-exp-move")
        .label("Move same-world transients here")
        .checked(state.royal_state.unit_exp_move_transients)
        .on_click(move |on, _window, cx| {
            let on = *on;
            move_entity.update(cx, |state, cx| {
                state.set_royal_unit_exp_move_transients(on, cx);
            });
        });
    let run_entity = cx.entity();
    let run = Button::new("royal-unit-exp-run")
        .label(if running { "Planning…" } else { "Plan" })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running || selected.is_none())
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_royal_unit_exp(cx));
        });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-unit-exp-cancel")
        .label("Cancel")
        .small()
        .icon(IconName::Square)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_unit_exp(cx));
        });
    // Two lines that never wrap: a wrapped row is measured as one line and
    // overlaps what follows it.
    v_flex()
        .id("royal-unit-exp-controls")
        .gap_3()
        .child(h_flex().gap_3().child(outpost).child(move_transients))
        .child(
            h_flex()
                .items_end()
                .gap_3()
                .children(fields)
                .child(run)
                .child(cancel)
                .child(Caption::new("0 leaves a target unset.")),
        )
        .into_any_element()
}

/// The stored unit EXP plan: when each target is reached, the steps and the
/// transient assignments the plan borrows.
fn unit_result(plan: &UnitExpPlan, stale: bool, cx: &App) -> AnyElement {
    let mut header = CardHeader::new("Unit EXP plan").description(format!(
        "Map {} · {} Traders / {} Surveyors now",
        plan.map,
        plan.current_traders,
        plan.pool - plan.current_traders
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("royal-unit-exp-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("royal-unit-exp-plan"));
    }
    let status = status_badge("royal-unit-exp-status", plan.plan.status);
    let eta = plan.plan.eta_hours.unwrap_or(f64::NAN);
    let saved = plan.saved_hours.unwrap_or(0.0);
    let mut body = v_flex()
        .id("royal-unit-exp-result")
        .gap_3()
        .child(status)
        .child(
            KpiStrip::new()
                .child(kpi(
                    "royal-kpi-unit-exp-eta",
                    "All targets",
                    if eta.is_finite() { eta } else { 0.0 },
                    "hours",
                    if eta.is_finite() {
                        format::projection(eta)
                    } else {
                        "no finite ETA".to_owned()
                    },
                ))
                .child(kpi(
                    "royal-kpi-unit-exp-current",
                    "Saved split",
                    plan.current.eta_hours.unwrap_or(0.0),
                    "hours",
                    plan.current
                        .eta_hours
                        .map(format::projection)
                        .unwrap_or_else(|| "never".to_owned()),
                ))
                .child(kpi(
                    "royal-kpi-unit-exp-saved",
                    "Saved",
                    saved,
                    "hours",
                    "the saved split minus the plan",
                )),
        )
        .child(targets_reached(plan, cx));
    if let Some(blocked) = plan.plan.blocked {
        body = body.child(Caption::new(format!(
            "Blocked on {}: the outpost earns no EXP for it{}.",
            blocked.label(),
            if plan.transients.is_some() {
                " even with the assignments it can borrow"
            } else {
                "; try moving same-world assignments"
            }
        )));
    }
    body = body.child(steps(&plan.plan.steps, cx));
    if let Some(transients) = &plan.transients {
        body = body.child(
            v_flex()
                .id("royal-unit-exp-transients")
                .gap_1()
                .child(Caption::new(format!(
                    "Borrowed assignments: {}",
                    transients
                        .moves
                        .iter()
                        .map(|donor| format!("{} from map {}", donor.name, donor.map))
                        .collect::<Vec<String>>()
                        .join(", ")
                )))
                .child(transient_rows(transients, cx)),
        );
    }
    card.child(body)
        .child(Caption::new(
            "Apply the steps in order: each one only retargets a slot the outpost already fields.",
        ))
        .into_any_element()
}

/// When each set target is reached by the plan and by the saved split.
fn targets_reached(plan: &UnitExpPlan, cx: &App) -> AnyElement {
    let reached = |at: Option<f64>| {
        at.map(format::projection)
            .unwrap_or_else(|| "not reached".to_owned())
    };
    let rows = Target::ALL
        .into_iter()
        .enumerate()
        .filter_map(|(index, target)| {
            let rank = plan.targets.get(target)?;
            Some(vec![
                text(format!("{} {rank}", target.label())),
                text(reached(plan.plan.reached_at.get(index).copied().flatten())),
                text(reached(
                    plan.current.reached_at.get(index).copied().flatten(),
                )),
            ])
        });
    div()
        .id("royal-unit-exp-targets")
        .child(table(
            "royal-unit-exp-target-row",
            &[
                Col::Text("Target"),
                Col::Text("Plan"),
                Col::Text("Saved split"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The split changes the plan makes, in order.
fn steps(steps: &[Step], cx: &App) -> AnyElement {
    let rows = steps.iter().map(|step| {
        let changes = if step.changes.is_empty() {
            "hold the saved split".to_owned()
        } else {
            step.changes
                .iter()
                .map(|change| {
                    format!(
                        "slot {} to {}",
                        change.slot,
                        idleon_royal_guardian::PROFESSIONS
                            .get(change.to)
                            .copied()
                            .unwrap_or("unit")
                    )
                })
                .collect::<Vec<String>>()
                .join(", ")
        };
        vec![
            text(format::projection(step.hours)),
            text(format!(
                "{} Traders / {} Surveyors",
                step.traders, step.surveyors
            )),
            text(changes),
        ]
    });
    div()
        .id("royal-unit-exp-steps")
        .child(table(
            "royal-unit-exp-step",
            &[
                Col::Text("When"),
                Col::Text("Split"),
                Col::Text("Profession changes"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The transient rows: current, available and best counts.
fn transient_rows(transients: &idleon_royal_guardian::Transients, cx: &App) -> AnyElement {
    let count = |value: f64| text(format::count(value.max(0.0) as u64));
    let rows = transients.rows.iter().map(|row| {
        vec![
            text(row.name.to_owned()),
            count(row.current),
            count(row.available),
            count(row.best),
        ]
    });
    table(
        "royal-unit-exp-transient",
        &[
            Col::Text("Assignment"),
            Col::Number("Now"),
            Col::Number("Available"),
            Col::Number("Plan"),
        ],
        rows,
        cx,
    )
    .into_any_element()
}

/// The next-shelf controls and their stored plan.
fn shelf_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.royal_shelf_task(cx);
    let field = &state.royal_controls.shelf_purchases;
    show_number(
        field,
        state.royal_state.shelf_purchases as f64,
        0,
        window,
        cx,
    );
    let running = task.as_ref().is_some_and(|task| task.running);
    let run_entity = cx.entity();
    let run = Button::new("royal-shelf-run")
        .label(if running { "Planning…" } else { "Plan shelf" })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_royal_shelf(cx));
        });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-shelf-cancel")
        .label("Cancel")
        .small()
        .icon(IconName::Square)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_shelf(cx));
        });
    let mut body = v_flex()
        .id("royal-shelf")
        .gap_3()
        .child(
            h_flex()
                .id("royal-shelf-controls")
                .gap_3()
                .child(Caption::new("Purchases"))
                .child(number_field("royal-shelf-purchases", field, "Purchases"))
                .child(run)
                .child(cancel),
        )
        .child(Caption::new(
            "Each step is a layout the plan keeps while it saves, then a purchase it spends the \
             balance on. Apply them in order.",
        ));
    if let Some(task) = task {
        body = body.children(run_chrome(
            task,
            (
                "royal-shelf-progress",
                "Planning the next shelf",
                "royal-shelf-cancelled",
            ),
            AppState::cancel_royal_shelf,
            |plan, stale, cx| shelf_result(&plan, stale, cx),
            cx,
        ));
    }
    Card::new()
        .header(
            CardHeader::new("Next Armory shelf")
                .description("The levels and layout steps that unlock the next shelf, in order."),
        )
        .child(body)
        .into_any_element()
}

/// The stored shelf plan: its purchases, steps and reason.
fn shelf_result(plan: &ShelfPlan, stale: bool, cx: &App) -> AnyElement {
    let mut header = CardHeader::new("Shelf plan").description(format!(
        "{} of {} levels · shelf {} of {}",
        plan.purchases_needed,
        plan.levels_to_next_threshold,
        plan.start_unlocked,
        plan.next_unlocked
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("royal-shelf-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("royal-shelf-plan"));
    }
    let mut body = v_flex()
        .id("royal-shelf-result")
        .gap_3()
        .child(
            KpiStrip::new()
                .child(kpi(
                    "royal-kpi-shelf-purchases",
                    "Purchases",
                    plan.purchases_needed as f64,
                    "",
                    format!("{} levels needed", plan.levels_to_next_threshold),
                ))
                .child(kpi(
                    "royal-kpi-shelf-steps",
                    "Steps",
                    plan.steps.len() as f64,
                    "",
                    "layout and purchase steps",
                ))
                .child(kpi(
                    "royal-kpi-shelf-eta",
                    "Reaches the shelf",
                    plan.eta_hours,
                    "hours",
                    if plan.complete {
                        format::projection(plan.eta_hours)
                    } else {
                        plan.reason.clone().unwrap_or_default()
                    },
                )),
        )
        .child(
            h_flex()
                .gap_2()
                .child(if plan.complete {
                    tag("unlocked", Tone::Success)
                } else {
                    tag("in progress", Tone::Warning)
                })
                .child(Caption::new(if plan.complete {
                    "The plan reaches the next shelf."
                } else {
                    "The plan stops short at the purchase limit."
                })),
        )
        .child(shelf_steps(plan, cx));
    if let Some(reason) = &plan.reason {
        body = body.child(Caption::new(reason.clone()));
    }
    card.child(body).into_any_element()
}

/// The shelf plan's steps, in the order the player applies them.
fn shelf_steps(plan: &ShelfPlan, cx: &App) -> AnyElement {
    let rows = plan.steps.iter().map(|step| {
        let label = match &step.action {
            ShelfAction::Layout { moves, mode } => format!(
                "{mode} layout: {}",
                moves
                    .iter()
                    .map(idleon_royal_guardian::Move::label)
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            action => action_label(action),
        };
        vec![text(format::projection(step.time_hours)), text(label)]
    });
    div()
        .id("royal-shelf-steps")
        .child(table(
            "royal-shelf-step",
            &[Col::Text("When"), Col::Text("Step")],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The line one non-layout shelf step prints.
fn action_label(action: &ShelfAction) -> String {
    match action {
        ShelfAction::Layout { .. } => String::new(),
        ShelfAction::Purchase {
            name,
            cost,
            level_after,
            ..
        } => format!(
            "Buy {name} to level {level_after} for {}",
            format::rate(*cost)
        ),
        ShelfAction::Reset {
            refills,
            grade_gains,
        } => format!("Reset: {refills} refills, {grade_gains} grades"),
        ShelfAction::Collect { hours, .. } => {
            format!("Collect the nodes over {}", format::projection(*hours))
        }
    }
}

/// The badge a plan status prints.
fn status_badge(id: &'static str, status: Status) -> AnyElement {
    let (label, tone) = match status {
        Status::Already | Status::Reached => ("reached", Tone::Success),
        Status::NoProgress => ("blocked", Tone::Danger),
        Status::Limit | Status::Cutoff => ("cut short", Tone::Warning),
    };
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(label)
        .child(tag(label, tone))
        .into_any_element()
}
