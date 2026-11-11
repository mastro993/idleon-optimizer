//! The Statue Calculator section: a character's saved statue levels, the EXP
//! their next level needs, and which rows the save carries.
//!
//! Ported from `statue-calc.html` and `js/stats/systems/common/statue-leveling.js`.
//! ST-01 covers the table, search and sort; the STARTUE bubble, time estimates
//! and batch actions arrive with ST-02 and ST-03.

use crate::state::AppState;
use crate::ui::{format, layout};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::{Banner, EmptyState};
use ely_gpui_component::forms::{Choice, SearchInput, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, div, px,
};
use idleon_core::statue::{self, NextLevel, Progress, StatueInfo, StatueSave, Unavailable};
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

/// The section's own state: the imported statue rows and the table controls.
pub struct State {
    pub save: StatueSave,
    pub character: usize,
    pub sort: Sort,
    pub search: Entity<TextInput>,
}

impl State {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Name or effect")
                .label("Find a statue")
        });
        Self {
            save: StatueSave::default(),
            character: 0,
            sort: Sort::Game,
            search,
        }
    }

    /// Replace the saved rows after an import, keeping the selected character
    /// when the new save still carries it.
    pub fn load(&mut self, save: StatueSave) {
        self.save = save;
        if !self.save.has_character(self.character) {
            self.character = self.save.first_character().unwrap_or(0);
        }
    }

    pub fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().to_string()
    }
}

/// One table row: static data plus what this save says about it.
struct Row {
    index: usize,
    info: &'static StatueInfo,
    progress: Option<Progress>,
    next: Result<NextLevel, Unavailable>,
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
            Err(Unavailable::NoRow(row)) if row == self.index => {
                ("Missing row".into(), Tone::Warning)
            }
            Err(Unavailable::NoRow(gate)) => match statue::info(gate) {
                Some(info) => (format!("Needs {}", info.name), Tone::Warning),
                None => ("Missing gate".into(), Tone::Warning),
            },
            Err(Unavailable::NoStatue(_)) => ("Unavailable".into(), Tone::Warning),
            Err(Unavailable::OutOfRange) => ("Out of range".into(), Tone::Danger),
        }
    }
}

fn rows(state: &State) -> Vec<Row> {
    (0..statue::count())
        .filter_map(|index| {
            let info = statue::info(index)?;
            Some(Row {
                index,
                info,
                progress: state.save.progress(state.character, index),
                next: statue::next_level(&state.save, state.character, index),
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
    let mut rows = rows(&state.statue);
    let total = rows.len();
    rows.retain(|row| matches(row.info, &query));
    sort_rows(&mut rows, state.statue.sort);
    let shown = rows.len();
    let table_height = layout::table_height(f32::from(window.viewport_size().height));

    div()
        .id("statue-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
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
        .child(toolbar(state, cx))
        .child(statues_card(
            state,
            cx,
            &rows,
            (shown, total),
            save_empty,
            table_height,
        ))
        .into_any_element()
}

/// Character, search and sort controls.
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

fn statues_card(
    state: &AppState,
    cx: &mut Context<AppState>,
    rows: &[Row],
    (shown, total): (usize, usize),
    save_empty: bool,
    table_height: gpui::Pixels,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let character = state.statue.save.name(state.statue.character);
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

    let mut table = Table::new([
        "Statue / effect",
        "Level / progress",
        "Next level requirement",
        "Availability",
    ]);
    for row in rows {
        table = table.row([
            statue_cell(row),
            level_cell(row),
            next_cell(row),
            availability_cell(row, !state.save_loaded),
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
                .child(body)
                .child(
                    div()
                        .id("statue-note")
                        .role(Role::Label)
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(
                            "Requirements are the full EXP bar for the saved level; remaining EXP uses the saved progress. \
                             Statues 10-19, 20-29 and 30-31 round with the saved level of statue 1, 2 and 3.",
                        ),
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

fn next_cell(row: &Row) -> AnyElement {
    match &row.next {
        Ok(next) => div()
            .id(SharedString::from(format!("statue-next-{}", row.index)))
            .role(Role::Label)
            .aria_label(format!(
                "{} EXP for level {}, {} left",
                format::rate(next.required),
                next.level + 1,
                format::rate(next.remaining)
            ))
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_size(px(13.0))
                    .child(format!("{} EXP", format::rate(next.required))),
            )
            .child(Caption::new(format!(
                "{} left",
                format::rate(next.remaining)
            )))
            .into_any_element(),
        Err(_) => div()
            .id(SharedString::from(format!("statue-next-{}", row.index)))
            .role(Role::Label)
            .aria_label("Requirement unavailable")
            .child("—")
            .into_any_element(),
    }
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
