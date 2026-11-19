//! The kit tables the tabs draw (`docs/ui-kit/data.md`). Every one is bounded
//! by a game catalog (the ten jar types, the forty collectibles, the schematics,
//! the six tiers) or by the twenty newest snapshots, so the tabs, which scroll
//! as one page, keep plain tables.

use gpui::prelude::*;
use gpui::{AnyElement, App, Div, ElementId, SharedString, div};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use idleon_ui::widgets::TEXT_MONO;

/// A column: its head, and whether it holds numbers (right-aligned, mono).
#[derive(Clone, Copy)]
pub(super) enum Col {
    Text(&'static str),
    Number(&'static str),
}

/// A table under the element id `id` with `cols` heads and one row per entry
/// of `rows`. Each row gets the short id `<id>-tr` with its index, which keeps
/// the ids inside it under the bridge's length limit.
pub(super) fn table(
    id: impl Into<SharedString>,
    cols: &[Col],
    rows: impl IntoIterator<Item = Vec<AnyElement>>,
    cx: &App,
) -> gpui::Stateful<Div> {
    let id = id.into();
    let mono = cx.theme().mono_font_family.clone();
    let row_id = SharedString::from(format!("{id}-tr"));
    let head = cols.iter().fold(
        TableRow::new().id(SharedString::from(format!("{id}-th"))),
        |row, col| {
            row.child(match col {
                Col::Text(name) => TableHead::new().child(*name),
                Col::Number(name) => TableHead::new().text_right().child(*name),
            })
        },
    );
    let body = rows.into_iter().enumerate().map(|(index, cells)| {
        cols.iter().zip(cells).fold(
            TableRow::new().id(ElementId::NamedInteger(row_id.clone(), index as u64)),
            |row, (col, cell)| {
                row.child(match col {
                    Col::Text(_) => TableCell::new().child(cell),
                    Col::Number(_) => TableCell::new()
                        .text_right()
                        .font_family(mono.clone())
                        .text_size(TEXT_MONO)
                        .child(cell),
                })
            },
        )
    });
    div().id(id).child(
        Table::new()
            .child(TableHeader::new().child(head))
            .child(TableBody::new().children(body)),
    )
}

/// A plain text cell.
pub(super) fn text(value: impl Into<SharedString>) -> AnyElement {
    div().child(value.into()).into_any_element()
}
