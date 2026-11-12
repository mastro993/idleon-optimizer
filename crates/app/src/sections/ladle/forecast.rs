//! The reset layout and the expandable reset instructions.
//!
//! Upstream draws the reset's kitchen cards, then one `<details>` per reset whose
//! summary is its headline and whose body is its spend order. This keeps the same
//! shape with the shared components: a card for the inspected reset's layout and
//! batch, then the forecast list, where each row expands into its phase tables and
//! copies the same instructions as plain text.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::CopyButton;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Disclosure;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::ladle::kitchen::{self, Layout, LayoutMode, Phase, PhaseKind};

/// The reset layout card and the forecast list.
pub fn pane(
    state: &AppState,
    reset: &kitchen::Reset,
    plan: &kitchen::Plan,
    cx: &mut Context<AppState>,
) -> AnyElement {
    div()
        .id("kitchens-forecast")
        .flex()
        .flex_col()
        .gap_3()
        .child(layout_card(state, reset, plan, cx))
        .child(day_list(state, plan, cx))
        .into_any_element()
}

/// The inspected reset's layout, batch and layout controls.
fn layout_card(
    state: &AppState,
    reset: &kitchen::Reset,
    plan: &kitchen::Plan,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let day = state.ladle_state.day.clamp(1, plan.timeline.len().max(1));
    let reset_day = plan.timeline.get(day - 1);
    let mut card = Card::new()
        .header(CardHeader::new("Reset layout").description(
            "Whole kitchens are indivisible · the batch cooks the grant's hours in every assigned \
             kitchen before any purchase",
        ))
        .child(selectors(state, plan, cx))
        .child(layout_controls(state, reset, cx));
    if let Some(layout) = reset_day.and_then(day_layout) {
        card = card.child(kitchen_grid(layout)).child(batch_table(layout));
    } else if let Some(reset_day) = reset_day {
        card = card.child(
            div().px_3().pb_3().child(
                Caption::new(format!(
                    "Reset {} grants {} hours per kitchen; the batch has no stock to buy from.",
                    reset_day.day,
                    format::hours(reset_day.jelly_hours)
                ))
                .text_color(muted),
            ),
        );
    }
    card.into_any_element()
}

/// The reset and assignment-mode selects.
fn selectors(state: &AppState, plan: &kitchen::Plan, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let reset_pick = entity.clone();
    let mode = entity.clone();
    let resets: Vec<Choice> = plan
        .timeline
        .iter()
        .map(|day| Choice::new(day.day.to_string(), format!("Reset {}", day.day)))
        .collect();
    let day = state.ladle_state.day.clamp(1, plan.timeline.len().max(1));
    div()
        .id("kitchens-layout-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .px_3()
        .pb_3()
        .child(
            column("Inspect reset", muted).child(
                Select::new("kitchens-reset-select", resets)
                    .label("Inspect reset")
                    .selected(day.to_string())
                    .on_change(move |value, _window, cx| {
                        if let Ok(day) = value.parse::<usize>() {
                            reset_pick.update(cx, |state, cx| {
                                state.ladle_state.day = day;
                                cx.notify();
                            });
                        }
                    }),
            ),
        )
        .child(
            column("Assignments", muted).child(
                Select::new(
                    "kitchens-layout-mode",
                    [
                        Choice::new("optimized", "Optimizer chooses the meals"),
                        Choice::new("saved", "Keep the saved assignments"),
                    ],
                )
                .label("Assignments")
                .selected(match state.ladle_state.kitchen.layout.mode {
                    LayoutMode::Optimized => "optimized",
                    LayoutMode::Saved => "saved",
                })
                .on_change(move |value, _window, cx| {
                    let choice = match value.as_ref() {
                        "saved" => LayoutMode::Saved,
                        _ => LayoutMode::Optimized,
                    };
                    mode.update(cx, |state, cx| {
                        state
                            .ladle_state
                            .set_kitchen(|inputs| inputs.layout.mode = choice);
                        cx.notify();
                    });
                }),
            ),
        )
        .into_any_element()
}

/// The pin selects and the meal exclusions, behind a toggle.
fn layout_controls(
    state: &AppState,
    reset: &kitchen::Reset,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let toggle = entity.clone();
    let mut controls = div().px_3().pb_3().child(
        Checkbox::new("kitchens-show-layout", state.ladle_state.show_layout)
            .label("Pin kitchens and exclude meals")
            .on_change(move |on, _window, cx| {
                toggle.update(cx, |state, cx| {
                    state.ladle_state.show_layout = on;
                    cx.notify();
                });
            }),
    );
    if !state.ladle_state.show_layout {
        return controls.into_any_element();
    }
    let discovered: Vec<usize> = (0..reset.meals.levels.len())
        .filter(|meal| kitchen::meal_requirement(*meal) > 0.0)
        .filter(|meal| {
            reset
                .meals
                .levels
                .get(*meal)
                .is_some_and(|level| *level >= 1.0)
        })
        .collect();
    let mut grid = div()
        .id("kitchens-pins")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3();
    for kitchen in reset.unlocked() {
        let entity = entity.clone();
        let index = kitchen.index;
        let mut choices = vec![
            Choice::new("auto", "Optimizer chooses"),
            Choice::new("-1", "Keep idle"),
        ];
        choices.extend(
            discovered
                .iter()
                .map(|meal| Choice::new(meal.to_string(), kitchen::meal_name(*meal))),
        );
        let selected = state
            .ladle_state
            .kitchen
            .layout
            .pins
            .get(&index)
            .map_or_else(|| "auto".to_owned(), |meal| meal.to_string());
        grid = grid.child(
            column_owned(format!("Kitchen {}", index + 1), muted).child(
                Select::new(format!("kitchens-pin-{index}"), choices)
                    .label(format!("Kitchen {}", index + 1))
                    .selected(selected)
                    .on_change(move |value, _window, cx| {
                        let meal = match value.as_ref() {
                            "auto" => None,
                            other => other.parse::<i32>().ok(),
                        };
                        entity.update(cx, |state, cx| {
                            state.ladle_state.set_pin(index, meal);
                            cx.notify();
                        });
                    }),
            ),
        );
    }
    let mut meals = div().id("kitchens-exclusions").flex().flex_wrap().gap_2();
    for meal in &discovered {
        let entity = entity.clone();
        let index = *meal;
        let excluded = state.ladle_state.kitchen.layout.excluded.contains(&index);
        meals = meals.child(
            Checkbox::new(format!("kitchens-exclude-{index}"), excluded)
                .label(kitchen::meal_name(index))
                .on_change(move |_, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.ladle_state.toggle_excluded(index);
                        cx.notify();
                    });
                }),
        );
    }
    controls = controls
        .child(grid)
        .child(
            div().px_3().pb_3().child(
                Caption::new(
                    "Exclusions and pins affect the reset assignments only, not direct spending.",
                )
                .text_color(muted),
            ),
        )
        .child(div().px_3().pb_3().child(meals));
    controls.into_any_element()
}

/// The reset's kitchen cards, one per unlocked kitchen.
fn kitchen_grid(layout: &Layout) -> AnyElement {
    let mut grid = div()
        .id("kitchens-layout")
        .flex()
        .flex_wrap()
        .gap_2()
        .px_3()
        .pb_3();
    for kitchen in &layout.kitchens {
        let mut header = div().flex().items_center().gap_2().child(
            div()
                .text_sm()
                .child(format!("Kitchen {}", kitchen.index + 1)),
        );
        if kitchen.pinned {
            header = header.child(Badge::new("pinned").tone(Tone::Warning));
        }
        let mut card = div()
            .id(format!("kitchens-kitchen-{}", kitchen.index))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .min_w(gpui::px(200.0))
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
        grid = grid.child(card);
    }
    grid.into_any_element()
}

/// The meals the batch cooks and the levels their stock buys.
fn batch_table(layout: &Layout) -> AnyElement {
    let mut table = Table::new(["Meal", "Levels", "Stock left", "Progress"]);
    for meal in &layout.meals {
        table = table.row([
            div()
                .child(kitchen::meal_name(meal.meal))
                .into_any_element(),
            div()
                .child(format!(
                    "Lv {} → {}",
                    format::number(meal.from),
                    format::number(meal.to)
                ))
                .into_any_element(),
            div().child(format::number(meal.stock)).into_any_element(),
            div()
                .child(format!(
                    "{}% to Lv {}",
                    (meal.progress * 100.0).round(),
                    format::number(meal.to + 1.0)
                ))
                .into_any_element(),
        ]);
    }
    div()
        .id("kitchens-batch")
        .px_3()
        .pb_3()
        .child(table)
        .into_any_element()
}

/// The forecast: one expandable row per reset.
fn day_list(state: &AppState, plan: &kitchen::Plan, cx: &mut Context<AppState>) -> AnyElement {
    let mut list = div().id("kitchens-days").flex().flex_col();
    for day in &plan.timeline {
        let number = day.day;
        let open = state.ladle_state.open_days.contains(&number);
        let entity = cx.entity();
        let toggle = entity.clone();
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
            })
            .child(Disclosure::new(
                format!("kitchens-day-disclosure-{number}"),
                open,
            ))
            .child(div().flex_1().min_w_0().child(day.headline()));
        let mut row = div()
            .id(format!("kitchens-day-{}", day.day))
            .flex()
            .flex_col()
            .gap_2()
            .py_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(header)
                    .child(CopyButton::new(
                        format!("kitchens-day-copy-{}", day.day),
                        day.instructions(),
                    )),
            );
        if open {
            row = row.child(day_body(day, cx));
        }
        list = list.child(row);
    }
    Card::new()
        .header(CardHeader::new("Daily reset forecast").description(
            "Expand a reset for its spending order and instructions · the copy button takes the \
             same text",
        ))
        .child(list)
        .into_any_element()
}

/// One reset's expanded body: its phases, their spend order and its leftovers.
fn day_body(day: &kitchen::Day, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut body = div()
        .id(format!("kitchens-day-body-{}", day.day))
        .flex()
        .flex_col()
        .gap_2()
        .pl_5();
    for phase in &day.phases {
        body = body.child(phase_block(day.day, phase, muted));
    }
    for saving in &day.savings {
        body = body.child(Caption::new(saving_line(saving, PhaseKind::Claim)).text_color(muted));
    }
    body.into_any_element()
}

/// One phase's heading, spend order and banked time.
fn phase_block(day: u32, phase: &Phase, muted: gpui::Hsla) -> AnyElement {
    let mut block = div()
        .id(format!("kitchens-phase-{day}-{:?}", phase.kind))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child(phase.title(day)));
    if !phase.rows.is_empty() {
        let mut table = Table::new(["#", "Meal (spend order)", "Levels", "Ladles", "Used so far"]);
        for (index, row) in phase.rows.iter().enumerate() {
            table = table.row([
                div().child((index + 1).to_string()).into_any_element(),
                div().child(kitchen::meal_name(row.meal)).into_any_element(),
                div()
                    .child(format!(
                        "Lv {} → {}{}",
                        format::number(row.from),
                        format::number(row.to),
                        match row.levels > 1 {
                            true => format!(" ({})", row.levels),
                            false => String::new(),
                        }
                    ))
                    .into_any_element(),
                div()
                    .child(match phase.kind {
                        PhaseKind::Jelly => "jelly grant".to_owned(),
                        _ => match (row.ladles <= 0.0, row.hours <= 0.0) {
                            (true, false) => "passive".to_owned(),
                            (true, true) => "stock".to_owned(),
                            _ => format::number(row.ladles),
                        },
                    })
                    .into_any_element(),
                div()
                    .child(format::number(row.cumulative_ladles))
                    .into_any_element(),
            ]);
        }
        block = block.child(table);
    }
    if let Some(saving) = &phase.saving {
        block = block.child(Caption::new(saving_line(saving, phase.kind)).text_color(muted));
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

/// A labelled select column.
fn column(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

/// A labelled select column whose label is built at runtime.
fn column_owned(label: String, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}
