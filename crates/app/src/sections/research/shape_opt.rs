//! Shape Optimization: priority tiers, presets and the optimized placement.
//!
//! Ported from the upstream `Shape Optimization` tab in
//! `js/renderers/upgrade-eval.js`. The two stored lists come from
//! [`idleon_research::tiers`]: `above` is pinned ahead of the Res EXP divider and
//! `below` holds the non-EXP nodes the leftover shapes cover. Editing a tier
//! detaches the active preset; a built-in preset is never rewritten, and a
//! custom one only changes when the user saves it again.
//!
//! The preview and the breakdown are the real optimizer's output
//! ([`idleon_research::optimize::shapes`]), run through the shared task runner, so
//! a tier edit marks the stored layout stale instead of hiding it.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::Input;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Point, Render, Role, SharedString, Styled,
    Window, div, px,
};
use idleon_research::game;
use idleon_research::model::{Effort, coord};
use idleon_research::optimize::shapes::ShapesOutcome;
use idleon_research::sim;
use idleon_research::tiers::{Tier, is_exp_node};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// Logical pixels per preview cell; the board keeps the 20×12 upstream shape.
const PREVIEW_CELL: f32 = 22.0;

/// The Shape Optimization pane: tiers, presets, placement preview, breakdown.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let run = state.shape_optimize_state(cx);
    let overlay = run
        .result
        .as_ref()
        .map(|result| result.overlay.clone())
        .unwrap_or_else(|| vec![-1; game().cells()]);
    ScrollArea::new("shape-opt-scroll")
        .vertical()
        .shadows()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("shape-opt")
                .flex()
                .flex_col()
                .gap_3()
                .min_w_0()
                .child(tiers_card(state, window, &overlay, cx))
                .child(preview_card(state, &run, cx))
                .child(breakdown_card(&run, cx)),
        )
}

/// The tier panel: the preset rail and the three drop zones.
fn tiers_card(
    state: &mut AppState,
    window: &mut Window,
    overlay: &[i32],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tiers = state.simulation.shape_tiers.clone();
    let (above, exp, below) = zones(state);
    let border = cx.theme().colors.border;
    let split = layout::split_pane_reversed(
        "shape-tier-split",
        Breakpoint::of(window),
        div()
            .id("shape-tier-list")
            .flex()
            .flex_col()
            .gap_2()
            .min_w_0()
            .child(zone(
                state,
                cx,
                "shape-zone-above",
                "Drag here to prioritize over EXP",
                Tier::Above,
                &above,
                overlay,
            ))
            .child(divider("Res EXP", border))
            .child(zone(
                state,
                cx,
                "shape-zone-exp",
                "Drag EXP nodes here to unpin them",
                Tier::Exp,
                &exp,
                overlay,
            ))
            .child(divider("Below EXP", border))
            .child(zone(
                state,
                cx,
                "shape-zone-below",
                "Drag here for below EXP",
                Tier::Below,
                &below,
                overlay,
            )),
        preset_rail(state, cx),
        px(250.0),
    );
    let body = div()
        .id("shape-tiers-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .when(!tiers.above.is_empty(), |body| {
            body.child(Banner::new(
                "shape-above-warning",
                Severity::Warning,
                "Nodes above Res EXP always have shapes placed on them before any EXP nodes.",
            ))
        })
        .child(split);
    Card::new()
        .header(CardHeader::new("Shape priority tiers").description(
            "Drag nodes above or below the Res EXP divider. Click a node to pin it above, or to send it back.",
        ))
        .child(body)
        .into_any_element()
}

/// A labelled divider between two zones.
fn divider(label: &'static str, border: gpui::Hsla) -> AnyElement {
    div()
        .id(SharedString::from(format!("shape-divider-{label}")))
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .items_center()
        .gap_2()
        .py_1()
        .child(div().flex_1().h(px(1.0)).bg(border))
        .child(Caption::new(label))
        .child(div().flex_1().h(px(1.0)).bg(border))
        .into_any_element()
}

/// One drop zone: its squares, and a drop that appends to the zone.
fn zone(
    state: &AppState,
    cx: &mut Context<AppState>,
    id: &'static str,
    hint: &'static str,
    tier: Tier,
    cells: &[usize],
    overlay: &[i32],
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let drop = cx.entity();
    let drop_tier = tier;
    let mut squares = div()
        .id(SharedString::from(format!("shape-squares-{id}")))
        .flex()
        .flex_wrap()
        .gap_1();
    for cell in cells {
        squares = squares.child(tier_square(state, cx, tier, *cell, overlay));
    }
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{hint}: {} nodes", cells.len()))
        .flex()
        .flex_col()
        .gap_1()
        .min_h(px(46.0))
        .p_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
        .when(cells.is_empty(), |zone| {
            zone.child(Caption::new(hint).text_color(colors.fg_muted))
        })
        .child(squares)
        .on_drop(move |drag: &TierDrag, _window, cx| {
            drop.update(cx, |state, cx| {
                state.move_tier_node(drag.cell, drop_tier, None, cx);
            });
        })
        .into_any_element()
}

/// One tier chip: its coordinate, dragged to move it and clicked to toggle.
fn tier_square(
    state: &AppState,
    cx: &mut Context<AppState>,
    tier: Tier,
    cell: usize,
    overlay: &[i32],
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let node = game().node_at(cell);
    let level = state.simulation.grid.level(cell);
    let name = node
        .map(|node| node.name.replace('_', " "))
        .unwrap_or_else(|| "empty".into());
    let shape = overlay.get(cell).copied().unwrap_or(-1);
    let placed = usize::try_from(shape)
        .ok()
        .and_then(|index| game().shapes.get(index))
        .map(|shape| shape.name.clone());
    let border = placed
        .as_ref()
        .map(|_| super::colors::shape_color(shape as usize))
        .unwrap_or(colors.border);
    let zone = match tier {
        Tier::Above => "above",
        Tier::Exp => "exp",
        Tier::Below => "below",
    };
    let label = format!(
        "{} · {name} · level {level} · {zone} tier{}",
        coord(cell),
        placed
            .map(|shape| format!(" · shape: {shape}"))
            .unwrap_or_default()
    );
    let click = cx.entity();
    let drop = cx.entity();
    let before = cell;
    let drop_tier = tier;
    div()
        .id(SharedString::from(format!("shape-tier-{}", coord(cell))))
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(30.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(border)
        .bg(if tier == Tier::Above {
            colors.accent.opacity(0.14)
        } else {
            colors.bg
        })
        .text_size(px(10.0))
        .text_color(colors.fg)
        .cursor_pointer()
        .tooltip(ely_gpui_component::primitives::Tooltip::text(label))
        .on_click(move |_event, _window, cx| {
            click.update(cx, |state, cx| {
                let above = state.simulation.shape_tiers.zone(before) == Some(Tier::Above);
                let to = if above {
                    if is_exp_node(before) {
                        Tier::Exp
                    } else {
                        Tier::Below
                    }
                } else {
                    Tier::Above
                };
                state.move_tier_node(before, to, None, cx);
            });
        })
        .on_drag(TierDrag { cell }, |drag, offset, _, cx| {
            cx.new(|_| TierGhost {
                cell: drag.cell,
                offset,
            })
        })
        .on_drop(move |drag: &TierDrag, _window, cx| {
            if drag.cell == before {
                return;
            }
            drop.update(cx, |state, cx| {
                state.move_tier_node(drag.cell, drop_tier, Some(before), cx);
            });
        })
        .child(coord(cell))
        .into_any_element()
}

/// The built-in and custom preset rail, plus the name field that saves one.
fn preset_rail(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let active = state.research.active_shape_preset.clone();
    let custom = state.research.shape_presets.clone();
    let mut rail = div()
        .id("shape-presets")
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(220.0))
        .child(Caption::new("Built-in presets").text_color(colors.fg_muted));
    for preset in &game().presets {
        let entity = cx.entity();
        let id = preset.id.clone();
        let is_active = active.as_deref() == Some(preset.id.as_str());
        rail = rail.child(
            Button::new(
                SharedString::from(format!("shape-preset-{}", preset.id)),
                preset.name.clone(),
            )
            .size(ControlSize::Sm)
            .full_width()
            .variant(if is_active {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .on_click(move |_event, _window, cx| {
                let id = id.clone();
                entity.update(cx, |state, cx| state.apply_shape_preset(&id, cx));
            }),
        );
    }
    rail = rail.child(
        div()
            .id("shape-presets-saved")
            .flex()
            .flex_col()
            .gap_1()
            .pt_2()
            .child(Caption::new("Saved presets").text_color(colors.fg_muted))
            .when(custom.is_empty(), |list| {
                list.child(
                    Caption::new("No saved presets yet; name the tiers and save them.")
                        .text_color(colors.fg_muted),
                )
            }),
    );
    for (index, preset) in custom.iter().enumerate() {
        let load = cx.entity();
        let delete = cx.entity();
        let name = preset.name.clone();
        let is_active = active.as_deref() == Some(preset.name.as_str());
        rail = rail.child(
            div()
                .id(SharedString::from(format!("shape-custom-row-{index}")))
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("shape-custom-preset-{index}")),
                        preset.name.clone(),
                    )
                    .size(ControlSize::Sm)
                    .variant(if is_active {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .on_click(move |_event, _window, cx| {
                        let name = name.clone();
                        load.update(cx, |state, cx| {
                            state.apply_custom_shape_preset(&name, cx);
                        });
                    }),
                )
                .child(div().flex_1())
                .child(
                    Button::new(
                        SharedString::from(format!("shape-custom-delete-{index}")),
                        "",
                    )
                    .icon(IconName::Trash2)
                    .name("Delete preset")
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_event, _window, cx| {
                        delete.update(cx, |state, cx| state.delete_shape_preset(index, cx));
                    }),
                ),
        );
    }
    let focus = state.research.preset_name.read(cx).focus().clone();
    let save = cx.entity();
    let shown = match active.as_deref() {
        Some(id) if id.starts_with('_') => game()
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .map(|preset| preset.name.clone())
            .unwrap_or_else(|| id.to_string()),
        Some(name) => name.to_string(),
        None => "Custom tiers (no preset active)".into(),
    };
    rail.child(div().pt_2())
        .child(
            div()
                .id("shape-preset-name")
                .role(Role::TextInput)
                .aria_label("Shape preset name")
                .on_click(move |_event, window, cx| window.focus(&focus, cx))
                .child(Input::new(&state.research.preset_name).size(ControlSize::Sm)),
        )
        .child(
            Button::new("shape-save-preset", "Save preset")
                .size(ControlSize::Sm)
                .icon(IconName::Save)
                .full_width()
                .on_click(move |_event, _window, cx| {
                    save.update(cx, |state, cx| state.save_shape_preset(cx));
                }),
        )
        .child(
            div()
                .id("shape-active")
                .role(Role::Label)
                .aria_label(format!("Active shape preset: {shown}"))
                .pt_1()
                .child(Caption::new(format!("Active: {shown}")).text_color(colors.fg_muted)),
        )
        .into_any_element()
}

/// The optimized placement card: effort, the run, the summary and the board.
fn preview_card(
    state: &mut AppState,
    run: &idleon_ui::runner::TaskState<ShapesOutcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let run_entity = entity.clone();
    let effort = match state.effort {
        Effort::Quick => "quick",
        Effort::Balanced => "balanced",
        Effort::Thorough => "thorough",
    };
    let effort_entity = entity.clone();
    let effort_control = SegmentedControl::new("shape-effort", effort)
        .size(ControlSize::Sm)
        .segment("quick", "Quick", None)
        .segment("balanced", "Balanced", None)
        .segment("thorough", "Thorough", None)
        .on_change(move |value, _window, cx| {
            let effort = match value.as_ref() {
                "quick" => Effort::Quick,
                "thorough" => Effort::Thorough,
                _ => Effort::Balanced,
            };
            effort_entity.update(cx, |state, cx| state.set_effort(effort, cx));
        });
    let run_button = Button::new(
        "shape-optimize",
        if run.running {
            "Optimizing…"
        } else {
            "Optimize placement"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(run.running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.optimize_shapes(cx));
    });

    let mut body = div()
        .id("shape-preview-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(effort_control)
                .child(div().flex_1())
                .child(run_button)
                .when(run.stale, |row| {
                    row.child(
                        div()
                            .id("shape-stale")
                            .role(Role::Label)
                            .aria_label("Stale: the tiers changed after this run")
                            .child(Badge::new("Stale").tone(Tone::Warning)),
                    )
                }),
        );
    if run.running {
        if let Some(progress) = run.progress.clone() {
            let cancel = entity.clone();
            body = body.child(run_chrome::progress_card(
                "shape",
                "Optimizing shapes",
                "Runs off the UI thread; a cancelled run keeps the previous layout.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_shape_optimize(cx));
                },
            ));
        }
    } else if run.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "shape-cancelled",
            run.result.is_some(),
        ));
    }
    if run.stale {
        body = body.child(run_chrome::stale_banner("shape-stale-banner"));
    }

    match &run.result {
        Some(result) => {
            let gain = if result.before > 0.0 {
                (result.rate / result.before - 1.0) * 100.0
            } else {
                0.0
            };
            let summary = format!(
                "Optimized EXP/hr: {} ({}{:.2}% vs current) · {} shapes placed",
                format::rate(result.rate),
                if gain >= 0.0 { "+" } else { "" },
                gain,
                result.placements.len()
            );
            let apply = entity.clone();
            body = body
                .child(
                    div()
                        .id("shape-summary")
                        .role(Role::Label)
                        .aria_label(summary.clone())
                        .text_size(px(13.0))
                        .text_color(colors.success)
                        .child(summary),
                )
                .child(
                    div()
                        .id("shape-preview")
                        .role(Role::Label)
                        .aria_label(format!(
                            "Optimized placement preview: {} shaped cells",
                            result.overlay.iter().filter(|shape| **shape >= 0).count()
                        ))
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .children(preview_rows(state, &result.overlay, cx)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("shape-apply", "Apply layout")
                                .size(ControlSize::Sm)
                                .disabled(run.stale)
                                .on_click(move |_event, _window, cx| {
                                    apply.update(cx, |state, cx| {
                                        state.apply_shape_layout(cx);
                                    });
                                }),
                        )
                        .child(
                            Caption::new(
                                "Applying writes these placements to the grid editor's shape layout.",
                            )
                            .text_color(colors.fg_muted),
                        ),
                );
        }
        None if !run.running => {
            body = body.child(Banner::new(
                "shape-preview-empty",
                Severity::Info,
                if state.shapes_owned() == 0 {
                    "No shapes are unlocked at this research level yet."
                } else {
                    "Run the placement optimizer to see the layout these tiers place."
                },
            ));
        }
        None => {}
    }
    Card::new()
        .header(
            CardHeader::new("Optimized placement")
                .description("What the real shape optimizer places for these tiers"),
        )
        .child(body)
        .into_any_element()
}

/// The preview board's rows, 20 cells each, in the game's coordinate order.
fn preview_rows(state: &AppState, overlay: &[i32], cx: &mut Context<AppState>) -> Vec<AnyElement> {
    let game = game();
    (0..game.grid.rows)
        .map(|row| {
            let mut line = div().flex().flex_row().gap_0p5();
            for col in 0..game.grid.cols {
                line = line.child(preview_cell(state, row * game.grid.cols + col, overlay, cx));
            }
            line.into_any_element()
        })
        .collect()
}

fn preview_cell(
    state: &AppState,
    cell: usize,
    overlay: &[i32],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let node = game().node_at(cell);
    let level = state.simulation.grid.level(cell);
    let tint = super::colors::goal_color(node.and_then(|node| node.goal.as_deref()));
    let fill = if node.is_none() {
        colors.bg
    } else if level == 0 {
        tint.opacity(0.08)
    } else {
        let strength = (level as f32 / node.map(|node| node.max_lv).unwrap_or(1).max(1) as f32)
            .clamp(0.2, 1.0);
        tint.opacity(0.12 + 0.33 * strength)
    };
    let shape = overlay.get(cell).copied().unwrap_or(-1);
    let border = usize::try_from(shape)
        .ok()
        .and_then(|index| game().shapes.get(index))
        .map(|_| super::colors::shape_color(shape as usize))
        .unwrap_or(colors.border);
    let name = node
        .map(|node| node.name.replace('_', " "))
        .unwrap_or_else(|| "empty".into());
    div()
        .id(SharedString::from(format!("shape-cell-{cell}")))
        .role(Role::Label)
        .aria_label(format!("{} · {name} · level {level}", coord(cell)))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(PREVIEW_CELL))
        .bg(fill)
        .border_1()
        .border_color(border)
        .rounded(px(3.0))
        .text_size(px(9.0))
        .text_color(colors.fg)
        .child(if level > 0 {
            level.to_string()
        } else {
            String::new()
        })
        .into_any_element()
}

/// The coverage breakdown: one row per placement, in the order to drop them.
fn breakdown_card(
    run: &idleon_ui::runner::TaskState<ShapesOutcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let mut body = div().id("shape-breakdown-body").p_3().flex().flex_col();
    match &run.result {
        Some(result) if !result.placements.is_empty() => {
            let mut table = Table::new(["#", "Shape", "Bonus", "Place at", "Nodes"]);
            for (order, placement) in result.placements.iter().enumerate() {
                let shape = game().shapes.get(placement.shape);
                let name = shape
                    .map(|shape| shape.name.clone())
                    .unwrap_or_else(|| format!("Shape {}", placement.shape));
                let bonus = shape.map(|shape| shape.bonus_pct).unwrap_or(0.0);
                let nodes: Vec<String> = result
                    .overlay
                    .iter()
                    .enumerate()
                    .filter(|(_, owner)| **owner == placement.shape as i32)
                    .map(|(cell, _)| coord(cell))
                    .collect();
                table = table.row([
                    div()
                        .id(SharedString::from(format!("shape-breakdown-order-{order}")))
                        .child(format!("{}", order + 1))
                        .into_any_element(),
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex_none()
                                .size(px(8.0))
                                .rounded_full()
                                .bg(super::colors::shape_color(placement.shape)),
                        )
                        .child(name)
                        .into_any_element(),
                    div().child(format!("+{bonus:.0}%")).into_any_element(),
                    div()
                        .child(format!(
                            "({}, {}){}",
                            placement.x.round() as i64,
                            placement.y.round() as i64,
                            if placement.rot != 0.0 {
                                format!(" rot {}°", placement.rot)
                            } else {
                                String::new()
                            }
                        ))
                        .into_any_element(),
                    div()
                        .min_w_0()
                        .text_size(px(11.0))
                        .child(if nodes.is_empty() {
                            "—".to_string()
                        } else {
                            nodes.join(", ")
                        })
                        .into_any_element(),
                ]);
            }
            body = body.child(table).child(
                div()
                    .pt_2()
                    .text_size(px(11.0))
                    .text_color(colors.fg_muted)
                    .child(
                        "Drop shapes in this order: a node keeps the first shape that covers it.",
                    ),
            );
        }
        Some(_) => {
            body = body
                .child(Caption::new("The optimizer placed no shapes.").text_color(colors.fg_muted));
        }
        None => {
            body = body.child(
                Caption::new("Run the placement optimizer to see the coverage breakdown.")
                    .text_color(colors.fg_muted),
            );
        }
    }
    Card::new()
        .header(
            CardHeader::new("Placement breakdown")
                .description("Shape positions in game coordinates and the nodes each owns"),
        )
        .child(body)
        .into_any_element()
}

/// The cells of the three zones, as the upstream tier list renders them.
fn zones(state: &mut AppState) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let tiers = state.simulation.shape_tiers.clone();
    let fresh = state
        .research
        .shape_values
        .as_ref()
        .is_none_or(|(simulation, context, _)| {
            simulation != &state.simulation || context != &state.context
        });
    if fresh {
        let values = sim::bare_cell_values(&state.simulation, &state.context);
        state.research.shape_values =
            Some((state.simulation.clone(), state.context.clone(), values));
    }
    let values = &state
        .research
        .shape_values
        .as_ref()
        .expect("the cell values were just computed")
        .2;
    let noise = values.iter().cloned().fold(0.0, f64::max) * 1e-12;
    let exp = |cell: usize| is_exp_node(cell) || values[cell] > noise;
    let live = |cell: usize| state.simulation.grid.level(cell) > 0;
    let above: Vec<usize> = tiers
        .above
        .iter()
        .copied()
        .filter(|cell| live(*cell))
        .collect();
    let mut exp_cells: Vec<usize> = (0..game().cells())
        .filter(|cell| live(*cell) && !above.contains(cell) && exp(*cell))
        .collect();
    exp_cells.sort_by(|left, right| values[*right].total_cmp(&values[*left]));
    let below: Vec<usize> = tiers
        .below
        .iter()
        .copied()
        .filter(|cell| live(*cell) && !exp(*cell))
        .collect();
    (above, exp_cells, below)
}

/// A chip dragged between the tier zones.
#[derive(Clone, Copy)]
struct TierDrag {
    cell: usize,
}

/// The drag's floating chip, centred under the pointer.
struct TierGhost {
    cell: usize,
    offset: Point<gpui::Pixels>,
}

impl Render for TierGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        div()
            .pl(self.offset.x - px(15.0))
            .pt(self.offset.y - px(13.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(colors.accent)
                    .bg(colors.surface)
                    .text_size(px(10.0))
                    .child(coord(self.cell)),
            )
    }
}
