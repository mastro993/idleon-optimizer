//! Dashboard pane: the summary, grid and observation previews and the three
//! breakdown trees, ported from the upstream `js/ui/dashboard.js`.
//!
//! Every number comes from [`idleon_research::dashboard`], which composes the same
//! simulation functions the Optimizer tab reads. The previews are read-only:
//! clicking one opens the editor it belongs to.

use crate::sections::research::Tab;
use crate::state::AppState;
use crate::ui::formula_tree;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::dashboard;
use idleon_research::game;
use idleon_research::model::coord;
use idleon_ui::format;

use idleon_ui::formula::FormulaTree;

/// Logical pixels per preview cell.
const CELL_PX: f32 = 24.0;
/// Observation cards hold their name and two rates; this keeps eight a row at
/// the minimum window width, as the upstream `.obs-grid` does.
const OBS_WIDTH: f32 = 132.0;

/// The three formula trees plus the grid-point tree the dashboard owns.
pub(crate) struct Trees {
    pub(crate) exp: Entity<FormulaTree>,
    pub(crate) afk: Entity<FormulaTree>,
    pub(crate) insight: Entity<FormulaTree>,
    pub(crate) points: Entity<FormulaTree>,
}

impl Trees {
    /// Create the dashboard's trees; the pane replaces their roots each render.
    pub(crate) fn new(window: &mut Window, cx: &mut gpui::App) -> Self {
        Self {
            exp: formula_tree("dash-exp-formula", window, cx),
            afk: formula_tree("dash-afk-formula", window, cx),
            insight: formula_tree("dash-insight-formula", window, cx),
            points: formula_tree("dash-points-formula", window, cx),
        }
    }
}

pub(crate) fn dashboard_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let summary = dashboard::summary(&state.simulation, &state.context);
    let cards = dashboard::observations(&state.simulation, &state.context);
    let exp_root = idleon_research::breakdown::exp(&state.simulation, &state.context);
    let afk_root = idleon_research::breakdown::afk(&state.simulation, &state.context);
    let insight_root = idleon_research::breakdown::insight(&state.simulation, &state.context);
    let points_root = idleon_research::breakdown::grid_points(&state.simulation, &state.context);
    state
        .research_trees
        .exp
        .update(cx, |tree, cx| tree.set_root(exp_root, cx));
    state
        .research_trees
        .afk
        .update(cx, |tree, cx| tree.set_root(afk_root, cx));
    state
        .research_trees
        .insight
        .update(cx, |tree, cx| tree.set_root(insight_root, cx));
    state
        .research_trees
        .points
        .update(cx, |tree, cx| tree.set_root(points_root, cx));

    ScrollArea::new("dashboard-scroll")
        .vertical()
        .shadows()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("research-dashboard")
                .flex()
                .flex_col()
                .gap_3()
                .child(summary_card(state, summary, cx))
                .child(grid_card(state, cx))
                .child(observations_card(state, cards, cx))
                .child(
                    div()
                        .id("dashboard-breakdowns")
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .items_start()
                        .child(breakdown_card(
                            "dash-exp-formula",
                            "EXP rate breakdown",
                            "Every factor in the EXP/hr result",
                            state.research_trees.exp.clone(),
                            340.0,
                        ))
                        .child(breakdown_card(
                            "dash-afk-formula",
                            "AFK rate",
                            "The share of active EXP earned while away",
                            state.research_trees.afk.clone(),
                            340.0,
                        ))
                        .child(breakdown_card(
                            "dash-insight-formula",
                            "Insight multiplier",
                            "What one monocle applies before neighbours",
                            state.research_trees.insight.clone(),
                            340.0,
                        )),
                ),
        )
        .into_any_element()
}

fn summary_card(
    state: &mut AppState,
    summary: dashboard::Summary,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = &cx.theme().colors;
    let level = state.simulation.research_level;
    let kpis = div()
        .id("dash-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Research level {level} · {} /hr active · {} /hr offline · AFK {:.1}%",
            format::rate(summary.active_rate),
            format::rate(summary.offline_rate),
            summary.afk_rate * 100.0
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi("dash-level", "Research level", level as f64, 0, None))
        .child(kpi(
            "dash-active",
            "Active EXP/hr",
            summary.active_rate,
            0,
            Some(format::rate(summary.active_rate)),
        ))
        .child(kpi(
            "dash-offline",
            "Offline EXP/hr",
            summary.offline_rate,
            0,
            Some(format::rate(summary.offline_rate)),
        ))
        .child(kpi(
            "dash-afk",
            "AFK rate",
            summary.afk_rate * 100.0,
            1,
            Some(format!("{:.1}%", summary.afk_rate * 100.0)),
        ))
        .child(kpi(
            "dash-magnifiers",
            "Magnifiers",
            summary.magnifiers as f64,
            0,
            Some(format!("{} owned", summary.magnifiers)),
        ))
        .child(kpi(
            "dash-mag-cap",
            "Max per slot",
            summary.mag_cap as f64,
            0,
            Some(format!("level {}", level)),
        ))
        .child(kpi(
            "dash-points",
            "Grid points",
            summary.grid_points.free as f64,
            0,
            Some(format!(
                "{} free of {} earned",
                summary.grid_points.free, summary.grid_points.earned
            )),
        ));

    let times = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(time_stat(
            "dash-hours-active",
            "Active time to next level",
            summary.hours_active,
            colors.fg_muted,
        ))
        .child(time_stat(
            "dash-hours-offline",
            "Offline time to next level",
            summary.hours_offline,
            colors.fg_muted,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(220.0))
                .flex_1()
                .child(
                    div()
                        .id("dash-exp")
                        .role(gpui::Role::Label)
                        .aria_label(format!(
                            "Next level: {} of {} EXP",
                            format::rate(summary.exp_current),
                            format::rate(summary.exp_required)
                        ))
                        .text_size(px(11.0))
                        .text_color(colors.fg_muted)
                        .child(format!(
                            "EXP {} / {}",
                            format::rate(summary.exp_current),
                            format::rate(summary.exp_required)
                        )),
                )
                .child(ProgressBar::new("dash-progress", summary.progress)),
        );

    Card::new()
        .header(
            CardHeader::new("Summary")
                .description("The loaded save, the current rate and the next level"),
        )
        .child(kpis)
        .child(times)
        .child(
            div()
                .id("dash-points-card")
                .flex()
                .flex_col()
                .gap_2()
                .child(Caption::new("Grid points").text_color(colors.fg_muted))
                .child(
                    div()
                        .id("dash-points-formula")
                        .h(px(190.0))
                        .min_h_0()
                        .child(state.research_trees.points.clone()),
                ),
        )
        .into_any_element()
}

/// One KPI with its stable id; the caption names the scaled value.
fn kpi(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    caption: Option<String>,
) -> AnyElement {
    let card = KpiCard::new(
        Statistic::new(id, label, value)
            .decimals(decimals)
            .size(TextSize::Lg),
    );
    match caption {
        Some(caption) => card.caption(caption).into_any_element(),
        None => card.into_any_element(),
    }
}

fn time_stat(id: &'static str, label: &'static str, hours: f64, muted: gpui::Hsla) -> AnyElement {
    let reading = format::hours(hours);
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label}: {reading}"))
        .flex()
        .flex_col()
        .gap_0p5()
        .min_w(px(190.0))
        .flex_1()
        .child(div().text_size(px(11.0)).text_color(muted).child(label))
        .child(div().text_size(px(16.0)).child(reading))
        .into_any_element()
}

fn grid_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let game = game();
    let overlay = state.simulation.overlay();
    let entity = cx.entity();
    let open = entity.clone();
    // The cells are laid out in rows so the board keeps the upstream 20 columns.
    let mut rows = div().flex().flex_col().gap_0p5();
    for row in 0..game.grid.rows {
        let mut line = div().flex().flex_row().gap_0p5();
        for col in 0..game.grid.cols {
            let cell = row * game.grid.cols + col;
            line = line.child(preview_cell(state, cell, overlay[cell], cx));
        }
        rows = rows.child(line);
    }
    let board = div()
        .id("dash-grid")
        .flex()
        .flex_col()
        .gap_0p5()
        .child(rows);
    Card::new()
        .header(
            CardHeader::new("Research grid")
                .description("A read-only preview; open the editor to change a square")
                .action(
                    Button::new("dash-grid-open", "Open editor")
                        .size(ControlSize::Sm)
                        .on_click(move |_event, window, cx| {
                            open.update(cx, |state, cx| {
                                state.set_tab(Tab::Grid, cx);
                                window.focus(&state.focus, cx);
                            });
                        }),
                ),
        )
        .child(board)
        .child(
            div()
                .id("dash-grid-note")
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!(
                    "{} squares leveled · {} cells under a shape",
                    state.simulation.grid.live_cells().count(),
                    overlay.iter().filter(|shape| **shape >= 0).count()
                )),
        )
        .into_any_element()
}

fn preview_cell(
    state: &AppState,
    cell: usize,
    shape: i32,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let level = state.simulation.grid.level(cell);
    let node = game().node_at(cell);
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
    let border = match usize::try_from(shape)
        .ok()
        .and_then(|index| game().shapes.get(index))
    {
        Some(_) => super::colors::shape_color(shape as usize),
        None if level > 0 => tint.opacity(0.55),
        None => colors.border,
    };
    let name = node
        .map(|node| node.name.replace('_', " "))
        .unwrap_or_else(|| "empty".into());
    let label = format!("{} · {name} · level {level}", coord(cell));
    let id = cx.entity();
    div()
        .id(format!("dash-cell-{cell}"))
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(CELL_PX))
        .bg(fill)
        .border_1()
        .border_color(border)
        .rounded(px(3.0))
        .text_size(px(10.0))
        .text_color(colors.fg)
        .tooltip(ely_gpui_component::primitives::Tooltip::text(label))
        .on_click(move |_event, window, cx| {
            id.update(cx, |state, cx| state.open_grid_cell(cell, window, cx));
        })
        .child(if level > 0 {
            level.to_string()
        } else {
            String::new()
        })
        .into_any_element()
}

fn observations_card(
    state: &mut AppState,
    cards: Vec<dashboard::ObservationCard>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let found = cards.iter().filter(|card| card.found).count();
    let total = cards.len();
    let mut board = div().id("dash-observations").flex().flex_wrap().gap_2();
    for card in cards {
        board = board.child(observation_cell(card, cx));
    }
    Card::new()
        .header(CardHeader::new("Observations").description(format!(
            "{found} found of {total} slots at research level {}",
            state.simulation.research_level
        )))
        .child(board)
        .child(
            div()
                .id("dash-obs-note")
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "M: magnifying glasses · O: optical monocles · K: kaleidoscopes · Adj: neighbouring kaleidoscopes. Click one to open its editor.",
                ),
        )
        .into_any_element()
}

fn observation_cell(card: dashboard::ObservationCard, cx: &mut Context<AppState>) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let obs = card.obs;
    let id = cx.entity();
    let mags = format!(
        "{}{}{}{}",
        if card.regular > 0 {
            format!("M:{} ", card.regular)
        } else {
            String::new()
        },
        if card.monocles > 0 {
            format!("O:{} ", card.monocles)
        } else {
            String::new()
        },
        if card.kaleidoscopes > 0 {
            format!("K:{} ", card.kaleidoscopes)
        } else {
            String::new()
        },
        if card.neighbours > 0 {
            format!("Adj:{}", card.neighbours)
        } else {
            String::new()
        },
    );
    let label = format!(
        "{} · insight level {} · {} exp/hr · {} insight/hr",
        card.name,
        card.insight_level,
        format::rate(card.exp),
        format::rate(card.insight)
    );
    div()
        .id(format!("dash-obs-{obs}"))
        .role(gpui::Role::Button)
        .aria_label(label)
        .flex()
        .flex_col()
        .gap_0p5()
        .w(px(OBS_WIDTH))
        .min_h(px(86.0))
        .p_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(if card.found {
            colors.success.opacity(0.5)
        } else {
            colors.border
        })
        .bg(if card.found {
            colors.success.opacity(0.08)
        } else {
            colors.surface
        })
        .when(!card.found, |el| el.opacity(0.45))
        .hover(|style| style.bg(colors.hover))
        .cursor_pointer()
        .on_click(move |_event, _window, cx| {
            id.update(cx, |state, cx| state.open_observation(obs, cx));
        })
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(card.name.clone()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(colors.warning)
                .child(format!("LV {}", card.insight_level)),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(colors.info)
                .child(if mags.is_empty() {
                    "—".to_string()
                } else {
                    mags
                }),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(colors.success)
                .child(if card.exp > 0.0 {
                    format!("{} exp", format::rate(card.exp))
                } else {
                    String::new()
                }),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(colors.accent)
                .child(if card.insight > 0.0 {
                    format!("{} ins", format::rate(card.insight))
                } else {
                    String::new()
                }),
        )
        .into_any_element()
}

fn breakdown_card(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    tree: Entity<FormulaTree>,
    height: f32,
) -> AnyElement {
    Card::new()
        .header(CardHeader::new(title).description(description))
        .child(div().id(id).h(px(height)).min_h_0().flex_1().child(tree))
        .into_any_element()
}
