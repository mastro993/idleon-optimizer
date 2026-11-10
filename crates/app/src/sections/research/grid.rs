//! The 20×12 research grid: cells, level badges and the shape polygons over them.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    App, Context, Hsla, IntoElement, KeyBinding, ParentElement, PathBuilder, Pixels, Point, Styled,
    Window, canvas, div, point, px,
};
use idleon_core::game;

gpui::actions!(
    idleon_grid,
    [
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        LevelUp,
        LevelDown,
        Deselect,
    ]
);

/// Key context of the grid canvas: bindings here never fire while an input has focus.
pub const GRID_CONTEXT: &str = "ResearchGrid";

/// Arrow keys move the selected square, `+`/`-` level it, Escape deselects.
pub fn bind_keys(cx: &mut App) {
    let context = Some(GRID_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", SelectLeft, context),
        KeyBinding::new("right", SelectRight, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("down", SelectDown, context),
        KeyBinding::new("=", LevelUp, context),
        KeyBinding::new("shift-=", LevelUp, context),
        KeyBinding::new("-", LevelDown, context),
        KeyBinding::new("escape", Deselect, context),
    ]);
}

/// Logical pixels per grid cell.
pub const CELL_PX: f32 = 46.0;
const CELL: f32 = CELL_PX;
/// The geometry data is authored at 30 px per cell.
const DATA_CELL: f32 = 30.0;

/// The canvas plus its legend.
pub fn grid_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let colors = {
        let palette = &cx.theme().colors;
        (palette.surface, palette.border, palette.fg_muted)
    };
    let game = game();
    let width = CELL * game.grid.cols as f32;
    let height = CELL * game.grid.rows as f32;

    let mut board = div()
        .id("research-grid")
        .relative()
        .on_mouse_move(
            cx.listener(|state, event: &gpui::MouseMoveEvent, _window, cx| {
                if state.dragging_shape().is_some() {
                    state.drag_shape_to((event.position.x.into(), event.position.y.into()), cx);
                }
            }),
        )
        .on_mouse_up(
            gpui::MouseButton::Left,
            cx.listener(|state, _event, _window, cx| state.end_shape_drag(cx)),
        )
        .flex()
        .flex_col()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(6.0))
        .overflow_hidden();

    // The shape whose row was last picked gets its squares outlined.
    let picked: Vec<usize> = state
        .research
        .selected_shape
        .and_then(|shape| state.simulation.placements.get(shape))
        .map(idleon_core::shape::cells_at)
        .unwrap_or_default();

    for row in 0..game.grid.rows {
        let mut line = div()
            .id(("grid-row", row))
            .flex()
            .flex_row()
            .flex_none()
            .h(px(CELL));
        for col in 0..game.grid.cols {
            let cell = row * game.grid.cols + col;
            line = line.child(grid_cell(state, cell, picked.contains(&cell), cx));
        }
        board = board.child(line);
    }

    let overlay = shape_overlay(state, cx);
    board = board.child(
        div()
            .id("shape-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(overlay),
    );

    div()
        .id("grid-card")
        .key_context(GRID_CONTEXT)
        .track_focus(&state.focus)
        .on_action(cx.listener(|state, _: &SelectLeft, _window, cx| {
            state.move_selection(-1, 0, cx);
        }))
        .on_action(cx.listener(|state, _: &SelectRight, _window, cx| {
            state.move_selection(1, 0, cx);
        }))
        .on_action(cx.listener(|state, _: &SelectUp, _window, cx| {
            state.move_selection(0, -1, cx);
        }))
        .on_action(cx.listener(|state, _: &SelectDown, _window, cx| {
            state.move_selection(0, 1, cx);
        }))
        .on_action(cx.listener(|state, _: &LevelUp, _window, cx| {
            state.bump_selected(1, cx);
        }))
        .on_action(cx.listener(|state, _: &LevelDown, _window, cx| {
            state.bump_selected(-1, cx);
        }))
        .on_action(cx.listener(|state, _: &Deselect, _window, cx| {
            state.deselect(cx);
        }))
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .rounded(px(10.0))
        .bg(colors.0)
        .border_1()
        .border_color(colors.1)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .h(px(36.0))
                .border_b_1()
                .border_color(colors.1)
                .child(div().child("Research grid"))
                .child(
                    Caption::new(format!(
                        "{} squares · {} shaped cells",
                        game.nodes.len(),
                        shaped_cells(state)
                    ))
                    .text_color(colors.2),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .p_3()
                .child(board),
        )
        .child(legend(state, cx))
}

fn shaped_cells(state: &AppState) -> usize {
    state
        .simulation
        .overlay()
        .into_iter()
        .filter(|shape| *shape >= 0)
        .count()
}

fn grid_cell(
    state: &AppState,
    cell: usize,
    picked: bool,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let game = game();
    let level = state.simulation.grid.level(cell);
    let selected = state.research.selected_cell == Some(cell);
    let node = game.node_at(cell);
    let tint = format::goal_color(node.and_then(|node| node.goal.as_deref()));
    let is_node = node.is_some();
    let fill: Hsla = if !is_node {
        colors.bg
    } else if level == 0 {
        tint.opacity(0.10)
    } else {
        let strength =
            (level as f32 / node.map(|n| n.max_lv).unwrap_or(1).max(1) as f32).clamp(0.2, 1.0);
        tint.opacity(0.12 + 0.33 * strength)
    };
    let border = if selected {
        colors.accent
    } else if picked {
        colors.accent.opacity(0.7)
    } else if level > 0 {
        tint.opacity(0.55)
    } else {
        colors.border
    };
    let label = if level == 0 {
        String::new()
    } else {
        level.to_string()
    };
    let title = node
        .map(|node| {
            format!(
                "{} · {} · level {level}",
                node.name.replace('_', " "),
                idleon_core::model::coord(cell)
            )
        })
        .unwrap_or_else(|| format!("{} · empty", idleon_core::model::coord(cell)));
    let id = format!("cell-{cell}");

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(CELL))
        .bg(fill)
        .border_1()
        .border_color(border)
        .text_size(px(13.0))
        .text_color(colors.fg)
        .when(selected || picked, |el| {
            el.font_weight(gpui::FontWeight::BOLD)
        })
        .aria_label(title.clone())
        .aria_selected(selected)
        .tooltip(ely_gpui_component::primitives::Tooltip::text(title))
        .on_click(cx.listener(move |state, _event, window, cx| state.select(cell, window, cx)))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(move |state, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(shape) = state.shape_at(cell) {
                    state.begin_shape_drag(
                        shape,
                        (event.position.x.into(), event.position.y.into()),
                        window,
                        cx,
                    );
                }
            }),
        )
        .child(label)
}

fn shape_overlay(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let scale = (CELL / DATA_CELL) as f64;
    let mut polygons: Vec<(Vec<(f32, f32)>, Hsla)> = Vec::new();
    for placement in &state.simulation.placements {
        let Some(def) = game().shapes.get(placement.shape) else {
            continue;
        };
        let polygon = idleon_core::shape::polygon_at(def, placement.x, placement.y, placement.rot);
        let points = polygon
            .iter()
            .map(|[x, y]| ((x * scale) as f32, (y * scale) as f32))
            .collect::<Vec<_>>();
        polygons.push((points, format::shape_color(placement.shape)));
    }
    let _ = cx;
    canvas(
        move |_bounds, _window, _cx| polygons,
        move |bounds, polygons, window, _cx| {
            for (points, color) in polygons {
                let path_points: Vec<Point<Pixels>> = points
                    .iter()
                    .map(|(x, y)| point(bounds.origin.x + px(*x), bounds.origin.y + px(*y)))
                    .collect();
                let mut fill = PathBuilder::fill();
                fill.add_polygon(&path_points, true);
                if let Ok(path) = fill.build() {
                    window.paint_path(path, color.opacity(0.22));
                }
                let mut stroke = PathBuilder::stroke(px(1.5));
                stroke.add_polygon(&path_points, true);
                if let Ok(path) = stroke.build() {
                    window.paint_path(path, color.opacity(0.95));
                }
            }
        },
    )
    .size_full()
}

fn legend(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let border = cx.theme().colors.border;
    let mut row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .border_t_1()
        .border_color(border);
    for placement in &state.simulation.placements {
        let Some(def) = game().shapes.get(placement.shape) else {
            continue;
        };
        row = row.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(format::shape_color(placement.shape)),
                )
                .child(Caption::new(format!("{} +{:.0}%", def.name, def.bonus_pct))),
        );
    }
    if state.simulation.placements.is_empty() {
        row = row.child(Caption::new(
            "No shapes placed. Add one from the inspector.",
        ));
    }
    row
}
