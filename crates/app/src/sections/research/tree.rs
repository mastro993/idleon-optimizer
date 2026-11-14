//! Decision Tree sandbox: the research-event tree, its node editor and the
//! branch comparison.
//!
//! Ported from the upstream Decision Tree tab (`js/dt/decision-tree.js`,
//! `js/dt/dt-sim.js`, `js/dt/dt-grid.js`, `js/dt/dt-obs-editor.js`). The maths
//! lives in [`idleon_research::tree`]; this pane draws the tree, edits the
//! selected node's layout and runs the long steps — advancing a chain,
//! branching, and the auto magnifier and shape optimizers — through the shared
//! task runner.

use crate::sections::research::colors;
use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::{Button, IconButton, SegmentedControl};
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Tooltip};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Hsla, IntoElement, ParentElement, PathBuilder, Role, SharedString, Styled,
    Window, canvas, div, point, px, rgb,
};
use idleon_research::model::MagKind;
use idleon_research::sim::{self, Event};
use idleon_research::tree::Tree;
use idleon_research::{game, shape};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// One logical pixel of tree canvas at zoom 1.
pub(crate) const NODE_W: f32 = 120.0;
pub(crate) const NODE_H: f32 = 58.0;
/// Side of one editor board cell.
const BOARD_CELL: f32 = 12.0;

/// Which board the node editor shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Click a square to buy its next level.
    #[default]
    Upgrades,
    /// Nudge and turn the placed shapes.
    Shapes,
}

impl Mode {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Mode::Upgrades => "upgrades",
            Mode::Shapes => "shapes",
        }
    }
}

/// The sandbox's own state, kept with the research section.
pub(crate) struct State {
    /// The tree; created from the loaded save the first time the tab opens.
    pub(crate) tree: Option<Tree>,
    /// The node the editor shows.
    pub(crate) selected: Option<u64>,
    pub(crate) mode: Mode,
    /// The observation the magnifier buttons place on.
    pub(crate) observation: Option<usize>,
    pub(crate) zoom: f32,
    pub(crate) pan: (f32, f32),
    /// A pan drag in flight: the pointer and pan it started at.
    pub(crate) drag: Option<Pan>,
    /// Whether the last press turned into a pan, so its release is not a click.
    pub(crate) panned: bool,
}

/// A pan drag in flight.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pan {
    pub(crate) start: (f32, f32),
    pub(crate) pan: (f32, f32),
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            tree: None,
            selected: None,
            mode: Mode::default(),
            observation: None,
            zoom: 1.0,
            pan: (0.0, 0.0),
            drag: None,
            panned: false,
        }
    }

    /// The tree the pane draws, seeded from `simulation` on first use.
    pub(crate) fn ensure(&mut self, simulation: &idleon_research::Simulation) {
        if self.selected.is_none() {
            self.selected = Some(1);
        }
        self.tree
            .get_or_insert_with(|| Tree::from_state(simulation));
    }
}

/// The long jobs the sandbox hands to the task runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Job {
    /// Save the node: step it to its next event, or re-simulate its children.
    Save,
    /// Fork the node and step the copy.
    Branch,
    /// The upstream Auto Mags action.
    Mags,
    /// The upstream Auto Shapes action.
    Shapes,
}

impl Job {
    pub(crate) fn line(self) -> &'static str {
        match self {
            Job::Save => "Stepping the branch…",
            Job::Branch => "Branching the tree…",
            Job::Mags => "Re-fitting magnifiers…",
            Job::Shapes => "Placing shapes…",
        }
    }
}

/// The inputs a stored tree result was produced from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) node: u64,
    pub(crate) job: Job,
}

/// What a finished tree job leaves for the pane to adopt.
#[derive(Clone, Debug)]
pub(crate) struct Outcome {
    pub(crate) tree: Tree,
    pub(crate) select: Option<u64>,
    pub(crate) message: String,
}

/// The Decision Tree pane: the tree on the left, the node editor on the right.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let simulation = state.simulation.clone();
    state.research.tree.ensure(&simulation);
    layout::split_pane(
        "decision-tree",
        Breakpoint::of(window),
        ScrollArea::new("dt-tree-scroll")
            .vertical()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .id("dt-tree-column")
                    .flex()
                    .flex_col()
                    .gap_3()
                    .min_w_0()
                    .child(tree_card(state, window, cx))
                    .child(comparison_card(state, cx)),
            ),
        div()
            .id("dt-editor-column")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(editor(state, cx)),
        px(420.0),
    )
}

/// The tree card: the toolbar and the pan/zoom canvas.
fn tree_card(state: &mut AppState, window: &Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(selected) = state.research.tree.selected else {
        return div().into_any_element();
    };
    let comparisons = state
        .research
        .tree
        .tree
        .as_ref()
        .map(Tree::compared)
        .map(<[u64]>::len);
    let entity = cx.entity();

    let level_ups_only = state
        .research
        .tree
        .tree
        .as_ref()
        .is_some_and(|tree| tree.level_ups_only);
    let toggle = entity.clone();
    let level_ups = Checkbox::new("dt-level-ups-only", level_ups_only)
        .label("Level-ups only")
        .on_change(move |on, _window, cx| {
            toggle.update(cx, |state, cx| state.tree_level_ups_only(on, cx));
        });

    let zoom_out = entity.clone();
    let zoom_in = entity.clone();
    let zoom_reset = entity.clone();
    let fit = entity.clone();
    let reset = entity.clone();
    let clear = entity.clone();
    let viewport_width = (f32::from(window.bounds().size.width) - 420.0 - 88.0).max(320.0);
    let layout_width = state
        .research
        .tree
        .tree
        .as_ref()
        .map(|tree| tree.layout().width)
        .unwrap_or(1.0);
    let mut tools = div().flex().items_center().gap_1().child(level_ups);
    tools = tools
        .child(
            IconButton::new("dt-zoom-out", IconName::Minus)
                .tooltip("Zoom out")
                .on_click(move |_event, _window, cx| {
                    zoom_out.update(cx, |state, cx| state.tree_zoom(-0.15, cx));
                }),
        )
        .child(
            IconButton::new("dt-zoom-in", IconName::Plus)
                .tooltip("Zoom in")
                .on_click(move |_event, _window, cx| {
                    zoom_in.update(cx, |state, cx| state.tree_zoom(0.15, cx));
                }),
        )
        .child(
            Button::new(
                "dt-zoom-reset",
                format!("{:.0}%", state.research.tree.zoom * 100.0),
            )
            .size(ControlSize::Sm)
            .on_click(move |_event, _window, cx| {
                zoom_reset.update(cx, |state, cx| state.tree_zoom_reset(cx));
            }),
        )
        .child(Button::new("dt-fit", "Fit").size(ControlSize::Sm).on_click(
            move |_event, _window, cx| {
                fit.update(cx, |state, cx| {
                    state.tree_fit(viewport_width, layout_width, cx)
                });
            },
        ))
        .child(
            Button::new("dt-reset", "Restart")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    reset.update(cx, |state, cx| state.reset_tree(cx));
                }),
        );
    if comparisons.unwrap_or(0) > 0 {
        tools = tools.child(
            Button::new("dt-compare-clear", "Clear compare")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    clear.update(cx, |state, cx| state.tree_clear_compare(cx));
                }),
        );
    }

    Card::new()
        .header(
            CardHeader::new("Decision Tree")
                .description(
                    "Pick a node to edit its layout. Save steps it to the next event; Branch races a copy.",
                )
                .action(tools),
        )
        .child(viewport(state, selected, cx))
        .into_any_element()
}

/// The pannable, zoomable tree canvas.
fn viewport(state: &AppState, selected: u64, cx: &mut Context<AppState>) -> AnyElement {
    let Some(tree) = state.research.tree.tree.as_ref() else {
        return div().into_any_element();
    };
    let ctx = &state.context;
    let zoom = state.research.tree.zoom;
    let pan = state.research.tree.pan;
    let layout = tree.layout();
    let place = |x: f32, y: f32| (x * zoom + pan.0, y * zoom + pan.1);

    let mut edges = Vec::new();
    for node in &layout.nodes {
        let Some(parent) = tree.visible_parent(node.id) else {
            continue;
        };
        let Some(parent) = layout.nodes.iter().find(|placed| placed.id == parent) else {
            continue;
        };
        let (x1, y1) = place(parent.x + NODE_W, parent.y + NODE_H / 2.0);
        let (x2, y2) = place(node.x, node.y + NODE_H / 2.0);
        let event = tree
            .node(node.id)
            .map(|node| node.event)
            .unwrap_or_default();
        edges.push((x1, y1, x2, y2, event_color(event)));
    }

    let entity = cx.entity();
    let mut body = div()
        .id("dt-viewport")
        .relative()
        .flex_none()
        .h(px(360.0))
        .w_full()
        .overflow_hidden()
        .border_t_1()
        .border_color(cx.theme().colors.border)
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|state, event: &gpui::MouseDownEvent, _window, cx| {
                state.begin_tree_pan((event.position.x.into(), event.position.y.into()), cx);
            }),
        )
        .on_mouse_move(
            cx.listener(|state, event: &gpui::MouseMoveEvent, _window, cx| {
                state.drag_tree_pan((event.position.x.into(), event.position.y.into()), cx);
            }),
        )
        .on_mouse_up(
            gpui::MouseButton::Left,
            cx.listener(|state, _event, _window, cx| state.end_tree_pan(cx)),
        );

    body = body.child(
        canvas(
            move |_bounds, _window, _cx| edges,
            move |bounds, edges, window, _cx| {
                for (x1, y1, x2, y2, color) in edges {
                    let from = point(bounds.origin.x + px(x1), bounds.origin.y + px(y1));
                    let to = point(bounds.origin.x + px(x2), bounds.origin.y + px(y2));
                    let middle = (x1 + x2) / 2.0;
                    let mut stroke = PathBuilder::stroke(px(1.6));
                    stroke.move_to(from);
                    stroke.cubic_bezier_to(
                        to,
                        point(bounds.origin.x + px(middle), bounds.origin.y + px(y1)),
                        point(bounds.origin.x + px(middle), bounds.origin.y + px(y2)),
                    );
                    if let Ok(path) = stroke.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full(),
    );
    for placed in &layout.nodes {
        let node_id = placed.id;
        let Some(node) = tree.node(node_id) else {
            continue;
        };
        let (x, y) = place(placed.x, placed.y);
        let rate = sim::exp_rate(&node.state, ctx).total;
        let parent_rate = tree.visible_parent(node_id).and_then(|parent| {
            tree.node(parent)
                .map(|node| sim::exp_rate(&node.state, ctx).total)
        });
        let points = sim::grid_points(&node.state, ctx);
        let label = event_label(node.event);
        let color = event_color(node.event);
        let active = node_id == selected;
        let compared = tree.compared().contains(&node_id);
        let compare = entity.clone();
        let focus = entity.clone();
        let delta = parent_rate
            .filter(|parent| *parent > 0.0)
            .and_then(|parent| rate_delta(parent, rate));
        let when = format::projection(tree.total_hours(node_id));
        let name = format!(
            "{label} · level {} · {}/hr · {when}",
            node.state.research_level,
            format::rate(rate)
        );
        let mut card = div()
            .id(SharedString::from(format!("dt-node-{node_id}")))
            .role(Role::Button)
            .aria_label(name.clone())
            .aria_selected(active)
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(NODE_W * zoom))
            .h(px(NODE_H * zoom))
            .flex()
            .flex_col()
            .justify_center()
            .gap_1()
            .px(px(7.0 * zoom))
            .rounded(px(7.0))
            .border_1()
            .border_color(if compared {
                rgb(0x4caf50).into()
            } else if active {
                rgb(0x64b5f6).into()
            } else {
                color
            })
            .bg(cx.theme().colors.surface)
            .text_size(px((11.0 * zoom).clamp(8.0, 14.0)))
            .cursor_pointer()
            .tooltip(Tooltip::text(format!(
                "{name}\n{}",
                tree.changes(node_id).join("\n")
            )))
            .on_click(move |_event, window, cx| {
                focus.update(cx, |state, cx| state.tree_select(node_id, window, cx));
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .text_color(color)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(label),
                    )
                    .when(node.auto_insight, |row| {
                        row.child(Caption::new("AI▸").text_color(rgb(0xce93d8)))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(format!("Lv {}", node.state.research_level))
                    .child(
                        div()
                            .text_color(rgb(0x81c784))
                            .child(format!("{}/hr", format::rate(rate))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Caption::new(when))
                    .when(points.free > 0, |row| {
                        row.child(
                            Caption::new(format!("+{} pts", points.free)).text_color(rgb(0xffd700)),
                        )
                    })
                    .when_some(delta, |row, (value, pct)| {
                        row.child(Caption::new(format!("{value} {pct}")).text_color(
                            if value.starts_with('-') {
                                Hsla::from(rgb(0xe57373))
                            } else {
                                Hsla::from(rgb(0x81c784))
                            },
                        ))
                    }),
            );
        if node.implicit {
            card = card.opacity(0.4);
        }
        body = body.child(card);
        body = body.child(
            div()
                .id(SharedString::from(format!("dt-compare-{node_id}")))
                .role(Role::CheckBox)
                .aria_label(format!("Compare node {node_id}"))
                .aria_selected(compared)
                .absolute()
                .left(px(x + NODE_W * zoom - 26.0 * zoom))
                .top(px(y + NODE_H * zoom - 20.0 * zoom))
                .size(px(16.0 * zoom))
                .cursor_pointer()
                .tooltip(Tooltip::text("Add to the branch comparison"))
                .on_click(move |_event, _window, cx| {
                    compare.update(cx, |state, cx| state.tree_toggle_compare(node_id, cx));
                })
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(0x4caf50))
                        .text_size(px(10.0 * zoom))
                        .text_color(rgb(0x102010))
                        .when(compared, |box_| box_.bg(rgb(0x4caf50)).child("✓")),
                ),
        );
    }

    body.into_any_element()
}

/// The comparison panel: every picked node against the first one.
fn comparison_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(tree) = state.research.tree.tree.as_ref() else {
        return div().into_any_element();
    };
    let rows = tree.comparison(&state.context);
    if rows.is_empty() {
        return div().into_any_element();
    }
    let entity = cx.entity();
    let mut body = div().id("dt-comparison").flex().flex_wrap().gap_2().p_3();
    for (index, row) in rows.iter().enumerate() {
        let row_id = row.id;
        let event = tree
            .node(row.id)
            .map(|node| event_label(node.event))
            .unwrap_or("Node");
        let open = entity.clone();
        let remove = entity.clone();
        let baseline = index == 0;
        let mut card = div()
            .id(SharedString::from(format!("dt-compare-row-{}", row.id)))
            .role(Role::Label)
            .aria_label(format!(
                "Node #{} · level {} ({:+}) · {}/hr ({:+.3}%) · {} ({})",
                row.id,
                row.level,
                row.level_delta,
                format::rate(row.rate),
                row.rate_delta / row.rate.max(1.0) * 100.0,
                format::projection(row.hours.abs()),
                if row.hours_delta.abs() < 1.0 / 3600.0 {
                    "±0".to_string()
                } else {
                    format!(
                        "{}{}",
                        if row.hours_delta >= 0.0 { "+" } else { "−" },
                        format::projection(row.hours_delta.abs())
                    )
                }
            ))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .min_w(px(190.0))
            .rounded(px(6.0))
            .border_2()
            .border_color(rgb(0x4caf50))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(format!("Node #{} · {event}", row.id)),
                    )
                    .when(baseline, |line| line.child(Badge::new("Baseline"))),
            )
            .child(comparison_line(
                "Level",
                row.level.to_string(),
                row.level_delta,
            ))
            .child(comparison_line(
                "EXP/hr",
                format::rate(row.rate),
                row.rate_delta,
            ))
            .child(comparison_line(
                "Time",
                format::projection(row.hours.abs()),
                Hours(row.hours_delta),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("dt-open-{}", row.id)), "Open")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, window, cx| {
                                open.update(cx, |state, cx| state.tree_select(row_id, window, cx));
                            }),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("dt-remove-{}", row.id)),
                            "Remove",
                        )
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            remove.update(cx, |state, cx| state.tree_toggle_compare(row_id, cx));
                        }),
                    ),
            );
        if baseline {
            card = card.opacity(0.95);
        }
        body = body.child(card);
    }
    Card::new()
        .header(CardHeader::new("Branch comparison").description(
            "Deltas are measured against the first node picked; the baseline row repeats its own figures.",
        ))
        .child(body)
        .into_any_element()
}

fn comparison_line(label: &'static str, value: String, delta: impl OrderedDelta) -> AnyElement {
    let (text, color) = delta.format();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(Caption::new(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(value)
                .when_some(text, |line, text| {
                    line.child(Caption::new(text).text_color(color))
                }),
        )
        .into_any_element()
}

/// The three delta shapes the comparison line can print.
trait OrderedDelta {
    fn format(self) -> (Option<String>, Hsla);
}

impl OrderedDelta for i64 {
    fn format(self) -> (Option<String>, Hsla) {
        let color = tone(self as f64);
        (Some(format!("{self:+}")), color)
    }
}

impl OrderedDelta for f64 {
    fn format(self) -> (Option<String>, Hsla) {
        let color = tone(self);
        (Some(delta_text(self)), color)
    }
}

/// An hours delta, printed as a duration rather than a bare number.
struct Hours(f64);

impl OrderedDelta for Hours {
    fn format(self) -> (Option<String>, Hsla) {
        let delta = self.0;
        if delta.abs() < 1.0 / 3600.0 {
            return (Some("±0".into()), tone(0.0));
        }
        let sign = if delta >= 0.0 { "+" } else { "−" };
        (
            Some(format!("{sign}{}", format::projection(delta.abs()))),
            tone(delta),
        )
    }
}

fn tone(delta: f64) -> Hsla {
    if delta > 0.0 {
        rgb(0x81c784).into()
    } else if delta < 0.0 {
        rgb(0xe57373).into()
    } else {
        rgb(0x8a8a98).into()
    }
}

fn delta_text(delta: f64) -> String {
    if delta.abs() < 1e-9 {
        return "±0".into();
    }
    if delta.abs() >= 1.0 {
        format!("{delta:+.0}")
    } else {
        format!("{delta:+.2}")
    }
}

/// The selected node's editor.
fn editor(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(selected) = state.research.tree.selected else {
        return div().into_any_element();
    };
    let Some(tree) = state.research.tree.tree.as_ref() else {
        return div().into_any_element();
    };
    let Some(node) = tree.node(selected) else {
        return div().into_any_element();
    };
    let event = event_label(node.event);
    let level = node.state.research_level;
    let rate = sim::exp_rate(&node.state, &state.context).total;
    let when = format::projection(tree.total_hours(selected));
    let auto = node.auto_insight;
    let children = tree.children(selected).len();
    let is_root = node.parent.is_none();
    let runner = state.tree_run.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let entity = cx.entity();

    let actions = div()
        .id("dt-node-actions")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(
            Button::new(
                "dt-save",
                if is_root {
                    "Start branch"
                } else if children == 0 {
                    "Save & next"
                } else {
                    "Save & re-sim"
                },
            )
            .primary()
            .size(ControlSize::Sm)
            .disabled(running)
            .on_click({
                let entity = entity.clone();
                move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.start_tree_job(Job::Save, cx));
                }
            }),
        )
        .child(
            Button::new("dt-branch", "Branch")
                .size(ControlSize::Sm)
                .disabled(running || is_root)
                .on_click({
                    let entity = entity.clone();
                    move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.start_tree_job(Job::Branch, cx));
                    }
                }),
        )
        .child(
            Button::new("dt-delete", "Delete")
                .size(ControlSize::Sm)
                .disabled(running || is_root)
                .on_click({
                    let delete = entity.clone();
                    move |_event, _window, cx| {
                        delete.update(cx, |state, cx| state.tree_delete(cx));
                    }
                }),
        )
        .child(
            Checkbox::new("dt-auto-insight", auto)
                .label("Auto Insight")
                .on_change({
                    let entity = cx.entity();
                    move |on, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.tree_set_auto_insight(selected, on, cx);
                        });
                    }
                }),
        );

    let mode = state.research.tree.mode;
    let mode_entity = cx.entity();
    let modes = SegmentedControl::new("dt-mode", mode.id())
        .size(ControlSize::Sm)
        .segment("upgrades", "Upgrades", None)
        .segment("shapes", "Shapes", None)
        .on_change(move |value, _window, cx| {
            let value = value.to_string();
            mode_entity.update(cx, |state, cx| {
                state.set_tree_mode(if value == "shapes" {
                    Mode::Shapes
                } else {
                    Mode::Upgrades
                });
                cx.notify();
            });
        });

    let mut body = div()
        .id("dt-editor-body")
        .flex()
        .flex_col()
        .gap_3()
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
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("dt-node-title")
                                .role(Role::Label)
                                .aria_label(format!("{event} · level {level}"))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(format!("{event} · Lv {level}")),
                        )
                        .child(Badge::new(format!("{}/hr", format::rate(rate)))),
                )
                .child(Caption::new(when)),
        )
        .child(actions);

    if running {
        if let Some(progress) = progress {
            let cancel = cx.entity();
            body = body.child(run_chrome::progress_card(
                "dt-run",
                "Working",
                "The step runs off the UI thread; cancelling leaves the tree as it was.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_tree_job(cx));
                },
            ));
        }
    } else if cancelled {
        body = body.child(run_chrome::cancelled_banner("dt-run-cancelled", false));
    }

    // The mode strip stays out of the scroll body: the segmented control
    // claims its own pointer press, which a scroll body swallows.
    let modes_row = div()
        .id("dt-mode-row")
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .pb_2()
        .child(modes)
        .child(
            div()
                .id("dt-mode-hint")
                .role(Role::Label)
                .aria_label(mode.id())
                .child(Caption::new(match mode {
                    Mode::Upgrades => "Buy levels; right-click lowers one.",
                    Mode::Shapes => "Nudge and turn the placed shapes.",
                })),
        );

    body = body.child(board(state, selected, cx));

    let auto_entity = entity.clone();
    let shapes_entity = entity.clone();
    let auto_rows = div()
        .id("dt-auto")
        .flex()
        .items_center()
        .gap_2()
        .child(Caption::new("Auto"))
        .child(
            Button::new("dt-auto-mags", "Magnifiers")
                .size(ControlSize::Sm)
                .disabled(running)
                .on_click(move |_event, _window, cx| {
                    auto_entity.update(cx, |state, cx| state.start_tree_job(Job::Mags, cx));
                }),
        )
        .child(
            Button::new("dt-auto-shapes", "Shapes")
                .size(ControlSize::Sm)
                .disabled(running)
                .on_click(move |_event, _window, cx| {
                    shapes_entity.update(cx, |state, cx| state.start_tree_job(Job::Shapes, cx));
                }),
        );
    body = body
        .child(auto_rows)
        .child(observations(state, selected, cx));

    div()
        .id("dt-editor")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(modes_row)
        .child(
            ScrollArea::new("dt-editor-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

/// The node's board: upgrades in one mode, shape nudging in the other.
fn board(state: &AppState, selected: u64, cx: &mut Context<AppState>) -> AnyElement {
    let Some(node) = state
        .research
        .tree
        .tree
        .as_ref()
        .and_then(|tree| tree.node(selected))
    else {
        return div().into_any_element();
    };
    let game = game();
    let overlay = node.state.overlay();
    let mode = state.research.tree.mode;
    let entity = cx.entity();
    let mut grid = div()
        .id("dt-board")
        .flex()
        .flex_col()
        .flex_none()
        .w(px(BOARD_CELL * game.grid.cols as f32))
        .rounded(px(4.0))
        .overflow_hidden();
    for row in 0..game.grid.rows {
        let mut line = div()
            .id(("dt-board-row", row))
            .flex()
            .flex_row()
            .flex_none()
            .h(px(BOARD_CELL));
        for col in 0..game.grid.cols {
            let cell = row * game.grid.cols + col;
            let info = game.node_at(cell);
            let level = node.state.grid.level(cell);
            let tint = colors::goal_color(info.and_then(|node| node.goal.as_deref()));
            let fill = match info {
                None => cx.theme().colors.bg,
                Some(_) if level == 0 => tint.opacity(0.08),
                Some(info) => {
                    let strength = (level as f32 / info.max_lv.max(1) as f32).clamp(0.2, 1.0);
                    tint.opacity(0.12 + 0.33 * strength)
                }
            };
            let border = match usize::try_from(overlay[cell])
                .ok()
                .and_then(|shape| game.shapes.get(shape))
            {
                Some(_) => colors::shape_color(overlay[cell] as usize),
                None => cx.theme().colors.border,
            };
            let name = info
                .map(|node| node.name.replace('_', " "))
                .unwrap_or_else(|| "empty".into());
            let bump = entity.clone();
            let lower = entity.clone();
            let mut square = div()
                .id(SharedString::from(format!("dt-cell-{cell}")))
                .role(Role::Button)
                .aria_label(format!(
                    "{} · {name} · level {level}",
                    idleon_research::model::coord(cell)
                ))
                .flex_none()
                .size(px(BOARD_CELL))
                .bg(fill)
                .border_1()
                .border_color(border)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |state, _event, _window, cx| {
                        state.tree_bump_grid(cell, 1, cx);
                    }),
                );
            if mode == Mode::Upgrades {
                square = square.on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(move |state, _event, _window, cx| {
                        state.tree_bump_grid(cell, -1, cx);
                    }),
                );
            } else {
                square = square.child(div().size_full().child(Caption::new(if level > 0 {
                    level.to_string()
                } else {
                    String::new()
                })));
            }
            let _ = (bump, lower);
            line = line.child(square);
        }
        grid = grid.child(line);
    }

    let shapes: AnyElement = if mode == Mode::Shapes {
        let mut rows = div().id("dt-shapes").flex().flex_col().gap_1();
        for (index, placement) in node.state.placements.iter().enumerate() {
            let def = game.shapes.get(placement.shape);
            let name = def
                .map(|def| def.name.clone())
                .unwrap_or_else(|| format!("Shape {}", placement.shape + 1));
            let left = entity.clone();
            let right = entity.clone();
            let up = entity.clone();
            let down = entity.clone();
            let turn = entity.clone();
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("dt-shape-{index}")))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(70.0)).truncate().child(Caption::new(name)))
                    .child(
                        Caption::new(format!(
                            "{} · {}°",
                            idleon_research::model::coord(
                                shape::cells_at(placement).first().copied().unwrap_or(0)
                            ),
                            placement.rot.round() as i64
                        ))
                        .text_color(cx.theme().colors.fg_muted),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dt-shape-left-{index}")), "◀")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                left.update(cx, |state, cx| {
                                    state.tree_nudge_shape(index, -5.0, 0.0, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dt-shape-right-{index}")), "▶")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                right.update(cx, |state, cx| {
                                    state.tree_nudge_shape(index, 5.0, 0.0, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dt-shape-up-{index}")), "▲")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                up.update(cx, |state, cx| {
                                    state.tree_nudge_shape(index, 0.0, -5.0, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dt-shape-down-{index}")), "▼")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                down.update(cx, |state, cx| {
                                    state.tree_nudge_shape(index, 0.0, 5.0, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dt-shape-turn-{index}")), "⟳")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                turn.update(cx, |state, cx| state.tree_turn_shape(index, cx));
                            }),
                    ),
            );
        }
        if node.state.placements.is_empty() {
            rows = rows.child(Caption::new("No shapes placed; run Auto Shapes."));
        }
        rows.into_any_element()
    } else {
        div().into_any_element()
    };

    div()
        .id("dt-board-wrap")
        .flex()
        .flex_col()
        .gap_2()
        .child(grid)
        .child(shapes)
        .into_any_element()
}

/// The observation editor: the magnifier pool and the slots it can fill.
fn observations(state: &AppState, selected: u64, cx: &mut Context<AppState>) -> AnyElement {
    let Some(node) = state
        .research
        .tree
        .tree
        .as_ref()
        .and_then(|tree| tree.node(selected))
    else {
        return div().into_any_element();
    };
    let context = &state.context;
    let pool = sim::mag_pool(&node.state, context);
    let cap = sim::mag_cap(node.state.research_level);
    let usable = sim::available_slots(node.state.research_level, &node.state.observations);
    let picked = state
        .research
        .tree
        .observation
        .filter(|obs| usable.contains(obs))
        .or_else(|| usable.first().copied());
    let entity = cx.entity();

    let mut pool_row = div()
        .id("dt-mag-pool")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2();
    for kind in [MagKind::Regular, MagKind::Monocle, MagKind::Kaleidoscope] {
        pool_row = pool_row.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size(px(8.0)).rounded_full().bg(mag_color(kind)))
                .child(Caption::new(format!(
                    "{} {}/{}",
                    kind_label(kind),
                    pool.free(kind, &node.state.mags),
                    pool.of(kind)
                ))),
        );
    }

    let mut slots = div()
        .id("dt-observations")
        .flex()
        .flex_wrap()
        .gap_1()
        .max_w(px(400.0));
    for obs in usable.iter().copied() {
        let count = node
            .state
            .mags
            .iter()
            .filter(|mag| mag.slot == Some(obs))
            .count();
        let name = game()
            .observations
            .get(obs)
            .map(|observation| observation.name.replace('_', " "))
            .unwrap_or_else(|| format!("Observation {obs}"));
        let pick = entity.clone();
        let active = picked == Some(obs);
        slots = slots.child(
            div()
                .id(SharedString::from(format!("dt-obs-{obs}")))
                .role(Role::Button)
                .aria_selected(active)
                .aria_label(format!(
                    "{name} · insight {} · {count}/{cap} magnifiers",
                    node.state.observations.insight(obs)
                ))
                .flex()
                .flex_col()
                .gap_0p5()
                .p_1()
                .min_w(px(88.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(if active {
                    rgb(0x64b5f6).into()
                } else {
                    cx.theme().colors.border
                })
                .cursor_pointer()
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.tree_pick_observation(obs, cx));
                })
                .child(div().text_size(px(10.0)).truncate().child(name))
                .child(
                    Caption::new(format!(
                        "IL {} · {count}/{cap}",
                        node.state.observations.insight(obs)
                    ))
                    .text_color(cx.theme().colors.fg_muted),
                ),
        );
    }

    let mut assign = div()
        .id("dt-mag-assign")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1();
    if let Some(obs) = picked {
        for kind in [MagKind::Regular, MagKind::Monocle, MagKind::Kaleidoscope] {
            let assign_entity = entity.clone();
            let free = pool.free(kind, &node.state.mags);
            assign = assign.child(
                Button::new(
                    SharedString::from(format!("dt-mag-add-{}", kind_label(kind))),
                    kind_label(kind),
                )
                .size(ControlSize::Sm)
                .disabled(free == 0)
                .on_click(move |_event, _window, cx| {
                    assign_entity.update(cx, |state, cx| state.tree_assign_mag(kind, obs, cx));
                }),
            );
        }
        let lift = entity.clone();
        assign = assign.child(
            Button::new("dt-mag-lift", "Remove")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    lift.update(cx, |state, cx| state.tree_lift_mag(obs, cx));
                }),
        );
    } else {
        assign = assign.child(Caption::new("No observation is usable at this level."));
    }

    Card::new()
        .header(CardHeader::new("Observations").description(format!(
            "{} slots · {cap} magnifiers per slot",
            usable.len()
        )))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(pool_row)
                .child(slots)
                .child(assign),
        )
        .into_any_element()
}

fn event_label(event: Event) -> &'static str {
    match event {
        Event::Start => "▶ Start",
        Event::Level => "▲ Level up",
        Event::Insight => "◈ Insight",
        Event::LevelAndInsight => "▲◈ Both",
        Event::Tournament => "⚑ Tournament",
        Event::End => "■ End",
    }
}

fn event_color(event: Event) -> Hsla {
    match event {
        Event::Start => rgb(0x9a9a9a).into(),
        Event::Level => rgb(0xffd700).into(),
        Event::Insight => rgb(0xce93d8).into(),
        Event::LevelAndInsight => rgb(0x4dd0e1).into(),
        Event::Tournament => rgb(0xff9800).into(),
        Event::End => rgb(0x9a9a9a).into(),
    }
}

fn mag_color(kind: MagKind) -> Hsla {
    match kind {
        MagKind::Regular => rgb(0x81c784).into(),
        MagKind::Monocle => rgb(0xce93d8).into(),
        MagKind::Kaleidoscope => rgb(0x4dd0e1).into(),
    }
}

fn kind_label(kind: MagKind) -> &'static str {
    match kind {
        MagKind::Regular => "Glass",
        MagKind::Monocle => "Monocle",
        MagKind::Kaleidoscope => "Kaleido",
    }
}

/// `(+1.2M, +3.4%)` for a child rate against its parent, when it moved.
fn rate_delta(parent: f64, child: f64) -> Option<(String, String)> {
    if parent <= 0.0 || (child - parent).abs() < 1e-9 {
        return None;
    }
    let diff = child - parent;
    let sign = if diff >= 0.0 { "+" } else { "" };
    let pct = diff / parent * 100.0;
    Some((
        format!("{sign}{}", format::rate(diff)),
        format!("{sign}{pct:.1}%"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_change_prints_both_the_amount_and_the_share() {
        let (amount, pct) = rate_delta(100.0, 110.0).expect("a change");
        assert_eq!(amount, "+10");
        assert_eq!(pct, "+10.0%");
        assert!(rate_delta(100.0, 100.0).is_none());
        assert!(rate_delta(0.0, 10.0).is_none());
    }

    #[test]
    fn the_mode_ids_round_trip_with_the_segmented_control() {
        assert_eq!(Mode::Upgrades.id(), "upgrades");
        assert_eq!(Mode::Shapes.id(), "shapes");
    }
}
