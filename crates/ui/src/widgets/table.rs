//! A `Vec<T>` of rows plus column definitions as a kit [`TableDelegate`], so a
//! section shows a long table through the virtualized `DataTable` without
//! writing a delegate per table.
//!
//! The rows live in the delegate, inside the section's `TableState` entity, and
//! change only through [`RowsDelegate::set_rows`]. The table asks for the cells
//! of the rows in view, so a frame formats a screenful of cells however many
//! rows there are. See `docs/ui-kit/data.md`.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, FontFeatures, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Column, TableDelegate, TableState};

use super::TEXT_MONO;

/// Digits of equal width (OpenType `tnum`), so numbers line up down a column
/// while the mono face falls back to the system font (until #360 embeds it).
fn tabular_figures() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// One column: its kit [`Column`] and how a row reads as the cell's text.
pub struct RowColumn<T> {
    column: Column,
    numeric: bool,
    text: Rc<dyn Fn(&T) -> SharedString>,
}

impl<T> RowColumn<T> {
    /// A text column, left-aligned.
    pub fn text(
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
        text: impl Fn(&T) -> SharedString + 'static,
    ) -> Self {
        Self {
            column: Column::new(key, name),
            numeric: false,
            text: Rc::new(text),
        }
    }

    /// A number column: right-aligned, in the mono face with tabular figures.
    pub fn number(
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
        text: impl Fn(&T) -> SharedString + 'static,
    ) -> Self {
        Self {
            column: Column::new(key, name).text_right(),
            numeric: true,
            text: Rc::new(text),
        }
    }

    /// Adjusts the kit column (width, fixed side, resizing) beyond its kind.
    pub fn column(mut self, edit: impl FnOnce(Column) -> Column) -> Self {
        self.column = edit(self.column);
        self
    }
}

/// The rows and columns of one table.
pub struct RowsDelegate<T> {
    columns: Vec<RowColumn<T>>,
    rows: Vec<T>,
}

impl<T: 'static> RowsDelegate<T> {
    /// An empty table with these columns.
    pub fn new(columns: Vec<RowColumn<T>>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    /// Replaces the rows, built once by whatever changed them. Notify the
    /// `TableState` afterwards so the table redraws.
    pub fn set_rows(&mut self, rows: Vec<T>) {
        self.rows = rows;
    }

    fn cell(&self, row_ix: usize, col_ix: usize) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        let column = self.columns.get(col_ix)?;
        Some((column.text)(row))
    }
}

impl<T: 'static> TableDelegate for RowsDelegate<T> {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].column.clone()
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let numeric = self
            .columns
            .get(col_ix)
            .is_some_and(|column| column.numeric);
        div()
            .when(numeric, |cell| {
                cell.font_family(cx.theme().mono_font_family.clone())
                    .text_size(TEXT_MONO)
                    .font_features(tabular_figures())
            })
            .children(self.cell(row_ix, col_ix))
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        self.cell(row_ix, col_ix)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.cell(row_ix, col_ix)
            .map(String::from)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    use gpui::{AppContext as _, Entity, Render, TestAppContext, TextAlign, VisualTestContext, px};
    use gpui_kit::component::table::DataTable;

    use super::*;

    const ROWS: usize = 5_000;

    /// The rows whose cells the table asked for since the last `take`.
    type Read = Rc<RefCell<BTreeSet<usize>>>;

    struct Harness {
        table: Entity<TableState<RowsDelegate<usize>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(480.))
                .h(px(400.))
                .child(DataTable::new(&self.table))
        }
    }

    fn table(cx: &mut TestAppContext) -> (Entity<Harness>, Read, &mut VisualTestContext) {
        cx.update(gpui_kit::init);
        let read: Read = Rc::default();
        let seen = read.clone();
        let (view, cx) = cx.add_window_view(move |window, cx| {
            let mut delegate = RowsDelegate::new(vec![
                RowColumn::text("name", "Name", move |row: &usize| {
                    seen.borrow_mut().insert(*row);
                    format!("Row {row}").into()
                }),
                RowColumn::number("value", "Value", |row: &usize| (row * 7).to_string().into()),
            ]);
            delegate.set_rows((0..ROWS).collect());
            Harness {
                table: cx.new(|cx| TableState::new(delegate, window, cx)),
            }
        });
        cx.run_until_parked();
        (view, read, cx)
    }

    #[gpui::test]
    fn a_long_table_builds_only_the_rows_in_view(cx: &mut TestAppContext) {
        let (view, read, cx) = table(cx);
        let state = view.read_with(cx, |view, _| view.table.clone());
        assert_eq!(
            state.read_with(cx, |state, cx| state.delegate().rows_count(cx)),
            ROWS
        );

        let first = std::mem::take(&mut *read.borrow_mut());
        assert!(first.contains(&0), "the first row is in view: {first:?}");
        assert!(
            first.len() < 50,
            "{} of {ROWS} rows were built for a 400 px table",
            first.len()
        );

        state.update(cx, |state, cx| state.scroll_to_row(4_000, cx));
        cx.run_until_parked();
        let mut scrolled = std::mem::take(&mut *read.borrow_mut());
        // The row list lays out its first row to measure the row height.
        scrolled.remove(&0);
        assert!(
            scrolled.contains(&4_000),
            "row 4000 is in view: {scrolled:?}"
        );
        assert!(
            scrolled.len() < 50 && scrolled.iter().all(|row| *row >= 3_950),
            "scrolling built rows outside the view: {scrolled:?}"
        );
    }

    #[gpui::test]
    fn a_number_column_is_right_aligned(cx: &mut TestAppContext) {
        let (view, _, cx) = table(cx);
        let state = view.read_with(cx, |view, _| view.table.clone());
        let (name, value, text) = state.read_with(cx, |state, cx| {
            let delegate = state.delegate();
            (
                delegate.column(0, cx).align,
                delegate.column(1, cx).align,
                delegate.cell_text(3, 1, cx),
            )
        });
        assert_eq!(name, TextAlign::Left);
        assert_eq!(value, TextAlign::Right);
        assert_eq!(text, "21");
    }
}
