//! A virtualized `DataTable` whose columns fill its width.
//!
//! A `DataTable` lays its columns out at their kit widths and leaves the rest
//! of its width empty, which reads as a blank trailing column; a pane
//! narrower than the columns scrolls sideways. A delegate that implements
//! [`FitColumns`] scales its natural columns by its [`ColumnFit`], and
//! [`fit_columns`] measures the table as it draws and rescales them when its
//! width changes: the columns keep their proportions and end at the table's
//! right edge. No column narrows below its kit `min_width`, so a column whose
//! content has a fixed size (a stepper, a text field) sets that as its floor.

use gpui::{App, Div, Entity, ParentElement as _, Pixels, Styled as _, div, px};
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};

/// What a `DataTable` keeps after its last column: the kit's 12 px trailing
/// filler and the table's border.
const TRAILING: Pixels = px(14.);

/// The factor a delegate's natural columns are scaled by to fill the table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnFit {
    scale: f32,
}

impl Default for ColumnFit {
    fn default() -> Self {
        Self { scale: 1. }
    }
}

impl ColumnFit {
    /// `column`, at its natural width, scaled to its share of the table.
    pub fn apply(self, column: Column) -> Column {
        let width = (column.width * self.scale).max(column.min_width);
        column.width(width)
    }

    /// Scale `columns` (natural width, floor) to fill `available`. Returns
    /// whether the scale changed, so the table needs its columns again.
    fn fill(&mut self, columns: &[(Pixels, Pixels)], available: Pixels) -> bool {
        let natural: Pixels = columns.iter().map(|(width, _)| *width).sum();
        if natural <= Pixels::ZERO || available <= Pixels::ZERO {
            return false;
        }
        // Columns held at their floor give up no width; the rest share what
        // is left. A few passes settle which columns sit at their floor.
        let mut scale = available / natural;
        for _ in 0..4 {
            let (floors, free) = columns.iter().fold(
                (Pixels::ZERO, Pixels::ZERO),
                |(floors, free), (width, min)| {
                    if *width * scale < *min {
                        (floors + *min, free)
                    } else {
                        (floors, free + *width)
                    }
                },
            );
            if free <= Pixels::ZERO {
                break;
            }
            scale = ((available - floors) / free).max(0.);
        }
        if (scale - self.scale).abs() < 0.001 {
            return false;
        }
        self.scale = scale;
        true
    }
}

/// A table delegate whose columns go through a [`ColumnFit`]: its
/// [`TableDelegate::column`] returns `fit.apply(natural_column(..))`.
pub trait FitColumns: TableDelegate {
    /// The fit the delegate applies to every column.
    fn column_fit(&mut self) -> &mut ColumnFit;

    /// Column `col_ix` at its natural width, before the fit.
    fn natural_column(&self, col_ix: usize, cx: &App) -> Column;
}

/// `table`, drawn from `state`, with its columns stretched or narrowed to fill
/// the table's width. The table fills the returned element, so size that.
pub fn fit_columns<D: FitColumns>(table: DataTable<D>, state: &Entity<TableState<D>>) -> Div {
    let state = state.clone();
    div()
        .size_full()
        .child(table)
        .on_prepaint(move |bounds, _, cx| {
            state.update(cx, |state, cx| {
                let delegate = state.delegate();
                let columns: Vec<_> = (0..delegate.columns_count(cx))
                    .map(|col_ix| {
                        let column = delegate.natural_column(col_ix, cx);
                        (column.width, column.min_width)
                    })
                    .collect();
                let available = bounds.size.width - TRAILING;
                if state.delegate_mut().column_fit().fill(&columns, available) {
                    state.refresh(cx);
                    cx.notify();
                }
            });
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(width: f32) -> Column {
        Column::new("a", "A").width(px(width))
    }

    #[test]
    fn a_fit_scales_the_columns_to_the_width() {
        let columns = [(px(100.), px(20.)), (px(300.), px(20.))];
        let mut fit = ColumnFit::default();
        assert!(fit.fill(&columns, px(600.)));
        assert_eq!(fit.apply(column(100.)).width, px(150.));
        assert!(
            !fit.fill(&columns, px(600.)),
            "an unchanged width keeps the scale"
        );
        assert!(fit.fill(&columns, px(300.)));
        assert_eq!(fit.apply(column(100.)).width, px(75.));
        assert!(
            !fit.fill(&[], px(300.)),
            "a table without columns keeps its scale"
        );
    }

    #[test]
    fn a_column_never_narrows_below_its_floor() {
        // A 120 px stepper beside a 280 px name, in a 300 px table: the
        // stepper keeps its 120 px and the name takes the other 180.
        let columns = [(px(120.), px(120.)), (px(280.), px(20.))];
        let mut fit = ColumnFit::default();
        assert!(fit.fill(&columns, px(300.)));
        let stepper = fit.apply(column(120.).min_width(px(120.)));
        let name = fit.apply(column(280.));
        assert_eq!(stepper.width, px(120.));
        assert!((name.width - px(180.)).abs() < px(0.01), "{:?}", name.width);
    }
}
