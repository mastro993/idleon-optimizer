//! The bounded kit tables the panes draw (`docs/ui-kit/data.md`): every one
//! lists a world's outposts or nodes, a catalog or a plan's steps, so a frame
//! builds at most a few dozen rows.

use gpui::prelude::*;
use gpui::{AnyElement, App, ElementId, SharedString, div};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Table, TableBody, TableHeader, TableRow};
use idleon_ui::widgets::{TEXT_MONO, TableColumn};

/// A table with `cols` heads and one row per entry of `rows`. Each row gets
/// the short id `<id>-tr` with its index, which keeps the ids inside it under
/// the bridge's length limit and apart from the ids the cells carry.
pub(super) fn table(
    id: &'static str,
    cols: &[TableColumn],
    rows: impl IntoIterator<Item = Vec<AnyElement>>,
    cx: &App,
) -> Table {
    let mono = cx.theme().mono_font_family.clone();
    let row_id = SharedString::from(format!("{id}-tr"));
    let head_id = SharedString::from(format!("{id}-th"));
    let head = cols.iter().fold(TableRow::new().id(head_id), |row, col| {
        row.child(col.head())
    });
    let body = rows.into_iter().enumerate().map(|(index, cells)| {
        cols.iter().zip(cells).fold(
            TableRow::new().id(ElementId::NamedInteger(row_id.clone(), index as u64)),
            |row, (col, cell)| {
                row.child(if col.is_number() {
                    col.cell().child(
                        div()
                            .font_family(mono.clone())
                            .text_size(TEXT_MONO)
                            .child(cell),
                    )
                } else {
                    col.cell().child(cell)
                })
            },
        )
    });
    Table::new()
        .child(TableHeader::new().child(head))
        .child(TableBody::new().children(body))
}

/// A plain text cell.
pub(super) fn text(value: impl Into<SharedString>) -> AnyElement {
    div().child(value.into()).into_any_element()
}

/// A text cell carrying the element id the harness selects.
pub(super) fn text_id(id: impl Into<ElementId>, value: impl Into<SharedString>) -> AnyElement {
    div().id(id).child(value.into()).into_any_element()
}
