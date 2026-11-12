//! Right rail: the selected square, the shape list and the magnifier pool.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, IconButton};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::game;
use idleon_research::model::{MagKind, coord};
use idleon_research::{shape, sim};

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
    let colors = &cx.theme().colors;
    let selected = state.research.selected_cell;
    let node = state.selected_node();
    let card = Card::new().header(CardHeader::new("Selected square"));

    let Some(node) = node else {
        return card.child(div().p_3().child(Caption::new(
            "Pick a square on the grid to see what it does.",
        )));
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
            .p_3()
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
                        Badge::new(node.goal.clone().unwrap_or_else(|| "Square".into()))
                            .tone(Tone::Neutral),
                    ),
            )
            .child(Caption::new(node.desc.replace('_', " ")).text_color(colors.fg_muted))
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
                                IconButton::new("square-minus", IconName::Minus)
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
                                IconButton::new("square-plus", IconName::Plus)
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
    let colors = &cx.theme().colors;
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
                        Button::new("shape-add", "Add shape")
                            .size(ely_gpui_component::theme::ControlSize::Sm)
                            .icon(IconName::Plus)
                            .disabled(placed >= owned)
                            .on_click(move |_event, _window, cx| {
                                add.update(cx, |state, cx| state.add_shape(cx));
                            }),
                    )
                    .child(
                        Button::new("layout-reset", "Clear")
                            .size(ely_gpui_component::theme::ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                clear.update(cx, |state, cx| state.reset_layout(cx));
                            }),
                    ),
            ),
    );

    if placed == 0 {
        return card.child(div().p_3().child(Caption::new(
            "No shapes yet. Add one, or run the optimizer to place them for you.",
        )));
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

    let mut list = div().flex().flex_col().gap_1().p_3();
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
                .bg(if picked { colors.hover } else { colors.sunken })
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
                            IconButton::new(("shape-left", index), IconName::ArrowLeft)
                                .tooltip("Move left")
                                .on_click(move |_event, _window, cx| {
                                    left.update(cx, |state, cx| {
                                        state.nudge_shape(index, -5.0, 0.0, cx)
                                    });
                                }),
                        )
                        .child(
                            IconButton::new(("shape-up", index), IconName::ArrowUp)
                                .tooltip("Move up")
                                .on_click(move |_event, _window, cx| {
                                    up.update(cx, |state, cx| {
                                        state.nudge_shape(index, 0.0, -5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            IconButton::new(("shape-down", index), IconName::ArrowDown)
                                .tooltip("Move down")
                                .on_click(move |_event, _window, cx| {
                                    down.update(cx, |state, cx| {
                                        state.nudge_shape(index, 0.0, 5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            IconButton::new(("shape-right", index), IconName::ArrowRight)
                                .tooltip("Move right")
                                .on_click(move |_event, _window, cx| {
                                    right.update(cx, |state, cx| {
                                        state.nudge_shape(index, 5.0, 0.0, cx)
                                    });
                                }),
                        )
                        .child(
                            IconButton::new(("shape-rotate-left", index), IconName::RotateCcw)
                                .tooltip("Rotate left")
                                .on_click(move |_event, _window, cx| {
                                    rotate_left.update(cx, |state, cx| {
                                        state.rotate_shape(index, -5.0, cx)
                                    });
                                }),
                        )
                        .child(
                            IconButton::new(("shape-rotate-right", index), IconName::RotateCw)
                                .tooltip("Rotate right")
                                .on_click(move |_event, _window, cx| {
                                    rotate_right
                                        .update(cx, |state, cx| state.rotate_shape(index, 5.0, cx));
                                }),
                        )
                        .child(
                            IconButton::new(("shape-remove", index), IconName::X)
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
    let colors = &cx.theme().colors;
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
            .action(
                Button::new("mag-clear", "Clear")
                    .size(ely_gpui_component::theme::ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        clear.update(cx, |state, cx| state.clear_mags(cx));
                    }),
            ),
    );

    let mut pool_row = div()
        .id("mag-pool")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_3()
        .pb_2();
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
                .child(div().size(px(8.0)).rounded_full().bg(mag_color(kind)))
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
            div().px_3().pb_3().child(
                Caption::new("Pick an observation on the Rates tab to assign magnifiers.")
                    .text_color(colors.fg_muted),
            ),
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
        .gap_2()
        .px_3()
        .pb_3();
    for kind in [MagKind::Regular, MagKind::Monocle, MagKind::Kaleidoscope] {
        let assign = entity.clone();
        let free = pool.free(kind, &state.simulation.mags);
        assign_row = assign_row.child(
            Button::new(format!("mag-add-{}", kind_label(kind)), kind_label(kind))
                .size(ely_gpui_component::theme::ControlSize::Sm)
                .disabled(free == 0 || on_slot >= cap as usize)
                .on_click(move |_event, _window, cx| {
                    assign.update(cx, |state, cx| state.assign_mag(kind, obs, cx));
                }),
        );
    }
    assign_row = assign_row.child(
        Button::new("mag-lift", "Remove")
            .size(ely_gpui_component::theme::ControlSize::Sm)
            .disabled(on_slot == 0)
            .on_click(move |_event, _window, cx| {
                lift.update(cx, |state, cx| state.lift_mag(obs, cx));
            }),
    );

    card.child(pool_row)
        .child(div().px_3().pb_1().child(Caption::new(format!(
            "#{} {name} · {on_slot}/{cap} magnifiers",
            obs + 1
        ))))
        .child(assign_row)
}

/// The pool colour of a magnifier kind, as upstream's observation editor draws it.
fn mag_color(kind: MagKind) -> gpui::Hsla {
    use gpui::rgb;
    match kind {
        MagKind::Regular => rgb(0x81c784),
        MagKind::Monocle => rgb(0xce93d8),
        MagKind::Kaleidoscope => rgb(0x4dd0e1),
    }
    .into()
}

fn kind_label(kind: MagKind) -> &'static str {
    match kind {
        MagKind::Regular => "Glass",
        MagKind::Monocle => "Monocle",
        MagKind::Kaleidoscope => "Kaleido",
    }
}
