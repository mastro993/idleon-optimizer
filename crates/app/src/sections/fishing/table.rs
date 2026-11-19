//! The kit table and figure pieces the fishing cards share.

use gpui::prelude::*;
use gpui::{AnyElement, ElementId, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use idleon_ui::widgets::{KpiStrip, Statistic, Tone};

/// One column of a kit table: its title, whether it holds numbers (they sit
/// right-aligned, as `docs/ui-kit/data.md` sets them) and its width. A width of
/// `0.0` shares what the fixed columns leave.
pub(super) type Col = (SharedString, bool, f32);

/// `(title, numeric, width)` columns from static titles.
pub(super) fn columns(columns: &[(&'static str, bool, f32)]) -> Vec<Col> {
    columns
        .iter()
        .map(|(title, numeric, width)| (SharedString::from(*title), *numeric, *width))
        .collect()
}

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

/// A kit table over `columns` and `rows` under `id`. The kit shares a table's
/// width evenly between its columns, so each column takes its own width, and
/// the table scrolls sideways when the pane is narrower than them.
pub(super) fn table(
    id: &'static str,
    columns: &[Col],
    rows: impl IntoIterator<Item = Row>,
) -> AnyElement {
    let width = |width: f32| (width > 0.0).then(|| px(width));
    let head = TableRow::new().children(columns.iter().map(|(title, numeric, size)| {
        TableHead::new()
            .when(*numeric, TableHead::text_right)
            .when_some(width(*size), |head, size| head.flex_none().w(size))
            .child(title.clone())
    }));
    let body = TableBody::new().children(rows.into_iter().map(|row| {
        TableRow::new()
            .id(row.id)
            .children(
                row.cells
                    .into_iter()
                    .zip(columns)
                    .map(|(cell, (_, _, size))| {
                        cell.when_some(width(*size), |cell, size| cell.flex_none().w(size))
                    }),
            )
    }));
    let min_width: f32 = columns
        .iter()
        .map(|(_, _, size)| if *size > 0.0 { *size } else { 120.0 })
        .sum();
    div()
        .child(
            div().min_w(px(min_width)).child(
                Table::new()
                    .child(TableHeader::new().child(head))
                    .child(body),
            ),
        )
        .overflow_x_scrollbar()
        .id(id)
        .into_any_element()
}

/// A text cell.
pub(super) fn text(content: impl IntoElement) -> TableCell {
    TableCell::new().child(div().min_w_0().child(content))
}

/// A right-aligned number cell.
pub(super) fn number(content: impl IntoElement) -> TableCell {
    TableCell::new().text_right().child(content)
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
