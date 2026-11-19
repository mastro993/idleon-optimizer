//! The kit table pieces the sailing tabs share.
//!
//! The tables whose rows grow with the fleet (one per boat, captain or boat
//! upgrade) are virtualized `DataTable`s over [`Rows`]: their rows are built
//! when the tab's inputs change, never in render (`docs/ui-kit/data.md`). The
//! cells carry the element ids and tooltips the harness reads, so the delegate
//! draws elements instead of the text-only `idleon_ui::widgets::RowsDelegate`.
//! Small fixed tables are kit `Table`s through [`table`].

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, ElementId, Entity, FontFeatures, IntoElement, ParentElement, Pixels,
    SharedString, Styled, Window, div, px,
};
use gpui_kit::component::table::{
    Column, Table, TableBody, TableCell, TableDelegate, TableRow, TableState,
};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_ui::widgets::{ColumnFit, FitColumns, TEXT_MONO, TableColumn, Tone, table_header};
use std::sync::Arc;

/// How one column draws a row.
enum Draw<T> {
    Text,
    Number,
    Element(fn(&T, &App) -> AnyElement),
}

/// One column: the kit column, how a row draws in it and how it reads aloud.
pub(crate) struct Cell<T> {
    column: Column,
    draw: Draw<T>,
    text: fn(&T) -> SharedString,
}

impl<T> Cell<T> {
    /// A left-aligned text column `width` wide.
    pub(crate) fn text(name: &'static str, width: f32, text: fn(&T) -> SharedString) -> Self {
        Self {
            column: Column::new(name, name).width(px(width)),
            draw: Draw::Text,
            text,
        }
    }

    /// A right-aligned number in the mono face.
    pub(crate) fn number(name: &'static str, width: f32, text: fn(&T) -> SharedString) -> Self {
        Self {
            column: Column::new(name, name).width(px(width)).text_right(),
            draw: Draw::Number,
            text,
        }
    }

    /// A column whose cells are elements (an id, a tooltip, a tag); `text` is
    /// what the cell reads as.
    pub(crate) fn element(
        name: &'static str,
        width: f32,
        draw: fn(&T, &App) -> AnyElement,
        text: fn(&T) -> SharedString,
    ) -> Self {
        Self {
            column: Column::new(name, name).width(px(width)),
            draw: Draw::Element(draw),
            text,
        }
    }
}

/// A table's rows and columns, owned by its `TableState`.
pub(crate) struct Rows<T> {
    columns: Vec<Cell<T>>,
    rows: Vec<T>,
    fit: ColumnFit,
}

impl<T: 'static> Rows<T> {
    fn text(&self, row_ix: usize, col_ix: usize) -> Option<SharedString> {
        Some((self.columns.get(col_ix)?.text)(self.rows.get(row_ix)?))
    }
}

impl<T: 'static> TableDelegate for Rows<T> {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        self.fit.apply(self.natural_column(col_ix, cx))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(row), Some(cell)) = (self.rows.get(row_ix), self.columns.get(col_ix)) else {
            return div().into_any_element();
        };
        match cell.draw {
            Draw::Text => div().child((cell.text)(row)).into_any_element(),
            Draw::Number => mono((cell.text)(row), cx),
            Draw::Element(draw) => draw(row, cx),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        self.text(row_ix, col_ix)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.text(row_ix, col_ix)
            .map(String::from)
            .unwrap_or_default()
    }
}

impl<T: 'static> FitColumns for Rows<T> {
    fn column_fit(&mut self) -> &mut ColumnFit {
        &mut self.fit
    }

    fn natural_column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].column.clone()
    }
}

/// A table state over `columns`: no selection, sorting or column moves.
pub(crate) fn state<T: 'static>(
    columns: Vec<Cell<T>>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TableState<Rows<T>>> {
    cx.new(|cx| {
        TableState::new(
            Rows {
                columns,
                rows: Vec::new(),
                fit: ColumnFit::default(),
            },
            window,
            cx,
        )
        .row_selectable(false)
        .col_selectable(false)
        .col_movable(false)
        .sortable(false)
    })
}

/// Replace a table's rows and redraw it.
pub(crate) fn set_rows<T: 'static>(
    table: &Entity<TableState<Rows<T>>>,
    rows: Vec<T>,
    cx: &mut App,
) {
    table.update(cx, |table, cx| {
        table.delegate_mut().rows = rows;
        cx.notify();
    });
}

/// One body row: its element id and its cells, in column order.
pub(crate) struct Row {
    id: ElementId,
    cells: Vec<TableCell>,
}

impl Row {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            cells: Vec::new(),
        }
    }

    pub(crate) fn child(mut self, cell: TableCell) -> Self {
        self.cells.push(cell);
        self
    }

    pub(crate) fn children(mut self, cells: impl IntoIterator<Item = TableCell>) -> Self {
        self.cells.extend(cells);
        self
    }
}

/// A kit table over `columns` and `rows` under `id`: the figure columns are
/// pinned and the text columns share the rest of the pane's width, so the table
/// fits its pane instead of scrolling sideways (`docs/ui-kit/data.md`).
pub(crate) fn table(
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
pub(crate) fn cell(content: impl IntoElement) -> TableCell {
    TableCell::new().child(content)
}

/// A small status tag.
pub(crate) fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// `body` under `id`, with `tip` on hover.
pub(crate) fn tip(
    id: impl Into<ElementId>,
    tip: SharedString,
    body: impl IntoElement,
) -> AnyElement {
    div()
        .id(id)
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .child(body)
        .into_any_element()
}

/// A number in the mono face with tabular figures.
pub(crate) fn mono(text: SharedString, cx: &App) -> AnyElement {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(TEXT_MONO)
        .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        .child(text)
        .into_any_element()
}

/// The height a virtualized table takes inside a pane `max` tall: its rows up
/// to that, so a short fleet does not leave a tall empty panel.
pub(crate) fn data_height(rows: usize, row_height: f32, max: Pixels) -> Pixels {
    px(row_height * (rows + 1) as f32 + 2.0).min(max)
}
