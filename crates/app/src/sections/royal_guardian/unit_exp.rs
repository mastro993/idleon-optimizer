//! The Unit EXP tab: the trader/surveyor split that reaches the rank targets,
//! and the steps toward the next Armory shelf.
//!
//! Both plans run through the shared [`idleon_ui::runner::TaskRunner`], so the
//! progress card, the cancel action and the stale marker are the ones every
//! other section uses. A cancelled plan keeps the result the section showed.

use super::overview::kpi;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div};
use idleon_royal_guardian::{ShelfAction, ShelfPlan, Status, Step, Target, UnitExpPlan};
use idleon_ui::format;

/// The Unit EXP pane: the unit plan and the shelf steps.
pub(crate) fn unit_exp(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("royal-unit-exp")
        .flex()
        .flex_col()
        .gap_3()
        .child(super::world_rail(state.royal_state.world, cx))
        .child(unit_card(state, cx))
        .child(shelf_card(state, cx))
        .into_any_element()
}

/// The unit EXP controls and their stored plan.
fn unit_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.royal_unit_exp_task(cx);
    let mut body = div()
        .id("royal-unit-exp-plan")
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(unit_controls(state, cx))
        .child(
            Caption::new(
                "Trading EXP comes from Traders and Intel EXP from Surveyors, so the plan is a \
                 split. Each step retargets a slot the outpost already fields, and the moves apply \
                 in order.",
            )
            .text_color(muted),
        );
    if let Some(task) = &task {
        if task.running
            && let Some(progress) = task.progress.clone()
        {
            let cancel = cx.entity();
            body = body.child(idleon_ui::run_chrome::progress_card(
                "royal-unit-exp-progress",
                "Planning the unit EXP",
                "Runs off the UI thread; cancelling keeps the plan this run replaced.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_royal_unit_exp(cx));
                },
            ));
        }
        if task.cancelled {
            body = body.child(idleon_ui::run_chrome::cancelled_banner(
                "royal-unit-exp-cancelled",
                task.result.is_some(),
            ));
        }
        if let Some(plan) = task.result.clone() {
            body = body.child(unit_result(&plan, task.stale, cx));
        }
    }
    Card::new()
        .header(
            CardHeader::new("Unit EXP moves")
                .description("The split that reaches the target ranks fastest."),
        )
        .child(body)
        .into_any_element()
}

/// The outpost, the three targets and the run action.
fn unit_controls(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world;
    let built: Vec<(usize, usize)> = royal
        .world_outposts(world)
        .into_iter()
        .filter(|outpost| outpost.built)
        .map(|outpost| {
            (
                outpost.map,
                royal.outpost_type(outpost.map).round() as usize,
            )
        })
        .collect();
    let selected = state.royal_unit_exp_map();
    let entity = cx.entity();
    let choices: Vec<Choice> = built
        .iter()
        .map(|(map, kind)| {
            Choice::new(
                map.to_string(),
                format!(
                    "Map {map} · {}",
                    idleon_royal_guardian::sandbox::outpost_type_name(*kind)
                ),
            )
        })
        .collect();
    let outpost = Select::new("royal-unit-exp-outpost", choices)
        .label("Outpost")
        .selected(selected.map(|map| map.to_string()).unwrap_or_default())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(map) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_royal_unit_exp_map(map, cx));
            }
        });
    let targets = state.royal_state.unit_exp_targets;
    let target_control = |x: &mut Context<AppState>, target: Target| {
        let value = targets.get(target).unwrap_or(0) as f64;
        let entity = x.entity();
        Stepper::new(format!("royal-unit-exp-target-{}", target.id()), value)
            .range(0.0, 500.0)
            .step(1.0)
            .precision(0)
            .on_change(move |value, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_royal_unit_exp_target(target, value.round() as u32, cx);
                });
            })
            .into_any_element()
    };
    let trading = target_control(cx, Target::Trading);
    let command = target_control(cx, Target::Command);
    let military = target_control(cx, Target::Military);
    let move_entity = cx.entity();
    let move_transients = Checkbox::new(
        "royal-unit-exp-move",
        state.royal_state.unit_exp_move_transients,
    )
    .label("Move same-world transients here")
    .on_change(move |on, _window, cx| {
        move_entity.update(cx, |state, cx| {
            state.set_royal_unit_exp_move_transients(on, cx);
        });
    });
    let running = state
        .royal_unit_exp_task(cx)
        .map(|task| task.running)
        .unwrap_or(false);
    let run_entity = cx.entity();
    let run = Button::new(
        "royal-unit-exp-run",
        if running { "Planning…" } else { "Plan" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || selected.is_none())
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_royal_unit_exp(cx));
    });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-unit-exp-cancel", "Cancel")
        .icon(IconName::Square)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_unit_exp(cx));
        });
    let target = |label: &str, control: AnyElement| {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Caption::new(label.to_owned()).text_color(muted))
            .child(control)
    };
    div()
        .id("royal-unit-exp-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(outpost)
        .child(target("Trading Rank", trading))
        .child(target("Command Rank", command))
        .child(target("Military Rank", military))
        .child(move_transients)
        .child(run)
        .child(cancel)
        .child(Caption::new("0 leaves a target unset.").text_color(muted))
        .into_any_element()
}

/// The stored unit EXP plan: when each target is reached, the steps and the
/// transient assignments the plan borrows.
fn unit_result(plan: &UnitExpPlan, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
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
    let mut body = div()
        .id("royal-unit-exp-result")
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(status)
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
        .child(targets_reached(plan));
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
    body = body.child(steps(&plan.plan.steps));
    if let Some(transients) = &plan.transients {
        body = body.child(
            div()
                .id("royal-unit-exp-transients")
                .flex()
                .flex_col()
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
                .child(transient_rows(transients)),
        );
    }
    card.child(body)
        .child(
            div().p_3().child(
                Caption::new(
                    "Apply the steps in order: each one only retargets a slot the outpost                      already fields.",
                )
                .text_color(muted),
            ),
        )
        .into_any_element()
}

/// When each set target is reached by the plan and by the saved split.
fn targets_reached(plan: &UnitExpPlan) -> AnyElement {
    let mut table = Table::new(["Target", "Plan", "Saved split"]);
    for (index, target) in Target::ALL.into_iter().enumerate() {
        let Some(rank) = plan.targets.get(target) else {
            continue;
        };
        table = table.row([
            div()
                .child(format!("{} {rank}", target.label()))
                .into_any_element(),
            div()
                .child(
                    plan.plan
                        .reached_at
                        .get(index)
                        .copied()
                        .flatten()
                        .map(format::projection)
                        .unwrap_or_else(|| "not reached".to_owned()),
                )
                .into_any_element(),
            div()
                .child(
                    plan.current
                        .reached_at
                        .get(index)
                        .copied()
                        .flatten()
                        .map(format::projection)
                        .unwrap_or_else(|| "not reached".to_owned()),
                )
                .into_any_element(),
        ]);
    }
    div()
        .id("royal-unit-exp-targets")
        .child(table)
        .into_any_element()
}

/// The split changes the plan makes, in order.
fn steps(steps: &[Step]) -> AnyElement {
    let mut table = Table::new(["When", "Split", "Profession changes"]);
    for step in steps {
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
        table = table.row([
            div()
                .child(format::projection(step.hours))
                .into_any_element(),
            div()
                .child(format!(
                    "{} Traders / {} Surveyors",
                    step.traders, step.surveyors
                ))
                .into_any_element(),
            div().child(changes).into_any_element(),
        ]);
    }
    div()
        .id("royal-unit-exp-steps")
        .child(table)
        .into_any_element()
}

/// The transient rows: current, available and best counts.
fn transient_rows(transients: &idleon_royal_guardian::Transients) -> AnyElement {
    let mut table = Table::new(["Assignment", "Now", "Available", "Plan"]);
    for row in &transients.rows {
        table = table.row([
            div().child(row.name.to_owned()).into_any_element(),
            div()
                .child(format::count(row.current.max(0.0) as u64))
                .into_any_element(),
            div()
                .child(format::count(row.available.max(0.0) as u64))
                .into_any_element(),
            div()
                .child(format::count(row.best.max(0.0) as u64))
                .into_any_element(),
        ]);
    }
    table.into_any_element()
}

/// The next-shelf controls and their stored plan.
fn shelf_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.royal_shelf_task(cx);
    let entity = cx.entity();
    let purchases = Stepper::new(
        "royal-shelf-purchases",
        state.royal_state.shelf_purchases as f64,
    )
    .range(1.0, 50.0)
    .step(1.0)
    .precision(0)
    .on_change(move |value, _window, cx| {
        entity.update(cx, |state, cx| {
            state.set_royal_shelf_purchases(value.round() as usize, cx);
        });
    });
    let running = task.as_ref().map(|task| task.running).unwrap_or(false);
    let run_entity = cx.entity();
    let run = Button::new(
        "royal-shelf-run",
        if running { "Planning…" } else { "Plan shelf" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_royal_shelf(cx));
    });
    let cancel_entity = cx.entity();
    let cancel = Button::new("royal-shelf-cancel", "Cancel")
        .icon(IconName::Square)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_royal_shelf(cx));
        });
    let mut body = div()
        .id("royal-shelf")
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .id("royal-shelf-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(Caption::new("Purchases").text_color(muted))
                .child(purchases)
                .child(run)
                .child(cancel),
        )
        .child(
            Caption::new(
                "Each step is a layout the plan keeps while it saves, then a purchase it spends \
                 the balance on. Apply them in order.",
            )
            .text_color(muted),
        );
    if let Some(task) = &task {
        if task.running
            && let Some(progress) = task.progress.clone()
        {
            let cancel = cx.entity();
            body = body.child(idleon_ui::run_chrome::progress_card(
                "royal-shelf-progress",
                "Planning the next shelf",
                "Runs off the UI thread; cancelling keeps the plan this run replaced.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_royal_shelf(cx));
                },
            ));
        }
        if task.cancelled {
            body = body.child(idleon_ui::run_chrome::cancelled_banner(
                "royal-shelf-cancelled",
                task.result.is_some(),
            ));
        }
        if let Some(plan) = task.result.clone() {
            body = body.child(shelf_result(&plan, task.stale));
        }
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
fn shelf_result(plan: &ShelfPlan, stale: bool) -> AnyElement {
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
    let mut body = div()
        .id("royal-shelf-result")
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
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
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Badge::new(if plan.complete {
                        "unlocked"
                    } else {
                        "in progress"
                    })
                    .tone(if plan.complete {
                        Tone::Success
                    } else {
                        Tone::Warning
                    }),
                )
                .child(Caption::new(if plan.complete {
                    "The plan reaches the next shelf."
                } else {
                    "The plan stops short at the purchase limit."
                })),
        )
        .child(shelf_steps(plan));
    if let Some(reason) = &plan.reason {
        body = body.child(Caption::new(reason.clone()));
    }
    card.child(body).into_any_element()
}

/// The shelf plan's steps, in the order the player applies them.
fn shelf_steps(plan: &ShelfPlan) -> AnyElement {
    let mut table = Table::new(["When", "Step"]);
    for step in &plan.steps {
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
        table = table.row([
            div()
                .child(format::projection(step.time_hours))
                .into_any_element(),
            div().child(label).into_any_element(),
        ]);
    }
    div()
        .id("royal-shelf-steps")
        .child(table)
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
        .child(Badge::new(label).tone(tone))
        .into_any_element()
}
