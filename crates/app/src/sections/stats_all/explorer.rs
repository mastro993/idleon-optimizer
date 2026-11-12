//! SA-03, the system formula explorer: the ledger of every descriptor the
//! registry declares, the filters its list applies and the card of the formula
//! it shows.
//!
//! Upstream `js/ui/stats-explorer.js` searches the system catalogue and
//! `js/ui/formula-ledger.js` reports source coverage. This build keeps one
//! catalogue — the breakdown descriptors — so a card can never disagree with
//! the breakdowns above it, and the ledger reports the coverage this port can
//! prove instead of a source audit. The formula itself is the shared
//! [`FormulaTree`], which carries each term's saved input, multiplier and source.

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, SearchInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_stats::{Breakdown, BreakdownCategory, ExplorerFilter, ExplorerStatus};
use idleon_ui::formula::FormulaTree;

/// The explorer's own selection: its filters and the formula card it shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    /// One category, or every category.
    pub(crate) category: Option<BreakdownCategory>,
    pub(crate) status: ExplorerStatus,
    /// The descriptor whose formula card the explorer shows.
    pub(crate) selected: Option<String>,
}

impl State {
    /// The filter the list applies.
    pub(crate) fn filter(&self) -> ExplorerFilter {
        ExplorerFilter {
            category: self.category,
            status: self.status,
        }
    }

    /// Keep the card on a row the filters still leave, otherwise take the first:
    /// a search or a filter change never leaves the card on a hidden formula.
    pub(crate) fn select(&mut self, rows: &[&Breakdown]) {
        let kept = self
            .selected
            .as_deref()
            .is_some_and(|id| rows.iter().any(|row| row.id == id));
        if !kept {
            self.selected = rows.first().map(|row| row.id.to_string());
        }
    }
}

/// The explorer region: the ledger line, the filters, the formula rows and the
/// selected card.
pub(crate) fn region(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let tree = state.stats_explorer_formula.clone();
    let search = state.stats_explorer_search.clone();
    let query = search.read(cx).text().trim().to_string();
    let stats = &mut state.stats_all;
    let Some(sheet) = stats.sheet.as_ref() else {
        return Card::new()
            .header(CardHeader::new("System Formula Explorer").description(
                "Every formula the registry declares, the terms it reads and where it is defined.",
            ))
            .child(
                div()
                    .id("stats-explorer-empty")
                    .role(Role::Label)
                    .aria_label("Load a save to explore the formulas")
                    .px_3()
                    .pb_3()
                    .text_size(px(12.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child("Load a save to explore the formulas."),
            )
            .into_any_element();
    };
    let rows = sheet.matching(&query, stats.explorer.filter());
    stats.explorer.select(&rows);
    let coverage = sheet.coverage();
    let coverage_line = coverage.summary();
    let count = format!("{} of {} formulas match", rows.len(), coverage.total);
    let selected = stats
        .explorer
        .selected
        .as_deref()
        .and_then(|id| sheet.get(id));

    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let search_focus = search.read(cx).focus().clone();
    let category = entity.clone();
    let status = entity.clone();

    let controls = div()
        .id("stats-explorer-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(
            field("Search formulas", muted).child(
                div()
                    .id("stats-explorer-search")
                    .role(Role::Search)
                    .aria_label("Search formulas")
                    .w(px(260.0))
                    .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                    .child(SearchInput::new("stats-explorer-search-field", &search)),
            ),
        )
        .child(
            field("Category", muted).child(
                Select::new(
                    "stats-explorer-category",
                    std::iter::once(Choice::new("all", "All categories")).chain(
                        BreakdownCategory::ALL.map(|entry| Choice::new(entry.id(), entry.label())),
                    ),
                )
                .label("Formula category")
                .selected(
                    stats
                        .explorer
                        .category
                        .map(BreakdownCategory::id)
                        .unwrap_or("all"),
                )
                .on_change(move |value, _window, cx| {
                    let next = BreakdownCategory::from_id(value.as_ref());
                    category.update(cx, |state, cx| state.set_stats_explorer_category(next, cx));
                }),
            ),
        )
        .child(
            field("Status", muted).child(
                Select::new(
                    "stats-explorer-status",
                    ExplorerStatus::ALL.map(|entry| Choice::new(entry.id(), entry.label())),
                )
                .label("Formula status")
                .selected(stats.explorer.status.id())
                .on_change(move |value, _window, cx| {
                    if let Some(next) = ExplorerStatus::from_id(value.as_ref()) {
                        status.update(cx, |state, cx| state.set_stats_explorer_status(next, cx));
                    }
                }),
            ),
        );

    let has_query = !query.is_empty();
    let list = rows.iter().fold(
        div()
            .id("stats-explorer-rows")
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .pb_3(),
        |list, row| {
            let id = row.id;
            let value = row.display_value();
            let label = match &value {
                Some(value) => format!("{}: {value}", row.name),
                None => format!("{}: not modelled", row.name),
            };
            let source = row.source;
            let is_selected = selected.is_some_and(|selected| selected.id == id);
            let click = entity.clone();
            let row_id = id.to_string();
            // The same path the card footer shows, opened in the repository the
            // port reads, so "where does this formula come from" is one click.
            let url = row.source_url();
            list.child(
                div()
                    .id(format!("stats-formula-{id}"))
                    .role(Role::Button)
                    .aria_label(format!("{label} · {} · {source}", row.category.label()))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |row| row.bg(accent.opacity(0.12)))
                    .on_click(move |_, _window, cx| {
                        let id = row_id.clone();
                        click.update(cx, |state, cx| state.set_stats_formula(id, cx));
                    })
                    .child(div().flex_1().min_w_0().child(row.name))
                    .child(Caption::new(row.category.label()).text_color(muted))
                    .when_some(value, |row, value| row.child(div().child(value)))
                    .when(row.display_value().is_none(), |row| {
                        row.child(Badge::new("Not modelled").tone(Tone::Warning))
                    })
                    .child(
                        div()
                            .id(format!("stats-formula-{id}-source"))
                            .role(Role::Link)
                            .aria_label(format!("Open {source} in the upstream repository"))
                            .text_size(px(11.0))
                            .text_color(muted)
                            .hover(|style| style.text_color(accent))
                            .on_click(move |_event, _window, cx| {
                                cx.stop_propagation();
                                cx.open_url(&url);
                            })
                            .child(source),
                    ),
            )
        },
    );

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Card::new()
                .header(CardHeader::new("System Formula Explorer").description(
                    "Every formula the descriptor registry declares, grouped by category. A card \
                     shows the same value the breakdown resolves, the saved inputs behind its \
                     terms, and the upstream definition it ports.",
                ))
                .child(
                    div()
                        .id("stats-explorer-ledger")
                        .role(Role::Label)
                        .aria_label(coverage_line.clone())
                        .px_3()
                        .pb_1()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(coverage_line),
                )
                .child(controls)
                .child(
                    div()
                        .id("stats-explorer-count")
                        .role(Role::Label)
                        .aria_label(count.clone())
                        .px_3()
                        .pb_2()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(count),
                )
                .when(rows.is_empty(), |card| {
                    card.child(
                        div()
                            .id("stats-explorer-none")
                            .role(Role::Label)
                            .aria_label("No formula matches the filters")
                            .px_3()
                            .pb_3()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child(if has_query {
                                "No formula matches the search and the filters."
                            } else {
                                "No formula matches the filters."
                            }),
                    )
                })
                .child(
                    div()
                        .id("stats-explorer-panel")
                        .role(Role::Label)
                        .aria_label(format!("{} formulas", rows.len()))
                        .child(list),
                ),
        )
        .child(formula_card(selected, tree, cx))
        .into_any_element()
}

/// The selected formula's card: its value, its saved inputs and its tree, or
/// the source it still needs.
fn formula_card(
    row: Option<&Breakdown>,
    tree: Entity<FormulaTree>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let Some(row) = row else {
        return Card::new()
            .header(
                CardHeader::new("Formula Card")
                    .description("Pick a formula above to read its terms."),
            )
            .child(
                div()
                    .id("stats-formula-empty")
                    .role(Role::Label)
                    .aria_label("No formula selected")
                    .px_3()
                    .pb_3()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child("Pick a formula above to read its terms."),
            )
            .into_any_element();
    };
    let value = row
        .display_value()
        .unwrap_or_else(|| "not modelled".to_string());
    let scope = row.scope.label();
    let source = row.source;
    let url = row.source_url();
    let explanation = describe(row);

    let mut card = Card::new()
        .header(
            CardHeader::new(format!("{} · {scope}", row.name))
                .description(format!("{source} · {}", row.category.label())),
        )
        .child(
            div()
                .id("stats-formula-value")
                .role(Role::Label)
                .aria_label(format!("{}: {value}", row.name))
                .px_3()
                .pb_1()
                .text_size(px(12.0))
                .child(format!("{value} · {source}")),
        )
        .when(!explanation.is_empty(), |card| {
            card.child(
                div()
                    .id("stats-formula-explanation")
                    .role(Role::Label)
                    .aria_label(explanation.clone())
                    .px_3()
                    .pb_2()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(explanation),
            )
        })
        .child(
            div().px_3().pb_3().child(
                div()
                    .id("stats-formula-source")
                    .role(Role::Link)
                    .aria_label(format!("Open {source} in the upstream repository"))
                    .text_size(px(11.0))
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(|style| style.text_color(accent))
                    .on_click(move |_event, _window, cx| cx.open_url(&url))
                    .child(format!("Upstream source · {source}")),
            ),
        );
    if let Some(root) = row.resolution.tree() {
        let root = root.clone();
        tree.update(cx, |tree, cx| tree.set_root(root, cx));
        card = card.child(
            div()
                .id("stats-formula-tree")
                .h(px(320.0))
                .min_h_0()
                .child(tree),
        );
    }
    card.into_any_element()
}

/// The card's explanation: what the formula computes over and any reason the
/// total is partial or unavailable.
fn describe(row: &Breakdown) -> String {
    let mut parts = Vec::new();
    if let Some(root) = row.resolution.tree() {
        if let Some(input) = &root.input {
            parts.push(input.clone());
        }
        if let Some(note) = &root.note {
            parts.push(note.clone());
        }
    }
    if let Some(warning) = row.warning() {
        parts.push(warning.to_string());
    }
    parts.join(" · ")
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &'static str) -> Breakdown {
        Breakdown {
            id,
            name: id,
            category: BreakdownCategory::Combat,
            scope: idleon_stats::Scope::Account,
            source: "js/stats/defs/drop-rate.js",
            resolution: idleon_stats::Resolution::Unavailable {
                reason: "not modelled".into(),
            },
        }
    }

    #[test]
    fn the_card_follows_the_filters_and_takes_the_first_row_when_it_drops_out() {
        let a = row("a");
        let b = row("b");
        let mut state = State::default();
        state.select(&[&a, &b]);
        assert_eq!(state.selected.as_deref(), Some("a"));

        // A selection the filters still leave is kept.
        state.selected = Some("b".into());
        state.select(&[&a, &b]);
        assert_eq!(state.selected.as_deref(), Some("b"));

        // A selection the filters drop falls back to the first row.
        state.select(&[&a]);
        assert_eq!(state.selected.as_deref(), Some("a"));

        // No rows leaves the card empty.
        state.select(&[]);
        assert_eq!(state.selected, None);
    }
}
