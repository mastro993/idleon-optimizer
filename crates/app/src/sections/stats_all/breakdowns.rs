//! SA-02, the category breakdowns: one tab per category, its descriptor rows
//! and the selected descriptor's formula tree.

use super::tag;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme as _;
use idleon_stats::{self as stats, BreakdownCategory, Resolution};
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

/// A category tab's element id: stable for the keyboard and the automation
/// harness.
fn tab_id(category: BreakdownCategory) -> SharedString {
    SharedString::from(format!("stats-{}-tab", category.id()))
}

/// The category tabs, the descriptor rows and the selected formula tree.
pub(super) fn region(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let theme = cx.theme();
    let (muted, accent, warning) = (theme.muted_foreground, theme.primary, theme.warning);
    let stats = &state.stats_all;
    let category = stats.category;
    let selected = stats.selected.as_deref();
    let sheet = stats.sheet.as_ref();
    let counts = BreakdownCategory::ALL.map(|entry| sheet.map_or(0, |sheet| sheet.count(entry)));
    let summary = BreakdownCategory::ALL
        .iter()
        .zip(counts)
        .map(|(entry, count)| format!("{} {count}", entry.label()))
        .collect::<Vec<_>>()
        .join(" · ");
    let tabs = BreakdownCategory::ALL
        .iter()
        .zip(counts)
        .map(|(entry, count)| {
            (
                tab_id(*entry),
                format!("{} ({count})", entry.label()).into(),
            )
        });
    let open = BreakdownCategory::ALL
        .iter()
        .position(|entry| *entry == category)
        .unwrap_or_default();
    let tabs = crate::ui::layout::tab_bar("stats-categories", tabs, open, move |index, _, cx| {
        let entry = BreakdownCategory::ALL[index];
        entity.update(cx, |state, cx| state.set_stats_category(entry, cx));
    });

    let rows: Vec<&stats::Breakdown> = sheet
        .map(|sheet| sheet.in_category(category).collect())
        .unwrap_or_default();
    let count = rows.len();
    let list = div()
        .id("stats-breakdown-rows")
        .flex()
        .flex_col()
        .gap_1()
        .children(rows.into_iter().map(|row| {
            let label = match row.display_value() {
                Some(value) => format!("{}: {value}", row.name),
                None => format!("{}: not modelled", row.name),
            };
            // A pending edit annotates the saved value with a
            // `Saved: <value> (±%)` line under the descriptor header.
            let saved = stats
                .affected
                .iter()
                .find(|entry| entry.id == row.id)
                .map(stats::Affected::saved_note);
            let aria = match saved.as_deref() {
                Some(note) => format!("{label} · {} · {note}", row.scope.label()),
                None => format!("{label} · {}", row.scope.label()),
            };
            let partial = matches!(
                &row.resolution,
                Resolution::Value {
                    partial: Some(_),
                    ..
                }
            );
            let click = cx.entity();
            let row_id = row.id.to_string();
            div()
                .id(format!("stats-row-{}", row.id))
                .role(Role::Button)
                .aria_label(aria)
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py_1()
                .rounded(cx.theme().radius)
                .cursor_pointer()
                .when(selected == Some(row.id), |line| {
                    line.bg(accent.opacity(0.12))
                })
                .on_click(move |_, _window, cx| {
                    let id = row_id.clone();
                    click.update(cx, |state, cx| state.set_stats_descriptor(id, cx));
                })
                .child(div().flex_1().child(row.name))
                .child(Caption::new(row.scope.label()).text_color(muted))
                .when_some(row.display_value(), |line, value| {
                    line.child(div().child(value))
                })
                .when_some(saved, |line, note| {
                    line.child(Caption::new(note).text_color(warning))
                })
                .when(partial, |line| line.child(tag("Partial", Tone::Warning)))
                .when(row.display_value().is_none(), |line| {
                    line.child(tag("Not modelled", Tone::Warning))
                })
        }));

    let detail = detail_card(state, cx);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Card::new()
                .header(CardHeader::new("Category Breakdowns").description(
                    "Every descriptor the registry lists, grouped by its category. A row whose \
                     formula this build does not model says so instead of showing a guess.",
                ))
                .child(
                    div()
                        .id("stats-category-summary")
                        .role(Role::Label)
                        .aria_label(summary.clone())
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(summary),
                )
                .child(tabs)
                .child(
                    div()
                        .id("stats-category-panel")
                        .role(Role::Label)
                        .aria_label(format!("{} breakdowns: {count} rows", category.label()))
                        .child(list),
                ),
        )
        .child(detail)
        .into_any_element()
}

/// The selected descriptor: its provenance, its value and the shared tree.
fn detail_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, warning) = (theme.muted_foreground, theme.warning);
    let Some(row) = state.stats_all.selected_row() else {
        return Card::new()
            .header(
                CardHeader::new("Descriptor Breakdown")
                    .description("Select a row above to see its formula tree."),
            )
            .child(
                div()
                    .id("stats-descriptor-empty")
                    .role(Role::Label)
                    .aria_label("No descriptor selected")
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child("Select a row above to see its formula tree."),
            )
            .into_any_element();
    };
    let value = row
        .display_value()
        .unwrap_or_else(|| "not modelled".to_string());
    let saved = state
        .stats_all
        .affected
        .iter()
        .find(|entry| entry.id == row.id)
        .map(stats::Affected::saved_note);
    let header = CardHeader::new(format!("{} · {}", row.name, row.scope.label()))
        .description("The formula tree is the shared breakdown component");
    let mut card = Card::new().header(header).child(
        div()
            .id("stats-descriptor-summary")
            .role(Role::Label)
            .aria_label(format!("{}: {value}", row.name))
            .text_size(TEXT_CAPTION)
            .text_color(muted)
            .child(value),
    );
    if let Some(note) = saved {
        card = card.child(
            div()
                .id("stats-descriptor-saved")
                .role(Role::Label)
                .aria_label(note.clone())
                .text_size(TEXT_CAPTION)
                .text_color(warning)
                .child(note),
        );
    }
    let shown = (state.stats_all.revision, row.id.to_string());
    if let Some(root) = row.resolution.tree() {
        // Hand the tree its root only when the row or the sheet changed.
        if state.stats_controls.breakdown_root.as_ref() != Some(&shown) {
            let root = root.clone();
            state
                .stats_breakdown
                .update(cx, |tree, cx| tree.set_root(root, cx));
            state.stats_controls.breakdown_root = Some(shown);
        }
        card = card.child(
            div()
                .id("stats-descriptor-formula")
                .h(px(320.0))
                .min_h_0()
                .child(state.stats_breakdown.clone()),
        );
    } else if let Some(reason) = row.warning() {
        card = card.child(
            div()
                .id("stats-descriptor-warning")
                .role(Role::Label)
                .aria_label(reason.to_string())
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(reason.to_string()),
        );
    }
    card.into_any_element()
}
