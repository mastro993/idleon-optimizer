//! The Optimizer pane: the search inputs, its progress, and the recommended
//! plan against the baselines.
//!
//! The search runs `idleon_worship::optimize` off the UI thread through the
//! shared task runner, so it reports progress and can be cancelled. Every
//! candidate is simulated by the same engine the Plan pane runs; the pane shows
//! the winner's plan, the finalists and the baselines on shared final seeds, and
//! adopts the winner into the Plan pane.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Styled, div};
use idleon_ui::run_chrome;
use idleon_worship::{self as worship, Mode, Plan, Search};

/// The Optimizer pane.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure_plan();
    let muted = cx.theme().colors.fg_muted;
    let task_state = state.worship_optimize_task(cx);
    let running = task_state.running;
    let stale = task_state.stale;
    let cancelled = task_state.cancelled;
    let progress = task_state.progress.clone();
    let result = task_state.result.clone();
    let entity = cx.entity();

    let mut body = div()
        .id("worship-optimizer-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            Card::new()
                .header(CardHeader::new("Search").description(
                    "Every candidate is simulated by the same engine the Plan pane runs. \
                     The finalists, the coverage heuristic and the guide presets are then \
                     compared on shared fresh seeds.",
                ))
                .child(controls(state, running, cx)),
        );

    if running {
        if let Some(progress) = progress {
            body = body.child(run_chrome::progress_card(
                "worship-optimize",
                "Searching strategies",
                "Runs off the UI thread. Cancelling keeps the result this run replaced.",
                progress,
                move |_window, cx| {
                    entity.update(cx, |state, cx| state.cancel_worship_optimize(cx));
                },
            ));
        }
    } else if cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "worship-optimize-cancelled",
            result.is_some(),
        ));
    }

    match result {
        Some(Ok(result)) => {
            body = body.child(result_card(&result, muted, stale, cx));
            body = body.child(baselines_card(&result, muted));
            body = body.child(plan_card(&result.plan));
            body = body.child(final_towers_card(&result));
        }
        Some(Err(reason)) => {
            body = body.child(
                div().id("worship-optimize-refused").p_3().child(
                    Callout::new(Severity::Warning)
                        .title("The search cannot run with this save")
                        .child(Caption::new(reason).text_color(muted)),
                ),
            );
        }
        None if !running => {
            body = body.child(
                div().id("worship-optimizer-empty").p_3().child(
                    Callout::new(Severity::Info).title("No search yet").child(
                        Caption::new(
                            "Pick a budget and a seed, then Optimize. The search builds plans, \
                             screens them, and recommends the one that clears the most waves.",
                        )
                        .text_color(muted),
                    ),
                ),
            );
        }
        None => {}
    }

    ScrollArea::new("worship-optimizer-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

fn controls(state: &mut AppState, running: bool, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let config = state.worship.optimize;
    let trial = state.worship.trial;

    let mut mode =
        SegmentedControl::new("worship-optimize-mode", config.mode.id()).size(ControlSize::Sm);
    for option in Mode::ALL {
        mode = mode.segment(option.id(), option.label(), None);
    }
    let mode = mode.on_change({
        let entity = entity.clone();
        move |value, _window, cx| {
            if let Some(mode) = Mode::from_id(value.as_ref()) {
                entity.update(cx, |state, cx| state.worship_set_optimize_mode(mode, cx));
            }
        }
    });

    let seeds = Stepper::new("worship-optimize-seeds", config.seeds as f64)
        .range(1.0, 6.0)
        .step(1.0)
        .on_change({
            let entity = entity.clone();
            move |value, _window, cx| {
                entity.update(cx, |state, cx| state.worship_set_optimize_seeds(value, cx));
            }
        });
    let seed = Stepper::new("worship-optimize-seed", config.base_seed as f64)
        .range(0.0, 999_999.0)
        .step(1.0)
        .on_change({
            let entity = entity.clone();
            move |value, _window, cx| {
                entity.update(cx, |state, cx| state.worship_set_optimize_seed(value, cx));
            }
        });
    let wave = Stepper::new("worship-optimize-wave", f64::from(config.max_wave))
        .range(10.0, 1000.0)
        .step(10.0)
        .on_change({
            let entity = entity.clone();
            move |value, _window, cx| {
                entity.update(cx, |state, cx| state.worship_set_optimize_wave(value, cx));
            }
        });

    let run = {
        let entity = entity.clone();
        Button::new(
            "worship-optimize-run",
            if running { "Searching…" } else { "Optimize" },
        )
        .primary()
        .icon(IconName::Play)
        .size(ControlSize::Sm)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.optimize_worship(cx));
        })
    };

    let mut row = div()
        .id("worship-optimize-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(field(
            "Mode",
            mode.into_any_element(),
            "Search budget",
            muted,
        ))
        .child(field(
            "Runs per plan",
            seeds.into_any_element(),
            "Screening seeds",
            muted,
        ))
        .child(field(
            "Seed",
            seed.into_any_element(),
            "Reproducible search",
            muted,
        ))
        .child(field(
            "Stop after wave",
            wave.into_any_element(),
            "Run cap",
            muted,
        ))
        .child(run);
    if running {
        let cancel = entity.clone();
        row = row.child(
            Button::new("worship-optimize-cancel", "Cancel")
                .variant(ButtonVariant::Subtle)
                .icon(IconName::X)
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_worship_optimize(cx));
                }),
        );
    }
    row.child(
        Caption::new(format!(
            "{} · trial {} · {} screening plans · {} final seeds",
            config.mode.label(),
            trial + 1,
            config.options(trial, None).budget(),
            config.seeds.max(1) * 2
        ))
        .text_color(muted),
    )
    .into_any_element()
}

fn field(label: &'static str, control: AnyElement, hint: &'static str, muted: Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .child(Caption::new(hint).text_color(muted))
        .into_any_element()
}

fn result_card(
    result: &Search,
    muted: Hsla,
    stale: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let tone = if result.beats_baseline() {
        Tone::Success
    } else {
        Tone::Warning
    };
    let mut actions = div().flex().items_center().gap_2();
    if stale {
        actions = actions.child(run_chrome::stale_badge("worship-optimize-stale"));
    }
    actions = actions.child(
        div()
            .id("worship-optimize-verdict")
            .role(gpui::Role::Label)
            .aria_label(format!(
                "Recommended plan clears wave {:.0}",
                result.score.primary
            ))
            .child(Badge::new(format!("Wave {:.0}", result.score.primary)).tone(tone)),
    );

    let mut card = Card::new()
        .header(
            CardHeader::new("Recommended plan")
                .description(format!(
                    "{} · {} families · {} evaluations · {} duplicates skipped",
                    result.label, result.families, result.evaluations, result.duplicates_skipped
                ))
                .action(actions),
        )
        .child(
            div()
                .id("worship-optimize-result")
                .flex()
                .flex_wrap()
                .gap_3()
                .p_3()
                .child(kv(
                    "Mean cleared",
                    format!("{:.1}", result.mean_best_wave),
                    muted,
                ))
                .child(kv(
                    "Range",
                    format!("{}–{}", result.min_best_wave, result.max_best_wave),
                    muted,
                ))
                .child(kv("Saved best", result.saved_best.to_string(), muted))
                .child(kv("Reached", format!("{:.1}", result.score.reached), muted))
                .child(kv("Final seeds", result.seeds.len().to_string(), muted))
                .child(kv(
                    "Plan",
                    format!(
                        "{} towers · {} phases",
                        result.plan.towers.len(),
                        result.plan.phases.len()
                    ),
                    muted,
                ))
                .child(kv("Found by", result.kind.to_string(), muted))
                .child(
                    Button::new("worship-optimize-use", "Use this plan")
                        .primary()
                        .size(ControlSize::Sm)
                        .disabled(stale)
                        .on_click(move |_event, _window, cx| {
                            entity.update(cx, |state, cx| state.worship_use_optimized_plan(cx));
                        }),
                ),
        );
    if stale {
        card = card.child(
            div()
                .px_3()
                .pb_3()
                .child(run_chrome::stale_banner("worship-optimize-stale")),
        );
    }
    card.into_any_element()
}

/// One comparable plan row: its label, its score and whether it is a baseline.
type Row = (String, worship::Score, bool);

/// Add a plan once; a plan that is both a baseline and a finalist stays one row.
fn add_row(
    rows: &mut Vec<Row>,
    seen: &mut Vec<(String, worship::Score)>,
    label: &str,
    score: worship::Score,
    baseline: bool,
) {
    if seen
        .iter()
        .any(|(known, known_score)| known == label && *known_score == score)
    {
        return;
    }
    seen.push((label.to_string(), score));
    rows.push((label.to_string(), score, baseline));
}

fn baselines_card(result: &Search, muted: Hsla) -> AnyElement {
    let best = result.best_baseline();
    let is_baseline = |label: &str, score: worship::Score| {
        result
            .baselines
            .iter()
            .any(|baseline| baseline.label == label && baseline.score == score)
    };
    // The recommendation first, then the plans this build ships, then the other
    // finalists.
    let mut rows: Vec<Row> = Vec::new();
    let mut seen: Vec<(String, worship::Score)> = Vec::new();
    add_row(&mut rows, &mut seen, &result.label, result.score, false);
    for baseline in &result.baselines {
        add_row(&mut rows, &mut seen, &baseline.label, baseline.score, true);
    }
    for finalist in &result.finalists {
        add_row(
            &mut rows,
            &mut seen,
            &finalist.label,
            finalist.score,
            is_baseline(&finalist.label, finalist.score),
        );
    }
    let mut table = Table::new(["Plan", "Cleared", "Reached", "Verdict"]);
    for (index, (label, score, baseline)) in rows.iter().enumerate() {
        let winner = *label == result.label && *score == result.score;
        let verdict = if winner {
            Badge::new("Recommended")
                .tone(Tone::Success)
                .into_any_element()
        } else if *baseline {
            let beats = result.score.primary >= score.primary;
            div()
                .child(Caption::new(if beats { "Beaten" } else { "Held" }).text_color(muted))
                .into_any_element()
        } else {
            div()
                .child(Caption::new("Finalist").text_color(muted))
                .into_any_element()
        };
        table = table.row([
            div()
                .id(("worship-optimize-row", index))
                .child(label.to_string())
                .into_any_element(),
            div()
                .child(format!("{:.1}", score.primary))
                .into_any_element(),
            div()
                .child(format!("{:.1}", score.reached))
                .into_any_element(),
            div().child(verdict).into_any_element(),
        ]);
    }
    let summary = best.map_or_else(
        || "No baseline was compared.".to_string(),
        |baseline| {
            let gap = result.score.primary - baseline.score.primary;
            if gap > 0.0 {
                format!(
                    "The recommendation clears {gap:.1} more waves than {}.",
                    baseline.label
                )
            } else if gap < 0.0 {
                format!("{} still clears {:.1} more waves.", baseline.label, -gap)
            } else {
                format!(
                    "The recommendation ties {}; the search found no better plan.",
                    baseline.label
                )
            }
        },
    );
    Card::new()
        .header(CardHeader::new("Baselines").description(
            "The plans this build ships and the plan on screen, then the other finalists, \
             all on the same final seeds.",
        ))
        .child(
            div()
                .id("worship-optimize-baselines")
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(div().min_w_0().child(table))
                .child(Caption::new(summary).text_color(muted)),
        )
        .into_any_element()
}

fn plan_card(plan: &Plan) -> AnyElement {
    let mut table = Table::new(["#", "Tower", "Node / X", "Traits", "Max level", "Weight"]);
    for (index, tower) in plan.towers.iter().enumerate() {
        let name = worship::data()
            .towers
            .get(tower.type_index)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| format!("Type {}", tower.type_index));
        table = table.row([
            div().child(format!("#{index}")).into_any_element(),
            div()
                .id(("worship-optimize-tower", index))
                .child(name)
                .into_any_element(),
            div()
                .child(format!("{} / {:.0}", tower.slot.node, tower.slot.x))
                .into_any_element(),
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
            div()
                .child(format!("{:.2}", tower.weight))
                .into_any_element(),
        ]);
    }
    let mut phases = div().flex().flex_col().gap_1();
    for (index, phase) in plan.phases.iter().enumerate() {
        phases = phases.child(
            div()
                .id(("worship-optimize-phase", index))
                .child(Caption::new(format!(
                    "{} · {} towers · sells {} · wait wave ≥ {}",
                    phase.label,
                    phase.towers.len(),
                    sell_text(phase),
                    phase
                        .when
                        .min_wave
                        .map(|wave| wave.to_string())
                        .unwrap_or_else(|| "—".into())
                ))),
        );
    }
    Card::new()
        .header(CardHeader::new("Recommended plan").description(
            "The towers the search recommends, in plan order, with the plan's phases.",
        ))
        .child(
            div()
                .id("worship-optimize-plan")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(if plan.towers.is_empty() {
                    Caption::new("The plan places no tower.").into_any_element()
                } else {
                    div().min_w_0().child(table).into_any_element()
                })
                .child(phases),
        )
        .into_any_element()
}

fn final_towers_card(result: &Search) -> AnyElement {
    let detail = &result.detail;
    let total: f64 = detail.towers.iter().map(|tower| tower.damage).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    let mut table = Table::new(["#", "Tower", "Node / X", "Level", "Traits", "Damage share"]);
    for tower in &detail.towers {
        let name = worship::data()
            .towers
            .get(tower.type_index)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| format!("Type {}", tower.type_index));
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
                .id(("worship-optimize-final-tower", tower.id))
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
                .child(format!("{:.1}%", tower.damage / total * 100.0))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Final towers").description(format!(
            "The towers standing when the recommended plan's first final run (seed {}) ended at wave {}.",
            result.seeds.first().copied().unwrap_or(1),
            detail.best_wave
        )))
        .child(
            div()
                .id("worship-optimize-final-towers")
                .p_3()
                .child(if detail.towers.is_empty() {
                    Caption::new("No tower was standing at the end.").into_any_element()
                } else {
                    div().min_w_0().child(table).into_any_element()
                }),
        )
        .into_any_element()
}

fn sell_text(phase: &worship::PlanPhase) -> String {
    let sell = &phase.sell;
    let mut parts = Vec::new();
    if sell.all {
        parts.push(if sell.keep_types.is_empty() {
            "every tower".to_string()
        } else {
            format!("all but {}", sell.keep_types.len())
        });
    } else {
        if !sell.types.is_empty() {
            parts.push(format!("{} types", sell.types.len()));
        }
        if !sell.plan_idx.is_empty() {
            parts.push(format!("{} towers", sell.plan_idx.len()));
        }
    }
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join(" + ")
    }
}

fn kv(label: &'static str, value: String, muted: Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().child(value))
        .into_any_element()
}
