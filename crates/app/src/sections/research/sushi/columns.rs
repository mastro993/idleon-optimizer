//! Kit table columns with a fixed or shared width, for the Sushi and
//! Minehead tables.
//!
//! A kit `Table` gives every column the same share of its width; these tables
//! pin their narrow figure columns and let the names and descriptions share
//! the rest, as Ely's tables sized them.

use gpui::prelude::*;
use gpui::{Styled, px};
use gpui_kit::component::table::{TableCell, TableHead, TableHeader, TableRow};

/// One column: its title, a fixed width in pixels (`None` shares the rest)
/// and whether it holds numbers, which sit right-aligned.
pub(crate) type Column = (&'static str, Option<f32>, bool);

/// The header row naming `columns`.
pub(crate) fn header(columns: &[Column]) -> TableHeader {
    TableHeader::new().child(TableRow::new().children(columns.iter().map(
        |(title, width, number)| {
            TableHead::new()
                .when_some(*width, |head, width| head.w(px(width)).min_w(px(width)))
                .when(*number, TableHead::text_right)
                .child(*title)
        },
    )))
}

/// A cell of column `index`, sized and aligned like its head. A shared
/// column wraps its text inside its share instead of spilling into the next.
pub(crate) fn cell(columns: &[Column], index: usize) -> TableCell {
    let (_, width, number) = columns[index];
    TableCell::new()
        .map(|cell| match width {
            Some(width) => cell.w(px(width)).min_w(px(width)),
            None => cell.overflow_hidden().whitespace_normal(),
        })
        .when(number, TableCell::text_right)
}
