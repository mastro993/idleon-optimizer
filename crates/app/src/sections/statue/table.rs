//! The statue table: a virtualized kit `DataTable`, so a frame draws only the
//! rows in view with their target and rate fields (`docs/ui-kit/data.md`).
//!
//! [`super::State::sync`] hands it the rows when the save, the plan, the
//! search or the sort change; render only names the table.

use super::{Row, show_text};
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, div, px,
};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::progress::Progress as ProgressBar;
use gpui_kit::component::table::{Column, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, v_flex};
use idleon_statue::Estimate;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, TEXT_BODY, TEXT_MONO, show_number};
use std::rc::Rc;

/// The statue rows the table shows and the fields their cells draw.
pub(crate) struct Rows {
    /// Every statue, in game order.
    pub(super) all: Rc<[Row]>,
    /// Indices into `all`, in the order the table lists them.
    pub(super) shown: Rc<[usize]>,
    pub(super) estimate: Estimate,
    pub(super) flair_available: bool,
    pub(super) no_save: bool,
    /// One target and one rate field per statue, owned by the section state.
    pub(super) targets: Vec<Entity<InputState>>,
    pub(super) rates: Vec<Entity<InputState>>,
}

impl Rows {
    pub(super) fn new(targets: Vec<Entity<InputState>>, rates: Vec<Entity<InputState>>) -> Self {
        Self {
            all: Rc::from([]),
            shown: Rc::from([]),
            estimate: Estimate::Guaranteed,
            flair_available: false,
            no_save: true,
            targets,
            rates,
        }
    }

    fn row(&self, row_ix: usize) -> Option<&Row> {
        self.all.get(*self.shown.get(row_ix)?)
    }
}

/// A table state over the statue rows: no selection, sorting or column moves,
/// as the Ely table had none.
pub(super) fn state(rows: Rows, window: &mut Window, cx: &mut App) -> Entity<TableState<Rows>> {
    cx.new(|cx| {
        TableState::new(rows, window, cx)
            .row_selectable(false)
            .col_selectable(false)
            .col_movable(false)
            .sortable(false)
    })
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &App) -> usize {
        8
    }

    fn rows_count(&self, _: &App) -> usize {
        self.shown.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("statue", "Statue / effect").width(px(190.0)),
            1 => Column::new("level", "Level / progress").width(px(170.0)),
            2 => Column::new("availability", "Availability").width(px(130.0)),
            3 => Column::new("target", "Target").width(px(130.0)),
            4 => Column::new("exp", "EXP per statue")
                .width(px(120.0))
                .text_right(),
            5 => Column::new("needed", "Statues needed")
                .width(px(120.0))
                .text_right(),
            6 => Column::new("rate", "Raw statues/hr").width(px(130.0)),
            _ => Column::new("time", "Time").width(px(110.0)),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.row(row_ix) else {
            return div().into_any_element();
        };
        match col_ix {
            0 => statue_cell(row),
            1 => level_cell(row),
            2 => availability_cell(row, self.no_save),
            3 => target_cell(self.targets.get(row.index), row, window, cx),
            4 => exp_cell(row, self.estimate, self.flair_available),
            5 => needed_cell(row),
            6 => rate_cell(self.rates.get(row.index), row, window, cx),
            _ => time_cell(row),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.row(row_ix)?;
        Some(match col_ix {
            0 => row.info.name.clone().into(),
            1 => row
                .progress
                .map(|progress| format!("Lv {}", progress.level))
                .unwrap_or_default()
                .into(),
            3 => row
                .target
                .map(|target| target.to_string())
                .unwrap_or_default()
                .into(),
            6 => row.rate_text.clone().into(),
            _ => return None,
        })
    }
}

fn statue_cell(row: &Row) -> AnyElement {
    v_flex()
        .id(SharedString::from(format!("statue-row-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{} · {}", row.info.name, row.info.effect))
        .gap_0p5()
        .child(
            div()
                .text_size(TEXT_BODY)
                .font_weight(FontWeight::SEMIBOLD)
                .child(row.info.name.clone()),
        )
        .child(Caption::new(row.info.effect.clone()))
        .into_any_element()
}

fn level_cell(row: &Row) -> AnyElement {
    let Some(progress) = row.progress else {
        return unavailable_cell();
    };
    let level = div()
        .text_size(TEXT_BODY)
        .font_weight(FontWeight::MEDIUM)
        .child(format!("Lv {}", progress.level));
    let Ok(next) = &row.next else {
        return v_flex()
            .id(SharedString::from(format!("statue-level-{}", row.index)))
            .role(Role::Label)
            .aria_label(format!(
                "Level {} · requirement unavailable",
                progress.level
            ))
            .gap_0p5()
            .child(level)
            .child(Caption::new("Requirement unavailable"))
            .into_any_element();
    };
    let required = next.required;
    let share = if required > 0.0 {
        (progress.exp / required).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    v_flex()
        .id(SharedString::from(format!("statue-level-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!(
            "Level {} · {:.1}% · {} of {} EXP",
            progress.level,
            share * 100.0,
            format::rate(progress.exp),
            format::rate(required)
        ))
        .gap_1()
        .child(level)
        .child(
            div()
                .w(px(96.0))
                .child(ProgressBar::new(("statue-progress", row.index)).value(share * 100.0)),
        )
        .child(Caption::new(format!(
            "{:.1}% · {} / {}",
            share * 100.0,
            format::rate(progress.exp),
            format::rate(required)
        )))
        .into_any_element()
}

fn target_cell(
    field: Option<&Entity<InputState>>,
    row: &Row,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (Some(target), Some(field)) = (row.target, field) else {
        return unavailable_cell();
    };
    show_number(field, f64::from(target), 0, window, cx);
    number_field(
        ("statue-target-field", row.index),
        field,
        format!("{} target level", row.info.name),
    )
    .into_any_element()
}

fn exp_cell(row: &Row, estimate: Estimate, flair_available: bool) -> AnyElement {
    let Ok(cost) = &row.cost else {
        return unavailable_cell();
    };
    let deposit = estimate.deposit(cost);
    let flair = if flair_available {
        format!(
            "Flair {} · {}x",
            cost.flair_level,
            format::fmt_val(cost.flair_multiplier)
        )
    } else {
        "Flair unknown".to_string()
    };
    v_flex()
        .id(SharedString::from(format!("statue-exp-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!(
            "{} EXP per statue, {flair}",
            format::fmt_val(deposit)
        ))
        .items_end()
        .gap_0p5()
        .child(div().text_size(TEXT_MONO).child(format::fmt_val(deposit)))
        .child(Caption::new(flair))
        .into_any_element()
}

fn needed_cell(row: &Row) -> AnyElement {
    let Some(statues) = row.statues else {
        return unavailable_cell();
    };
    div()
        .id(SharedString::from(format!("statue-needed-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{statues} statues needed"))
        .text_size(TEXT_MONO)
        .child(format::fmt_val(statues as f64))
        .into_any_element()
}

fn rate_cell(
    input: Option<&Entity<InputState>>,
    row: &Row,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(input) = input else {
        return unavailable_cell();
    };
    show_text(input, &row.rate_text, window, cx);
    let theme = cx.theme();
    let (danger, radius) = (theme.danger, theme.radius);
    let field = input.clone();
    div()
        .id(SharedString::from(format!("statue-rate-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("{} raw statues per hour", row.info.name))
        .w(px(112.0))
        .when(row.rate.is_err(), |cell| {
            cell.border_1().border_color(danger).rounded(radius)
        })
        .on_click(move |_event, window, cx| {
            field.update(cx, |field, cx| field.focus(window, cx));
        })
        .child(
            Input::new(input)
                .aria_label(format!("{} raw statues per hour", row.info.name))
                .small(),
        )
        .into_any_element()
}

fn time_cell(row: &Row) -> AnyElement {
    let label = if row.statues.is_none() || row.rate.is_err() {
        "—".to_string()
    } else {
        format::statue_time(row.hours)
    };
    div()
        .id(SharedString::from(format!("statue-time-{}", row.index)))
        .role(Role::Label)
        .aria_label(format!("Time to target: {label}"))
        .child(label)
        .into_any_element()
}

fn availability_cell(row: &Row, no_save: bool) -> AnyElement {
    let (label, tone) = row.availability(no_save);
    div()
        .id(SharedString::from(format!(
            "statue-availability-{}",
            row.index
        )))
        .role(Role::Label)
        .aria_label(label.clone())
        .flex()
        .child(
            Tag::new()
                .with_variant(tone.into())
                .outline()
                .small()
                .child(label),
        )
        .into_any_element()
}

fn unavailable_cell() -> AnyElement {
    div().child("—").into_any_element()
}
