//! SA-04, the game-input sandbox: the editable inputs with their saved and
//! pending values, and the formulas the pending edits move.

use super::{State, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_stats as stats;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

/// The editable game inputs, the pending edits and the saved values.
///
/// There is no per-node editor, so the ledger rows carry the controls: each row
/// shows the saved and current value, marks a pending edit and resets that one
/// input, and `Reset all edits` clears the whole sandbox.
pub(super) fn input_ledger(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, warning, danger) = (theme.muted_foreground, theme.warning, theme.danger);
    let entity = cx.entity();
    let stats = &state.stats_all;
    let sandbox = stats.sandbox.as_ref();
    let status = sandbox
        .map(stats::Sandbox::status)
        .unwrap_or_else(|| "Game-input sandbox: no edits.".to_string());
    let count = sandbox.map(stats::Sandbox::count).unwrap_or_default();
    let error = stats.input_error.clone();

    let reset_all = entity.clone();
    let mut rows = div().id("stats-input-rows").flex().flex_col().gap_1();
    if let Some(sandbox) = sandbox {
        if sandbox.inputs().is_empty() {
            let line = "This save carries none of the inputs the modelled formulas read.";
            rows = rows.child(
                div()
                    .id("stats-input-empty")
                    .role(Role::Label)
                    .aria_label(line)
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(line),
            );
        }
        let fields = &state.stats_controls.inputs;
        for (input, field) in sandbox.inputs().iter().zip(fields) {
            let id = input.id.clone();
            let current = sandbox.value(&id).unwrap_or(input.saved);
            let edited = sandbox.edited(&id);
            let label = if edited {
                format!(
                    "{}: {} → {}",
                    input.title(),
                    stats::GameInput::show(input.saved),
                    stats::GameInput::show(current)
                )
            } else {
                format!("{}: {}", input.title(), stats::GameInput::show(input.saved))
            };
            let reset = entity.clone();
            let row = div()
                .id(format!("stats-input-row-{id}"))
                .role(Role::Label)
                .aria_label(label)
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py_1()
                .rounded(cx.theme().radius)
                .when(edited, |row| row.bg(warning.opacity(0.12)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(TEXT_CAPTION)
                        .child(input.name.clone()),
                )
                .child(Caption::new(input.label).text_color(muted))
                .child(
                    crate::ui::number_field(
                        format!("stats-input-{id}"),
                        &field.field,
                        input.title(),
                    )
                    .w(px(140.0)),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .when(edited, |saved| {
                            saved.child(format!("Saved: {}", stats::GameInput::show(input.saved)))
                        }),
                )
                .when(edited, |row| {
                    row.child(tag("Changed", Tone::Warning)).child(
                        Button::new(format!("stats-input-reset-{id}"))
                            .label("Reset")
                            .small()
                            .on_click(move |_event, _window, cx| {
                                let id = id.clone();
                                reset.update(cx, |state, cx| state.reset_stats_input(id, cx));
                            }),
                    )
                });
            rows = rows.child(row);
        }
    }

    Card::new()
        .header(CardHeader::new("Game Input Sandbox").description(
            "Edit a game input to resolve a hypothetical save. Nothing is written to the save: \
             Reset puts every value back.",
        ))
        .child(
            div()
                .id("stats-input-status")
                .role(Role::Label)
                .aria_label(status.clone())
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(status),
        )
        .when(stats.building, |card| {
            card.child(Caption::new("Resolving the edited save…").text_color(muted))
        })
        .when(!error.is_empty(), |card| {
            card.child(
                div()
                    .id("stats-inputs-error")
                    .role(Role::Label)
                    .aria_label(error.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(danger)
                    .child(error),
            )
        })
        .child(
            div().flex().child(
                Button::new("stats-inputs-reset")
                    .label("Reset all edits")
                    .small()
                    .disabled(count == 0)
                    .on_click(move |_event, _window, cx| {
                        reset_all.update(cx, |state, cx| state.reset_stats_inputs(cx));
                    }),
            ),
        )
        .child(rows)
        .into_any_element()
}

/// SA-04's other half: every descriptor the pending edits moved.
///
/// Each row links back to the descriptor it names, so the tab and the formula
/// tree follow the click here too.
pub(super) fn affected_formulas(state: &State, cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, warning, hover) = (theme.muted_foreground, theme.warning, theme.list_hover);
    let entity = cx.entity();
    let rows = &state.affected;
    let count = format!("Affected formulas ({})", rows.len());
    let mut list = div().id("stats-affected-rows").flex().flex_col().gap_1();
    if rows.is_empty() {
        let line = match state.sandbox.as_ref().map(stats::Sandbox::count) {
            Some(0) | None => "No formula changed: every value resolves from the save.",
            Some(_) => "The pending edits do not move a computed formula.",
        };
        list = list.child(
            div()
                .id("stats-affected-empty")
                .role(Role::Label)
                .aria_label(line)
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(line),
        );
    }
    for row in rows {
        let id = row.id.clone();
        let line = row.line();
        let pick = entity.clone();
        list = list.child(
            div()
                .id(format!("stats-affected-{id}"))
                .role(Role::Button)
                .aria_label(format!("{}: {line}", row.name))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py_1()
                .rounded(cx.theme().radius)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_event, _window, cx| {
                    let id = id.clone();
                    pick.update(cx, |state, cx| state.select_stats_descriptor(id, cx));
                })
                .child(
                    div()
                        .flex_1()
                        .text_size(TEXT_CAPTION)
                        .child(row.name.clone()),
                )
                .child(
                    div()
                        .text_size(TEXT_CAPTION)
                        .text_color(warning)
                        .child(line),
                ),
        );
    }

    // The header names the list; the count line under it is what the
    // harness and a screen reader read.
    Card::new()
        .header(CardHeader::new("Affected Formulas").description(
            "The formulas whose value the pending edits move, with their saved value and \
             the hypothetical one.",
        ))
        .child(
            div()
                .id("stats-affected-count")
                .role(Role::Label)
                .aria_label(count.clone())
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(count),
        )
        .child(list)
        .into_any_element()
}
