//! The kit table pieces the fountain tabs share.

use gpui::prelude::*;
use gpui::{IntoElement, ParentElement, Pixels, SharedString, Styled, div, px};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use idleon_ui::widgets::Tone;

/// A kit table with these `(title, numeric)` columns over `rows`. Number
/// columns sit right-aligned, as `docs/ui-kit/data.md` sets them.
pub(super) fn table(
    columns: &[(&'static str, bool)],
    rows: impl IntoIterator<Item = TableRow>,
) -> Table {
    Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(columns.iter().map(
                |(title, numeric)| {
                    TableHead::new()
                        .when(*numeric, TableHead::text_right)
                        .child(*title)
                },
            ))),
        )
        .child(TableBody::new().children(rows))
}

/// A cell whose content wraps inside its column.
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

/// A kit table row's height, with its hairline.
const ROW_HEIGHT: f32 = 33.0;

/// The most rows a virtualized table shows before it scrolls.
const VISIBLE_ROWS: usize = 12;

/// The height of a virtualized table of `rows` rows: sized to them up to a
/// dozen, then it scrolls.
pub(super) fn data_height(rows: usize) -> Pixels {
    px(ROW_HEIGHT * (rows.min(VISIBLE_ROWS) + 1) as f32 + 2.0)
}
