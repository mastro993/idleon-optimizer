//! The Plan pane: the source, its staged layout, the plan run and its final
//! towers.
//!
//! The plan is generated from the save's unlocked tower types or the community
//! guide preset, and every stage is laid out by the same placement validation
//! the simulator uses. Simulating the plan runs the shared [`Run`] with a plan
//! policy, off the UI thread through the shared task runner.

use super::controls::segmented;
use super::table::{Col, table, text, text_id};
use super::{PlanSource, format_time};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Hsla, IntoElement, ParentElement, Styled, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, Tone};
use idleon_worship::{
    self as worship, PhaseSell, PhaseWhen, Plan, PlanPhase, PlanStage, RunResult,
};

/// The Plan pane.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure_plan();
    let task_state = state.worship_plan_task(cx);
    let running = task_state.running;
    let stale = task_state.stale;
    let cancelled = task_state.cancelled;
    // The plan, its stages and its issues are cached with the plan; the pane
    // borrows them instead of copying them every frame.
    let worship = &state.worship;
    let empty = Plan::default();
    let plan = worship.plan.as_ref().unwrap_or(&empty);
    let trial_index = worship.trial;
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();

    let run = {
        let entity = entity.clone();
        Button::new("worship-plan-simulate")
            .label(if running {
                "Simulating…"
            } else {
                "Simulate plan"
            })
            .primary()
            .icon(IconName::Play)
            .small()
            .disabled(running)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.simulate_worship_plan(cx));
            })
    };
    let sources: Vec<(PlanSource, &'static str)> = [PlanSource::Guide, PlanSource::Heuristic]
        .into_iter()
        .filter(|source| source.available(trial_index))
        .map(|source| (source, source.label()))
        .collect();
    let source = segmented("worship-plan-source", &sources, worship.plan_source, {
        let entity = entity.clone();
        move |source, _window, cx| {
            entity.update(cx, |state, cx| state.worship_set_plan_source(source, cx));
        }
    });

    let mut controls = h_flex()
        .id("worship-plan-controls")
        .gap_3()
        .child(source)
        .child(run);
    if worship.plan_from_optimizer {
        controls = controls.child(
            div()
                .id("worship-plan-optimized")
                .role(gpui::Role::Label)
                .aria_label("Optimizer plan")
                .child(
                    Tag::new()
                        .with_variant(Tone::Success.into())
                        .outline()
                        .small()
                        .child("Optimizer plan"),
                ),
        );
    }
    if running {
        let entity = entity.clone();
        controls = controls.child(
            Button::new("worship-plan-cancel")
                .label("Cancel")
                .secondary()
                .icon(IconName::X)
                .small()
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.cancel_worship_plan(cx));
                }),
        );
    }
    let mut summary = v_flex().gap_1().child(
        Caption::new(format!(
            "{} towers · {} phases · skip {}, roots {}",
            plan.towers.len(),
            plan.phases.len(),
            plan.skip_policy.label(),
            plan.roots.id()
        ))
        .text_color(muted),
    );
    if worship.plan_source == PlanSource::Guide
        && !worship.plan_from_optimizer
        && let Some(preset) = worship::guide_presets_for_trial(trial_index).next()
    {
        summary = summary.child(
            Caption::new(format!(
                "{} · target wave {} · {}",
                preset.label, preset.target_wave, preset.source
            ))
            .text_color(muted),
        );
    }

    let mut body = v_flex()
        .id("worship-plan-body")
        .gap_3()
        .p_3()
        .child(
            Card::new()
                .header(CardHeader::new("Plan").description(
                    "Towers are placed in order and upgraded by weight per point. \
                     Each phase waits for its conditions, sells, then builds.",
                ))
                .child(controls)
                .child(summary),
        )
        .child(warnings_card(&worship.plan_issues));

    if running {
        if let Some(progress) = task_state.progress {
            body = body.child(run_chrome::progress_card(
                "worship-plan",
                "Plan run",
                "The same simulator engine builds this plan; the run can be cancelled.",
                progress,
                move |_window, cx| {
                    entity.update(cx, |state, cx| state.cancel_worship_plan(cx));
                },
            ));
        }
    } else if cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "worship-plan-cancelled",
            task_state.result.is_some(),
        ));
    }

    match &task_state.result {
        Some(result) => {
            body = body.child(result_card(result, muted, stale));
            body = body.child(final_towers_card(result, cx));
            if !result.phases.is_empty() {
                body = body.child(phase_log_card(result, cx));
            }
            if !result.placement_fails.is_empty() {
                body = body.child(failures_card(result));
            }
        }
        None if !running => {
            body = body.child(
                div().id("worship-plan-empty").child(
                    Callout::new(Tone::Info, "No plan run yet").child(
                        Caption::new(
                            "Simulate the plan to place every stage through the simulator \
                             and see its final towers and phase timing.",
                        )
                        .text_color(muted),
                    ),
                ),
            );
        }
        None => {}
    }

    body = body.child(stages_card(&worship.plan_stages, plan, muted, cx));

    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("worship-plan-scroll")
        .into_any_element()
}

fn warnings_card(issues: &[worship::PlanIssue]) -> AnyElement {
    let body = if issues.is_empty() {
        Callout::new(Tone::Success, "Plan inputs are complete.").into_any_element()
    } else {
        v_flex()
            .id("worship-plan-warnings")
            .gap_1()
            .children(issues.iter().enumerate().map(|(index, issue)| {
                div()
                    .id(("worship-plan-warning", index))
                    .child(Callout::new(Tone::Warning, issue.message.clone()))
            }))
            .into_any_element()
    };
    Card::new()
        .header(
            CardHeader::new("Missing data")
                .description("What the plan cannot do with the inputs this build has, and why."),
        )
        .child(body)
        .into_any_element()
}

/// A tower type's name.
pub(super) fn tower_name(type_index: usize) -> String {
    worship::data()
        .towers
        .get(type_index)
        .map(|tower| tower.name.clone())
        .unwrap_or_else(|| format!("Type {type_index}"))
}

/// A plan tower's traits, or a dash.
pub(super) fn traits_text(traits: &str) -> String {
    if traits.is_empty() {
        "—".into()
    } else {
        traits.to_string()
    }
}

fn stages_card(stages: &[PlanStage], plan: &Plan, muted: Hsla, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("#"),
        Col::Text("Tower"),
        Col::Number("Node"),
        Col::Text("X"),
        Col::Text("Traits"),
        Col::Number("Max level"),
        Col::Number("Weight"),
    ];
    let mut column = v_flex().id("worship-plan-stages").gap_3();
    for (index, stage) in stages.iter().enumerate() {
        let phase = index
            .checked_sub(1)
            .and_then(|phase| plan.phases.get(phase));
        let description = phase
            .map(phase_text)
            .unwrap_or_else(|| "The opening layout, in plan order and snapped by the game.".into());
        let rows = stage.towers.iter().map(|tower| {
            let weight = plan
                .towers
                .iter()
                .chain(plan.phases.iter().flat_map(|phase| phase.towers.iter()))
                .nth(tower.plan_index)
                .map(|tower| tower.weight)
                .unwrap_or(1.0);
            let x = if (tower.x - tower.requested_x).abs() < 0.5 {
                format!("{:.0}", tower.x)
            } else {
                format!("{:.0} (from {:.0})", tower.x, tower.requested_x)
            };
            vec![
                text(format!("#{}", tower.plan_index)),
                text_id(
                    ("worship-plan-tower", tower.plan_index),
                    tower_name(tower.type_index),
                ),
                text(tower.node.to_string()),
                text(x),
                text(traits_text(&tower.traits)),
                text(
                    tower
                        .max_level
                        .map(|level| level.to_string())
                        .unwrap_or_else(|| "—".into()),
                ),
                text(format!("{weight:.2}")),
            ]
        });
        let mut card = Card::new()
            .header(
                CardHeader::new(format!("{}. {}", index + 1, stage.label)).description(description),
            )
            .child(
                Caption::new(format!(
                    "{} towers standing · {} placed in this stage",
                    stage.towers.len(),
                    phase
                        .map(|phase| phase.towers.len())
                        .unwrap_or(stage.towers.len())
                ))
                .text_color(muted),
            )
            .child(div().min_w_0().child(table(
                format!("worship-plan-stage-{index}"),
                &cols,
                rows,
                cx,
            )));
        for (rejected_index, rejected) in stage.rejected.iter().enumerate() {
            card = card.child(div().id(("worship-plan-rejected", rejected_index)).child(
                Callout::new(
                    Tone::Warning,
                    format!(
                        "#{} {}: {}",
                        rejected.plan_index,
                        tower_name(rejected.type_index),
                        rejected.reason
                    ),
                ),
            ));
        }
        column = column.child(card);
    }
    column.into_any_element()
}

fn phase_text(phase: &PlanPhase) -> String {
    let conditions = conditions_text(&phase.when);
    let sold = sell_text(&phase.sell);
    if conditions.is_empty() {
        format!(
            "When it starts: sell {sold}, then build {} towers.",
            phase.towers.len()
        )
    } else {
        format!(
            "When {}: sell {sold}, then build {} towers.",
            conditions.join(" and "),
            phase.towers.len()
        )
    }
}

fn conditions_text(when: &PhaseWhen) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(wave) = when.min_wave {
        out.push(format!("wave ≥ {wave}"));
    }
    if let Some(alive) = when.alive_max {
        out.push(format!("≤ {alive} mobs alive"));
    }
    if let Some(frac) = when.boss_hp_frac_max {
        out.push(format!("boss HP ≤ {:.0}%", frac * 100.0));
    }
    if let Some(hold) = when.boss_hold {
        out.push(if hold {
            "boss hold".into()
        } else {
            "no boss hold".into()
        });
    }
    if let Some(seconds) = when.min_time_sec {
        out.push(format!("after {}", format_time(seconds)));
    }
    out
}

fn sell_text(sell: &PhaseSell) -> String {
    let mut parts = Vec::new();
    if sell.all {
        if sell.keep_types.is_empty() {
            parts.push("every tower".into());
        } else {
            parts.push(format!("all except {}", names(&sell.keep_types).join(", ")));
        }
    } else {
        if !sell.types.is_empty() {
            parts.push(names(&sell.types).join(", "));
        }
        if !sell.plan_idx.is_empty() {
            parts.push(
                sell.plan_idx
                    .iter()
                    .map(|index| format!("#{index}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
    }
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join("; ")
    }
}

fn names(types: &[usize]) -> Vec<String> {
    types.iter().map(|index| tower_name(*index)).collect()
}

fn result_card(result: &RunResult, muted: Hsla, stale: bool) -> AnyElement {
    let pairs = [
        ("Best wave", result.best_wave.to_string()),
        (
            "Time to best",
            result
                .best_wave_time_sec
                .map(format_time)
                .unwrap_or_else(|| "—".into()),
        ),
        ("Highest wave", result.highest_wave.to_string()),
        ("Ended", result.end_reason.label().to_string()),
        ("Time", format_time(result.time_sec)),
        ("Kills", result.kills.to_string()),
        ("EXP", format!("{:.0}", result.exp)),
        ("Kill points", format!("{:.0}", result.kill_pts)),
        ("Skip points", format!("{:.0}", result.skip_pts)),
        ("Sale points", format!("{:.0}", result.sell_pts)),
        ("Leaks", result.leaks.to_string()),
        ("Totem", format!("{:.0}", result.totem_left)),
        ("Unspent points", format!("{:.0}", result.pts)),
        ("Phases", result.phases.len().to_string()),
    ];
    Card::new()
        .header(
            CardHeader::new("Plan result")
                .description("The plan's own run: best wave, totals and the end reason."),
        )
        .child(kv_rows(&pairs, muted).id("worship-plan-result"))
        .when(stale, |card| {
            card.child(run_chrome::stale_banner("worship-plan-stale"))
        })
        .into_any_element()
}

/// Label-over-value pairs, seven to a row so a row never wraps.
pub(super) fn kv_rows(pairs: &[(&'static str, String)], muted: Hsla) -> gpui::Div {
    v_flex().gap_3().children(pairs.chunks(7).map(|row| {
        h_flex().gap_3().children(row.iter().map(|(label, value)| {
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(Caption::new(*label).text_color(muted))
                .child(div().child(value.clone()))
        }))
    }))
}

fn final_towers_card(result: &RunResult, cx: &App) -> AnyElement {
    let total: f64 = result.towers.iter().map(|tower| tower.damage).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    let cols = [
        Col::Text("#"),
        Col::Text("Tower"),
        Col::Text("Node / X"),
        Col::Number("Level"),
        Col::Text("Traits"),
        Col::Number("Damage share"),
    ];
    let rows = result.towers.iter().map(|tower| {
        vec![
            text(
                tower
                    .plan_index
                    .map(|index| format!("#{index}"))
                    .unwrap_or_else(|| "—".into()),
            ),
            text_id(
                ("worship-plan-final-tower", tower.id),
                tower_name(tower.type_index),
            ),
            text(format!("{} / {:.0}", tower.node, tower.x)),
            text(tower.level.to_string()),
            text(traits_text(&tower.traits)),
            text(format!("{:.1}%", tower.damage / total * 100.0)),
        ]
    });
    Card::new()
        .header(CardHeader::new("Final towers").description(
            "The towers standing at the end, with damage share over the whole plan run.",
        ))
        .child(
            div()
                .id("worship-plan-final")
                .child(if result.towers.is_empty() {
                    Caption::new("No tower was standing at the end.").into_any_element()
                } else {
                    div()
                        .min_w_0()
                        .child(table("worship-plan-final", &cols, rows, cx))
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn phase_log_card(result: &RunResult, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("Phase"),
        Col::Number("Time"),
        Col::Number("Wave"),
        Col::Number("Alive"),
        Col::Number("Sold"),
        Col::Number("Refund"),
        Col::Number("Queued"),
    ];
    let rows = result.phases.iter().map(|phase| {
        vec![
            text(phase.label.clone()),
            text(format_time(phase.t)),
            text(phase.wave.to_string()),
            text(phase.alive.to_string()),
            text(phase.sold.to_string()),
            text(format!("{:.0}", phase.refund)),
            text(phase.queued.to_string()),
        ]
    });
    Card::new()
        .header(
            CardHeader::new("Phases")
                .description("When each sell-and-rebuild stage fired, and what it moved."),
        )
        .child(div().id("worship-plan-phases").min_w_0().child(table(
            "worship-plan-phase",
            &cols,
            rows,
            cx,
        )))
        .into_any_element()
}

fn failures_card(result: &RunResult) -> AnyElement {
    let list = v_flex().id("worship-plan-failures").gap_1().children(
        result
            .placement_fails
            .iter()
            .enumerate()
            .map(|(index, failure)| {
                let prefix = failure
                    .plan_index
                    .map(|index| format!("#{index}: "))
                    .unwrap_or_default();
                div()
                    .id(("worship-plan-failure", index))
                    .child(Callout::new(
                        Tone::Warning,
                        format!("{prefix}{}", failure.reason),
                    ))
            }),
    );
    Card::new()
        .header(
            CardHeader::new("Tower action warnings")
                .description("Plan actions the game refused or would refuse to click."),
        )
        .child(list)
        .into_any_element()
}
