//! The Spelunking optimizer: the search controls, the run in flight and the
//! recommendation it leaves.
//!
//! The exhaustive configuration search and the bounded Amber phase search both
//! run through the shared task runner, so progress and cancel behave like the
//! expedition run. The result keeps the upstream rows, the winner against the
//! baseline, the party-size advice and a copyable plan.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_spelunking::{
    Account, Enabled, Inputs, Objective, Requested, ScopeRequest, SearchResult, search_space,
};
use idleon_ui::format;

use super::OptimizerMode;

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
pub(crate) fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.spelunk_optimizer(cx);
    let mut body = div()
        .id("spelunk-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, &task, cx));
    let space = state
        .spelunking
        .config(inputs)
        .ok()
        .and_then(|_| {
            search_space(
                inputs,
                &state.spelunking.run_requested(),
                &state.spelunking.optimizer_requested(),
            )
            .ok()
        })
        .map(|(count, _)| count);
    if state.spelunking.optimizer_mode == OptimizerMode::Configuration {
        body = body.child(
            Caption::new(match space {
                Some(count) => format!(
                    "{count} allowed configuration(s) in this scope. The search screens every one, \
                     then compares the strongest five on fresh seeds; party, cave, mode and \
                     stamina stay fixed unless the dimension is included above."
                ),
                None => "The controls above cannot form a valid expedition yet.".into(),
            })
            .text_color(muted),
        );
    } else {
        body = body.child(
            Caption::new(
                "Screens a diverse grid of dump depths, finishers, reserves, harvest floors and rope \
                 reserves, refines the leaders twice, then compares the finalists on fresh seeds. \
                 The belt must carry an orange elixir.",
            )
            .text_color(muted),
        );
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
    if let Some(result) = task.result.as_ref() {
        body = body.child(result_card(result, task.stale, inputs, cx));
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
    task: &idleon_ui::runner::TaskState<SearchResult>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let spelunking = &state.spelunking;
    let amber_mode = spelunking.optimizer_mode == OptimizerMode::Amber;

    let mode_entity = entity.clone();
    let mode = Select::new(
        "spelunk-optimizer-mode",
        [
            Choice::new("configuration", "Configurations"),
            Choice::new("amber", "Amber farming phases"),
        ],
    )
    .label("Optimization mode")
    .selected(spelunking.optimizer_mode.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        if let Some(mode) = OptimizerMode::from_id(value.as_ref()) {
            mode_entity.update(cx, |state, cx| state.set_spelunk_optimizer_mode(mode, cx));
        }
    });

    let objective_entity = entity.clone();
    let objectives: Vec<Choice> = Objective::ALL
        .into_iter()
        .filter(|objective| !amber_mode || objective.amber())
        .map(|objective| Choice::new(objective.id(), objective.label()))
        .collect();
    let objective = Select::new("spelunk-optimizer-objective", objectives)
        .label("Objective")
        .selected(spelunking.optimizer_objective.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(objective) = Objective::from_id(value.as_ref()) {
                objective_entity.update(cx, |state, cx| {
                    state.set_spelunk_optimizer_objective(objective, cx);
                });
            }
        });

    let screening_entity = entity.clone();
    let screening = NumberInput::new(
        "spelunk-optimizer-screening",
        spelunking.optimizer_screening as f64,
    )
    .range(1.0, 8.0)
    .step(1.0)
    .precision(0)
    .size(ControlSize::Sm)
    .label("Screening trials")
    .on_change(move |value, _window, cx| {
        screening_entity.update(cx, |state, cx| {
            state.set_spelunk_optimizer_screening(value.max(1.0) as usize, cx);
        });
    });

    let trials_entity = entity.clone();
    let trials = NumberInput::new(
        "spelunk-optimizer-trials",
        spelunking.optimizer_trials as f64,
    )
    .range(2.0, 64.0)
    .step(1.0)
    .precision(0)
    .size(ControlSize::Sm)
    .label("Final trials")
    .on_change(move |value, _window, cx| {
        trials_entity.update(cx, |state, cx| {
            state.set_spelunk_optimizer_trials(value.max(2.0) as usize, cx);
        });
    });

    let seed_entity = entity.clone();
    let seed = NumberInput::new(
        "spelunk-optimizer-seed",
        f64::from(spelunking.optimizer_seed),
    )
    .range(0.0, f64::from(u32::MAX))
    .step(1.0)
    .precision(0)
    .size(ControlSize::Sm)
    .label("Seed")
    .on_change(move |value, _window, cx| {
        seed_entity.update(cx, |state, cx| {
            state.set_spelunk_optimizer_seed(value.max(0.0) as u32, cx);
        });
    });

    let run_entity = entity.clone();
    let cancel_entity = entity.clone();
    let run = Button::new(
        "spelunk-optimizer-start",
        if task.running {
            "Optimizing…"
        } else {
            "Find strategy"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(task.running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_spelunk_optimizer(cx));
    });
    let cancel = Button::new("spelunk-optimizer-stop", "Cancel")
        .variant(ButtonVariant::Subtle)
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
        .child(super::field("Mode", mode.into_any_element()))
        .child(super::field("Objective", objective.into_any_element()))
        .child(super::field(
            "Screening trials",
            screening.into_any_element(),
        ))
        .child(super::field("Final trials", trials.into_any_element()))
        .child(super::field("Seed", seed))
        .child(div().flex().items_center().gap_2().child(run).child(cancel));
    if amber_mode {
        row = row
            .child(super::field(
                "Dump depth from",
                number(
                    "spelunk-optimizer-from",
                    spelunking.optimizer_from,
                    1.0,
                    10_000.0,
                    entity.clone(),
                    |state, value, cx| state.set_spelunk_optimizer_from(value, cx),
                ),
            ))
            .child(super::field(
                "Dump depth to",
                number(
                    "spelunk-optimizer-to",
                    spelunking.optimizer_to,
                    1.0,
                    10_000.0,
                    entity.clone(),
                    |state, value, cx| state.set_spelunk_optimizer_to(value, cx),
                ),
            ))
            .child(super::field(
                "Minimum rope exit %",
                number(
                    "spelunk-optimizer-min-rope",
                    spelunking.optimizer_min_rope,
                    0.0,
                    100.0,
                    entity.clone(),
                    |state, value, cx| state.set_spelunk_optimizer_min_rope(value, cx),
                ),
            ))
            .child(super::field(
                "Adaptive trial cap",
                number(
                    "spelunk-optimizer-max-trials",
                    spelunking.optimizer_max_trials,
                    2.0,
                    256.0,
                    entity.clone(),
                    |state, value, cx| state.set_spelunk_optimizer_max_trials(value, cx),
                ),
            ));
    }
    let mut scope = div()
        .id("spelunk-optimizer-scope")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_4()
        .px_3()
        .pb_3()
        .child(Caption::new("Search sections").text_color(muted))
        .child(toggle(
            "party",
            "Party",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "modes",
            "Play",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "clears",
            "Descend",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "speeds",
            "Auto speeds",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "elixirs",
            "Belt",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "memorized",
            "Memory",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle(
            "tools",
            "Tools",
            spelunking.optimizer_scope,
            entity.clone(),
        ))
        .child(toggle("entry", "Entry", spelunking.optimizer_scope, entity));
    if !amber_mode {
        scope = scope.child(
            Caption::new(
                "Unchecked sections stay at the captured controls. The party search keeps at \
                 least one eligible character; belt and memory stay fixed unless included.",
            )
            .text_color(muted),
        );
    }
    div()
        .flex()
        .flex_col()
        .child(row)
        .child(scope)
        .into_any_element()
}

fn number(
    id: &'static str,
    value: usize,
    min: f64,
    max: f64,
    entity: gpui::Entity<AppState>,
    set: fn(&mut AppState, usize, &mut Context<AppState>),
) -> AnyElement {
    NumberInput::new(id, value as f64)
        .range(min, max)
        .step(1.0)
        .precision(0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| set(state, value.max(min) as usize, cx));
        })
        .into_any_element()
}

fn toggle(
    key: &'static str,
    label: &'static str,
    scope: Enabled,
    entity: gpui::Entity<AppState>,
) -> AnyElement {
    let checked = match key {
        "party" => scope.party,
        "modes" => scope.modes,
        "clears" => scope.clears,
        "speeds" => scope.speeds,
        "elixirs" => scope.elixirs,
        "memorized" => scope.memorized,
        "tools" => scope.tools,
        "entry" => scope.entry,
        _ => true,
    };
    Checkbox::new(format!("spelunk-optimizer-{key}"), checked)
        .label(label)
        .on_change(move |on, _window, cx| {
            entity.update(cx, |state, cx| {
                let mut scope = state.spelunking.optimizer_scope;
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
    let muted = cx.theme().colors.fg_muted;
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
    let entity = cx.entity();
    let use_entity = entity.clone();
    let copy_entity = entity;
    let winner = result.winner.clone();
    let mut body = div()
        .id("spelunk-optimizer-result")
        .flex()
        .flex_col()
        .gap_3()
        .p_3();
    if stale {
        body = body.child(idleon_ui::run_chrome::stale_banner("spelunk-optimizer"));
    }
    if let Some(winner) = &winner {
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
        body = body
            .child(
                div()
                    .id("spelunk-optimizer-winner")
                    .role(gpui::Role::Label)
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
                    .child(
                        Badge::new(format!(
                            "{} {}",
                            result.objective.label(),
                            format::value(winner.summary.score)
                        ))
                        .tone(Tone::Accent),
                    )
                    .child(Badge::new(format!(
                        "Baseline {}",
                        format::value(result.baseline.summary.score)
                    )))
                    .child(
                        Badge::new(format!("Change {delta:+}")).tone(if delta >= 0.0 {
                            Tone::Success
                        } else {
                            Tone::Warning
                        }),
                    )
                    .child(Badge::new(format!(
                        "Depth {} · {} Amber",
                        format::value(winner.summary.depth),
                        format::value(winner.summary.amber)
                    ))),
            )
            .child(
                div()
                    .id("spelunk-optimizer-plan")
                    .role(gpui::Role::Label)
                    .aria_label(idleon_spelunking::plan_text(inputs, &winner.summary.config))
                    .text_size(px(13.0))
                    .child(idleon_spelunking::plan_text(inputs, &winner.summary.config)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("spelunk-optimizer-use", "Use recommendation")
                            .icon(IconName::Check)
                            .on_click(move |_event, _window, cx| {
                                use_entity.update(cx, |state, cx| {
                                    state.use_spelunk_optimizer_winner(cx);
                                });
                            }),
                    )
                    .child(
                        Button::new("spelunk-optimizer-copy", "Copy plan")
                            .icon(IconName::Copy)
                            .variant(ButtonVariant::Subtle)
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
            body = body.child(
                Caption::new(format!(
                    "Smallest measured party retaining 99% of the best observed {} gain: {size} \
                     character(s). This allowance is descriptive, not proven equivalence.",
                    result.objective.label()
                ))
                .text_color(muted),
            );
        }
    } else {
        body = body.child(
            Caption::new(
                "No fresh finalist met the selected rope-exit requirement, so there is no \
                 recommendation.",
            )
            .text_color(muted),
        );
    }
    if let Some(info) = &result.amber_search {
        body = body.child(
            Caption::new(format!(
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
            ))
            .text_color(muted),
        );
    }
    body = body.child(Caption::new(result.claim.clone()).text_color(muted));
    let mut table = Table::new([
        "Configuration",
        result.objective.label(),
        "Mean depth",
        "Amber",
        "Settled",
        "Rope / toll / blocked",
    ]);
    for (index, row) in result.rows.iter().enumerate() {
        table = table.row([
            div()
                .id(format!("spelunk-optimizer-row-{index}"))
                .flex()
                .flex_col()
                .child(if row.reference_only {
                    div().child(format!("{} (reference only)", row.label))
                } else {
                    div().child(row.label.clone())
                })
                .child(Caption::new(short_config(&row.summary.config)).text_color(muted))
                .into_any_element(),
            div()
                .child(format::value(row.summary.score))
                .into_any_element(),
            div()
                .child(format::value(row.summary.depth))
                .into_any_element(),
            div()
                .child(format::value(row.summary.amber))
                .into_any_element(),
            div()
                .child(format!("{}/{}", row.summary.settled, row.summary.total))
                .into_any_element(),
            div()
                .child(format!(
                    "{} / {} / {}",
                    row.summary.rope, row.summary.toll, row.summary.blocked
                ))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(header)
        .child(body.child(table))
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
