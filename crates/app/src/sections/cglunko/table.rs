//! The bounded kit tables the tabs draw (`docs/ui-kit/data.md`): a page's
//! twelve upgrades, its six stones and the plan's purchases, one row per
//! upgrade, so a frame builds at most a dozen rows each.

use gpui::prelude::*;
use gpui::{AnyElement, App, Div, ElementId, SharedString, Stateful, div};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Table, TableBody, TableHeader, TableRow};
use idleon_ui::widgets::{TEXT_MONO, TableColumn};

/// A table under the element id `id` with `cols` heads and one row per entry
/// of `rows`. Each row gets the short id `<id>-tr` with its index, which keeps
/// the ids inside it under the bridge's length limit.
pub(super) fn table(
    id: &'static str,
    cols: &[TableColumn],
    rows: impl IntoIterator<Item = Vec<AnyElement>>,
    cx: &App,
) -> Stateful<Div> {
    let mono = cx.theme().mono_font_family.clone();
    let row_id = SharedString::from(format!("{id}-tr"));
    let head = cols.iter().fold(
        TableRow::new().id(SharedString::from(format!("{id}-th"))),
        |row, col| row.child(col.head()),
    );
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
