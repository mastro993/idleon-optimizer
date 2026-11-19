//! The Prisma Calculator: the plan, the streamed impact search and its coverage.
//!
//! The search runs off the UI thread through the shared task runner, so it
//! reports progress and can be cancelled; cancelling keeps the result already
//! on screen, and changing the save, character, cauldron or AFK hours marks it
//! stale.
//!
//! Each row compares the saved scope without this bubble's Prisma with the
//! scope with it, so the columns are per-metric deltas: the bubble's own bonus
//! (an intermediate) and every downstream outcome the build models, today the
//! Statue EXP retention the STARTUE EXP bubble buys. The coverage table names
//! every effect, the native game systems that consume it and whether this build
//! computes a downstream metric for it.
//!
//! The section draws with GPUI Kit: its selects and fields are built once in
//! [`State::new`], and the table rows are formatted in [`rows`] when the run,
//! the metric, the filter or the width changes, never per frame.

mod actions;
mod rows;
mod table;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Pixels, Role,
    SharedString, Styled, Subscription, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{
    Button, ButtonVariants as _, Toggle, ToggleGroup, ToggleVariants as _,
};
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use idleon_prisma::{self as prisma, Cauldron, Plan};
use idleon_ui::run_chrome;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, Choice, ChoiceSelect, TEXT_BODY, Tone, number_state,
    number_value, search_input, show_number,
};
use rows::{CoverageRow, Derived, Row};
use table::{Col, table};

/// The AFK claim range the calculator accepts, in hours.
pub(crate) const MIN_HOURS: f64 = 0.034;
pub(crate) const MAX_HOURS: f64 = 55_555.0;

/// The section's own state: the run inputs, the last plan and the controls
/// that show them.
pub(crate) struct State {
    pub(crate) character: usize,
    pub(crate) cauldron: Cauldron,
    pub(crate) afk_hours: f64,
    /// The plan the save, character and cauldron resolve to.
    pub(crate) plan: Option<Plan>,
    /// Why the plan could not be built, when it could not.
    pub(crate) plan_error: Option<String>,
    /// The metric the rows rank by; empty is the bubble order.
    pub(crate) metric: Option<String>,
    pub(crate) search: Entity<InputState>,
    /// Bumped each time a run stores its result, so the rows rebuild.
    pub(crate) stored: u64,
    character_select: ChoiceSelect<usize>,
    metric_select: ChoiceSelect<Option<String>>,
    hours: Entity<InputState>,
    derived: Derived,
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let search = crate::ui::input_state("Name or changed outcome", window, cx);
        let character_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let metric_select = ChoiceSelect::new(Vec::new(), Some(&None), window, cx);
        let hours = number_state(MIN_HOURS, MAX_HOURS, 1.0, window, cx);
        // Edits flow control -> section value, through the section's setters.
        let subscriptions = vec![
            cx.subscribe(
                character_select.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    let SelectEvent::Confirm(Some(character)) = event else {
                        return;
                    };
                    state.set_prisma_character(*character, cx);
                },
            ),
            cx.subscribe(
                metric_select.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Option<String>>>>, cx| {
                    let SelectEvent::Confirm(Some(metric)) = event else {
                        return;
                    };
                    state.set_prisma_metric(metric.as_deref().unwrap_or_default(), cx);
                },
            ),
            cx.subscribe(&hours, |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(hours) = number_value(field.read(cx))
                {
                    state.set_prisma_hours(hours, cx);
                }
            }),
        ];
        Self {
            character: 0,
            cauldron: Cauldron::default(),
            afk_hours: 1.0,
            plan: None,
            plan_error: None,
            metric: None,
            search,
            stored: 0,
            character_select,
            metric_select,
            hours,
            derived: Derived::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Rebuild the plan from the save, keeping the character when it still exists.
    pub(crate) fn adopt(&mut self, save: Option<&idleon_save::Save>) {
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
    pub(crate) fn query(&self, cx: &App) -> SharedString {
        self.search.read(cx).value()
    }
}

/// The inputs a run captures, so a later edit marks its result stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunInputs {
    pub(crate) save_revision: u64,
    pub(crate) character: usize,
    pub(crate) cauldron: usize,
    /// The AFK claim in whole seconds; zero when the value is not usable.
    pub(crate) afk_seconds: u64,
}

/// Whether the AFK claim is inside the accepted range.
pub(crate) fn valid_hours(hours: f64) -> bool {
    hours.is_finite() && (MIN_HOURS..=MAX_HOURS).contains(&hours)
}

/// The Prisma Calculator page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let compact = Breakpoint::of(window, cx).is_compact();
    let query = state.prisma.query(cx);
    let run = state.prisma_run.clone();
    let prisma = &mut state.prisma;
    prisma.derived.sync(
        run.read(cx).result(),
        prisma.stored,
        prisma.metric.as_deref(),
        &query,
        compact,
    );
    show_controls(prisma, window, cx);

    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let inputs = state.prisma_inputs();
    let entity = cx.entity();
    let state: &AppState = state;
    let cx: &App = cx;
    let run = run.read(cx);
    let has_result = run.result().is_some();

    let mut body = div()
        .id("prisma-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(toolbar(state, run.running(), &entity));

    if run.running() {
        if let Some(progress) = run.progress().cloned() {
            let entity = entity.clone();
            body = body.child(run_chrome::progress_card(
                "prisma",
                "Comparing bubbles",
                "Runs off the UI thread. Cancelling keeps the result this run replaced.",
                progress,
                move |_window, cx| {
                    entity.update(cx, |state, cx| state.cancel_prisma(cx));
                },
            ));
        }
    } else if run.cancelled() {
        body = body.child(run_chrome::cancelled_banner("prisma-cancelled", has_result));
    }

    let stale = run.is_stale(&inputs);
    if stale {
        body = body.child(run_chrome::stale_banner("prisma-stale"));
    }

    match (&state.prisma.plan, &state.prisma.plan_error) {
        (Some(plan), _) => {
            body = body
                .child(summary_card(state, plan, has_result))
                .child(results_card(
                    state,
                    plan,
                    has_result,
                    stale,
                    table_height,
                    cx,
                ))
                .child(coverage_card(state, has_result, cx));
        }
        (None, Some(error)) => {
            body = body.child(
                div().id("prisma-refused").child(
                    Callout::new(Tone::Warning, "The Prisma calculator cannot read this save")
                        .child(Caption::new(error.clone())),
                ),
            );
        }
        (None, None) => {
            body = body.child(Card::new().header(CardHeader::new("Bubble impacts")).child(
                div().id("prisma-empty").child(empty(
                    "Import a save to compare Prisma bubbles",
                    "The calculator reads each cauldron's bubble levels and the Prisma \
                         ownership list from the save, then compares every eligible bubble.",
                    Some(import_button(entity)),
                    cx,
                )),
            ));
        }
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("prisma-scroll")
        .into_any_element()
}

/// Values flow section value -> control: `show`/`show_number` write only
/// when the control shows something else, and never emit an edit.
fn show_controls(prisma: &State, window: &mut Window, cx: &mut App) {
    let characters = prisma
        .plan
        .as_ref()
        .map(|plan| {
            plan.characters
                .iter()
                .map(|character| {
                    Choice::new(
                        character.index,
                        format!("{} — {}", character.name, character.class_name),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    prisma
        .character_select
        .show(characters, Some(&prisma.character), window, cx);
    prisma.metric_select.show(
        prisma.derived.metrics.clone(),
        Some(&prisma.metric),
        window,
        cx,
    );
    let precision = if prisma.afk_hours.fract() == 0.0 {
        0
    } else {
        3
    };
    show_number(&prisma.hours, prisma.afk_hours, precision, window, cx);
}

fn import_button(entity: Entity<AppState>) -> Button {
    Button::new("prisma-import")
        .label("Import a save")
        .on_click(move |_event, window, cx| {
            entity.update(cx, |state, cx| state.open_import(window, cx));
        })
}

/// The kit empty state with a sparkle, or the icon `SearchX` for a filter.
fn empty(
    title: &'static str,
    description: &'static str,
    action: Option<Button>,
    cx: &App,
) -> Empty {
    let muted = cx.theme().muted_foreground;
    let icon = if action.is_none() && title.starts_with("No bubbles") {
        IconName::SearchX
    } else {
        IconName::Sparkles
    };
    Empty::new()
        .header(
            EmptyHeader::new()
                .media(EmptyMedia::new().child(Icon::new(icon).text_color(muted)))
                .title(EmptyTitle::new().child(title))
                .description(EmptyDescription::new().child(description)),
        )
        .when_some(action, |empty, action| {
            empty.content(EmptyContent::new().child(action))
        })
}

/// A small outlined tag.
fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// The cauldron switch. Segment `n` keeps Ely's bare `segment-<n>` id, which
/// the harness clicks; a click on the checked segment keeps it.
fn cauldron_switch(current: Cauldron, entity: &Entity<AppState>) -> ToggleGroup {
    let entity = entity.clone();
    ToggleGroup::new("prisma-cauldron")
        .segmented()
        .single()
        .outline()
        .small()
        .children(
            Cauldron::ALL
                .into_iter()
                .enumerate()
                .map(|(index, option)| {
                    Toggle::new(("segment", index))
                        .label(option.label())
                        .checked(option == current)
                }),
        )
        .on_click(move |checked, _window, cx| {
            if let Some(index) = checked.iter().position(|checked| *checked) {
                let next = Cauldron::ALL[index];
                entity.update(cx, |state, cx| state.set_prisma_cauldron(next, cx));
            }
        })
}

/// Character, AFK hours, cauldron tabs and the run action.
fn toolbar(state: &AppState, running: bool, entity: &Entity<AppState>) -> AnyElement {
    let prisma = &state.prisma;
    let has_characters = prisma
        .plan
        .as_ref()
        .is_some_and(|plan| !plan.characters.is_empty());
    let hours_ok = valid_hours(prisma.afk_hours);

    let character = div().flex_none().w(px(240.0)).child(
        Select::new(prisma.character_select.state())
            .id("prisma-character")
            .accessibility_label("Character")
            .placeholder("No characters")
            .small()
            .disabled(!has_characters),
    );
    let hours = crate::ui::number_field("prisma-afk-hours", &prisma.hours, "AFK claim hours");

    let run_entity = entity.clone();
    let run = Button::new("prisma-run")
        .label("Recalculate")
        .primary()
        .small()
        .disabled(!has_characters || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_prisma(cx));
        });
    let cancel_entity = entity.clone();
    let cancel = Button::new("prisma-cancel")
        .label("Cancel")
        .small()
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_prisma(cx));
        });

    div()
        .id("prisma-toolbar")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(field("Character").child(character))
                .child(field("AFK claim hours").child(hours))
                .child(field("Cauldron").child(cauldron_switch(prisma.cauldron, entity)))
                .child(div().flex().items_center().gap_2().child(run).child(cancel)),
        )
        .when(!hours_ok, |toolbar| {
            toolbar.child(
                div()
                    .id("prisma-hours-warning")
                    .role(Role::Label)
                    .child(Caption::new(format!(
                        "Enter AFK hours between {MIN_HOURS} and {MAX_HOURS}."
                    ))),
            )
        })
        .into_any_element()
}

fn field(label: &'static str) -> gpui::Div {
    div().flex().flex_col().gap_1().child(Caption::new(label))
}

/// The saved multiplier, the fragments and the saved map, plus the last run's count.
fn summary_card(state: &AppState, plan: &Plan, has_result: bool) -> AnyElement {
    let counts = &state.prisma.derived.counts;
    let multiplier = format!("{:.4}", plan.multiplier);
    let fragments = plan
        .fragments
        .map(idleon_ui::format::number)
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
    let summary = if has_result {
        format!(
            "{} · Prisma ×{multiplier} · {fragments} fragments · saved map {map} · compared {} bubbles{affordable}",
            plan.cauldron_label(),
            counts.compared
        )
    } else {
        format!(
            "{} · Prisma ×{multiplier} · {fragments} fragments · saved map {map} · {} of {} bubbles comparable{affordable}",
            plan.cauldron_label(),
            plan.comparable(),
            plan.rows.len()
        )
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
                .text_size(TEXT_BODY)
                .font_weight(FontWeight::MEDIUM)
                .child(summary),
        )
        .child(Caption::new(if has_result {
            format!(
                "{} of {} effects have a downstream model in this build. The bubble bonus \
                 is always an intermediate; Coverage names the rest and the native systems \
                 that read them.",
                counts.modelled, counts.total
            )
        } else {
            "Each row measures the bubble bonus (an intermediate) and any \
             downstream outcome this build models. Coverage names the rest."
                .to_string()
        }))
        .into_any_element()
}

/// The filter controls and the ranked rows.
fn results_card(
    state: &AppState,
    plan: &Plan,
    has_result: bool,
    stale: bool,
    table_height: Pixels,
    cx: &App,
) -> AnyElement {
    let prisma = &state.prisma;
    let rows = &prisma.derived.rows;
    let counts = &prisma.derived.counts;
    let metric = div().flex_none().w(px(240.0)).child(
        Select::new(prisma.metric_select.state())
            .id("prisma-metric")
            .accessibility_label("Rank by")
            .small(),
    );
    let search_focus = prisma.search.clone();
    let search = div()
        .id("prisma-search")
        .role(Role::Search)
        .aria_label("Filter bubbles")
        .w(px(280.0))
        .on_click(move |_event, window, cx| {
            search_focus.update(cx, |field, cx| field.focus(window, cx));
        })
        .child(
            search_input(&prisma.search)
                .aria_label("Filter bubbles")
                .small(),
        );

    let body = if !has_result {
        div()
            .id("prisma-no-run")
            .child(empty(
                "No comparison yet.",
                "Recalculate compares every eligible bubble in this cauldron: each run is the saved \
                 state with one bubble's Prisma flag toggled.",
                None,
                cx,
            ))
            .into_any_element()
    } else if rows.is_empty() {
        div()
            .id("prisma-no-matches")
            .child(empty(
                "No bubbles match your filter.",
                "Try a bubble name, an effect key such as TotalSTR, or a changed outcome.",
                None,
                cx,
            ))
            .into_any_element()
    } else {
        div()
            .h(table_height)
            .child(table(
                "prisma-table",
                &[
                    Col::Text("Bubble"),
                    Col::Number("Level"),
                    Col::Text("Status"),
                    Col::Number("Bubble bonus"),
                    Col::Text("Downstream change"),
                ],
                rows.iter().map(row_cells),
                cx,
            ))
            .overflow_y_scrollbar()
            .id("prisma-table-scroll")
            .into_any_element()
    };

    let description = if has_result {
        format!(
            "{} bubbles compared · {} with a downstream change · {} shown",
            counts.compared,
            counts.changed,
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
                    header.action(run_chrome::stale_badge("prisma-result-stale"))
                }),
        )
        .child(
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(field("Rank by").child(metric))
                .child(field("Filter bubbles").child(search)),
        )
        .child(body)
        .child(
            div()
                .id("prisma-status")
                .role(Role::Label)
                .child(Caption::new(format!(
                    "Every row compares without → with this bubble's Prisma, so a ready row \
                     previews the gain and an already-Prisma row measures the benefit it \
                     retains. Bubble bonuses are intermediates; only the downstream change \
                     is an outcome. {} compared, {} moved a downstream metric.",
                    counts.compared, counts.changed
                ))),
        )
        .into_any_element()
}

/// One bubble's cells; each keeps its Ely `prisma-<cell>-<cauldron>-<index>` id.
fn row_cells(row: &Row) -> Vec<AnyElement> {
    let id = |cell: &str| SharedString::from(format!("prisma-{cell}-{}", row.key));
    vec![
        div()
            .id(id("row"))
            .role(Role::Label)
            .aria_label(row.spoken.clone())
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_size(TEXT_BODY)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(row.name.clone()),
            )
            .child(Caption::new(row.effect.clone()))
            .into_any_element(),
        div()
            .id(id("level"))
            .role(Role::Label)
            .child(row.level.clone())
            .into_any_element(),
        div()
            .id(id("status"))
            .role(Role::Label)
            .child(tag(row.status, row.status_tone))
            .into_any_element(),
        div()
            .id(id("bonus"))
            .role(Role::Label)
            .aria_label(row.bonus_spoken.clone())
            .flex()
            .flex_col()
            .items_end()
            .gap_0p5()
            .child(row.bonus.clone())
            .children(row.bonus_delta.clone().map(Caption::new))
            .into_any_element(),
        div()
            .id(id("change"))
            .role(Role::Label)
            .aria_label(row.change_spoken.clone())
            .flex()
            .flex_col()
            .items_start()
            .gap_0p5()
            .child(tag(row.change.clone(), row.change_tone))
            .children(row.change_name.clone().map(Caption::new))
            .into_any_element(),
    ]
}

/// Every effect in the cauldron, the native systems that read it and whether a
/// downstream model in this build reads it too.
fn coverage_card(state: &AppState, has_result: bool, cx: &App) -> AnyElement {
    if !has_result {
        return div().into_any_element();
    }
    let derived = &state.prisma.derived;
    let counts = &derived.counts;
    Card::new()
        .header(
            CardHeader::new("Coverage")
                .description(format!(
                    "{} of {} effects have a downstream model in this build.",
                    counts.modelled, counts.total
                ))
                .action(
                    div()
                        .id("prisma-coverage-count")
                        .role(Role::Label)
                        .aria_label(format!("{} missing", counts.missing))
                        .child(Caption::new(format!("{} missing", counts.missing))),
                ),
        )
        .child(
            div()
                .max_h(px(260.0))
                .child(table(
                    "prisma-coverage-table",
                    &[
                        Col::Text("Effect"),
                        Col::Text("Bubbles"),
                        Col::Text("Native consumers"),
                        Col::Text("Downstream model"),
                    ],
                    derived.coverage.iter().map(coverage_cells),
                    cx,
                ))
                .overflow_y_scrollbar()
                .id("prisma-coverage-scroll"),
        )
        .child(Caption::new(
            "Native consumers are the game scripts that read each effect; \
             they do not mean this build computes them. A missing effect has no damage, \
             stats, skills or account-total outcome yet: its bubble bonus is only an \
             intermediate.",
        ))
        .into_any_element()
}

fn coverage_cells(entry: &CoverageRow) -> Vec<AnyElement> {
    let id = |cell: &str| SharedString::from(format!("prisma-coverage{cell}-{}", entry.effect));
    vec![
        div()
            .id(id(""))
            .role(Role::Label)
            .child(entry.effect.clone())
            .into_any_element(),
        div().child(entry.bubbles.clone()).into_any_element(),
        div()
            .id(id("-consumers"))
            .role(Role::Label)
            .aria_label(entry.consumers.clone())
            .child(Caption::new(entry.consumers.clone()))
            .into_any_element(),
        div()
            .id(id("-model"))
            .role(Role::Label)
            .aria_label(entry.spoken.clone())
            .flex()
            .flex_col()
            .items_start()
            .gap_0p5()
            .child(if entry.modelled {
                tag("Modelled", Tone::Success)
            } else {
                tag("Missing", Tone::Warning)
            })
            .children((!entry.models.is_empty()).then(|| Caption::new(entry.models.clone())))
            .into_any_element(),
    ]
}
