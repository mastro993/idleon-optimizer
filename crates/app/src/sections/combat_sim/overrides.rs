//! The mechanics overrides pane (CS-02).
//!
//! The Mechanics overrides tab: a searchable catalog grouped by mechanic
//! family, one mode per row (Saved, Off, Set, Scale ×), a value field for the
//! two valued modes, and a reset that puts every mechanic back to its saved
//! value. The active rules are also chips above the editor, so what a run would
//! change is visible from any tab.
//!
//! The saved values the Scale mode multiplies, and the stat context that applies
//! a rule to a run, arrive with the component-term trace; the encounter run this
//! build ships reads the spawn roster only, so the pane still takes every rule
//! and persists it for that context.
//!
//! The catalog is a virtualized `DataTable`; each mechanic's mode select and
//! value field are built the first time its row is drawn and kept.

use super::talents::empty;
use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, Styled, Subscription, WeakEntity, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_combat::overrides::{self, Mode, Rule};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, PANEL_GAP, PANEL_PADDING, TEXT_CAPTION,
    TEXT_MONO, Tone, number_state, number_value, search_input, show_number,
};
use std::collections::HashMap;

/// A mechanic row's height: its name, its description and its save keys.
const ROW_HEIGHT: f32 = 64.0;

/// One catalog row: the mechanic and the rule it carries now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MechanicRow {
    /// The mechanic's index in [`overrides::data`].
    toggle: usize,
    rule: Option<Rule>,
}

impl MechanicRow {
    fn toggle(&self) -> &'static overrides::Toggle {
        &overrides::data().toggles[self.toggle]
    }
}

/// One mechanic's controls, kept once its row has been drawn.
struct MechanicControls {
    mode: ChoiceSelect<Option<Mode>>,
    value: gpui::Entity<InputState>,
    _subscriptions: [Subscription; 2],
}

/// The catalog rows the filters leave, and the controls drawn so far.
pub(crate) struct Rows {
    app: WeakEntity<AppState>,
    rows: Vec<MechanicRow>,
    controls: HashMap<usize, MechanicControls>,
}

impl Rows {
    pub(crate) fn new(app: WeakEntity<AppState>) -> Self {
        Self {
            app,
            rows: Vec::new(),
            controls: HashMap::new(),
        }
    }

    fn controls(
        &mut self,
        row: MechanicRow,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> &MechanicControls {
        let app = self.app.clone();
        self.controls.entry(row.toggle).or_insert_with(|| {
            let toggle = row.toggle();
            let mode = ChoiceSelect::new(mode_choices(toggle), Some(&None), window, cx);
            let value = number_state(-1e15, 1e15, 0.1, window, cx);
            let id = toggle.id.clone();
            let picked = {
                let (app, id) = (app.clone(), id.clone());
                cx.subscribe(
                    mode.state(),
                    move |_, _, event: &SelectEvent<Vec<Choice<Option<Mode>>>>, cx| {
                        if let SelectEvent::Confirm(Some(mode)) = event {
                            app.update(cx, |state, cx| {
                                state.set_combat_override_mode(&id, *mode, cx);
                            })
                            .ok();
                        }
                    },
                )
            };
            let typed = cx.subscribe(&value, move |_, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    app.update(cx, |state, cx| {
                        state.set_combat_override_value(&id, value, cx);
                    })
                    .ok();
                }
            });
            MechanicControls {
                mode,
                value,
                _subscriptions: [picked, typed],
            }
        })
    }
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &App) -> usize {
        4
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("mechanic", "Mechanic").width(px(380.0)),
            1 => Column::new("group", "Group").width(px(140.0)),
            2 => Column::new("mode", "Mode").width(px(190.0)),
            _ => Column::new("value", "Value").width(px(160.0)),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix).copied() else {
            return div().into_any_element();
        };
        let toggle = row.toggle();
        let muted = cx.theme().muted_foreground;
        match col_ix {
            0 => mechanic_cell(toggle, row.rule.is_some(), muted),
            1 => div()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(SharedString::from(toggle.group.clone()))
                .into_any_element(),
            2 => {
                let mode = row.rule.map(|rule| rule.mode);
                let controls = self.controls(row, window, cx);
                controls
                    .mode
                    .show(mode_choices(toggle), Some(&mode), window, cx);
                Select::new(controls.mode.state())
                    .id(SharedString::from(format!(
                        "combat-override-{}-mode",
                        toggle.id
                    )))
                    .accessibility_label(format!("{} mode", toggle.label))
                    .small()
                    .into_any_element()
            }
            _ => match row.rule {
                Some(rule) if matches!(rule.mode, Mode::Set | Mode::Scale) => {
                    let field = self.controls(row, window, cx).value.clone();
                    show_number(&field, rule.value.unwrap_or(0.0), 4, window, cx);
                    crate::ui::number_field(
                        SharedString::from(format!("combat-override-{}-value", toggle.id)),
                        &field,
                        format!("{} value", toggle.label),
                    )
                    .into_any_element()
                }
                _ => div()
                    .text_size(TEXT_MONO)
                    .text_color(muted)
                    .child("\u{2014}")
                    .into_any_element(),
            },
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        let toggle = row.toggle();
        Some(match col_ix {
            0 => toggle.label.clone().into(),
            1 => toggle.group.clone().into(),
            2 => row.rule.map_or("Saved", |rule| rule.mode.label()).into(),
            _ => row
                .rule
                .and_then(|rule| rule.value)
                .map_or_else(|| "\u{2014}".into(), |value| value.to_string().into()),
        })
    }
}

/// What a mechanic is: its name, what it does and the save keys it reads.
fn mechanic_cell(toggle: &overrides::Toggle, overridden: bool, muted: gpui::Hsla) -> AnyElement {
    div()
        .id(SharedString::from(format!("combat-override-{}", toggle.id)))
        .role(Role::Label)
        .aria_label(format!("{}: {}", toggle.group, toggle.label))
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(TEXT_MONO)
                        .font_weight(FontWeight::MEDIUM)
                        .child(SharedString::from(toggle.label.clone())),
                )
                .when(overridden, |row| {
                    row.child(
                        Tag::new()
                            .with_variant(Tone::Accent.into())
                            .outline()
                            .small()
                            .child("override"),
                    )
                }),
        )
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .truncate()
                .child(SharedString::from(toggle.desc.clone())),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(muted)
                .truncate()
                .child(SharedString::from(toggle.keys.join(", "))),
        )
        .into_any_element()
}

/// Saved, then every mode the mechanic takes.
fn mode_choices(toggle: &overrides::Toggle) -> Vec<Choice<Option<Mode>>> {
    std::iter::once(Choice::new(None, "Saved"))
        .chain(
            toggle
                .modes()
                .into_iter()
                .map(|mode| Choice::new(Some(mode), mode.label())),
        )
        .collect()
}

/// The mechanics pane: the toolbar and one row per mechanic.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let active = state.combat.overrides.len();
    let query = state.combat.override_query(cx).trim().to_lowercase();
    let group = state.combat.override_group.clone();
    let only_active = state.combat.override_only_active;
    let entity = cx.entity();
    let reset = entity.clone();

    // Filtering the fixed catalog is cheap; the table redraws only when the
    // rows it lists changed.
    let rows: Vec<MechanicRow> = overrides::data()
        .toggles
        .iter()
        .enumerate()
        .filter(|(_, toggle)| group.is_empty() || toggle.group == group)
        .filter(|(_, toggle)| !only_active || state.combat.overrides.active(&toggle.id))
        .filter(|(_, toggle)| {
            query.is_empty()
                || toggle.label.to_lowercase().contains(&query)
                || toggle.desc.to_lowercase().contains(&query)
                || toggle.group.to_lowercase().contains(&query)
                || toggle.id.to_lowercase().contains(&query)
                || toggle
                    .keys
                    .iter()
                    .any(|key| key.to_lowercase().contains(&query))
        })
        .map(|(index, toggle)| MechanicRow {
            toggle: index,
            rule: state.combat.overrides.rule(&toggle.id),
        })
        .collect();
    let matching = rows.len();
    let table = state.combat.controls.mechanics.clone();
    table.update(cx, |table, cx| {
        if table.delegate().rows != rows {
            table.delegate_mut().rows = rows;
            cx.notify();
        }
    });

    let body = if matching == 0 {
        empty(
            "combat-overrides-no-match",
            "No mechanic matches",
            Some("Clear the search or pick another group to reach the rest."),
            cx,
        )
    } else {
        div()
            .id("combat-overrides-scroll")
            .h(layout::table_height(f32::from(
                window.viewport_size().height,
            )))
            .child(DataTable::new(&table).with_size(px(ROW_HEIGHT)))
            .into_any_element()
    };

    let controls = &state.combat.controls;
    controls.group.show(
        super::controls::group_choices(),
        Some(&state.combat.override_group),
        window,
        cx,
    );
    let group_select = Select::new(controls.group.state())
        .id("combat-overrides-group")
        .accessibility_label("Mechanic group")
        .small();

    Card::new()
        .header(
            CardHeader::new("Mechanics overrides")
                .description(
                    "Turn a mechanic off, replace its saved input with a value, or scale it. \
                     Overrides belong to the working loadout and every run reads them; Reset \
                     puts each input back to the save.",
                )
                .action(
                    div()
                        .id("combat-overrides-count")
                        .role(Role::Label)
                        .aria_label(format!("{active} active overrides"))
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(format!("{active} active")),
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
                    div()
                        .id("combat-overrides-toolbar")
                        .flex_none()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(PANEL_GAP)
                        .child(
                            div().id("combat-overrides-search").w(px(240.0)).child(
                                search_input(&state.combat.override_search)
                                    .id("combat-overrides-search-field")
                                    .aria_label("Find a mechanic")
                                    .small(),
                            ),
                        )
                        .child(div().w(px(200.0)).child(group_select))
                        .child(only_active_check(state, cx))
                        .child(div().flex_1())
                        .child(
                            Button::new("combat-overrides-reset")
                                .label("Reset all")
                                .small()
                                .disabled(active == 0)
                                .on_click(move |_event, _window, cx| {
                                    reset.update(cx, |state, cx| {
                                        state.reset_combat_overrides(cx);
                                    });
                                }),
                        ),
                )
                .child(div().flex_none().child(Caption::new(
                    "The rules belong to the working loadout and travel in the project export. \
                     The run this build ships reads the map's spawn roster only, so an override \
                     changes nothing yet: the saved values Scale multiplies arrive with the \
                     component-term trace.",
                )))
                .child(body),
        )
        .into_any_element()
}

/// The chips above the editor: one removable tag per active rule.
pub(crate) fn chips(state: &AppState, cx: &mut Context<AppState>) -> Option<AnyElement> {
    let chips = state.combat.overrides.chips();
    if chips.is_empty() {
        return None;
    }
    let entity = cx.entity();
    let summary = chips
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let mut row = div()
        .id("combat-override-chips")
        .role(Role::Label)
        .aria_label(summary)
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2();
    for (id, line) in chips {
        let remove = entity.clone();
        let key = id.clone();
        // A removable tag: the kit tag holds a ghost button that drops the rule.
        row = row.child(
            div()
                .id(SharedString::from(format!("combat-override-chip-{id}")))
                .child(
                    Tag::new()
                        .with_variant(Tone::Accent.into())
                        .outline()
                        .small()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(SharedString::from(line.clone()))
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "combat-override-chip-{id}-remove"
                                    )))
                                    .icon(IconName::X)
                                    .ghost()
                                    .xsmall()
                                    .tooltip(format!("Remove {line}"))
                                    .on_click(
                                        move |_event, _window, cx| {
                                            remove.update(cx, |state, cx| {
                                                state.remove_combat_override(&key, cx);
                                            });
                                        },
                                    ),
                                ),
                        ),
                ),
        );
    }
    Some(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Caption::new("Overrides").text_color(cx.theme().muted_foreground))
            .child(row)
            .into_any_element(),
    )
}

/// The "only changed" filter.
fn only_active_check(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Checkbox::new("combat-overrides-only-active")
        .label("Only changed")
        .checked(state.combat.override_only_active)
        .on_click(move |checked, _window, cx| {
            let checked = *checked;
            entity.update(cx, |state, cx| {
                state.set_combat_overrides_only_active(checked, cx);
            });
        })
        .into_any_element()
}
