//! The kit table and figure pieces the fishing cards share.

use gpui::prelude::*;
use gpui::{AnyElement, ElementId, IntoElement, ParentElement, SharedString, div};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableRow};
use gpui_kit::component::tag::Tag;
use idleon_ui::widgets::{KpiStrip, Statistic, TableColumn, Tone, table_header};

/// One body row: its element id and its cells, in column order.
pub(super) struct Row {
    id: ElementId,
    cells: Vec<TableCell>,
}

impl Row {
    pub(super) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            cells: Vec::new(),
        }
    }

    pub(super) fn child(mut self, cell: TableCell) -> Self {
        self.cells.push(cell);
        self
    }

    pub(super) fn children(mut self, cells: impl IntoIterator<Item = TableCell>) -> Self {
        self.cells.extend(cells);
        self
    }
}

/// A kit table over `columns` and `rows` under `id`: the figure columns are
/// pinned and the text columns share the rest of the pane's width, so the table
/// fits its pane instead of scrolling sideways (`docs/ui-kit/data.md`).
pub(super) fn table(
    id: &'static str,
    columns: &[TableColumn],
    rows: impl IntoIterator<Item = Row>,
) -> AnyElement {
    let body = TableBody::new().children(rows.into_iter().map(|row| {
        TableRow::new().id(row.id).children(
            row.cells
                .into_iter()
                .zip(columns)
                .map(|(cell, column)| column.apply(cell)),
        )
    }));
    div()
        .id(id)
        .child(Table::new().child(table_header(columns)).child(body))
        .into_any_element()
}

/// A body cell holding `content`; its column sizes and aligns it.
pub(super) fn cell(content: impl IntoElement) -> TableCell {
    TableCell::new().child(content)
}

/// A small status tag.
pub(super) fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// One headline figure: element id, label, value and caption.
pub(super) type Kpi = (&'static str, SharedString, SharedString, SharedString);

/// A strip of figures.
pub(super) fn kpis<'a>(figures: impl IntoIterator<Item = &'a Kpi>) -> KpiStrip {
    KpiStrip::new().children(figures.into_iter().map(|(id, label, value, caption)| {
        Statistic::new(*id, label.clone(), value.clone())
            .caption(caption.clone())
            .small()
    }))
}
