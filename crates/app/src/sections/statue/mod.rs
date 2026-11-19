//! The Statue Calculator section: a character's saved statue levels, the EXP
//! their next level needs, and the statues and time a target level costs.
//!
//! ST-01 covers the table, search and sort; ST-02 the STARTUE EXP bubble, the
//! deposit estimate and the time estimate from a raw statues/hour rate; ST-03
//! the batch set/next-level actions with their undo step and the farming
//! breakdown behind one statue's row.

mod actions;
mod table;

use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, IntoElement, ParentElement, Role, Styled,
    Subscription, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::table::{DataTable, TableState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use idleon_statue::{
    self as statue, Estimate, Leveling, NextLevel, Plan, Progress, RateError, StatueInfo,
    StatueSave, TargetCost, Unavailable,
};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, KpiStrip, PANEL_GAP, Statistic, TEXT_CAPTION,
    Tone, number_state, number_value, search_input,
};
use std::cmp::Ordering;
use std::rc::Rc;

/// How the table is ordered when it is not in game order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Sort {
    #[default]
    Game,
    /// Lowest saved level first; statues without a row stay last.
    Level,
    /// Least EXP left to the next level first.
    Remaining,
}

impl Sort {
    const ALL: [Sort; 3] = [Sort::Game, Sort::Level, Sort::Remaining];

    fn label(self) -> &'static str {
        match self {
            Sort::Game => "Game order",
            Sort::Level => "Lowest level first",
            Sort::Remaining => "Least EXP to next level",
        }
    }
}

/// The inputs the table's rows are computed from.
#[derive(Clone, PartialEq)]
struct RowsKey {
    generation: u64,
    character: usize,
    estimate: Estimate,
    plan: Plan,
}

/// The inputs the farming breakdown is built from.
#[derive(Clone, PartialEq)]
struct BreakdownKey {
    generation: u64,
    character: usize,
    statue: usize,
    target: Option<u32>,
    estimate: Estimate,
    rate: String,
}

/// The rows a frame draws, built when what they read changes.
struct Rows {
    key: RowsKey,
    /// Every statue, in game order.
    all: Rc<[Row]>,
    /// The search and sort the shown order was built for.
    view: (String, Sort),
    /// Indices into `all`: the statues the search keeps, in the sort's order.
    shown: Rc<[usize]>,
}

/// The section's own state: the imported statue rows, the account multipliers,
/// the table controls and the plan the batch actions rewrite.
pub(crate) struct State {
    pub(crate) save: StatueSave,
    pub(crate) character: usize,
    pub(crate) sort: Sort,
    pub(crate) estimate: Estimate,
    /// The entered targets and raw rates, with the batch undo step.
    pub(crate) plan: Plan,
    /// The statue the farming breakdown describes.
    pub(crate) breakdown_statue: usize,
    pub(crate) breakdown: Entity<FormulaTree>,
    pub(crate) search: Entity<InputState>,
    /// The Set all targets entry, read when its button is pressed.
    pub(crate) batch_text: String,
    /// Why the last batch entry was rejected, or `None` when it applied.
    pub(crate) batch_error: Option<String>,
    /// What the last batch action did, shown under its buttons.
    pub(crate) batch_status: Option<String>,
    /// Bumped by every import, so the cached rows know the save changed.
    generation: u64,
    rows: Option<Rows>,
    breakdown_key: Option<BreakdownKey>,
    character_select: ChoiceSelect<usize>,
    estimate_select: ChoiceSelect<Estimate>,
    sort_select: ChoiceSelect<Sort>,
    breakdown_select: ChoiceSelect<usize>,
    batch_input: Entity<InputState>,
    /// The statue rows, with one target and one rate field per statue.
    table: Entity<TableState<table::Rows>>,
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let search = crate::ui::input_state("Name or effect", window, cx);
        let batch_input = crate::ui::input_state("Level", window, cx);
        let breakdown = crate::ui::formula_tree("statue-breakdown", window, cx);
        let target_inputs: Vec<Entity<InputState>> = (0..statue::count())
            .map(|_| number_state(0.0, 1_000_000.0, 1.0, window, cx))
            .collect();
        let rate_inputs: Vec<Entity<InputState>> = (0..statue::count())
            .map(|_| crate::ui::input_state("e.g. 1M", window, cx))
            .collect();
        let character_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let estimate_select = ChoiceSelect::new(estimate_choices(), None, window, cx);
        let sort_select = ChoiceSelect::new(sort_choices(), None, window, cx);
        let breakdown_select = ChoiceSelect::new(breakdown_choices(), None, window, cx);

        // Edits flow control -> section value, through the section's setters.
        let mut subscriptions = vec![
            pick(cx, &character_select, AppState::set_statue_character),
            pick(cx, &estimate_select, AppState::set_statue_estimate),
            pick(cx, &sort_select, AppState::set_statue_sort),
            pick(cx, &breakdown_select, AppState::set_statue_breakdown),
            cx.subscribe(&batch_input, |state, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    state.statue.batch_text = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
        ];
        for (index, input) in target_inputs.iter().enumerate() {
            subscriptions.push(cx.subscribe(input, move |state, input, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(target) = number_value(input.read(cx))
                    && target >= 0.0
                {
                    state.set_statue_target(index, target.round() as u32, cx);
                }
            }));
        }
        for (index, input) in rate_inputs.iter().enumerate() {
            subscriptions.push(cx.subscribe(input, move |state, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().trim().to_string();
                    state.statue.plan.set_rate(index, text);
                    cx.notify();
                }
            }));
        }
        Self {
            save: StatueSave::default(),
            character: 0,
            sort: Sort::Game,
            estimate: Estimate::Guaranteed,
            plan: Plan::new(),
            breakdown_statue: 0,
            breakdown,
            search,
            batch_text: String::new(),
            batch_error: None,
            batch_status: None,
            generation: 0,
            rows: None,
            breakdown_key: None,
            character_select,
            estimate_select,
            sort_select,
            breakdown_select,
            batch_input,
            table: table::state(table::Rows::new(target_inputs, rate_inputs), window, cx),
            _subscriptions: subscriptions,
        }
    }

    /// Replace the saved rows after an import, keeping the selected character
    /// when the new save still carries it and clearing the plan inputs.
    pub(crate) fn load(&mut self, save: StatueSave) {
        self.save = save;
        self.generation = self.generation.wrapping_add(1);
        if !self.save.has_character(self.character) {
            self.character = self.save.first_character().unwrap_or(0);
        }
        if !self.save.has_character(self.breakdown_statue) && self.save.first_character().is_some()
        {
            self.breakdown_statue = self.save.first_character().unwrap_or(0);
        }
        self.clear_inputs();
    }

    /// Change the table's character and clear its plan inputs.
    pub(crate) fn select_character(&mut self, character: usize) {
        self.character = character;
        self.clear_inputs();
    }

    fn clear_inputs(&mut self) {
        self.plan.clear();
        self.batch_error = None;
        self.batch_status = None;
        self.batch_text.clear();
    }

    /// Replace the plan inputs from a project config, keeping one entry per
    /// statue. The fields on screen follow on the next frame.
    pub(crate) fn set_plan(&mut self, targets: Vec<Option<u32>>, rates: Vec<String>) {
        self.plan.reset(targets, rates);
        self.batch_error = None;
        self.batch_status = None;
    }

    /// The target level for a statue: the entered target, else the saved level
    /// plus one.
    pub(crate) fn target(&self, statue: usize) -> Option<u32> {
        self.plan.target(statue).or_else(|| {
            self.save
                .progress(self.character, statue)
                .map(|progress| progress.level + 1)
        })
    }

    pub(crate) fn set_target(&mut self, statue: usize, target: u32) {
        self.plan.set_target(statue, target);
    }

    /// One statue's raw statues/hour entry, trimmed.
    pub(crate) fn rate_text(&self, statue: usize) -> String {
        self.plan.rate(statue).to_string()
    }

    /// The parsed rate the row and the breakdown farm at.
    pub(crate) fn rate(&self, statue: usize) -> Result<f64, RateError> {
        statue::parse_rate(self.plan.rate(statue))
    }

    /// The statue the breakdown describes, kept inside the game's table.
    pub(crate) fn breakdown_statue(&self) -> usize {
        self.breakdown_statue.min(statue::count().saturating_sub(1))
    }

    pub(crate) fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().to_string()
    }

    /// Bring the cached rows, the table and the breakdown tree in line with
    /// the save, the plan, the search and the sort. Each rebuilds only when
    /// what it reads changed, so most frames make a few comparisons.
    fn sync(&mut self, save_loaded: bool, cx: &mut Context<AppState>) {
        let key = RowsKey {
            generation: self.generation,
            character: self.character,
            estimate: self.estimate,
            plan: self.plan.clone(),
        };
        let view = (self.query(cx), self.sort);
        let all = match self.rows.take() {
            Some(rows) if rows.key == key => {
                if rows.view == view {
                    self.rows = Some(rows);
                    None
                } else {
                    Some(rows.all)
                }
            }
            _ => Some(build_rows(self).into()),
        };
        if let Some(all) = all {
            let mut shown: Vec<&Row> = all
                .iter()
                .filter(|row| matches(row.info, &view.0))
                .collect();
            sort_rows(&mut shown, view.1);
            let shown = shown.iter().map(|row| row.index).collect();
            self.rows = Some(Rows {
                key,
                all,
                view,
                shown,
            });
        }

        let rows = self.rows();
        let (all, shown) = (rows.all.clone(), rows.shown.clone());
        let (estimate, flair_available) = (self.estimate, self.save.leveling().flair_available);
        self.table.update(cx, |table, cx| {
            let rows = table.delegate_mut();
            if !Rc::ptr_eq(&rows.all, &all)
                || !Rc::ptr_eq(&rows.shown, &shown)
                || rows.estimate != estimate
                || rows.flair_available != flair_available
                || rows.no_save == save_loaded
            {
                rows.all = all;
                rows.shown = shown;
                rows.estimate = estimate;
                rows.flair_available = flair_available;
                rows.no_save = !save_loaded;
                cx.notify();
            }
        });

        let statue = self.breakdown_statue();
        let key = BreakdownKey {
            generation: self.generation,
            character: self.character,
            statue,
            target: self.target(statue),
            estimate: self.estimate,
            rate: self.rate_text(statue),
        };
        if self.breakdown_key.as_ref() != Some(&key) {
            let tree = statue::breakdown_tree(
                &self.save,
                self.character,
                statue,
                key.target,
                self.estimate,
                self.rate(statue).unwrap_or(0.0),
            );
            self.breakdown
                .update(cx, |breakdown, cx| breakdown.set_root(tree, cx));
            self.breakdown_key = Some(key);
        }
    }
}

/// A select whose pick calls `set`.
fn pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    set: fn(&mut AppState, V, &mut Context<AppState>),
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            let SelectEvent::Confirm(value) = event;
            if let Some(value) = value.clone() {
                set(state, value, cx);
            }
        },
    )
}

fn estimate_choices() -> Vec<Choice<Estimate>> {
    Estimate::ALL
        .into_iter()
        .map(|estimate| Choice::new(estimate, estimate.label()))
        .collect()
}

fn sort_choices() -> Vec<Choice<Sort>> {
    Sort::ALL
        .into_iter()
        .map(|sort| Choice::new(sort, sort.label()))
        .collect()
}

fn breakdown_choices() -> Vec<Choice<usize>> {
    (0..statue::count())
        .filter_map(|index| Some(Choice::new(index, statue::info(index)?.name.clone())))
        .collect()
}

/// Show `text` in `field` unless it already holds it or is being typed in.
/// Writing emits no `InputEvent::Change`, so the section value stays put.
fn show_text(field: &Entity<InputState>, text: &str, window: &mut Window, cx: &mut App) {
    if field.focus_handle(cx).is_focused(window) || field.read(cx).value() == text {
        return;
    }
    let text = text.to_owned();
    field.update(cx, |input, cx| input.set_value(text, window, cx));
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

fn build_rows(state: &State) -> Vec<Row> {
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

fn sort_rows<R: std::borrow::Borrow<Row>>(rows: &mut [R], sort: Sort) {
    match sort {
        Sort::Game => {}
        Sort::Level => rows.sort_by_key(|row| (row.borrow().level_key(), row.borrow().index)),
        Sort::Remaining => rows.sort_by(|a, b| {
            let (a, b) = (a.borrow(), b.borrow());
            a.remaining_key()
                .partial_cmp(&b.remaining_key())
                .unwrap_or(Ordering::Equal)
                .then(a.index.cmp(&b.index))
        }),
    }
}

/// The Statue Calculator page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let save_loaded = state.save_loaded;
    state.statue.sync(save_loaded, cx);
    let save_empty = state.statue.save.is_empty();
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let flair_missing = state.save_loaded && !state.statue.save.leveling().flair_available;
    let state = &*state;

    let page = v_flex()
        .id("statue-pane")
        .gap(PANEL_GAP)
        .p_3()
        .when(!state.save_loaded, |page| {
            page.child(div().id("statue-no-save").child(Caption::new(
                "Import a decoded save to read its per-character statue levels.",
            )))
        })
        .when(state.save_loaded && save_empty, |page| {
            let entity = cx.entity();
            page.child(
                h_flex()
                    .gap_2()
                    .child(
                        Alert::new(
                            "statue-no-rows",
                            "This save carries no StatueLevels_<n> rows, so no statue level is \
                             available.",
                        )
                        .with_variant(Tone::Warning.into())
                        .banner()
                        .flex_1(),
                    )
                    .child(
                        Button::new("statue-import")
                            .label("Import another save")
                            .small()
                            .on_click(move |_event, window, cx| {
                                entity.update(cx, |state, cx| state.open_import(window, cx));
                            }),
                    ),
            )
        })
        .when(flair_missing, |page| {
            page.child(
                Alert::new(
                    "statue-no-flair",
                    "Royal Guardian data is missing: saved Flair levels are unknown, so estimates \
                     assume no Flair bonus.",
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            )
        })
        .child(toolbar(state, window, cx))
        .when(state.save_loaded && !save_empty, |page| {
            page.child(summary(&state.statue))
        })
        .child(statues_card(state, table_height, window, cx))
        .child(farming_card(state, window, cx));

    div()
        .id("statue-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(page)
                .overflow_y_scrollbar()
                .id("statue-scroll"),
        )
        .into_any_element()
}

impl State {
    fn rows(&self) -> &Rows {
        self.rows.as_ref().expect("the view syncs the rows first")
    }
}

/// Character, estimate, search and sort controls.
fn toolbar(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let statue = &state.statue;
    let characters: Vec<Choice<usize>> = statue
        .save
        .character_indices()
        .map(|index| Choice::new(index, statue.save.name(index)))
        .collect();
    let has_characters = !characters.is_empty();
    statue
        .character_select
        .show(characters, Some(&statue.character), window, cx);
    statue
        .estimate_select
        .show(estimate_choices(), Some(&statue.estimate), window, cx);
    statue
        .sort_select
        .show(sort_choices(), Some(&statue.sort), window, cx);
    let search = statue.search.clone();

    h_flex()
        .id("statue-toolbar")
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            field("Character").child(
                Select::new(statue.character_select.state())
                    .id("statue-character")
                    .accessibility_label("Character")
                    .placeholder("No characters")
                    .disabled(!has_characters)
                    .small()
                    .w(px(200.0)),
            ),
        )
        .child(
            field("Deposit estimate").child(
                Select::new(statue.estimate_select.state())
                    .id("statue-estimate")
                    .accessibility_label("Deposit estimate")
                    .small()
                    .w(px(240.0)),
            ),
        )
        .child(
            field("Find a statue").child(
                div()
                    .id("statue-search")
                    .role(Role::Search)
                    .aria_label("Find a statue")
                    .w(px(280.0))
                    .on_click(move |_event, window, cx| {
                        search.update(cx, |field, cx| field.focus(window, cx));
                    })
                    .child(
                        search_input(&statue.search)
                            .id("statue-search-field")
                            .aria_label("Find a statue")
                            .small(),
                    ),
            ),
        )
        .child(
            field("Sort").child(
                Select::new(statue.sort_select.state())
                    .id("statue-sort")
                    .accessibility_label("Sort statues")
                    .small()
                    .w(px(220.0)),
            ),
        )
        .into_any_element()
}

fn field(label: &'static str) -> gpui::Div {
    v_flex().gap_1().child(Caption::new(label))
}

/// The four summary figures: statues needed, retention, the tier multiplier
/// and the parallel completion time.
fn summary(state: &State) -> AnyElement {
    let leveling: &Leveling = state.save.leveling();
    let rows = &state.rows().all;
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
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "kpi-statues-needed",
                        "Statues needed",
                        format::fmt_val(needed as f64),
                    )
                    .caption(needed_caption),
                )
                .child(
                    Statistic::new(
                        "kpi-retention",
                        "EXP retention",
                        format!("{:.1}%", leveling.retention_pct),
                    )
                    .caption(format!(
                        "STARTUE {}% · retention capped at 50%",
                        format::fmt_val(leveling.bubble_pct)
                    )),
                )
                .child(
                    Statistic::new(
                        "kpi-tier",
                        "Onyx / Zenith EXP",
                        format!("{}x", format::fmt_val(leveling.tier_multiplier)),
                    )
                    .caption(format!(
                        "{} Onyx+ and {} Zenith statues",
                        leveling.onyx, leveling.zenith
                    )),
                )
                .child(
                    Statistic::new("kpi-parallel", "Parallel completion", parallel).caption(
                        if unmeasured > 0 {
                            format!("{unmeasured} target rates missing")
                        } else {
                            "All target rates maintained together".to_string()
                        },
                    ),
                ),
        )
        .into_any_element()
}

fn statues_card(
    state: &AppState,
    table_height: gpui::Pixels,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let statue = &state.statue;
    let (all, shown) = (statue.rows().all.clone(), statue.rows().shown.clone());
    let total = all.len();
    let theme = cx.theme();
    let (muted, danger) = (theme.muted_foreground, theme.danger);
    let character = statue.save.name(statue.character);
    let save_empty = statue.save.is_empty();
    let missing = if save_empty {
        0
    } else {
        statue.save.missing_rows(statue.character)
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
    let rate_error = all.iter().find_map(|row| row.rate.as_ref().err().copied());
    let rates_entered = all.iter().any(|row| !row.rate_text.is_empty());
    let rate_source = if rates_entered {
        format!("Rate source: manual entries for {character}.")
    } else {
        format!("Rate source: none yet for {character}.")
    };

    let body = if shown.is_empty() {
        div()
            .id("statue-empty")
            .child(
                Empty::new().header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new().child(Icon::new(IconName::SearchX).text_color(muted)),
                        )
                        .title(EmptyTitle::new().child("No statues match your search."))
                        .description(
                            EmptyDescription::new()
                                .child("Try a statue name or effect, such as POWER or MOVE SPEED."),
                        ),
                ),
            )
            .into_any_element()
    } else {
        div()
            .id("statue-table-scroll")
            .h(table_height)
            // A row holds a name over its effect and a level over its bar.
            .child(DataTable::new(&statue.table).with_size(px(ROW_HEIGHT)))
            .into_any_element()
    };

    Card::new()
        .header(
            CardHeader::new("Statues").description(description).action(
                div()
                    .id("statue-count")
                    .role(Role::Label)
                    .aria_label(format!("{} of {total} statues shown", shown.len()))
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(format!("{} / {total} shown", shown.len())),
            ),
        )
        .child(
            v_flex()
                .gap_2()
                .min_h_0()
                .child(batch_row(state, window, cx))
                .child(batch_messages(statue, cx))
                .child(body)
                .child(
                    div()
                        .id("statue-rate-source")
                        .role(Role::Label)
                        .aria_label(rate_source.clone())
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(rate_source),
                )
                .when_some(rate_error, |column, error| {
                    column.child(
                        div()
                            .id("statue-rate-error")
                            .role(Role::Label)
                            .aria_label(error.message())
                            .text_size(TEXT_CAPTION)
                            .text_color(danger)
                            .child(error.message()),
                    )
                })
                .child(
                    div()
                        .id("statue-note")
                        .role(Role::Label)
                        .text_size(TEXT_CAPTION)
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

/// The height of a statue row: three lines of the level cell.
const ROW_HEIGHT: f32 = 68.0;

/// The batch controls: Set all targets, Next level for all, and the
/// undo step the batch actions leave.
fn batch_row(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let statue = &state.statue;
    let disabled = !state.save_loaded || statue.save.is_empty();
    let input = statue.batch_input.clone();
    show_text(&input, &statue.batch_text, window, cx);
    let focus = input.clone();
    let entity = cx.entity();
    let apply = entity.clone();
    let next = entity.clone();
    let undo = entity.clone();
    let undo_label = statue.plan.undo_label();

    h_flex()
        .id("statue-batch")
        .flex_wrap()
        .items_end()
        .gap_2()
        .child(
            div()
                .id("statue-set-all")
                .role(Role::Label)
                .aria_label("Set all targets")
                .w(px(120.0))
                .on_click(move |_event, window, cx| {
                    focus.update(cx, |field, cx| field.focus(window, cx));
                })
                .child(Input::new(&input).aria_label("Set all targets").small()),
        )
        .child(
            Button::new("statue-apply-all")
                .label("Set targets")
                .small()
                .disabled(disabled)
                .on_click(move |_event, _window, cx| {
                    apply.update(cx, |state, cx| state.set_statue_targets_all(cx));
                }),
        )
        .child(
            Button::new("statue-next-level")
                .label("Next level for all")
                .small()
                .disabled(disabled)
                .on_click(move |_event, _window, cx| {
                    next.update(cx, |state, cx| state.set_statue_next_level(cx));
                }),
        )
        .when_some(undo_label, |row, label| {
            row.child(
                Button::new("statue-undo-batch")
                    .label(format!("Undo {label}"))
                    .small()
                    .on_click(move |_event, _window, cx| {
                        undo.update(cx, |state, cx| state.undo_statue_batch(cx));
                    }),
            )
        })
        .child(Caption::new(
            "Set targets costs the level for every statue first; a level this save cannot plan changes nothing. \
             Rates are raw statues/hour, so K, M, B, T and scientific notation work.",
        ))
        .into_any_element()
}

/// What the last batch action did, and why one was rejected.
fn batch_messages(state: &State, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let (muted, danger) = (theme.muted_foreground, theme.danger);
    v_flex()
        .id("statue-batch-messages")
        .gap_1()
        .when_some(state.batch_status.clone(), |column, status| {
            column.child(
                div()
                    .id("statue-batch-status")
                    .role(Role::Label)
                    .aria_label(status.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(status),
            )
        })
        .when_some(state.batch_error.clone(), |column, error| {
            column.child(
                div()
                    .id("statue-batch-error")
                    .role(Role::Label)
                    .aria_label(error.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(danger)
                    .child(error),
            )
        })
        .into_any_element()
}

/// The farming breakdown behind one statue's row: the deposit terms the
/// detail block names, plus the statues, rate and time the table shows.
fn farming_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let statue = &state.statue;
    statue.breakdown_select.show(
        breakdown_choices(),
        Some(&statue.breakdown_statue()),
        window,
        cx,
    );
    Card::new()
        .header(
            CardHeader::new("Farming breakdown")
                .description("Where one statue's deposit EXP and farm time come from")
                .action(
                    Select::new(statue.breakdown_select.state())
                        .id("statue-breakdown-statue")
                        .accessibility_label("Break down statue")
                        .small()
                        .w(px(200.0)),
                ),
        )
        .child(
            div().id("statue-farming").child(
                div()
                    .id("statue-breakdown")
                    .h(px(300.0))
                    .min_h_0()
                    .child(statue.breakdown.clone()),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests;
