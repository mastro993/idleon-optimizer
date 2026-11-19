//! Kit `Table` columns with a pinned or a shared width.
//!
//! A kit `Table` gives every column the same share of its width, so narrow
//! figure columns waste room while names and long heads crowd, wrap or clip.
//! A table built from [`TableColumn`]s pins its figure columns (`#`, a level,
//! a short number) and lets the text columns share the rest. Every head and
//! cell wraps its text inside its column instead of spilling into the next
//! one. A kit cell lays its content out in a row, where text keeps its full
//! width, so these cells stack their content in a column instead, aligned to
//! the start or, for numbers, the end. See `docs/ui-kit/data.md`.

use gpui::{ParentElement as _, SharedString, Styled, px};
use gpui_kit::component::table::{TableCell, TableHead, TableHeader, TableRow};

/// The narrowest a shared column gets: below the kit's 100 px, so a wide
/// table's shared columns still fit beside its pinned ones.
const SHARED_MIN: f32 = 64.;

/// One column of a kit `Table`: its head, its width and its alignment.
#[derive(Clone)]
pub struct TableColumn {
    title: SharedString,
    /// Pinned width in pixels; `None` shares what the pinned columns leave.
    width: Option<f32>,
    /// A number column sits right-aligned (`docs/ui-kit/data.md`).
    number: bool,
}

impl TableColumn {
    /// A left-aligned column sharing the width the pinned columns leave.
    pub const fn text(title: &'static str) -> Self {
        Self {
            title: SharedString::new_static(title),
            width: None,
            number: false,
        }
    }

    /// A right-aligned number column sharing the width the pinned columns leave.
    pub const fn number(title: &'static str) -> Self {
        Self::text(title).numeric()
    }

    /// A left-aligned column whose head is built at run time.
    pub fn titled(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            width: None,
            number: false,
        }
    }

    /// Right-align the column.
    pub const fn numeric(mut self) -> Self {
        self.number = true;
        self
    }

    /// Pin the column to `width` pixels.
    pub const fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Whether the column holds numbers.
    pub fn is_number(&self) -> bool {
        self.number
    }

    /// The column's head.
    pub fn head(&self) -> TableHead {
        self.layout(TableHead::new()).child(self.title.clone())
    }

    /// A body cell, sized and aligned like the head.
    pub fn cell(&self) -> TableCell {
        self.apply(TableCell::new())
    }

    /// Size and align `cell`, built elsewhere, like the column's own cells.
    pub fn apply(&self, cell: TableCell) -> TableCell {
        self.layout(cell)
    }

    /// Size `cell` to the column and stack its content in a column, where
    /// text wraps at the cell's width.
    fn layout<E: Styled>(&self, cell: E) -> E {
        let cell = match self.width {
            Some(width) => cell.w(px(width)).min_w(px(width)),
            None => cell.min_w(px(SHARED_MIN)),
        };
        let cell = cell.overflow_hidden().flex_col().justify_center();
        if self.number {
            cell.items_end()
        } else {
            cell.items_start()
        }
    }
}

/// The header row naming `columns`.
pub fn table_header(columns: &[TableColumn]) -> TableHeader {
    TableHeader::new().child(TableRow::new().children(columns.iter().map(TableColumn::head)))
}
