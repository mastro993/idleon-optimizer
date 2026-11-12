//! The Statue Calculator section: a character's saved statue levels, the EXP
//! their next level needs, and the statues and time a target level costs.
//!
//! Ported from `statue-calc.html` and `js/stats/systems/common/statue-leveling.js`.
//! ST-01 covers the table, search and sort; ST-02 the STARTUE EXP bubble, the
//! deposit estimate and the time estimate from a raw statues/hour rate; ST-03
//! the batch set/next-level actions with their undo step and the farming
//! breakdown behind one statue's row.

use crate::state::AppState;
use crate::ui::formula::FormulaTree;
use crate::ui::{format, layout};
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::{Banner, EmptyState};
use ely_gpui_component::forms::{Choice, Input, InputEvent, SearchInput, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Subscription, Window, div, px,
};
use idleon_core::statue::{
    self, Estimate, Leveling, NextLevel, Plan, Progress, RateError, StatueInfo, StatueSave,
    TargetCost, Unavailable,
};
use std::cmp::Ordering;

/// How the table is ordered when it is not in game order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Game,
    /// Lowest saved level first; statues without a row stay last.
    Level,
    /// Least EXP left to the next level first.
    Remaining,
}

impl Sort {
    const ALL: [Sort; 3] = [Sort::Game, Sort::Level, Sort::Remaining];

    fn id(self) -> &'static str {
        match self {
            Sort::Game => "game",
            Sort::Level => "level",
            Sort::Remaining => "remaining",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Sort::Game => "Game order",
            Sort::Level => "Lowest level first",
            Sort::Remaining => "Least EXP to next level",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sort| sort.id() == id)
    }
}

/// The deposit estimate's id, label and id lookup, matching the upstream
/// selector's two options.
fn estimate_id(estimate: Estimate) -> &'static str {
    match estimate {
        Estimate::Guaranteed => "guaranteed",
        Estimate::Average => "average",
    }
}

fn estimate_from_id(id: &str) -> Option<Estimate> {
    Estimate::ALL
        .into_iter()
        .find(|estimate| estimate_id(*estimate) == id)
}

/// The section's own state: the imported statue rows, the account multipliers,
/// the table controls and the plan the batch actions rewrite.
pub struct State {
    pub save: StatueSave,
    pub character: usize,
    pub sort: Sort,
    pub estimate: Estimate,
    /// The entered targets and raw rates, with the batch undo step.
    pub plan: Plan,
    /// The statue the farming breakdown describes.
    pub breakdown_statue: usize,
    pub breakdown: Entity<FormulaTree>,
    pub search: Entity<TextInput>,
    /// One raw statues/hour field per statue, mirroring [`Plan::rates`].
    pub rate_inputs: Vec<Entity<TextInput>>,
    /// The Set all targets entry, read when its button is pressed.
    pub batch_input: Entity<TextInput>,
    /// Why the last batch entry was rejected, or `None` when it applied.
    pub batch_error: Option<String>,
    /// What the last batch action did, shown under its buttons.
    pub batch_status: Option<String>,
    _rate_subscriptions: Vec<Subscription>,
}

impl State {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Name or effect")
                .label("Find a statue")
        });
        let batch_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Level")
                .label("Set all targets")
        });
        let breakdown = cx.new(|cx| FormulaTree::new("statue-breakdown", window, cx));
        let rate_inputs: Vec<Entity<TextInput>> = (0..statue::count())
            .map(|_| {
                cx.new(|cx| {
                    TextInput::new(window, cx)
                        .placeholder("e.g. 1M")
                        .label("Raw statues per hour")
                })
            })
            .collect();
        let rate_subscriptions = rate_inputs
            .iter()
            .enumerate()
            .map(|(index, input)| {
                cx.subscribe(input, move |state, input, event: &InputEvent, cx| {
                    if *event == InputEvent::Changed {
                        let text = input.read(cx).text().trim().to_string();
                        state.statue.plan.set_rate(index, text);
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            save: StatueSave::default(),
            character: 0,
            sort: Sort::Game,
            estimate: Estimate::Guaranteed,
            plan: Plan::new(),
            breakdown_statue: 0,
            breakdown,
            search,
            rate_inputs,
            batch_input,
            batch_error: None,
            batch_status: None,
            _rate_subscriptions: rate_subscriptions,
        }
    }

    /// Replace the saved rows after an import, keeping the selected character
    /// when the new save still carries it and clearing the plan inputs.
    pub fn load(&mut self, save: StatueSave, cx: &mut Context<AppState>) {
        self.save = save;
        if !self.save.has_character(self.character) {
            self.character = self.save.first_character().unwrap_or(0);
        }
        if !self.save.has_character(self.breakdown_statue) && self.save.first_character().is_some()
        {
            self.breakdown_statue = self.save.first_character().unwrap_or(0);
        }
        self.clear_inputs(cx);
    }

    /// Change the table's character and clear its plan inputs, as the upstream
    /// character switch does.
    pub fn select_character(&mut self, character: usize, cx: &mut Context<AppState>) {
        self.character = character;
        self.clear_inputs(cx);
    }

    fn clear_inputs(&mut self, cx: &mut Context<AppState>) {
        self.plan.clear();
        self.batch_error = None;
        self.batch_status = None;
        self.batch_input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.sync_rates(cx);
    }

    /// Replace the plan inputs from a project config, keeping one entry per
    /// statue and syncing the fields on screen.
    pub fn set_plan(
        &mut self,
        targets: Vec<Option<u32>>,
        rates: Vec<String>,
        cx: &mut Context<AppState>,
    ) {
        self.plan.reset(targets, rates);
        self.batch_error = None;
        self.batch_status = None;
        self.sync_rates(cx);
    }

    fn sync_rates(&mut self, cx: &mut Context<AppState>) {
        for (index, input) in self.rate_inputs.iter().enumerate() {
            let text = self.plan.rate(index).to_string();
            input.update(cx, |input, cx| input.set_text(text, cx));
        }
    }

    /// The target level for a statue: the entered target, else the saved level
    /// plus one.
    pub fn target(&self, statue: usize) -> Option<u32> {
        self.plan.target(statue).or_else(|| {
            self.save
                .progress(self.character, statue)
                .map(|progress| progress.level + 1)
        })
    }

    pub fn set_target(&mut self, statue: usize, target: u32) {
        self.plan.set_target(statue, target);
    }

    /// One statue's raw statues/hour entry, trimmed.
    pub fn rate_text(&self, statue: usize) -> String {
        self.plan.rate(statue).to_string()
    }

    /// The parsed rate the row and the breakdown farm at.
    pub fn rate(&self, statue: usize) -> Result<f64, RateError> {
        statue::parse_rate(self.plan.rate(statue))
    }

    /// The statue the breakdown describes, kept inside the game's table.
    pub fn breakdown_statue(&self) -> usize {
        self.breakdown_statue.min(statue::count().saturating_sub(1))
    }

    pub fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().to_string()
    }
}

/// One table row: static data plus what this save and plan say about it.
#[derive(Clone)]
struct Row {
    index: usize,
    info: &'static StatueInfo,
    progress: Option<Progress>,
    next: Result<NextLevel, Unavailable>,
    target: Option<u32>,
    cost: Result<TargetCost, Unavailable>,
    rate_text: String,
    rate: Result<f64, RateError>,
    /// The statues the target needs under the selected estimate.
    statues: Option<u64>,
    hours: f64,
}

impl Row {
    fn level_key(&self) -> u32 {
        self.progress
            .map(|progress| progress.level)
            .unwrap_or(u32::MAX)
    }

    fn remaining_key(&self) -> f64 {
        self.next
            .as_ref()
            .map(|next| next.remaining)
            .unwrap_or(f64::INFINITY)
    }

    fn availability(&self, no_save: bool) -> (String, Tone) {
        if no_save {
            return ("No save".into(), Tone::Neutral);
        }
        match self.next {
            Ok(_) => ("Available".into(), Tone::Success),
            Err(error) => (
                error.label(self.index),
                match error {
                    Unavailable::OutOfRange | Unavailable::TargetOutOfRange => Tone::Danger,
                    _ => Tone::Warning,
                },
            ),
        }
    }
}

fn rows(state: &State) -> Vec<Row> {
    let leveling = state.save.leveling();
    (0..statue::count())
        .filter_map(|index| {
            let info = statue::info(index)?;
            let progress = state.save.progress(state.character, index);
            let target = state.target(index);
            let cost = target
                .map(|target| {
                    statue::target_cost(&state.save, leveling, state.character, index, target)
                })
                .unwrap_or(Err(Unavailable::NoRow(index)));
            let rate_text = state.rate_text(index);
            let rate = state.rate(index);
            let statues = cost.as_ref().ok().map(|cost| state.estimate.statues(cost));
            let hours = statues
                .map(|statues| statue::hours(statues, rate.unwrap_or(0.0)))
                .unwrap_or(f64::INFINITY);
            Some(Row {
                index,
                info,
                progress,
                next: statue::next_level(&state.save, state.character, index),
                target,
                cost,
                rate_text,
                rate,
                statues,
                hours,
            })
        })
        .collect()
}

fn matches(info: &StatueInfo, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || info.name.to_lowercase().contains(&query)
        || info.effect.to_lowercase().contains(&query)
}

fn sort_rows(rows: &mut [Row], sort: Sort) {
    match sort {
        Sort::Game => {}
        Sort::Level => rows.sort_by_key(|row| (row.level_key(), row.index)),
        Sort::Remaining => rows.sort_by(|a, b| {
            a.remaining_key()
                .partial_cmp(&b.remaining_key())
                .unwrap_or(Ordering::Equal)
                .then(a.index.cmp(&b.index))
        }),
    }
}

/// The Statue Calculator page.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let query = state.statue.query(cx);
    let save_empty = state.statue.save.is_empty();
    let all_rows = rows(&state.statue);
    let summary_cards = summary(&state.statue, &all_rows, cx);
    let mut rows = all_rows.clone();
    let total = all_rows.len();
    rows.retain(|row| matches(row.info, &query));
    sort_rows(&mut rows, state.statue.sort);
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let flair_missing = state.save_loaded && !state.statue.save.leveling().flair_available;
    let breakdown_statue = state.statue.breakdown_statue();
    let breakdown_tree = statue::breakdown_tree(
        &state.statue.save,
        state.statue.character,
        breakdown_statue,
        state.statue.target(breakdown_statue),
        state.statue.estimate,
        state.statue.rate(breakdown_statue).unwrap_or(0.0),
    );
    state
        .statue
        .breakdown
        .update(cx, |tree, cx| tree.set_root(breakdown_tree, cx));

    let page = div()
        .id("statue-pane")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .when(!state.save_loaded, |page| {
            page.child(div().id("statue-no-save").child(Caption::new(
                "Import a decoded save to read its per-character statue levels.",
            )))
        })
        .when(state.save_loaded && save_empty, |page| {
            page.child(
                Banner::new(
                    "statue-no-rows",
                    Severity::Warning,
                    "This save carries no StatueLevels_<n> rows, so no statue level is available.",
                )
                .action("Import another save", {
                    let entity = cx.entity();
                    move |window, cx| {
                        entity.update(cx, |state, cx| state.open_import(window, cx));
                    }
                }),
            )
        })
        .when(flair_missing, |page| {
            page.child(Banner::new(
                "statue-no-flair",
                Severity::Warning,
                "Royal Guardian data is missing: saved Flair levels are unknown, so estimates assume no Flair bonus.",
            ))
        })
        .child(toolbar(state, cx))
        .when(state.save_loaded && !save_empty, |page| {
            page.child(summary_cards)
        })
        .child(statues_card(
            state,
            cx,
            &all_rows,
            &rows,
            (rows.len(), total),
            table_height,
        ))
        .child(farming_card(state, breakdown_statue, cx));

    div()
        .id("statue-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("statue-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(page),
        )
        .into_any_element()
}

/// Character, estimate, search and sort controls.
fn toolbar(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let choices: Vec<Choice> = state
        .statue
        .save
        .character_indices()
        .map(|index| Choice::new(index.to_string(), state.statue.save.name(index)))
        .collect();
    let has_characters = !choices.is_empty();
    let entity = cx.entity();
    let character = entity.clone();
    let estimate = entity.clone();
    let sort = entity.clone();
    let search_focus = state.statue.search.read(cx).focus().clone();

    div()
        .id("statue-toolbar")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            field("Character", muted).child(
                Select::new("statue-character", choices)
                    .label("Character")
                    .placeholder("No characters")
                    .selected(state.statue.character.to_string())
                    .disabled(!has_characters)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            character.update(cx, |state, cx| state.set_statue_character(index, cx));
                        }
                    }),
            ),
        )
        .child(
            field("Deposit estimate", muted).child(
                Select::new(
                    "statue-estimate",
                    Estimate::ALL
                        .map(|estimate| Choice::new(estimate_id(estimate), estimate.label())),
                )
                .label("Deposit estimate")
                .selected(estimate_id(state.statue.estimate))
                .on_change(move |value, _window, cx| {
                    if let Some(next) = estimate_from_id(value.as_ref()) {
                        estimate.update(cx, |state, cx| state.set_statue_estimate(next, cx));
                    }
                }),
            ),
        )
        .child(
            field("Find a statue", muted).child(
                div()
                    .id("statue-search")
                    .role(Role::Search)
                    .aria_label("Find a statue")
                    .w(px(280.0))
                    .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                    .child(SearchInput::new(
                        "statue-search-field",
                        &state.statue.search,
                    )),
            ),
        )
        .child(
            field("Sort", muted).child(
                Select::new(
                    "statue-sort",
                    Sort::ALL.map(|sort| Choice::new(sort.id(), sort.label())),
                )
                .label("Sort statues")
                .selected(state.statue.sort.id())
                .on_change(move |value, _window, cx| {
                    if let Some(next) = Sort::from_id(value.as_ref()) {
                        sort.update(cx, |state, cx| state.set_statue_sort(next, cx));
                    }
                }),
            ),
        )
        .into_any_element()
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

/// The four upstream summary cards: statues needed, retention, the tier
/// multiplier and the parallel completion time.
fn summary(state: &State, rows: &[Row], cx: &mut Context<AppState>) -> AnyElement {
    let leveling: &Leveling = state.save.leveling();
    let muted = cx.theme().colors.fg_muted;
    let missing = rows.iter().filter(|row| row.cost.is_err()).count();
    let needed: u64 = rows.iter().filter_map(|row| row.statues).sum();
    let unmeasured = rows
        .iter()
        .filter(|row| {
            row.statues.is_some_and(|statues| statues > 0)
                && !matches!(row.rate, Ok(rate) if rate > 0.0)
        })
        .count();
    let parallel = if missing > 0 {
        "Unavailable".to_string()
    } else if unmeasured > 0 {
        "Add rates".to_string()
    } else {
        let longest = rows.iter().map(|row| row.hours).fold(0.0_f64, f64::max);
        format::statue_time(longest)
    };
    let needed_caption = format!(
        "{} · {}",
        if missing > 0 {
            format!("Partial: {missing} missing statue rows")
        } else {
            "All types combined".to_string()
        },
        match state.estimate {
            Estimate::Guaranteed => "guaranteed EXP",
            Estimate::Average => "average estimate",
        }
    );

    div()
        .id("statue-summary")
        .role(Role::Label)
        .aria_label(format!(
            "Statues needed {needed} · retention {:.1}% · tier ×{} · parallel completion {parallel}",
            leveling.retention_pct,
            format::fmt_val(leveling.tier_multiplier)
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(text_kpi(
            "kpi-statues-needed",
            "Statues needed",
            &format::fmt_val(needed as f64),
            &needed_caption,
            muted,
        ))
        .child(text_kpi(
            "kpi-retention",
            "EXP retention",
            &format!("{:.1}%", leveling.retention_pct),
            &format!(
                "STARTUE {}% · retention capped at 50%",
                format::fmt_val(leveling.bubble_pct)
            ),
            muted,
        ))
        .child(text_kpi(
            "kpi-tier",
            "Onyx / Zenith EXP",
            &format!("{}x", format::fmt_val(leveling.tier_multiplier)),
            &format!("{} Onyx+ and {} Zenith statues", leveling.onyx, leveling.zenith),
            muted,
        ))
        .child(text_kpi(
            "kpi-parallel",
            "Parallel completion",
            &parallel,
            &if unmeasured > 0 {
                format!("{unmeasured} target rates missing")
            } else {
                "All target rates maintained together".to_string()
            },
            muted,
        ))
        .into_any_element()
}

fn text_kpi(
    id: &'static str,
    label: &'static str,
    value: &str,
    caption: &str,
    muted: gpui::Hsla,
) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{label} {value}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .child(Caption::new(label).text_color(muted))
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(value.to_string()),
                )
                .child(Caption::new(caption.to_string()).text_color(muted)),
        )
        .into_any_element()
}

fn statues_card(
    state: &mut AppState,
    cx: &mut Context<AppState>,
    all: &[Row],
    rows: &[Row],
    (shown, total): (usize, usize),
    table_height: gpui::Pixels,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let character = state.statue.save.name(state.statue.character);
    let save_empty = state.statue.save.is_empty();
    let missing = if save_empty {
        0
    } else {
        state.statue.save.missing_rows(state.statue.character)
    };
    let description = if save_empty {
        "No saved statue rows; import a save to fill the table.".to_string()
    } else if missing > 0 {
        format!(
            "{character} · {} of {total} statues available; {missing} rows missing",
            total - missing
        )
    } else {
        format!("{character} · all {total} statues available")
    };
    let entity = cx.entity();
    let estimate = state.statue.estimate;
    let flair_available = state.statue.save.leveling().flair_available;
    let rate_error = all.iter().find_map(|row| row.rate.as_ref().err().copied());
    let rates_entered = all.iter().any(|row| !row.rate_text.is_empty());
    let rate_source = if rates_entered {
        format!("Rate source: manual entries for {character}.")
    } else {
        format!("Rate source: none yet for {character}.")
    };

    let mut table = Table::new([
        "Statue / effect",
        "Level / progress",
        "Availability",
        "Target",
        "EXP per statue",
        "Statues needed",
        "Raw statues/hr",
        "Time",
    ]);
    for row in rows {
        let target = entity.clone();
        table = table.row([
            statue_cell(row),
            level_cell(row),
            availability_cell(row, !state.save_loaded),
            target_cell(row, target),
            exp_cell(row, estimate, flair_available),
            needed_cell(row),
            rate_cell(state, row),
            time_cell(row),
        ]);
    }

    let body = if rows.is_empty() {
        EmptyState::new(
            "statue-empty",
            IconName::SearchX,
            "No statues match your search.",
        )
        .body("Try a statue name or effect, such as POWER or MOVE SPEED.")
        .into_any_element()
    } else {
        ScrollArea::new("statue-table-scroll")
            .vertical()
            .h(table_height)
            .child(div().min_w_0().child(table))
            .into_any_element()
    };

    Card::new()
        .header(
            CardHeader::new("Statues")
                .description(description)
                .action(
                    div()
                        .id("statue-count")
                        .role(Role::Label)
                        .aria_label(format!("{shown} of {total} statues shown"))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(format!("{shown} / {total} shown")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_3()
                .child(batch_row(state, cx))
                .child(batch_messages(state, cx))
                .child(body)
                .child(
                    div()
                        .id("statue-rate-source")
                        .role(Role::Label)
                        .aria_label(rate_source.clone())
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(rate_source),
                )
                .when_some(rate_error, |column, error| {
                    column.child(
                        div()
                            .id("statue-rate-error")
                            .role(Role::Label)
                            .aria_label(error.message())
                            .text_size(px(11.0))
                            .text_color(cx.theme().colors.danger)
                            .child(error.message()),
                    )
                })
                .child(
                    div()
                        .id("statue-note")
                        .role(Role::Label)
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(
                            "Requirements are the full EXP bar for the saved level; remaining EXP uses the saved progress. \
                             The STARTUE EXP bubble lowers the EXP a level consumes, while a full bar still triggers it. \
                             Deposits multiply the shared Onyx/Zenith bonus and each statue's saved Flair. \
                             Statues 10-19, 20-29 and 30-31 round with the saved level of statue 1, 2 and 3.",
                        ),
                ),
        )
        .into_any_element()
}

/// The upstream batch controls: Set all targets, Next level for all, and the
/// undo step the batch actions leave.
fn batch_row(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let disabled = !state.save_loaded || state.statue.save.is_empty();
    let input = state.statue.batch_input.clone();
    let focus = input.read(cx).focus().clone();
    let entity = cx.entity();
    let apply = entity.clone();
    let next = entity.clone();
    let undo = entity.clone();
    let undo_label = state.statue.plan.undo_label();

    div()
        .id("statue-batch")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_2()
        .child(
            div()
                .id("statue-set-all")
                .role(Role::Label)
                .aria_label("Set all targets")
                .w(px(120.0))
                .on_click(move |_event, window, cx| window.focus(&focus, cx))
                .child(Input::new(&input).size(ControlSize::Sm)),
        )
        .child(
            Button::new("statue-apply-all", "Set targets")
                .size(ControlSize::Sm)
                .disabled(disabled)
                .on_click(move |_event, _window, cx| {
                    apply.update(cx, |state, cx| state.set_statue_targets_all(cx));
                }),
        )
        .child(
            Button::new("statue-next-level", "Next level for all")
                .size(ControlSize::Sm)
                .disabled(disabled)
                .on_click(move |_event, _window, cx| {
                    next.update(cx, |state, cx| state.set_statue_next_level(cx));
                }),
        )
        .when_some(undo_label, |row, label| {
            row.child(
                Button::new("statue-undo-batch", format!("Undo {label}"))
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        undo.update(cx, |state, cx| state.undo_statue_batch(cx));
                    }),
            )
        })
        .child(Caption::new(
            "Set targets costs the level for every statue first; a level this save cannot plan changes nothing. \
             Rates are raw statues/hour, so K, M, B, T and scientific notation work.",
        )
        .text_color(muted))
        .into_any_element()
}

/// What the last batch action did, and why one was rejected.
fn batch_messages(state: &AppState, cx: &Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let danger = cx.theme().colors.danger;
    div()
        .id("statue-batch-messages")
        .flex()
        .flex_col()
        .gap_1()
        .when_some(state.statue.batch_status.clone(), |column, status| {
            column.child(
                div()
                    .id("statue-batch-status")
                    .role(Role::Label)
                    .aria_label(status.clone())
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(status),
            )
        })
        .when_some(state.statue.batch_error.clone(), |column, error| {
            column.child(
                div()
                    .id("statue-batch-error")
                    .role(Role::Label)
                    .aria_label(error.clone())
                    .text_size(px(11.0))
                    .text_color(danger)
                    .child(error),
            )
        })
        .into_any_element()
}

/// The farming breakdown behind one statue's row: the deposit terms upstream's
/// detail block names, plus the statues, rate and time the table shows.
fn farming_card(state: &AppState, selected: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let choices: Vec<Choice> = (0..statue::count())
        .filter_map(|index| {
            let info = statue::info(index)?;
            Some(Choice::new(index.to_string(), info.name.clone()))
        })
        .collect();
    Card::new()
        .header(
            CardHeader::new("Farming breakdown")
                .description("Where one statue's deposit EXP and farm time come from")
                .action(
                    Select::new("statue-breakdown-statue", choices)
                        .label("Break down statue")
                        .size(ControlSize::Sm)
                        .selected(selected.to_string())
                        .on_change(move |value, _window, cx| {
                            if let Ok(index) = value.parse::<usize>() {
                                entity
                                    .update(cx, |state, cx| state.set_statue_breakdown(index, cx));
                            }
                        }),
                ),
        )
        .child(
            div()
                .id("statue-farming")
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_3()
                .child(
                    div()
                        .id("statue-breakdown")
                        .h(px(300.0))
                        .min_h_0()
                        .child(state.statue.breakdown.clone()),
                ),
        )
        .into_any_element()
}

fn statue_cell(row: &Row) -> AnyElement {
    div()
        .id(SharedString::from(format!("statue-row-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{} · {}", row.info.name, row.info.effect))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(row.info.name.clone()),
        )
        .child(Caption::new(row.info.effect.clone()))
        .into_any_element()
}

fn level_cell(row: &Row) -> AnyElement {
    let Some(progress) = row.progress else {
        return unavailable_cell().into_any_element();
    };
    let Ok(next) = &row.next else {
        return div()
            .id(SharedString::from(format!("statue-level-{}", row.index)))
            .role(Role::Label)
            .aria_label(format!(
                "Level {} · requirement unavailable",
                progress.level
            ))
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("Lv {}", progress.level)),
            )
            .child(Caption::new("Requirement unavailable"))
            .into_any_element();
    };
    let required = next.required;
    let share = if required > 0.0 {
        (progress.exp / required).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    div()
        .id(SharedString::from(format!("statue-level-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!(
            "Level {} · {:.1}% · {} of {} EXP",
            progress.level,
            share * 100.0,
            format::rate(progress.exp),
            format::rate(required)
        ))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(format!("Lv {}", progress.level)),
        )
        .child(
            div()
                .w(px(96.0))
                .child(ProgressBar::new(("statue-progress", row.index), share)),
        )
        .child(Caption::new(format!(
            "{:.1}% · {} / {}",
            share * 100.0,
            format::rate(progress.exp),
            format::rate(required)
        )))
        .into_any_element()
}

fn target_cell(row: &Row, entity: Entity<AppState>) -> AnyElement {
    let Some(target) = row.target else {
        return unavailable_cell().into_any_element();
    };
    let index = row.index;
    div()
        .w(px(88.0))
        .child(
            ely_gpui_component::forms::NumberInput::new(
                ("statue-target-field", row.index),
                f64::from(target),
            )
            .label(format!("{} target level", row.info.name))
            .range(0.0, 1_000_000.0)
            .precision(0)
            .size(ControlSize::Sm)
            .on_change(move |value, _window, cx| {
                if value.is_finite() && value >= 0.0 {
                    let target = value.round() as u32;
                    entity.update(cx, |state, cx| state.set_statue_target(index, target, cx));
                }
            }),
        )
        .into_any_element()
}

fn exp_cell(row: &Row, estimate: Estimate, flair_available: bool) -> AnyElement {
    let Ok(cost) = &row.cost else {
        return unavailable_cell().into_any_element();
    };
    let deposit = estimate.deposit(cost);
    let flair = if flair_available {
        format!(
            "Flair {} · {}x",
            cost.flair_level,
            format::fmt_val(cost.flair_multiplier)
        )
    } else {
        "Flair unknown".to_string()
    };
    div()
        .id(SharedString::from(format!("statue-exp-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!(
            "{} EXP per statue, {flair}",
            format::fmt_val(deposit)
        ))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(div().text_size(px(13.0)).child(format::fmt_val(deposit)))
        .child(Caption::new(flair))
        .into_any_element()
}

fn needed_cell(row: &Row) -> AnyElement {
    let Some(statues) = row.statues else {
        return unavailable_cell().into_any_element();
    };
    div()
        .id(SharedString::from(format!("statue-needed-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{statues} statues needed"))
        .child(format::fmt_val(statues as f64))
        .into_any_element()
}

fn rate_cell(state: &AppState, row: &Row) -> AnyElement {
    let Some(input) = state.statue.rate_inputs.get(row.index) else {
        return unavailable_cell().into_any_element();
    };
    let invalid = row.rate.is_err();
    let field = input.clone();
    div()
        .id(SharedString::from(format!("statue-rate-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{} raw statues per hour", row.info.name))
        .w(px(112.0))
        .on_click(move |_event, window, cx| {
            let focus = field.read(cx).focus().clone();
            window.focus(&focus, cx);
        })
        .child(Input::new(input).size(ControlSize::Sm).invalid(invalid))
        .into_any_element()
}

fn time_cell(row: &Row) -> AnyElement {
    let label = if row.statues.is_none() || row.rate.is_err() {
        "—".to_string()
    } else {
        format::statue_time(row.hours)
    };
    div()
        .id(SharedString::from(format!("statue-time-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("Time to target: {label}"))
        .child(label)
        .into_any_element()
}

fn availability_cell(row: &Row, no_save: bool) -> AnyElement {
    let (label, tone) = row.availability(no_save);
    div()
        .id(SharedString::from(format!(
            "statue-availability-{}",
            row.index
        )))
        .role(Role::Label)
        .aria_label(label.clone())
        .flex()
        .child(Badge::new(label).tone(tone).dot())
        .into_any_element()
}

fn unavailable_cell() -> impl IntoElement {
    div().child("—")
}

#[cfg(test)]
mod tests;
