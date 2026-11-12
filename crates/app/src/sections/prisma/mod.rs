//! The Prisma Calculator: the plan, the streamed impact search and its coverage.
//!
//! Ported from `prisma-calc.html` and `js/ui/prisma.js`. The search runs off the
//! UI thread through the shared task runner, so it reports progress and can be
//! cancelled; cancelling keeps the result already on screen, and changing the
//! save, character, cauldron or AFK hours marks it stale.
//!
//! Each row compares the saved scope without this bubble's Prisma with the
//! scope with it, so the columns are per-metric deltas: the bubble's own bonus
//! (an intermediate) and every downstream outcome the build models, today the
//! Statue EXP retention the STARTUE EXP bubble buys. The coverage table names
//! every effect, the native systems upstream's source audit traced for it and
//! whether this build computes a downstream metric for it.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use crate::ui::{format, task};
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::{Callout, EmptyState};
use ely_gpui_component::forms::{Choice, NumberInput, SearchInput, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, div, px,
};
use idleon_core::prisma::{self, Cauldron, Change, Consumer, Outcome, Plan, RowResult, Status};

/// The AFK claim range upstream validates, in hours.
pub const MIN_HOURS: f64 = 0.034;
pub const MAX_HOURS: f64 = 55_555.0;

/// The section's own state: the run inputs and the last plan.
pub struct State {
    pub character: usize,
    pub cauldron: Cauldron,
    pub afk_hours: f64,
    /// The plan the save, character and cauldron resolve to.
    pub plan: Option<Plan>,
    /// Why the plan could not be built, when it could not.
    pub plan_error: Option<String>,
    /// The metric the rows rank by; empty is the bubble order.
    pub metric: Option<String>,
    pub search: Entity<TextInput>,
}

impl State {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Name or changed outcome")
                .label("Filter bubbles")
        });
        Self {
            character: 0,
            cauldron: Cauldron::default(),
            afk_hours: 1.0,
            plan: None,
            plan_error: None,
            metric: None,
            search,
        }
    }

    /// Rebuild the plan from the save, keeping the character when it still exists.
    pub fn adopt(&mut self, save: Option<&idleon_core::save::Save>) {
        let Some(save) = save else {
            self.plan = None;
            self.plan_error = None;
            return;
        };
        match prisma::plan(save, self.character, self.cauldron.index()) {
            Ok(plan) => {
                self.plan = Some(plan);
                self.plan_error = None;
            }
            Err(error) => {
                // Retry once at the first character; a save with no characters
                // reports character 0 again, so a second reset would recurse
                // until the stack runs out.
                if matches!(error, prisma::PlanError::UnknownCharacter(_)) && self.character != 0 {
                    self.character = 0;
                    return self.adopt(Some(save));
                }
                self.plan = None;
                self.plan_error = Some(error.to_string());
            }
        }
    }

    /// The filter text, for the row search.
    pub fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().to_string()
    }
}

/// The inputs a run captures, so a later edit marks its result stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunInputs {
    pub save_revision: u64,
    pub character: usize,
    pub cauldron: usize,
    /// The AFK claim in whole seconds; zero when the value is not usable.
    pub afk_seconds: u64,
}

/// Whether the AFK claim is inside the upstream range.
pub fn valid_hours(hours: f64) -> bool {
    hours.is_finite() && (MIN_HOURS..=MAX_HOURS).contains(&hours)
}

/// The Prisma Calculator page.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let query = state.prisma.query(cx);
    let task_state = state.prisma_task(cx);
    let plan = state.prisma.plan.clone();
    let plan_error = state.prisma.plan_error.clone();
    let running = task_state.running;
    let stale = task_state.stale;
    let cancelled = task_state.cancelled;
    let progress = task_state.progress.clone();
    let outcome = task_state.result.clone();
    let muted = cx.theme().colors.fg_muted;

    let mut body = div()
        .id("prisma-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(toolbar(state, plan.as_ref(), running, cx));

    if running {
        if let Some(progress) = progress {
            let entity = cx.entity();
            body = body.child(task::progress_card(
                "prisma",
                "Comparing bubbles",
                "Runs off the UI thread. Cancelling keeps the result this run replaced.",
                progress,
                move |_window, cx| {
                    entity.update(cx, |state, cx| state.cancel_prisma(cx));
                },
            ));
        }
    } else if cancelled {
        body = body.child(task::cancelled_banner(
            "prisma-cancelled",
            outcome.is_some(),
        ));
    }

    if stale {
        body = body.child(task::stale_banner("prisma-stale"));
    }

    match (&plan, plan_error) {
        (Some(plan), _) => {
            body = body.child(summary_card(plan, outcome.as_ref(), muted));
            body = body.child(results_card(
                state,
                plan,
                outcome.as_ref(),
                &query,
                stale,
                window,
                cx,
            ));
            body = body.child(coverage_card(outcome.as_ref(), muted));
        }
        (None, Some(error)) => {
            body = body.child(
                div().id("prisma-refused").p_3().child(
                    Callout::new(Severity::Warning)
                        .title("The Prisma calculator cannot read this save")
                        .child(Caption::new(error).text_color(muted)),
                ),
            );
        }
        (None, None) => {
            let entity = cx.entity();
            body = body.child(
                Card::new().header(CardHeader::new("Bubble impacts")).child(
                    EmptyState::new(
                        "prisma-empty",
                        IconName::Sparkles,
                        "Import a save to compare Prisma bubbles",
                    )
                    .body(
                        "The calculator reads each cauldron's bubble levels and the Prisma \
                             ownership list from the save, then compares every eligible bubble.",
                    )
                    .action(
                        Button::new("prisma-import", "Import a save").on_click(
                            move |_event, window, cx| {
                                entity.update(cx, |state, cx| state.open_import(window, cx));
                            },
                        ),
                    ),
                ),
            );
        }
    }

    ScrollArea::new("prisma-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// Character, AFK hours, cauldron tabs and the run action.
fn toolbar(
    state: &mut AppState,
    plan: Option<&Plan>,
    running: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let characters: Vec<Choice> = plan
        .map(|plan| {
            plan.characters
                .iter()
                .map(|character| {
                    Choice::new(
                        character.index.to_string(),
                        format!("{} — {}", character.name, character.class_name),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let has_characters = !characters.is_empty();
    let hours_ok = valid_hours(state.prisma.afk_hours);

    let character_entity = entity.clone();
    let character = Select::new("prisma-character", characters)
        .label("Character")
        .placeholder("No characters")
        .selected(state.prisma.character.to_string())
        .size(ControlSize::Sm)
        .disabled(!has_characters)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                character_entity.update(cx, |state, cx| state.set_prisma_character(index, cx));
            }
        });

    let hours_entity = entity.clone();
    let hours = NumberInput::new("prisma-afk-hours", state.prisma.afk_hours)
        .label("AFK claim hours")
        .range(MIN_HOURS, MAX_HOURS)
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            hours_entity.update(cx, |state, cx| state.set_prisma_hours(value, cx));
        });

    let cauldron_entity = entity.clone();
    let cauldron = Cauldron::ALL
        .into_iter()
        .fold(
            SegmentedControl::new("prisma-cauldron", state.prisma.cauldron.id())
                .size(ControlSize::Sm),
            |control, option| control.segment(option.id(), option.label(), None),
        )
        .on_change(move |value, _window, cx| {
            if let Some(next) = Cauldron::from_id(value.as_ref()) {
                cauldron_entity.update(cx, |state, cx| state.set_prisma_cauldron(next, cx));
            }
        });

    let run_entity = entity.clone();
    let run = Button::new("prisma-run", "Recalculate")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .disabled(!has_characters || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_prisma(cx));
        });

    let cancel_entity = entity.clone();
    let cancel = Button::new("prisma-cancel", "Cancel")
        .size(ControlSize::Sm)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_prisma(cx));
        });

    div()
        .id("prisma-toolbar")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(field("Character", muted).child(character))
        .child(field("AFK claim hours", muted).child(hours))
        .child(field("Cauldron", muted).child(cauldron))
        .child(div().flex().items_center().gap_2().child(run).child(cancel))
        .when(!hours_ok, |toolbar| {
            toolbar.child(
                div().id("prisma-hours-warning").role(Role::Label).child(
                    Caption::new(format!(
                        "Enter AFK hours between {MIN_HOURS} and {MAX_HOURS}."
                    ))
                    .text_color(muted),
                ),
            )
        })
        .into_any_element()
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

/// The saved multiplier, the fragments and the saved map, plus the last run's count.
fn summary_card(plan: &Plan, outcome: Option<&Outcome>, muted: gpui::Hsla) -> AnyElement {
    let multiplier = format!("{:.4}", plan.multiplier);
    let fragments = plan
        .fragments
        .map(format::number)
        .unwrap_or_else(|| "unavailable".into());
    let map = plan
        .map
        .map(|map| map.to_string())
        .unwrap_or_else(|| "unavailable".into());
    let affordable = if plan.affordable() {
        String::new()
    } else {
        " · fewer than 10 fragments: previews are hypothetical".to_string()
    };
    let summary = match outcome {
        Some(outcome) => format!(
            "{} · Prisma ×{multiplier} · {fragments} fragments · saved map {map} · compared {} bubbles{affordable}",
            plan.cauldron_label(),
            outcome.results.len()
        ),
        None => format!(
            "{} · Prisma ×{multiplier} · {fragments} fragments · saved map {map} · {} of {} bubbles comparable{affordable}",
            plan.cauldron_label(),
            plan.comparable(),
            plan.rows.len()
        ),
    };
    Card::new()
        .header(CardHeader::new("Saved state").description(
            "Prisma multiplies every Prisma-ed bubble's bonus; adding Prisma to one costs 10 fragments. \
             Previews never spend fragments or change the save.",
        ))
        .child(
            div()
                .id("prisma-summary")
                .role(Role::Label)
                .aria_label(summary.clone())
                .p_3()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(summary),
        )
        .child(
            div().px_3().pb_3().child(
                Caption::new(match outcome {
                    Some(outcome) => format!(
                        "{} of {} effects have a downstream model in this build. The bubble bonus \
                         is always an intermediate; Coverage names the rest and the native systems \
                         that read them.",
                        outcome.coverage.modelled(),
                        outcome.coverage.total()
                    ),
                    None => "Each row measures the bubble bonus (an intermediate) and any \
                             downstream outcome this build models. Coverage names the rest."
                        .to_string(),
                })
                .text_color(muted),
            ),
        )
        .into_any_element()
}

/// The filter controls and the ranked rows.
fn results_card(
    state: &mut AppState,
    plan: &Plan,
    outcome: Option<&Outcome>,
    query: &str,
    stale: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let compact = Breakpoint::of(window).is_compact();
    let table_height = layout::table_height(f32::from(window.viewport_size().height));

    let mut metrics: Vec<Choice> = vec![Choice::new("", "Bubble order")];
    if let Some(outcome) = outcome {
        metrics.extend(
            outcome
                .metrics()
                .iter()
                .map(|metric| Choice::new(metric.id.clone(), metric.name.clone())),
        );
    }
    let chosen = state.prisma.metric.clone().unwrap_or_default();
    let metric_entity = entity.clone();
    let metric = Select::new("prisma-metric", metrics)
        .label("Rank by")
        .selected(chosen.clone())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let value = value.to_string();
            metric_entity.update(cx, |state, cx| state.set_prisma_metric(&value, cx));
        });

    let search_focus = state.prisma.search.read(cx).focus().clone();
    let search = div()
        .id("prisma-search")
        .role(Role::Search)
        .aria_label("Filter bubbles")
        .w(px(280.0))
        .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
        .child(SearchInput::new(
            "prisma-search-field",
            &state.prisma.search,
        ));

    let rows: Vec<&RowResult> = match outcome {
        Some(outcome) if chosen.is_empty() => outcome
            .bubbles()
            .into_iter()
            .filter(|result| result.matches(query))
            .collect(),
        Some(outcome) => outcome
            .rank(&chosen)
            .into_iter()
            .filter(|result| result.matches(query))
            .collect(),
        None => Vec::new(),
    };

    let selected_metric = (!chosen.is_empty()).then_some(chosen.as_str());
    let table = rows.iter().fold(
        Table::new([
            "Bubble",
            "Level",
            "Status",
            "Bubble bonus",
            "Downstream change",
        ]),
        |table, result| {
            table.row([
                bubble_cell(result),
                level_cell(result),
                status_cell(result.row.status),
                bonus_cell(result),
                change_cell(result, selected_metric, compact),
            ])
        },
    );

    let body = if outcome.is_none() {
        EmptyState::new(
            "prisma-no-run",
            IconName::Sparkles,
            "No comparison yet.",
        )
        .body(
            "Recalculate compares every eligible bubble in this cauldron: each run is the saved \
             state with one bubble's Prisma flag toggled.",
        )
        .into_any_element()
    } else if rows.is_empty() {
        EmptyState::new(
            "prisma-no-matches",
            IconName::SearchX,
            "No bubbles match your filter.",
        )
        .body("Try a bubble name, an effect key such as TotalSTR, or a changed outcome.")
        .into_any_element()
    } else {
        ScrollArea::new("prisma-table-scroll")
            .vertical()
            .h(table_height)
            .child(div().min_w_0().child(table))
            .into_any_element()
    };

    let compared = outcome.map(|outcome| outcome.results.len()).unwrap_or(0);
    let changed = outcome
        .map(|outcome| {
            outcome
                .results
                .iter()
                .filter(|result| !result.changes.is_empty())
                .count()
        })
        .unwrap_or(0);
    let description = if outcome.is_some() {
        format!(
            "{compared} bubbles compared · {changed} with a downstream change · {} shown",
            rows.len()
        )
    } else {
        format!(
            "{} eligible bubbles in {}",
            plan.comparable(),
            plan.cauldron_label()
        )
    };

    Card::new()
        .header(
            CardHeader::new("Bubble impacts")
                .description(description)
                .when(stale, |header| {
                    header.action(task::stale_badge("prisma-result-stale"))
                }),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .px_3()
                .pt_3()
                .child(field("Rank by", muted).child(metric))
                .child(field("Filter bubbles", muted).child(search)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_3()
                .child(body)
                .child(
                div()
                    .id("prisma-status")
                    .role(Role::Label)
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(format!(
                        "Every row compares without → with this bubble's Prisma, so a ready row \
                             previews the gain and an already-Prisma row measures the benefit it \
                             retains. Bubble bonuses are intermediates; only the downstream change \
                             is an outcome. {compared} compared, {changed} moved a downstream metric."
                    )),
            ),
        )
        .into_any_element()
}

fn bubble_cell(result: &RowResult) -> AnyElement {
    div()
        .id(SharedString::from(format!(
            "prisma-row-{}-{}",
            result.row.cauldron, result.row.index
        )))
        .role(Role::Label)
        .aria_label(format!(
            "{} · {} · {}",
            result.row.name,
            result.row.effect,
            result.comparison.label()
        ))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(result.row.name.clone()),
        )
        .child(Caption::new(result.row.effect.clone()))
        .into_any_element()
}

fn level_cell(result: &RowResult) -> AnyElement {
    let label = match result.row.level {
        Some(level) => format::number(level),
        None => "unavailable".into(),
    };
    div()
        .id(SharedString::from(format!(
            "prisma-level-{}-{}",
            result.row.cauldron, result.row.index
        )))
        .role(Role::Label)
        .text_size(px(13.0))
        .child(label)
        .into_any_element()
}

fn status_cell(status: Status) -> AnyElement {
    let tone = match status {
        Status::Ready => Tone::Success,
        Status::Already => Tone::Info,
        Status::Ineligible => Tone::Neutral,
        Status::Unavailable | Status::Unpurchased => Tone::Warning,
    };
    div()
        .id(SharedString::from(format!("prisma-status-{}", status.id())))
        .role(Role::Label)
        .child(Badge::new(status.label()).tone(tone))
        .into_any_element()
}

/// The bubble's own processed bonus: the value without this bubble's Prisma and
/// the value with it, plus the delta the flip buys.
fn bonus_cell(result: &RowResult) -> AnyElement {
    let metric = format!("bubble-bonus:{}", result.row.id);
    let change = result
        .intermediates
        .first()
        .cloned()
        .or_else(|| result.view(&metric));
    let label = match &change {
        Some(change) => format!(
            "{} → {}",
            format::number(change.before),
            format::number(change.after)
        ),
        None => "unavailable".into(),
    };
    let delta = change.map(|change| change_text(&change, true));
    let spoken = match &delta {
        Some(delta) => format!("{label} ({delta})"),
        None => label.clone(),
    };
    div()
        .id(SharedString::from(format!(
            "prisma-bonus-{}-{}",
            result.row.cauldron, result.row.index
        )))
        .role(Role::Label)
        .aria_label(spoken)
        .flex()
        .flex_col()
        .gap_0p5()
        .child(div().text_size(px(13.0)).child(label))
        .children(delta.map(Caption::new))
        .into_any_element()
}

/// One row's downstream delta: the selected metric when one ranks the rows,
/// else the first measured change the build found.
fn change_cell(result: &RowResult, metric: Option<&str>, compact: bool) -> AnyElement {
    let change = match metric {
        Some(metric) => result.view(metric),
        None => result.changes.first().cloned(),
    };
    let tone = match &change {
        Some(change) if change.delta > 0.0 => Tone::Success,
        Some(change) if change.delta < 0.0 => Tone::Warning,
        _ => Tone::Neutral,
    };
    let (label, name) = match &change {
        Some(change) => (
            change_text(change, compact),
            metric.is_none().then(|| change.name.clone()),
        ),
        None => ("No measured change".to_string(), None),
    };
    let spoken = match &name {
        Some(name) => format!("{label} · {name}"),
        None => label.clone(),
    };
    div()
        .id(SharedString::from(format!(
            "prisma-change-{}-{}",
            result.row.cauldron, result.row.index
        )))
        .role(Role::Label)
        .aria_label(spoken)
        .flex()
        .flex_col()
        .gap_0p5()
        .child(Badge::new(label).tone(tone))
        .children(name.map(Caption::new))
        .into_any_element()
}

fn change_text(change: &Change, compact: bool) -> String {
    let percent = change
        .percent
        .map(|percent| format!(" ({percent:+.2}%)"))
        .unwrap_or_default();
    if compact {
        format!("{}{percent}", format::number(change.delta))
    } else {
        format!(
            "{}{percent} · {} → {}",
            format::number(change.delta),
            format::number(change.before),
            format::number(change.after)
        )
    }
}

/// The native reference a consumer row names, trimmed the way the upstream
/// detail list prints it.
fn consumer_text(consumer: &Consumer) -> String {
    let method = consumer
        .method
        .replace("_customBlock_", "")
        .replace("_customEvent_", "")
        .replace("_event_", "");
    match &consumer.mode {
        Some(mode) => format!("{method} · {}", mode.label()),
        None => method,
    }
}

/// The first few consumers, with the rest counted.
fn consumer_summary(consumers: &[Consumer]) -> String {
    const SHOWN: usize = 3;
    if consumers.is_empty() {
        return "No direct native reference".into();
    }
    let mut parts: Vec<String> = consumers.iter().take(SHOWN).map(consumer_text).collect();
    if consumers.len() > SHOWN {
        parts.push(format!("+{} more", consumers.len() - SHOWN));
    }
    parts.join(", ")
}

/// Every effect in the cauldron, the native systems that read it and whether a
/// downstream model in this build reads it too.
fn coverage_card(outcome: Option<&Outcome>, muted: gpui::Hsla) -> AnyElement {
    let Some(outcome) = outcome else {
        return div().into_any_element();
    };
    let coverage = &outcome.coverage;
    let description = format!(
        "{} of {} effects have a downstream model in this build.",
        coverage.modelled(),
        coverage.total()
    );
    let table = coverage.effects.iter().fold(
        Table::new(["Effect", "Bubbles", "Native consumers", "Downstream model"]),
        |table, entry| {
            let models = if entry.modelled() {
                entry
                    .metrics
                    .iter()
                    .map(|id| {
                        outcome
                            .metrics()
                            .iter()
                            .find(|metric| metric.id == *id)
                            .map(|metric| metric.name.clone())
                            .unwrap_or_else(|| id.clone())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            let spoken = if entry.modelled() {
                format!("Modelled · {models}")
            } else {
                "Missing".to_string()
            };
            table.row([
                div()
                    .id(SharedString::from(format!(
                        "prisma-coverage-{}",
                        entry.effect
                    )))
                    .role(Role::Label)
                    .text_size(px(13.0))
                    .child(entry.effect.clone())
                    .into_any_element(),
                div()
                    .text_size(px(13.0))
                    .child(entry.bubbles.join(", "))
                    .into_any_element(),
                div()
                    .id(SharedString::from(format!(
                        "prisma-coverage-consumers-{}",
                        entry.effect
                    )))
                    .role(Role::Label)
                    .aria_label(consumer_summary(&entry.consumers))
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(consumer_summary(&entry.consumers))
                    .into_any_element(),
                div()
                    .id(SharedString::from(format!(
                        "prisma-coverage-model-{}",
                        entry.effect
                    )))
                    .role(Role::Label)
                    .aria_label(spoken)
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(if entry.modelled() {
                        Badge::new("Modelled").tone(Tone::Success)
                    } else {
                        Badge::new("Missing").tone(Tone::Warning)
                    })
                    .children((!models.is_empty()).then(|| Caption::new(models).text_color(muted)))
                    .into_any_element(),
            ])
        },
    );
    Card::new()
        .header(
            CardHeader::new("Coverage").description(description).action(
                div()
                    .id("prisma-coverage-count")
                    .role(Role::Label)
                    .aria_label(format!("{} missing", coverage.missing()))
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(format!("{} missing", coverage.missing())),
            ),
        )
        .child(
            div().flex().flex_col().gap_2().p_3().child(
                ScrollArea::new("prisma-coverage-scroll")
                    .vertical()
                    .max_h(px(260.0))
                    .child(div().min_w_0().child(table)),
            ),
        )
        .child(
            div().px_3().pb_3().child(
                Caption::new(
                    "Native consumers are the game scripts upstream's audit traced for each effect; \
                     they do not mean this build computes them. A missing effect has no damage, \
                     stats, skills or account-total outcome yet: its bubble bonus is only an \
                     intermediate.",
                )
                .text_color(muted),
            ),
        )
        .into_any_element()
}
