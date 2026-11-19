//! The reset layout and the expandable reset instructions.
//!
//! A card for the inspected reset's layout and batch comes first, then the
//! forecast list, where each reset's row expands into its phase tables and
//! copies the same instructions as plain text. The horizon runs to 365 resets,
//! so the list draws only the rows in view: it is a GPUI `list` whose rows are
//! measured as they appear and remeasured when one expands.

use super::State;
use super::kitchens::Figures;
use super::summary::tag;
use super::table::{Col, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Hsla, IntoElement, ParentElement, Styled, Window, div, list, px,
    radians,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, Size};
use idleon_ladle::kitchen::{self, Layout, Phase, PhaseKind};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, Tone};
use std::f32::consts::FRAC_PI_2;

/// The height the forecast list scrolls in.
const DAYS_HEIGHT: f32 = 480.0;

/// The reset layout card and the forecast list.
pub(crate) fn pane(
    state: &State,
    reset: &kitchen::Reset,
    figures: &Figures,
    plan: &kitchen::Plan,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    div()
        .id("kitchens-forecast")
        .flex()
        .flex_col()
        .gap_3()
        .child(layout_card(state, reset, figures, plan, window, cx))
        .child(day_list(state, figures, cx))
        .into_any_element()
}

/// The inspected reset's layout, batch and layout controls.
fn layout_card(
    state: &State,
    reset: &kitchen::Reset,
    figures: &Figures,
    plan: &kitchen::Plan,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let day = state.day.clamp(1, plan.timeline.len().max(1));
    let reset_day = plan.timeline.get(day - 1);
    let mut card = Card::new()
        .header(CardHeader::new("Reset layout").description(
            "Whole kitchens are indivisible · the batch cooks the grant's hours in every assigned \
             kitchen before any purchase",
        ))
        .child(selectors(state, figures, day, window, cx))
        .child(layout_controls(state, reset, figures, window, cx));
    if let Some(layout) = reset_day.and_then(day_layout) {
        card = card
            .child(kitchen_grid(layout, cx))
            .child(batch_table(layout, cx));
    } else if let Some(reset_day) = reset_day {
        card = card.child(Caption::new(format!(
            "Reset {} grants {} hours per kitchen; the batch has no stock to buy from.",
            reset_day.day,
            format::hours(reset_day.jelly_hours)
        )));
    }
    card.into_any_element()
}

/// The reset and assignment-mode selects.
fn selectors(
    state: &State,
    figures: &Figures,
    day: usize,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let controls = &state.controls;
    controls
        .reset
        .show(figures.resets.clone(), Some(&day), window, cx);
    controls.layout_mode.show(
        super::controls::layout_mode_choices(),
        Some(&state.kitchen.layout.mode),
        window,
        cx,
    );
    div()
        .id("kitchens-layout-controls")
        .flex()
        .items_end()
        .gap_3()
        .child(
            super::field("Inspect reset").child(
                div().flex_none().w(px(160.0)).child(
                    Select::new(controls.reset.state())
                        .id("kitchens-reset-select")
                        .accessibility_label("Inspect reset")
                        .small(),
                ),
            ),
        )
        .child(
            super::field("Assignments").child(
                div().flex_none().w(px(260.0)).child(
                    Select::new(controls.layout_mode.state())
                        .id("kitchens-layout-mode")
                        .accessibility_label("Assignments")
                        .small(),
                ),
            ),
        )
        .into_any_element()
}

/// The pin selects and the meal exclusions, behind a toggle.
fn layout_controls(
    state: &State,
    reset: &kitchen::Reset,
    figures: &Figures,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let toggle = cx.entity();
    let mut controls = div().flex().flex_col().gap_3().child(
        Checkbox::new("kitchens-show-layout")
            .label("Pin kitchens and exclude meals")
            .checked(state.show_layout)
            .on_click(move |on, _window, cx| {
                let on = *on;
                toggle.update(cx, |state, cx| {
                    state.ladle_state.show_layout = on;
                    cx.notify();
                });
            }),
    );
    if !state.show_layout {
        return controls.into_any_element();
    }
    // Rows of four never wrap (`docs/ui-kit/display.md`).
    let mut grid = div().id("kitchens-pins").flex().flex_col().gap_2();
    let kitchens: Vec<_> = reset.unlocked().collect();
    for chunk in kitchens.chunks(4) {
        let mut row = div().flex().items_end().gap_3();
        for kitchen in chunk {
            let index = kitchen.index;
            let Some(select) = state.controls.pins.get(index) else {
                continue;
            };
            let pin = state.kitchen.layout.pins.get(&index).copied();
            select.show(figures.pins.clone(), Some(&pin), window, cx);
            row = row.child(
                super::field(format!("Kitchen {}", index + 1)).child(
                    div().flex_none().w(px(200.0)).child(
                        Select::new(select.state())
                            .id(format!("kitchens-pin-{index}"))
                            .accessibility_label(format!("Kitchen {}", index + 1))
                            .small(),
                    ),
                ),
            );
        }
        grid = grid.child(row);
    }
    let mut meals = div().id("kitchens-exclusions").flex().flex_col().gap_2();
    for chunk in figures.discovered.chunks(6) {
        let mut row = div().flex().gap_4();
        for meal in chunk {
            let entity = cx.entity();
            let index = *meal;
            row = row.child(
                div().w(px(150.0)).child(
                    Checkbox::new(format!("kitchens-exclude-{index}"))
                        .label(kitchen::meal_name(index))
                        .checked(state.kitchen.layout.excluded.contains(&index))
                        .on_click(move |_, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.ladle_state.toggle_excluded(index);
                                cx.notify();
                            });
                        }),
                ),
            );
        }
        meals = meals.child(row);
    }
    controls = controls
        .child(grid)
        .child(Caption::new(
            "Exclusions and pins affect the reset assignments only, not direct spending.",
        ))
        .child(meals);
    controls.into_any_element()
}

/// The reset's kitchen cards, one per unlocked kitchen, four to a row.
fn kitchen_grid(layout: &Layout, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let mut grid = div().id("kitchens-layout").flex().flex_col().gap_2();
    for chunk in layout.kitchens.chunks(4) {
        let mut row = div().flex().gap_2();
        for kitchen in chunk {
            let mut header = div().flex().items_center().gap_2().child(
                div()
                    .text_size(TEXT_BODY)
                    .child(format!("Kitchen {}", kitchen.index + 1)),
            );
            if kitchen.pinned {
                header = header.child(tag("pinned", Tone::Warning));
            }
            let mut card = div()
                .id(format!("kitchens-kitchen-{}", kitchen.index))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .w(px(220.0))
                .child(header)
                .child(Caption::new(format!(
                    "{} cooking speed/hr",
                    format::number(kitchen.speed)
                )))
                .child(div().child(meal_label(kitchen.meal)));
            if kitchen.meal >= 0 {
                card = card.child(Caption::new(format!(
                    "Produces {} stock",
                    format::number(kitchen.produced)
                )));
                if let Some(meal) = layout
                    .meals
                    .iter()
                    .find(|meal| meal.meal == kitchen.meal as usize)
                {
                    card = card.child(Caption::new(match meal.to > meal.from {
                        true => format!(
                            "Combined result: Lv {} → {} (+{})",
                            format::number(meal.from),
                            format::number(meal.to),
                            format::number(meal.to - meal.from)
                        ),
                        false => format!(
                            "Combined result: {}% toward Lv {}",
                            (meal.progress * 100.0).round(),
                            format::number(meal.to + 1.0)
                        ),
                    }));
                }
            }
            card = card.child(Caption::new(format!(
                "Saved: {}",
                meal_label(kitchen.saved_meal)
            )));
            row = row.child(card);
        }
        grid = grid.child(row);
    }
    grid.into_any_element()
}

/// The meals the batch cooks and the levels their stock buys.
fn batch_table(layout: &Layout, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("Meal"),
        Col::Text("Levels"),
        Col::Number("Stock left"),
        Col::Text("Progress"),
    ];
    let rows = layout.meals.iter().map(|meal| {
        vec![
            text(kitchen::meal_name(meal.meal)),
            text(format!(
                "Lv {} → {}",
                format::number(meal.from),
                format::number(meal.to)
            )),
            text(format::number(meal.stock)),
            text(format!(
                "{}% to Lv {}",
                (meal.progress * 100.0).round(),
                format::number(meal.to + 1.0)
            )),
        ]
    });
    table("kitchens-batch", &cols, rows, cx).into_any_element()
}

/// The forecast: one expandable row per reset, drawn only while in view.
fn day_list(state: &State, figures: &Figures, cx: &mut Context<AppState>) -> AnyElement {
    let days = state.controls.days_list.clone();
    let forecast = figures.forecast.clone();
    let scenario = state.scenario;
    let open = state.open_days.clone();
    let entity = cx.entity();
    let items = days.clone();
    let rows = list(days.clone(), move |ix, _window, cx| {
        let Ok(forecast) = forecast.as_ref() else {
            return div().into_any_element();
        };
        let Some(day) = forecast.plan(scenario).timeline.get(ix) else {
            return div().into_any_element();
        };
        day_row(day, ix, open.contains(&day.day), &entity, &items, cx)
    })
    .size_full();
    Card::new()
        .header(CardHeader::new("Daily reset forecast").description(
            "Expand a reset for its spending order and instructions · the copy button takes the \
             same text",
        ))
        .child(
            div()
                .id("kitchens-days")
                .relative()
                .h(px(DAYS_HEIGHT))
                .child(rows)
                .vertical_scrollbar(&days),
        )
        .into_any_element()
}

/// One reset's row: its headline, its copy button and, when open, its body.
fn day_row(
    day: &kitchen::Day,
    ix: usize,
    open: bool,
    entity: &gpui::Entity<AppState>,
    items: &gpui::ListState,
    cx: &mut App,
) -> AnyElement {
    let number = day.day;
    let muted = cx.theme().muted_foreground;
    let toggle = entity.clone();
    let remeasure = items.clone();
    let header = div()
        .id(format!("kitchens-day-header-{number}"))
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .on_click(move |_event, _window, cx| {
            toggle.update(cx, |state, cx| {
                toggle_day(&mut state.ladle_state.open_days, number);
                cx.notify();
            });
            remeasure.remeasure_items(ix..ix + 1);
        })
        .child(
            div().id(format!("kitchens-day-disclosure-{number}")).child(
                Icon::new(IconName::ChevronRight)
                    .with_size(Size::Small)
                    .text_color(muted)
                    .rotate(radians(if open { FRAC_PI_2 } else { 0.0 })),
            ),
        )
        .child(div().flex_1().min_w_0().child(day.headline()));
    let mut row = div()
        .id(format!("kitchens-day-{number}"))
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .py_2()
        .pr_3()
        .child(
            div().flex().items_center().gap_2().child(header).child(
                Clipboard::new(format!("kitchens-day-copy-{number}"))
                    .value(day.instructions())
                    .tooltip("Copy the instructions"),
            ),
        );
    if open {
        row = row.child(day_body(day, muted, cx));
    }
    row.into_any_element()
}

/// One reset's expanded body: its phases, their spend order and its leftovers.
fn day_body(day: &kitchen::Day, muted: Hsla, cx: &App) -> AnyElement {
    let mut body = div()
        .id(format!("kitchens-day-body-{}", day.day))
        .flex()
        .flex_col()
        .gap_2()
        .pl_5();
    for phase in &day.phases {
        body = body.child(phase_block(day.day, phase, cx));
    }
    for saving in &day.savings {
        body = body.child(Caption::new(saving_line(saving, PhaseKind::Claim)).text_color(muted));
    }
    body.into_any_element()
}

/// One phase's heading, spend order and banked time.
fn phase_block(day: u32, phase: &Phase, cx: &App) -> AnyElement {
    let id = format!("kitchens-phase-{day}-{:?}", phase.kind);
    let mut block = div()
        .id(id.clone())
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(TEXT_BODY).child(phase.title(day)));
    if !phase.rows.is_empty() {
        let cols = [
            Col::Number("#"),
            Col::Text("Meal (spend order)"),
            Col::Text("Levels"),
            Col::Number("Ladles"),
            Col::Number("Used so far"),
        ];
        let rows = phase.rows.iter().enumerate().map(|(index, row)| {
            vec![
                text((index + 1).to_string()),
                text(kitchen::meal_name(row.meal)),
                text(format!(
                    "Lv {} → {}{}",
                    format::number(row.from),
                    format::number(row.to),
                    match row.levels > 1 {
                        true => format!(" ({})", row.levels),
                        false => String::new(),
                    }
                )),
                text(match phase.kind {
                    PhaseKind::Jelly => "jelly grant".to_owned(),
                    _ => match (row.ladles <= 0.0, row.hours <= 0.0) {
                        (true, false) => "passive".to_owned(),
                        (true, true) => "stock".to_owned(),
                        _ => format::number(row.ladles),
                    },
                }),
                text(format::number(row.cumulative_ladles)),
            ]
        });
        block = block.child(table(format!("{id}-table"), &cols, rows, cx));
    }
    if let Some(saving) = &phase.saving {
        block = block.child(Caption::new(saving_line(saving, phase.kind)));
    }
    block.into_any_element()
}

/// The banked-time sentence a phase and a day both carry.
fn saving_line(saving: &kitchen::Saving, kind: PhaseKind) -> String {
    let what = match kind {
        PhaseKind::Jelly => "Leftover jellyfish time".to_owned(),
        _ => match (saving.ladles > 0.0, saving.hours > 0.0) {
            (true, _) => format!("Leftover {} ladles", format::number(saving.ladles)),
            (false, true) => "Leftover passive time".to_owned(),
            (false, false) => "Leftover time".to_owned(),
        },
    };
    format!(
        "{what} banks into {} toward Lv {} ({}% stocked){}",
        kitchen::meal_name(saving.meal),
        format::number(saving.to_level),
        (saving.progress * 100.0).round(),
        match saving.map_refresh {
            true => " · change map before spending more ladles",
            false => "",
        }
    )
}

/// The layout of one reset, from whichever phase cooked one.
pub(crate) fn day_layout(day: &kitchen::Day) -> Option<&Layout> {
    day.phases.iter().find_map(|phase| phase.layout.as_ref())
}

/// A kitchen's meal name, or `Idle`.
fn meal_label(meal: i32) -> String {
    match meal >= 0 {
        true => kitchen::meal_name(meal as usize).to_owned(),
        false => "Idle / recipe fire".to_owned(),
    }
}

/// Expand or collapse one reset's instructions, keeping the list in day order.
pub(crate) fn toggle_day(open: &mut Vec<u32>, day: u32) {
    match open.iter().position(|entry| *entry == day) {
        Some(position) => {
            open.remove(position);
        }
        None => {
            open.push(day);
            open.sort_unstable();
        }
    }
}
