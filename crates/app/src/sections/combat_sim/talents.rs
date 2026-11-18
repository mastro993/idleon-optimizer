//! The talent half of the loadout editor.
//!
//! Every talent id the save stores a level or a maximum for, its saved maximum,
//! and a level control that stops at that maximum. A save whose stored level is
//! above its own maximum is flagged, not silently clamped, so the editor shows
//! what the save really says.
//!
//! The table is a virtualized `DataTable`: its rows are rebuilt only when the
//! talents or the search change, and a frame draws the rows in view.

use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Subscription, WeakEntity, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::empty::{Empty, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use idleon_ui::widgets::{
    Card, CardHeader, PANEL_GAP, PANEL_PADDING, TEXT_CAPTION, TEXT_MONO, Tone, number_state,
    number_value, search_input, show_number,
};
use std::collections::HashMap;

/// One talent row: what the table draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TalentRow {
    id: u32,
    name: SharedString,
    level: u32,
    max: u32,
}

/// The talent table's rows and the level field of each talent shown so far.
pub(crate) struct Rows {
    app: WeakEntity<AppState>,
    rows: Vec<TalentRow>,
    /// One field per talent and maximum, built the first time its row is drawn
    /// and kept, so a field never loses focus to a redraw.
    fields: HashMap<(u32, u32), (Entity<InputState>, Subscription)>,
}

impl Rows {
    pub(crate) fn new(app: WeakEntity<AppState>) -> Self {
        Self {
            app,
            rows: Vec::new(),
            fields: HashMap::new(),
        }
    }

    fn field(
        &mut self,
        row: &TalentRow,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Entity<InputState> {
        let (id, max) = (row.id, row.max.max(row.level));
        let app = self.app.clone();
        let (field, _) = self.fields.entry((id, max)).or_insert_with(|| {
            let field = number_state(0.0, f64::from(max), 1.0, window, cx);
            let edits = cx.subscribe(&field, move |_, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(level) = number_value(field.read(cx))
                {
                    let level = level.max(0.0).round() as u32;
                    app.update(cx, |state, cx| state.set_combat_talent(id, level, cx))
                        .ok();
                }
            });
            (field, edits)
        });
        field.clone()
    }
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &gpui::App) -> usize {
        3
    }

    fn rows_count(&self, _: &gpui::App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &gpui::App) -> Column {
        match col_ix {
            0 => Column::new("talent", "Talent").width(px(320.0)),
            1 => Column::new("level", "Level").width(px(150.0)),
            _ => Column::new("max", "Maximum").width(px(100.0)).text_right(),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix).cloned() else {
            return div().into_any_element();
        };
        match col_ix {
            0 => div()
                .id(SharedString::from(format!("combat-talent-{}", row.id)))
                .role(Role::Label)
                .aria_label(format!("{}: {} of {}", row.name, row.level, row.max))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(TEXT_MONO)
                        .font_weight(FontWeight::MEDIUM)
                        .child(row.name.clone()),
                )
                .when(row.level > row.max, |cell| {
                    cell.child(
                        Tag::new()
                            .with_variant(Tone::Danger.into())
                            .outline()
                            .small()
                            .child(format!("above maximum {}", row.max)),
                    )
                })
                .into_any_element(),
            1 => {
                let field = self.field(&row, window, cx);
                show_number(&field, f64::from(row.level), 0, window, cx);
                crate::ui::number_field(
                    SharedString::from(format!("combat-talent-{}-level", row.id)),
                    &field,
                    format!("{} level", row.name),
                )
                .into_any_element()
            }
            _ => div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(TEXT_MONO)
                .text_color(cx.theme().muted_foreground)
                .child(row.max.to_string())
                .into_any_element(),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &gpui::App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match col_ix {
            0 => row.name.clone(),
            1 => row.level.to_string().into(),
            _ => row.max.to_string().into(),
        })
    }
}

/// The talents pane: the search field and one editable row per talent.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let query = state.combat.talent_query(cx);
    let muted = cx.theme().muted_foreground;
    let total = state.combat.loadout.talents.len();
    let needle = query.trim().to_lowercase();
    let rows: Vec<TalentRow> = state
        .combat
        .loadout
        .talents
        .iter()
        .filter(|talent| {
            needle.is_empty()
                || talent.name.to_lowercase().contains(&needle)
                || talent.id.to_string() == needle
        })
        .map(|talent| TalentRow {
            id: talent.id,
            name: talent.name.clone().into(),
            level: talent.level,
            max: talent.max,
        })
        .collect();
    let shown = rows.len();
    let table = state.combat.controls.talents.clone();
    // The table redraws only when the talents or the search changed.
    table.update(cx, |table, cx| {
        if table.delegate().rows != rows {
            table.delegate_mut().rows = rows;
            cx.notify();
        }
    });

    let body = if total == 0 {
        empty(
            "combat-talents-empty",
            "No talents in this save",
            Some("The save carries no SL_<n> or SM_<n> table for this character."),
            cx,
        )
    } else if shown == 0 {
        empty(
            "combat-talents-no-match",
            "No talent matches your search",
            None,
            cx,
        )
    } else {
        div()
            .id("combat-talent-scroll")
            .h(layout::table_height(f32::from(
                window.viewport_size().height,
            )))
            .child(DataTable::new(&table))
            .into_any_element()
    };

    Card::new()
        .header(
            CardHeader::new("Talents")
                .description(format!(
                    "{total} talent(s) in this save; the level stops at the saved maximum"
                ))
                .action(
                    div()
                        .id("combat-talent-count")
                        .role(Role::Label)
                        .aria_label(format!("{shown} of {total} talents shown"))
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(format!("{shown} / {total} shown")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(PANEL_GAP)
                .min_h_0()
                .p(PANEL_PADDING)
                .child(
                    div().id("combat-talent-search").w(px(280.0)).child(
                        search_input(&state.combat.talent_search)
                            .id("combat-talent-search-field")
                            .aria_label("Find a talent")
                            .small(),
                    ),
                )
                .child(body),
        )
        .into_any_element()
}

/// An empty table: why there is nothing to list.
pub(crate) fn empty(
    id: &'static str,
    title: &'static str,
    description: Option<&'static str>,
    cx: &Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    div()
        .id(id)
        .child(
            Empty::new().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(IconName::SearchX).text_color(muted)))
                    .title(EmptyTitle::new().child(title))
                    .when_some(description, |header, text| {
                        header.description(
                            gpui_kit::component::empty::EmptyDescription::new().child(text),
                        )
                    }),
            ),
        )
        .into_any_element()
}
