//! The kit pieces the clamworks tabs share: their tables, status tags and
//! notes.
//!
//! Every clamworks table is bounded by a game list (nine pearl upgrades, nine
//! Scurge upgrades, twelve worker-class rewards, two pearl drops), so each is a
//! kit `Table` (`docs/ui-kit/data.md`).

use gpui::prelude::*;
use gpui::{AnyElement, App, Div, ElementId, SharedString, Stateful, div};
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_ui::widgets::{Caption, TEXT_MONO, Tone};

/// A column: its head, and whether it holds numbers (right-aligned, mono).
#[derive(Clone, Copy)]
pub(super) enum Col {
    Text(&'static str),
    Number(&'static str),
}

/// A table with `cols` heads and one row per entry of `rows`, in a wrapper
/// with the id `<id>-table`: sibling kit tables need an id each, or their
/// accessibility nodes collide. Each row gets the short id `<id>-tr` with its
/// index, which keeps the ids inside it under the bridge's length limit.
pub(super) fn table<R: IntoIterator<Item = AnyElement>>(
    id: impl Into<SharedString>,
    cols: &[Col],
    rows: impl IntoIterator<Item = R>,
    cx: &App,
) -> Stateful<Div> {
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
    div().id(SharedString::from(format!("{id}-table"))).child(
        Table::new()
            .child(TableHeader::new().child(head))
            .child(TableBody::new().children(body)),
    )
}

/// A plain text cell.
pub(super) fn text(value: impl Into<SharedString>) -> AnyElement {
    value.into().into_any_element()
}

/// A small status tag.
pub(super) fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// A muted footnote under a card's body.
pub(super) fn note(text: impl Into<SharedString>, cx: &App) -> Caption {
    Caption::new(text).text_color(cx.theme().muted_foreground)
}
