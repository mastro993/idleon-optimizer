//! Right rail: the selected square, the shape list and the magnifier pool.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{App, Context, ElementId, Hsla, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_research::game;
use idleon_research::model::{MagKind, coord};
use idleon_research::{shape, sim};
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

pub(crate) fn inspector_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    div()
        .id("inspector")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(square_card(state, cx))
        .child(shapes_card(state, cx))
        .child(magnifiers_card(state, cx))
}

fn square_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = cx.theme();
    let selected = state.research.selected_cell;
    let node = state.selected_node();
    let card = Card::new().header(CardHeader::new("Selected square"));

    let Some(node) = node else {
        return card.child(Caption::new(
            "Pick a square on the grid to see what it does.",
        ));
    };
    let cell = selected.unwrap_or(node.id);
    let level = state.simulation.grid.level(cell);
    let bonus = sim::grid_bonus(
        &state.simulation.grid,
        &state.simulation.overlay(),
        cell,
        &state.context,
    );
    let id = cx.entity();

    let bump = {
        let id = id.clone();
        move |delta: i64| {
            let id = id.clone();
            move |_event: &gpui::ClickEvent, _window: &mut Window, cx: &mut gpui::App| {
                id.update(cx, |state, cx| state.bump_level(cell, delta, cx));
            }
        }
    };

    card.child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("square-name")
                                    .role(gpui::Role::Label)
                                    .aria_label(format!(
                                        "{} · {}",
                                        node.name.replace('_', " "),
                                        coord(cell)
                                    ))
                                    .text_size(px(14.0))
                                    .child(format!(
                                        "{} · {}",
                                        node.name.replace('_', " "),
                                        coord(cell)
                                    )),
                            )
                            .child(
                                div()
                                    .id("square-level")
                                    .role(gpui::Role::Label)
                                    .aria_label(format!("Level {level} / {}", node.max_lv))
                                    .child(Caption::new(format!(
                                        "Level {level} / {}",
                                        node.max_lv
                                    ))),
                            ),
                    )
                    .child(
                        Tag::new()
                            .with_variant(Tone::Neutral.into())
                            .outline()
                            .small()
                            .child(node.goal.clone().unwrap_or_else(|| "Square".into())),
                    ),
            )
            .child(Caption::new(node.desc.replace('_', " ")).text_color(colors.muted_foreground))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                icon_button("square-minus", IconName::Minus)
                                    .tooltip("Lower this square")
                                    .disabled(level == 0)
                                    .on_click(bump(-1)),
                            )
                            .child(
                                div()
                                    .id("square-level-inline")
                                    .child(format!("Level {level} / {}", node.max_lv)),
                            )
                            .child(
                                icon_button("square-plus", IconName::Plus)
                                    .tooltip("Level this square")
                                    .disabled(level >= node.max_lv)
                                    .on_click(bump(1)),
                            ),
                    )
                    .child(Caption::new(format!(
                        "+{:.1}% now · {:.1} per level",
                        bonus, node.bonus_per_lv
                    ))),
            ),
    )
}

fn shapes_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = cx.theme();
    let owned = state.shapes_owned();
    let placed = state.simulation.placements.len();
    let entity = cx.entity();
    let add = entity.clone();
    let clear = entity.clone();
    let card = Card::new().header(
        CardHeader::new("Shapes")
            .description(format!("{placed} of {owned} unlocked placed"))
            .action(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("shape-add")
                            .label("Add shape")
                            .small()
                            .icon(IconName::Plus)
                            .disabled(placed >= owned)
                            .on_click(move |_event, _window, cx| {
                                add.update(cx, |state, cx| state.add_shape(cx));
                            }),
                    )
                    .child(Button::new("layout-reset").label("Clear").small().on_click(
                        move |_event, _window, cx| {
                            clear.update(cx, |state, cx| state.reset_layout(cx));
                        },
                    )),
            ),
    );

    if placed == 0 {
        return card.child(Caption::new(
            "No shapes yet. Add one, or run the optimizer to place them for you.",
        ));
    }

    let overlay = state.simulation.overlay();
    let names: Vec<(usize, String, f64, usize, String)> = state
        .simulation
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let def = &game().shapes[placement.shape];
            let covered = shape::cells_at(placement)
                .into_iter()
                .filter(|cell| overlay[*cell] == placement.shape as i32)
                .count();
            let anchor = shape::cells_at(placement)
                .into_iter()
                .min()
                .map(coord)
                .unwrap_or_default();
            (index, def.name.clone(), def.bonus_pct, covered, anchor)
        })
        .collect();

    let mut list = div().flex().flex_col().gap_1();
    for (index, name, bonus, covered, anchor) in names {
        let rotate_left = entity.clone();
        let rotate_right = entity.clone();
        let remove = entity.clone();
        let left = entity.clone();
        let right = entity.clone();
        let up = entity.clone();
        let down = entity.clone();
        let pick = entity.clone();
        let picked = state.research.selected_shape == Some(index);
        list = list.child(
            div()
                .id(("shape-row", index))
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.select_shape(Some(index), cx));
                })
                .flex()
                .flex_col()
                .gap_1()
                .rounded(px(6.0))
                .px_2()
                .py_1()
                .bg(if picked { colors.accent } else { colors.muted })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(8.0))
                                .rounded_full()
                                .bg(super::colors::shape_color(index)),
                        )
                        .child(name)
                        .child(
                            div()
                                .id(("shape-summary", index))
                                .role(gpui::Role::Label)
                                .aria_label(format!(
                                    "{anchor} · {covered} cell{}",
                                    if covered == 1 { "" } else { "s" }
                                ))
                                .child(Caption::new(format!(
                                    "+{bonus:.0}% · {covered} cell{} at {anchor}",
                                    if covered == 1 { "" } else { "s" }
                                ))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_1()
                        .child(
                            icon_button(("shape-left", index), IconName::ArrowLeft)
                                .tooltip("Move left")
                                .on_click(move |_event, _window, cx| {
                                    left.update(cx, |state, cx| {
                                        state.nudge_shape(index, -5.0, 0.0, cx)
                                    });
                                }),
                        )
                        .child(
                            icon_button(("shape-up", index), IconName::ArrowUp)
                                .tooltip("Move up")
                                .on_click(move |_event, _window, cx| {
                                    up.update(cx, |state, cx| {
                                        state.nudge_shape(index, 0.0, -5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            icon_button(("shape-down", index), IconName::ArrowDown)
                                .tooltip("Move down")
                                .on_click(move |_event, _window, cx| {
                                    down.update(cx, |state, cx| {
                                        state.nudge_shape(index, 0.0, 5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            icon_button(("shape-right", index), IconName::ArrowRight)
                                .tooltip("Move right")
                                .on_click(move |_event, _window, cx| {
                                    right.update(cx, |state, cx| {
                                        state.nudge_shape(index, 5.0, 0.0, cx)
                                    });
                                }),
                        )
                        .child(
                            icon_button(("shape-rotate-left", index), IconName::RotateCcw)
                                .tooltip("Rotate left")
                                .on_click(move |_event, _window, cx| {
                                    rotate_left.update(cx, |state, cx| {
                                        state.rotate_shape(index, -5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            icon_button(("shape-rotate-right", index), IconName::RotateCw)
                                .tooltip("Rotate right")
                                .on_click(move |_event, _window, cx| {
                                    rotate_right
                                        .update(cx, |state, cx| state.rotate_shape(index, 5.0, cx));
                                }),
                        )
                        .child(
                            icon_button(("shape-remove", index), IconName::X)
                                .tooltip("Remove this shape")
                                .on_click(move |_event, _window, cx| {
                                    remove.update(cx, |state, cx| state.remove_shape(index, cx));
                                }),
                        ),
                ),
        );
    }
    card.child(list)
}

/// The magnifier assignment: the pool by kind, and the selected observation.
fn magnifiers_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = cx.theme();
    let owned = state.magnifiers_owned();
    let placed = state
        .simulation
        .mags
        .iter()
        .filter(|mag| mag.slot.is_some())
        .count();
    let cap = sim::mag_cap(state.simulation.research_level);
    let pool = sim::mag_pool(&state.simulation, &state.context);
    let selected = state
        .research
        .selected_observation
        .filter(|obs| *obs < game().observations.len());
    let entity = cx.entity();
    let clear = entity.clone();

    let card = Card::new().header(
        CardHeader::new("Magnifiers")
            .description(format!(
                "{placed} placed · {owned} owned · {cap} per observation"
            ))
            .action(Button::new("mag-clear").label("Clear").small().on_click(
                move |_event, _window, cx| {
                    clear.update(cx, |state, cx| state.clear_mags(cx));
                },
            )),
    );

    let mut pool_row = div()
        .id("mag-pool")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2();
    for kind in [MagKind::Regular, MagKind::Monocle, MagKind::Kaleidoscope] {
        pool_row = pool_row.child(
            div()
                .id(format!("mag-free-{}", kind_label(kind)))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "{}: {} free of {}",
                    kind_label(kind),
                    pool.free(kind, &state.simulation.mags),
                    pool.of(kind)
                ))
                .flex()
                .items_center()
                .gap_1()
                .child(div().size(px(8.0)).rounded_full().bg(mag_color(kind, cx)))
                .child(Caption::new(format!(
                    "{} {free}/{of}",
                    kind_label(kind),
                    free = pool.free(kind, &state.simulation.mags),
                    of = pool.of(kind)
                ))),
        );
    }

    let Some(obs) = selected else {
        return card.child(pool_row).child(
            Caption::new("Pick an observation on the Rates tab to assign magnifiers.")
                .text_color(colors.muted_foreground),
        );
    };

    let name = game().observations[obs].name.replace('_', " ");
    let on_slot = state
        .simulation
        .mags
        .iter()
        .filter(|mag| mag.slot == Some(obs))
        .count();
    let lift = entity.clone();
    let mut assign_row = div()
        .id("mag-assign")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2();
    for kind in [MagKind::Regular, MagKind::Monocle, MagKind::Kaleidoscope] {
        let assign = entity.clone();
        let free = pool.free(kind, &state.simulation.mags);
        assign_row = assign_row.child(
            Button::new(format!("mag-add-{}", kind_label(kind)))
                .label(kind_label(kind))
                .small()
                .disabled(free == 0 || on_slot >= cap as usize)
                .on_click(move |_event, _window, cx| {
                    assign.update(cx, |state, cx| state.assign_mag(kind, obs, cx));
                }),
        );
    }
    assign_row = assign_row.child(
        Button::new("mag-lift")
            .label("Remove")
            .small()
            .disabled(on_slot == 0)
            .on_click(move |_event, _window, cx| {
                lift.update(cx, |state, cx| state.lift_mag(obs, cx));
            }),
    );

    card.child(pool_row)
        .child(Caption::new(format!(
            "#{} {name} · {on_slot}/{cap} magnifiers",
            obs + 1
        )))
        .child(assign_row)
}

/// The pool colour of a magnifier kind, as the observation editor draws it.
fn mag_color(kind: MagKind, cx: &App) -> Hsla {
    let theme = cx.theme();
    match kind {
        MagKind::Regular => theme.success,
        MagKind::Monocle => theme.chart_5,
        MagKind::Kaleidoscope => theme.primary,
    }
}

/// A small ghost icon button, named by its tooltip.
fn icon_button(id: impl Into<ElementId>, icon: IconName) -> Button {
    Button::new(id).icon(icon).ghost().small()
}

fn kind_label(kind: MagKind) -> &'static str {
    match kind {
        MagKind::Regular => "Glass",
        MagKind::Monocle => "Monocle",
        MagKind::Kaleidoscope => "Kaleido",
    }
}
