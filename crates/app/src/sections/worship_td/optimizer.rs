//! The Optimizer pane: the search inputs, its progress, and the recommended
//! plan against the baselines.
//!
//! The search runs `idleon_worship::optimize` off the UI thread through the
//! shared task runner, so it reports progress and can be cancelled. Every
//! candidate is simulated by the same engine the Plan pane runs; the pane shows
//! the winner's plan, the finalists and the baselines on shared final seeds, and
//! adopts the winner into the Plan pane.

use super::controls::segmented;
use super::plan::{kv_rows, tower_name, traits_text};
use super::table::{Col, table, text, text_id};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Hsla, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, Tone, show_number};
use idleon_worship::{self as worship, Mode, Plan, Search};

/// The Optimizer pane.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.worship_ensure_plan();
    let muted = cx.theme().muted_foreground;
    let task_state = state.worship_optimize_task(cx);
    let running = task_state.running;
    let stale = task_state.stale;
    let entity = cx.entity();

    let mut body = v_flex().id("worship-optimizer-body").gap_3().p_3().child(
        Card::new()
            .header(CardHeader::new("Search").description(
                "Every candidate is simulated by the same engine the Plan pane runs. \
                     The finalists, the coverage heuristic and the guide presets are then \
                     compared on shared fresh seeds.",
            ))
            .child(controls(state, running, window, cx)),
    );

    if running {
        if let Some(progress) = task_state.progress {
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
    } else if task_state.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "worship-optimize-cancelled",
            task_state.result.is_some(),
        ));
    }

    match &task_state.result {
        Some(Ok(result)) => {
            body = body.child(result_card(result, muted, stale, cx));
            body = body.child(baselines_card(result, muted, cx));
            body = body.child(plan_card(&result.plan, cx));
            body = body.child(final_towers_card(result, cx));
        }
        Some(Err(reason)) => {
            body = body.child(
                div().id("worship-optimize-refused").child(
                    Callout::new(Tone::Warning, "The search cannot run with this save")
                        .child(Caption::new(reason.clone()).text_color(muted)),
                ),
            );
        }
        None if !running => {
            body = body.child(
                div().id("worship-optimizer-empty").child(
                    Callout::new(Tone::Info, "No search yet").child(
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

    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("worship-optimizer-scroll")
        .into_any_element()
}

fn controls(
    state: &mut AppState,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
    let config = state.worship.optimize;
    let trial = state.worship.trial;
    let fields = &state.worship_controls;
    show_number(&fields.seeds, config.seeds as f64, 0, window, cx);
    show_number(&fields.seed, config.base_seed as f64, 0, window, cx);
    show_number(&fields.wave, f64::from(config.max_wave), 0, window, cx);

    let modes: Vec<(Mode, &'static str)> =
        Mode::ALL.iter().map(|mode| (*mode, mode.label())).collect();
    let mode = segmented("worship-optimize-mode", &modes, config.mode, {
        let entity = entity.clone();
        move |mode, _window, cx| {
            entity.update(cx, |state, cx| state.worship_set_optimize_mode(mode, cx));
        }
    });

    let run = {
        let entity = entity.clone();
        Button::new("worship-optimize-run")
            .label(if running { "Searching…" } else { "Optimize" })
            .primary()
            .icon(IconName::Play)
            .small()
            .disabled(running)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.optimize_worship(cx));
            })
    };

    let mut row = h_flex()
        .id("worship-optimize-controls")
        .items_end()
        .gap_3()
        .child(field(
            "Mode",
            mode.into_any_element(),
            "Search budget",
            muted,
        ))
        .child(field(
            "Runs per plan",
            crate::ui::number_field("worship-optimize-seeds", &fields.seeds, "Runs per plan")
                .into_any_element(),
            "Screening seeds",
            muted,
        ))
        .child(field(
            "Seed",
            crate::ui::number_field("worship-optimize-seed", &fields.seed, "Seed")
                .into_any_element(),
            "Reproducible search",
            muted,
        ))
        .child(field(
            "Stop after wave",
            crate::ui::number_field("worship-optimize-wave", &fields.wave, "Stop after wave")
                .into_any_element(),
            "Run cap",
            muted,
        ))
        .child(run);
    if running {
        row = row.child(
            Button::new("worship-optimize-cancel")
                .label("Cancel")
                .secondary()
                .icon(IconName::X)
                .small()
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.cancel_worship_optimize(cx));
                }),
        );
    }
    v_flex()
        .gap_2()
        .child(row)
        .child(
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
    v_flex()
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
    let mut actions = h_flex().gap_2();
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
            .child(
                Tag::new()
                    .with_variant(tone.into())
                    .outline()
                    .small()
                    .child(format!("Wave {:.0}", result.score.primary)),
            ),
    );
    let pairs = [
        ("Mean cleared", format!("{:.1}", result.mean_best_wave)),
        (
            "Range",
            format!("{}–{}", result.min_best_wave, result.max_best_wave),
        ),
        ("Saved best", result.saved_best.to_string()),
        ("Reached", format!("{:.1}", result.score.reached)),
        ("Final seeds", result.seeds.len().to_string()),
        (
            "Plan",
            format!(
                "{} towers · {} phases",
                result.plan.towers.len(),
                result.plan.phases.len()
            ),
        ),
        ("Found by", result.kind.to_string()),
    ];
    Card::new()
        .header(
            CardHeader::new("Recommended plan")
                .description(format!(
                    "{} · {} families · {} evaluations · {} duplicates skipped",
                    result.label, result.families, result.evaluations, result.duplicates_skipped
                ))
                .action(actions),
        )
        .child(
            v_flex()
                .id("worship-optimize-result")
                .gap_3()
                .child(kv_rows(&pairs, muted))
                .child(
                    h_flex().child(
                        Button::new("worship-optimize-use")
                            .label("Use this plan")
                            .primary()
                            .small()
                            .disabled(stale)
                            .on_click(move |_event, _window, cx| {
                                entity.update(cx, |state, cx| state.worship_use_optimized_plan(cx));
                            }),
                    ),
                ),
        )
        .when(stale, |card| {
            card.child(run_chrome::stale_banner("worship-optimize-stale"))
        })
        .into_any_element()
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

fn baselines_card(result: &Search, muted: Hsla, cx: &App) -> AnyElement {
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
    let cols = [
        Col::Text("Plan"),
        Col::Number("Cleared"),
        Col::Number("Reached"),
        Col::Text("Verdict"),
    ];
    let cells = rows
        .iter()
        .enumerate()
        .map(|(index, (label, score, baseline))| {
            let winner = *label == result.label && *score == result.score;
            let verdict = if winner {
                Tag::new()
                    .with_variant(Tone::Success.into())
                    .outline()
                    .small()
                    .child("Recommended")
                    .into_any_element()
            } else {
                let text = if !*baseline {
                    "Finalist"
                } else if result.score.primary >= score.primary {
                    "Beaten"
                } else {
                    "Held"
                };
                Caption::new(text).text_color(muted).into_any_element()
            };
            vec![
                text_id(("worship-optimize-row", index), label.clone()),
                text(format!("{:.1}", score.primary)),
                text(format!("{:.1}", score.reached)),
                verdict,
            ]
        });
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
            v_flex()
                .id("worship-optimize-baselines")
                .gap_2()
                .child(
                    div()
                        .min_w_0()
                        .child(table("worship-optimize-baseline", &cols, cells, cx)),
                )
                .child(Caption::new(summary).text_color(muted)),
        )
        .into_any_element()
}

fn plan_card(plan: &Plan, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("#"),
        Col::Text("Tower"),
        Col::Text("Node / X"),
        Col::Text("Traits"),
        Col::Number("Max level"),
        Col::Number("Weight"),
    ];
    let rows = plan.towers.iter().enumerate().map(|(index, tower)| {
        vec![
            text(format!("#{index}")),
            text_id(
                ("worship-optimize-tower", index),
                tower_name(tower.type_index),
            ),
            text(format!("{} / {:.0}", tower.slot.node, tower.slot.x)),
            text(traits_text(&tower.traits)),
            text(
                tower
                    .max_level
                    .map(|level| level.to_string())
                    .unwrap_or_else(|| "—".into()),
            ),
            text(format!("{:.2}", tower.weight)),
        ]
    });
    let phases = v_flex()
        .gap_1()
        .children(plan.phases.iter().enumerate().map(|(index, phase)| {
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
                )))
        }));
    Card::new()
        .header(CardHeader::new("Recommended plan").description(
            "The towers the search recommends, in plan order, with the plan's phases.",
        ))
        .child(
            v_flex()
                .id("worship-optimize-plan")
                .gap_3()
                .child(if plan.towers.is_empty() {
                    Caption::new("The plan places no tower.").into_any_element()
                } else {
                    div()
                        .min_w_0()
                        .child(table("worship-optimize-plan", &cols, rows, cx))
                        .into_any_element()
                })
                .child(phases),
        )
        .into_any_element()
}

fn final_towers_card(result: &Search, cx: &App) -> AnyElement {
    let detail = &result.detail;
    let total: f64 = detail.towers.iter().map(|tower| tower.damage).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    let cols = [
        Col::Text("#"),
        Col::Text("Tower"),
        Col::Text("Node / X"),
        Col::Number("Level"),
        Col::Text("Traits"),
        Col::Number("Damage share"),
    ];
    let rows = detail.towers.iter().map(|tower| {
        vec![
            text(
                tower
                    .plan_index
                    .map(|index| format!("#{index}"))
                    .unwrap_or_else(|| "—".into()),
            ),
            text_id(
                ("worship-optimize-final-tower", tower.id),
                tower_name(tower.type_index),
            ),
            text(format!("{} / {:.0}", tower.node, tower.x)),
            text(tower.level.to_string()),
            text(traits_text(&tower.traits)),
            text(format!("{:.1}%", tower.damage / total * 100.0)),
        ]
    });
    Card::new()
        .header(CardHeader::new("Final towers").description(format!(
            "The towers standing when the recommended plan's first final run (seed {}) ended at wave {}.",
            result.seeds.first().copied().unwrap_or(1),
            detail.best_wave
        )))
        .child(
            div()
                .id("worship-optimize-final-towers")
                .child(if detail.towers.is_empty() {
                    Caption::new("No tower was standing at the end.").into_any_element()
                } else {
                    div()
                        .min_w_0()
                        .child(table("worship-optimize-final", &cols, rows, cx))
                        .into_any_element()
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
