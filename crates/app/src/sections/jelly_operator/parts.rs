//! The builders every tab shares: KPI figures and strips, number fields, the
//! run button, the info banner and the small text tables.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Div, Entity, IntoElement, ParentElement, Pixels, SharedString, Styled,
    Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputState, NumberInput};
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::v_flex;
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_ui::format;
use idleon_ui::widgets::{Choice, ChoiceSelect, KpiStrip, PANEL_GAP, Statistic, Tone, show_number};

/// How many figures one KPI strip holds before the next row starts.
const STRIP_CELLS: usize = 4;
/// Kit fields fill their parent, so a wrapping row of controls boxes each one:
/// numbers sized to a ten-digit seed and its steppers, selects to a label.
const NUMBER_WIDTH: Pixels = px(160.);
pub(crate) const SELECT_WIDTH: Pixels = px(240.);

/// A number with a game suffix (`1.23M`), as the KPI cells read.
pub(crate) fn stat(id: &'static str, label: &'static str, value: f64) -> Statistic {
    let (scaled, suffix) = format::scaled(value);
    let decimals = if suffix.is_empty() { 1 } else { 2 };
    let figure = figure(id, label, scaled, decimals);
    if suffix.is_empty() {
        figure
    } else {
        figure.suffix(suffix)
    }
}

/// A plain number to `decimals` places; whole numbers group their thousands.
pub(crate) fn figure(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
) -> Statistic {
    let text = if decimals == 0 {
        format::exact(value)
    } else {
        format!("{value:.decimals$}")
    };
    Statistic::new(id, label, text)
}

/// A headline that is not a number, with its caption.
pub(crate) fn text_kpi(
    id: &'static str,
    label: impl Into<String>,
    value: String,
    caption: String,
) -> Statistic {
    Statistic::new(id, label.into(), value).caption(caption)
}

/// The figures as KPI strips of at most [`STRIP_CELLS`] cells each.
pub(crate) fn strips(id: &'static str, figures: Vec<Statistic>) -> AnyElement {
    let mut figures = figures.into_iter().peekable();
    let mut rows = Vec::new();
    while figures.peek().is_some() {
        rows.push(KpiStrip::new().children(figures.by_ref().take(STRIP_CELLS)));
    }
    v_flex()
        .id(id)
        .gap(PANEL_GAP)
        .children(rows)
        .into_any_element()
}

/// A kit number field showing `value`, under the old control's id.
pub(crate) fn number_field(
    id: &'static str,
    label: &'static str,
    field: &Entity<InputState>,
    value: f64,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    show_number(field, value, 0, window, cx);
    div()
        .id(id)
        .flex_none()
        .w(NUMBER_WIDTH)
        .h_6()
        .child(NumberInput::new(field).accessibility_label(label).small())
        .into_any_element()
}

/// A kit select under the old control's id; [`boxed`] gives it a place in a row.
pub(crate) fn select<V: Clone + PartialEq + 'static>(
    control: &ChoiceSelect<V>,
    id: &'static str,
    label: &'static str,
) -> Select<Vec<Choice<V>>> {
    Select::new(control.state())
        .id(id)
        .accessibility_label(label)
        .small()
}

/// A kit field in a box of its own: the field's root fills its parent, so in a
/// wrapping row it needs a fixed width and the small control height.
pub(crate) fn boxed(width: Pixels, field: impl IntoElement) -> Div {
    div().flex_none().w(width).h_6().child(field)
}

/// A primary run button.
pub(crate) fn run_button(
    id: &'static str,
    label: &'static str,
    running: bool,
    on_run: impl Fn(&mut AppState, &mut Context<AppState>) + 'static,
    cx: &mut Context<AppState>,
) -> Button {
    let entity = cx.entity();
    Button::new(id)
        .label(label)
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| on_run(state, cx));
        })
}

/// An informational banner.
pub(crate) fn info(id: &'static str, message: impl Into<SharedString>) -> Alert {
    Alert::new(id, message.into())
        .with_variant(Tone::Info.into())
        .banner()
}

/// A table column: its head and whether it holds numbers (right-aligned).
pub(crate) type Column = (&'static str, bool);

/// A small table of text cells, for the bounded tables (`docs/ui-kit/data.md`):
/// a cell type each, a few obstructions, the finalists.
pub(crate) fn text_table(columns: &[Column], rows: impl IntoIterator<Item = Vec<String>>) -> Table {
    let head = |(name, numeric): &Column| {
        let head = TableHead::new();
        if *numeric { head.text_right() } else { head }.child(*name)
    };
    let columns = columns.to_vec();
    Table::new()
        .child(TableHeader::new().child(TableRow::new().children(columns.iter().map(head))))
        .child(
            TableBody::new().children(rows.into_iter().map(move |cells| {
                TableRow::new().children(cells.into_iter().zip(&columns).map(
                    |(text, (_, numeric))| {
                        if *numeric {
                            number_cell(text)
                        } else {
                            cell(text)
                        }
                    },
                ))
            })),
        )
}

/// The width of a table's `#` column.
pub(crate) const INDEX_WIDTH: Pixels = px(56.);

/// A table cell whose content wraps inside its column instead of running into
/// the next one.
pub(crate) fn cell(content: impl IntoElement) -> TableCell {
    TableCell::new().child(div().flex_1().min_w_0().child(content))
}

/// A right-aligned number cell.
pub(crate) fn number_cell(content: impl IntoElement) -> TableCell {
    TableCell::new()
        .text_right()
        .child(div().min_w_0().child(content))
}
