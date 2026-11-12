//! The Plan pane: the source, its staged layout, the plan run and its final
//! towers.
//!
//! The plan is generated from the save's unlocked tower types or the community
//! guide preset, and every stage is laid out by the same placement validation
//! the simulator uses. Simulating the plan runs the shared [`Run`] with a plan
//! policy, off the UI thread through the shared task runner.

use super::{PlanSource, format_time};
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_ui::run_chrome;
use idleon_worship::{
    self as worship, PhaseSell, PhaseWhen, Plan, PlanPhase, PlanStage, RunResult,
};

/// The Plan pane.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure_plan();
    let plan = state.worship.plan.clone().unwrap_or_default();
    let trial_index = state.worship.trial;
    let issues = state.worship.plan_issues.clone();
    let stages = state.worship.plan_stages.clone();
    let task_state = state.worship_plan_task(cx);
    let running = task_state.running;
    let stale = task_state.stale;
    let result = task_state.result.clone();
    let progress = task_state.progress.clone();
    let cancelled = task_state.cancelled;
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();

    let run = {
        let entity = entity.clone();
        Button::new(
            "worship-plan-simulate",
            if running {
                "Simulating…"
            } else {
                "Simulate plan"
            },
        )
        .primary()
        .icon(IconName::Play)
        .size(ControlSize::Sm)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.simulate_worship_plan(cx));
        })
    };
    let cancel = {
        let entity = entity.clone();
        Button::new("worship-plan-cancel", "Cancel")
            .variant(ButtonVariant::Subtle)
            .icon(IconName::X)
            .size(ControlSize::Sm)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.cancel_worship_plan(cx));
            })
    };
    let mut source = SegmentedControl::new("worship-plan-source", state.worship.plan_source.id())
        .size(ControlSize::Sm);
    for option in [PlanSource::Guide, PlanSource::Heuristic] {
        if option.available(trial_index) {
            source = source.segment(option.id(), option.label(), None);
        }
    }
    let source = source.on_change({
        let entity = entity.clone();
        move |value, _window, cx| {
            if let Some(source) = PlanSource::from_id(value.as_ref()) {
                entity.update(cx, |state, cx| state.worship_set_plan_source(source, cx));
            }
        }
    });

    let mut controls = div()
        .id("worship-plan-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(source)
        .child(run);
    if state.worship.plan_from_optimizer {
        controls = controls.child(
            div()
                .id("worship-plan-optimized")
                .role(gpui::Role::Label)
                .aria_label("Optimizer plan")
                .child(Badge::new("Optimizer plan").tone(Tone::Success)),
        );
    }
    if running {
        controls = controls.child(cancel);
    }
    controls = controls.child(
        Caption::new(format!(
            "{} towers · {} phases · skip {}, roots {}",
            plan.towers.len(),
            plan.phases.len(),
            plan.skip_policy.label(),
            plan.roots.id()
        ))
        .text_color(muted),
    );
    if state.worship.plan_source == PlanSource::Guide
        && !state.worship.plan_from_optimizer
        && let Some(preset) = worship::guide_presets_for_trial(trial_index).next()
    {
        controls = controls.child(
            Caption::new(format!(
                "{} · target wave {} · {}",
                preset.label, preset.target_wave, preset.source
            ))
            .text_color(muted),
        );
    }

    let mut body = div()
        .id("worship-plan-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            Card::new()
                .header(CardHeader::new("Plan").description(
                    "Towers are placed in order and upgraded by weight per point. \
                     Each phase waits for its conditions, sells, then builds.",
                ))
                .child(div().p_3().child(controls)),
        )
        .child(warnings_card(&issues));

    if running {
        if let Some(progress) = progress {
            body = body.child(run_chrome::progress_card(
                "worship-plan",
                "Plan run",
                "The same simulator engine builds this plan; the run can be cancelled.",
                progress,
                {
                    let entity = cx.entity();
                    move |_window, cx| {
                        entity.update(cx, |state, cx| state.cancel_worship_plan(cx));
                    }
                },
            ));
        }
    } else if cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "worship-plan-cancelled",
            result.is_some(),
        ));
    }

    match result {
        Some(result) => {
            body = body.child(result_card(&result, muted, stale));
            body = body.child(final_towers_card(&result));
            if !result.phases.is_empty() {
                body = body.child(phase_log_card(&result));
            }
            if !result.placement_fails.is_empty() {
                body = body.child(failures_card(&result));
            }
        }
        None if !running => {
            body = body.child(
                div().id("worship-plan-empty").p_3().child(
                    Callout::new(Severity::Info).title("No plan run yet").child(
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

    body = body.child(stages_card(&stages, &plan, muted));

    ScrollArea::new("worship-plan-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

fn warnings_card(issues: &[worship::PlanIssue]) -> AnyElement {
    let body = if issues.is_empty() {
        div()
            .p_3()
            .child(Callout::new(Severity::Success).title("Plan inputs are complete."))
            .into_any_element()
    } else {
        let mut list = div()
            .id("worship-plan-warnings")
            .flex()
            .flex_col()
            .gap_1()
            .p_3();
        for (index, issue) in issues.iter().enumerate() {
            list = list.child(
                div()
                    .id(("worship-plan-warning", index))
                    .child(Callout::new(Severity::Warning).title(issue.message.clone())),
            );
        }
        list.into_any_element()
    };
    Card::new()
        .header(
            CardHeader::new("Missing data")
                .description("What the plan cannot do with the inputs this build has, and why."),
        )
        .child(body)
        .into_any_element()
}

fn stages_card(stages: &[PlanStage], plan: &Plan, muted: gpui::Hsla) -> AnyElement {
    let mut column = div().id("worship-plan-stages").flex().flex_col().gap_3();
    for (index, stage) in stages.iter().enumerate() {
        let phase = index
            .checked_sub(1)
            .and_then(|phase| plan.phases.get(phase));
        let description = phase
            .map(phase_text)
            .unwrap_or_else(|| "The opening layout, in plan order and snapped by the game.".into());
        let mut table = Table::new(["#", "Tower", "Node", "X", "Traits", "Max level", "Weight"]);
        for tower in &stage.towers {
            let name = worship::data()
                .towers
                .get(tower.type_index)
                .map(|tower| tower.name.clone())
                .unwrap_or_else(|| format!("Type {}", tower.type_index));
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
            table = table.row([
                div()
                    .child(format!("#{}", tower.plan_index))
                    .into_any_element(),
                div()
                    .id(("worship-plan-tower", tower.plan_index))
                    .child(name)
                    .into_any_element(),
                div().child(tower.node.to_string()).into_any_element(),
                div().child(x).into_any_element(),
                div()
                    .child(if tower.traits.is_empty() {
                        "—".into()
                    } else {
                        tower.traits.clone()
                    })
                    .into_any_element(),
                div()
                    .child(
                        tower
                            .max_level
                            .map(|level| level.to_string())
                            .unwrap_or_else(|| "—".into()),
                    )
                    .into_any_element(),
                div().child(format!("{weight:.2}")).into_any_element(),
            ]);
        }
        let mut card = Card::new()
            .header(
                CardHeader::new(format!("{}. {}", index + 1, stage.label)).description(description),
            )
            .child(
                div()
                    .p_3()
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
                    .child(div().min_w_0().child(table)),
            );
        for (rejected_index, rejected) in stage.rejected.iter().enumerate() {
            let name = worship::data()
                .towers
                .get(rejected.type_index)
                .map(|tower| tower.name.clone())
                .unwrap_or_else(|| format!("Type {}", rejected.type_index));
            card = card.child(
                div()
                    .id(("worship-plan-rejected", rejected_index))
                    .px_3()
                    .pb_3()
                    .child(Callout::new(Severity::Warning).title(format!(
                        "#{} {name}: {}",
                        rejected.plan_index, rejected.reason
                    ))),
            );
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
    types
        .iter()
        .map(|index| {
            worship::data()
                .towers
                .get(*index)
                .map(|tower| tower.name.clone())
                .unwrap_or_else(|| format!("Type {index}"))
        })
        .collect()
}

fn result_card(result: &RunResult, muted: gpui::Hsla, stale: bool) -> AnyElement {
    let mut card = Card::new()
        .header(
            CardHeader::new("Plan result")
                .description("The plan's own run: best wave, totals and the end reason."),
        )
        .child(
            div()
                .id("worship-plan-result")
                .flex()
                .flex_wrap()
                .gap_3()
                .p_3()
                .child(kv("Best wave", result.best_wave.to_string(), muted))
                .child(kv(
                    "Time to best",
                    result
                        .best_wave_time_sec
                        .map(format_time)
                        .unwrap_or_else(|| "—".into()),
                    muted,
                ))
                .child(kv("Highest wave", result.highest_wave.to_string(), muted))
                .child(kv("Ended", result.end_reason.label().to_string(), muted))
                .child(kv("Time", format_time(result.time_sec), muted))
                .child(kv("Kills", result.kills.to_string(), muted))
                .child(kv("EXP", format!("{:.0}", result.exp), muted))
                .child(kv("Kill points", format!("{:.0}", result.kill_pts), muted))
                .child(kv("Skip points", format!("{:.0}", result.skip_pts), muted))
                .child(kv("Sale points", format!("{:.0}", result.sell_pts), muted))
                .child(kv("Leaks", result.leaks.to_string(), muted))
                .child(kv("Totem", format!("{:.0}", result.totem_left), muted))
                .child(kv("Unspent points", format!("{:.0}", result.pts), muted))
                .child(kv("Phases", result.phases.len().to_string(), muted)),
        );
    if stale {
        card = card.child(
            div()
                .px_3()
                .pb_3()
                .child(run_chrome::stale_banner("worship-plan-stale")),
        );
    }
    card.into_any_element()
}

fn final_towers_card(result: &RunResult) -> AnyElement {
    let total: f64 = result.towers.iter().map(|tower| tower.damage).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    let mut table = Table::new(["#", "Tower", "Node / X", "Level", "Traits", "Damage share"]);
    for tower in &result.towers {
        let name = worship::data()
            .towers
            .get(tower.type_index)
            .map(|tower| tower.name.clone())
            .unwrap_or_else(|| format!("Type {}", tower.type_index));
        let share = tower.damage / total;
        table = table.row([
            div()
                .child(
                    tower
                        .plan_index
                        .map(|index| format!("#{index}"))
                        .unwrap_or_else(|| "—".into()),
                )
                .into_any_element(),
            div()
                .id(("worship-plan-final-tower", tower.id))
                .child(name)
                .into_any_element(),
            div()
                .child(format!("{} / {:.0}", tower.node, tower.x))
                .into_any_element(),
            div().child(tower.level.to_string()).into_any_element(),
            div()
                .child(if tower.traits.is_empty() {
                    "—".into()
                } else {
                    tower.traits.clone()
                })
                .into_any_element(),
            div()
                .child(format!("{:.1}%", share * 100.0))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Final towers").description(
            "The towers standing at the end, with damage share over the whole plan run.",
        ))
        .child(
            div()
                .id("worship-plan-final")
                .p_3()
                .child(if result.towers.is_empty() {
                    Caption::new("No tower was standing at the end.").into_any_element()
                } else {
                    div().min_w_0().child(table).into_any_element()
                }),
        )
        .into_any_element()
}

fn phase_log_card(result: &RunResult) -> AnyElement {
    let mut table = Table::new(["Phase", "Time", "Wave", "Alive", "Sold", "Refund", "Queued"]);
    for phase in &result.phases {
        table = table.row([
            div().child(phase.label.clone()).into_any_element(),
            div().child(format_time(phase.t)).into_any_element(),
            div().child(phase.wave.to_string()).into_any_element(),
            div().child(phase.alive.to_string()).into_any_element(),
            div().child(phase.sold.to_string()).into_any_element(),
            div()
                .child(format!("{:.0}", phase.refund))
                .into_any_element(),
            div().child(phase.queued.to_string()).into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Phases")
                .description("When each sell-and-rebuild stage fired, and what it moved."),
        )
        .child(
            div()
                .id("worship-plan-phases")
                .p_3()
                .child(div().min_w_0().child(table)),
        )
        .into_any_element()
}

fn failures_card(result: &RunResult) -> AnyElement {
    let mut list = div()
        .id("worship-plan-failures")
        .flex()
        .flex_col()
        .gap_1()
        .p_3();
    for (index, failure) in result.placement_fails.iter().enumerate() {
        let prefix = failure
            .plan_index
            .map(|index| format!("#{index}: "))
            .unwrap_or_default();
        list =
            list.child(div().id(("worship-plan-failure", index)).child(
                Callout::new(Severity::Warning).title(format!("{prefix}{}", failure.reason)),
            ));
    }
    Card::new()
        .header(
            CardHeader::new("Tower action warnings")
                .description("Plan actions the game refused or would refuse to click."),
        )
        .child(list)
        .into_any_element()
}

fn kv(label: &'static str, value: String, muted: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().child(value))
        .into_any_element()
}
