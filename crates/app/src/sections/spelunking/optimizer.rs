//! The Spelunking optimizer: the search controls, the run in flight and the
//! recommendation it leaves.
//!
//! The exhaustive configuration search and the bounded Amber phase search both
//! run through the shared task runner, so progress and cancel behave like the
//! expedition run. The result keeps the ranked rows, the winner against the
//! baseline, the party-size advice and a copyable plan.

use super::controls::{Derived, optimizer_mode_choices};
use super::{OptimizerMode, boxed, field, number, select, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_spelunking::{
    Account, Enabled, Inputs, Objective, Requested, ScopeRequest, SearchResult,
};
use idleon_ui::format;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, Tone};
use std::rc::Rc;

/// What an optimizer run was produced from; a change marks the result stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimizerInputs {
    pub(crate) requested: Requested,
    pub(crate) account: Account,
    pub(crate) mode: OptimizerMode,
    pub(crate) objective: Objective,
    pub(crate) screening: usize,
    pub(crate) trials: usize,
    pub(crate) seed: u32,
    pub(crate) scope: ScopeRequest,
    pub(crate) amber_from: usize,
    pub(crate) amber_to: usize,
    pub(crate) amber_min_rope: usize,
    pub(crate) amber_max_trials: usize,
    /// The save this run read, so a new import is stale.
    pub(crate) revision: u64,
}

/// The optimizer card: the mode, the search scope, the run and its result.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.spelunk_optimizer(cx);
    let mut body = div()
        .id("spelunk-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, derived, &task, window, cx));
    if state.spelunking.optimizer_mode == OptimizerMode::Configuration {
        body = body.child(Caption::new(match derived.space {
            Some(count) => format!(
                "{count} allowed configuration(s) in this scope. The search screens every one, \
                 then compares the strongest five on fresh seeds; party, cave, mode and stamina \
                 stay fixed unless the dimension is included above."
            ),
            None => "The controls above cannot form a valid expedition yet.".into(),
        }));
    } else {
        body = body.child(Caption::new(
            "Screens a diverse grid of dump depths, finishers, reserves, harvest floors and rope \
             reserves, refines the leaders twice, then compares the finalists on fresh seeds. The \
             belt must carry an orange elixir.",
        ));
    }
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "spelunk-optimizer-progress",
            "Optimizing the expedition",
            "Runs off the UI thread; cancelling keeps the previous recommendation.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_spelunk_optimizer(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "spelunk-optimizer-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(result) = &task.result {
        body = body.child(result_card(result, task.stale, &derived.inputs, cx));
    }
    Card::new()
        .header(CardHeader::new("Depth optimizer").description(
            "Maximises the selected objective for the cave, party and belt on screen. The \
             recommendation is reproducible from the seed shown in the run result.",
        ))
        .child(body)
        .into_any_element()
}

/// The mode, objective, trial counts and search dimensions.
fn controls(
    state: &AppState,
    derived: &Derived,
    task: &TaskState<Rc<SearchResult>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let spelunking = &state.spelunking;
    let controls = &state.spelunking_controls;
    let amber_mode = spelunking.optimizer_mode == OptimizerMode::Amber;
    let mode = select(
        &controls.optimizer_mode,
        "spelunk-optimizer-mode",
        "Optimization mode",
        &optimizer_mode_choices(),
        &spelunking.optimizer_mode,
        window,
        cx,
    );
    let objective = select(
        &controls.objective,
        "spelunk-optimizer-objective",
        "Objective",
        &derived.objectives,
        &spelunking.optimizer_objective,
        window,
        cx,
    );
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let run = Button::new("spelunk-optimizer-start")
        .label(if task.running {
            "Optimizing…"
        } else {
            "Find strategy"
        })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(task.running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_spelunk_optimizer(cx));
        });
    let cancel = Button::new("spelunk-optimizer-stop")
        .label("Cancel")
        .secondary()
        .small()
        .icon(IconName::X)
        .disabled(!task.running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_spelunk_optimizer(cx));
        });

    let mut row = div()
        .id("spelunk-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_4()
        .p_3()
        .child(field(
            "Mode",
            boxed(&controls.optimizer_mode, mode, window, cx),
        ))
        .child(field(
            "Objective",
            boxed(&controls.objective, objective, window, cx),
        ))
        .child(field(
            "Screening trials",
            number(
                "spelunk-optimizer-screening",
                &controls.screening,
                "Screening trials",
                spelunking.optimizer_screening as f64,
                0,
                window,
                cx,
            ),
        ))
        .child(field(
            "Final trials",
            number(
                "spelunk-optimizer-trials",
                &controls.trials,
                "Final trials",
                spelunking.optimizer_trials as f64,
                0,
                window,
                cx,
            ),
        ))
        .child(field(
            "Seed",
            number(
                "spelunk-optimizer-seed",
                &controls.optimizer_seed,
                "Seed",
                f64::from(spelunking.optimizer_seed),
                0,
                window,
                cx,
            ),
        ))
        .child(div().flex().items_center().gap_2().child(run).child(cancel));
    if amber_mode {
        let amber_fields = [
            (
                "Dump depth from",
                "spelunk-optimizer-from",
                &controls.optimizer_from,
                spelunking.optimizer_from,
            ),
            (
                "Dump depth to",
                "spelunk-optimizer-to",
                &controls.optimizer_to,
                spelunking.optimizer_to,
            ),
            (
                "Minimum rope exit %",
                "spelunk-optimizer-min-rope",
                &controls.min_rope,
                spelunking.optimizer_min_rope,
            ),
            (
                "Adaptive trial cap",
                "spelunk-optimizer-max-trials",
                &controls.max_trials,
                spelunking.optimizer_max_trials,
            ),
        ];
        for (label, id, control, value) in amber_fields {
            row = row.child(field(
                label,
                number(id, control, label, value as f64, 0, window, cx),
            ));
        }
    }
    let scope = spelunking.optimizer_scope;
    let toggles = [
        ("party", "Party", scope.party),
        ("modes", "Play", scope.modes),
        ("clears", "Descend", scope.clears),
        ("speeds", "Auto speeds", scope.speeds),
        ("elixirs", "Belt", scope.elixirs),
        ("memorized", "Memory", scope.memorized),
        ("tools", "Tools", scope.tools),
        ("entry", "Entry", scope.entry),
    ];
    let scope = div()
        .id("spelunk-optimizer-scope")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_4()
        .px_3()
        .pb_3()
        .child(Caption::new("Search sections"))
        .children(
            toggles
                .into_iter()
                .map(|(key, label, checked)| toggle(key, label, checked, cx)),
        )
        .when(!amber_mode, |scope| {
            scope.child(Caption::new(
                "Unchecked sections stay at the captured controls. The party search keeps at \
                 least one eligible character; belt and memory stay fixed unless included.",
            ))
        });
    div()
        .flex()
        .flex_col()
        .child(row)
        .child(scope)
        .into_any_element()
}

fn toggle(
    key: &'static str,
    label: &'static str,
    checked: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    Checkbox::new(format!("spelunk-optimizer-{key}"))
        .label(label)
        .checked(checked)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| {
                let mut scope: Enabled = state.spelunking.optimizer_scope;
                match key {
                    "party" => scope.party = on,
                    "modes" => scope.modes = on,
                    "clears" => scope.clears = on,
                    "speeds" => scope.speeds = on,
                    "elixirs" => scope.elixirs = on,
                    "memorized" => scope.memorized = on,
                    "tools" => scope.tools = on,
                    "entry" => scope.entry = on,
                    _ => {}
                }
                state.set_spelunk_optimizer_scope(scope, cx);
            });
        })
        .into_any_element()
}

/// The stored search: the winner, the baseline and every compared row.
fn result_card(
    result: &SearchResult,
    stale: bool,
    inputs: &Inputs,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut header = CardHeader::new(format!("Optimizer result · {}", result.objective.label()))
        .description(format!(
            "{} · {} configurations · {} trials per finalist · seed {}",
            if result.kind == "search" {
                "Every allowed configuration screened"
            } else {
                "Bounded Amber phase search"
            },
            result.configurations,
            result.baseline.summary.total,
            result.seed
        ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("spelunk-optimizer"));
    }
    let use_entity = cx.entity();
    let copy_entity = cx.entity();
    let mut body = div()
        .id("spelunk-optimizer-result")
        .flex()
        .flex_col()
        .gap_3()
        .p_3();
    if stale {
        body = body.child(idleon_ui::run_chrome::stale_banner("spelunk-optimizer"));
    }
    if let Some(winner) = &result.winner {
        let delta = result.delta.unwrap_or(0.0);
        let interval = result
            .interval
            .map(|[low, high]| format!(" (95% {} to {})", format::value(low), format::value(high)))
            .unwrap_or_default();
        let change = if delta >= 0.0 {
            format!("+{}", format::value(delta))
        } else {
            format::value(delta)
        };
        let plan = idleon_spelunking::plan_text(inputs, &winner.summary.config);
        body = body
            .child(
                div()
                    .id("spelunk-optimizer-winner")
                    .role(Role::Label)
                    .aria_label(format!(
                        "Best measured {}: {} against baseline {}. Change {change}{interval}.",
                        result.objective.label(),
                        format::value(winner.summary.score),
                        format::value(result.baseline.summary.score),
                    ))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(tag(
                        format!(
                            "{} {}",
                            result.objective.label(),
                            format::value(winner.summary.score)
                        ),
                        Tone::Accent,
                    ))
                    .child(tag(
                        format!("Baseline {}", format::value(result.baseline.summary.score)),
                        Tone::Neutral,
                    ))
                    .child(tag(
                        format!("Change {delta:+}"),
                        if delta >= 0.0 {
                            Tone::Success
                        } else {
                            Tone::Warning
                        },
                    ))
                    .child(tag(
                        format!(
                            "Depth {} · {} Amber",
                            format::value(winner.summary.depth),
                            format::value(winner.summary.amber)
                        ),
                        Tone::Neutral,
                    )),
            )
            .child(
                div()
                    .id("spelunk-optimizer-plan")
                    .role(Role::Label)
                    .aria_label(plan.clone())
                    .text_size(TEXT_BODY)
                    .child(plan),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("spelunk-optimizer-use")
                            .label("Use recommendation")
                            .small()
                            .icon(IconName::Check)
                            .on_click(move |_event, _window, cx| {
                                use_entity.update(cx, |state, cx| {
                                    state.use_spelunk_optimizer_winner(cx);
                                });
                            }),
                    )
                    .child(
                        Button::new("spelunk-optimizer-copy")
                            .label("Copy plan")
                            .secondary()
                            .small()
                            .icon(IconName::Copy)
                            .on_click(move |_event, _window, cx| {
                                copy_entity.update(cx, |state, cx| {
                                    state.copy_spelunk_optimizer_plan(cx);
                                });
                            }),
                    ),
            );
        if let Some(party) = &winner.party_analysis
            && let Some(size) = party.recommended_size
        {
            body = body.child(Caption::new(format!(
                "Smallest measured party retaining 99% of the best observed {} gain: {size} \
                 character(s). This allowance is descriptive, not proven equivalence.",
                result.objective.label()
            )));
        }
    } else {
        body = body.child(Caption::new(
            "No fresh finalist met the selected rope-exit requirement, so there is no \
             recommendation.",
        ));
    }
    if let Some(info) = &result.amber_search {
        body = body.child(Caption::new(format!(
            "Amber phase search: {} strategies from D{} to D{} (effective D{}), screening {} \
             trial(s), {} racing trials, {} finalists, {} trials each, {} adaptive extension(s). \
             {} distinct behaviours observed. Unresolved leads: {}; rope rate uncertain: {}.",
            result.screened,
            info.from,
            info.to,
            info.effective_from,
            info.screening,
            info.racing_trials,
            info.finalist_strategies,
            info.trials,
            info.adaptive_rounds,
            info.observed_behaviors,
            info.unresolved,
            info.risk_uncertain,
        )));
    }
    body = body.child(Caption::new(result.claim.clone()));
    // The compared rows: the finalists and the baseline, a handful.
    let rows = result.rows.iter().enumerate().map(|(index, row)| {
        TableRow::new()
            .id(("spelunk-optimizer-compared", index))
            .child(
                // The configuration line wraps inside its share instead of
                // spilling under the figures.
                TableCell::new()
                    .overflow_hidden()
                    .whitespace_normal()
                    .child(
                        div()
                            .id(format!("spelunk-optimizer-row-{index}"))
                            .flex()
                            .flex_col()
                            .w_full()
                            .min_w_0()
                            .child(if row.reference_only {
                                format!("{} (reference only)", row.label)
                            } else {
                                row.label.clone()
                            })
                            .child(Caption::new(short_config(&row.summary.config))),
                    ),
            )
            .child(
                TableCell::new()
                    .text_right()
                    .child(format::value(row.summary.score)),
            )
            .child(
                TableCell::new()
                    .text_right()
                    .child(format::value(row.summary.depth)),
            )
            .child(
                TableCell::new()
                    .text_right()
                    .child(format::value(row.summary.amber)),
            )
            .child(
                TableCell::new()
                    .text_right()
                    .child(format!("{}/{}", row.summary.settled, row.summary.total)),
            )
            .child(TableCell::new().text_right().child(format!(
                "{} / {} / {}",
                row.summary.rope, row.summary.toll, row.summary.blocked
            )))
    });
    let heads = [
        ("Configuration", false),
        (result.objective.label(), true),
        ("Mean depth", true),
        ("Amber", true),
        ("Settled", true),
        ("Rope / toll / blocked", true),
    ];
    let table = Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(heads.map(|(head, numeric)| {
                TableHead::new()
                    .when(numeric, |head| head.text_right())
                    .child(head)
            }))),
        )
        .child(TableBody::new().children(rows));
    Card::new()
        .header(header)
        .child(body.child(div().id("spelunk-optimizer-table").child(table)))
        .into_any_element()
}

/// A compact configuration line for the result table.
fn short_config(config: &idleon_spelunking::Config) -> String {
    format!(
        "{} · {} · speed {} · {} · {}",
        if config.manic { "Manic" } else { "Normal" },
        config.policy.label(),
        config.speed,
        match config.tool {
            idleon_spelunking::ToolChoice::Adaptive => "adaptive".to_string(),
            idleon_spelunking::ToolChoice::Tool(id) => format!("tool {id}"),
        },
        config.entry.label(),
    )
}
