//! Rates pane: the EXP breakdown and the observation board.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled,
    Subscription, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_research::model::SimContext;
use idleon_research::{game, sim};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, Statistic, TEXT_CAPTION, number_state, number_value,
    show_number,
};
use std::rc::Rc;

pub(crate) fn rates_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let breakpoint = Breakpoint::of(window, cx);
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let rate = state.rate();
    let points = state.grid_points();
    let previous = state.research.observation_rows.rows.clone();
    let rows = observation_rows(state);
    let tree = state.breakdown.clone();
    // The breakdown reads the same layout as the rows, so it is rebuilt with them.
    if !Rc::ptr_eq(&previous, &rows) {
        let root = idleon_research::breakdown::exp(&state.simulation, &state.context);
        tree.update(cx, |tree, cx| tree.set_root(root, cx));
    }
    let cards = KpiStrip::new()
        .child(
            Statistic::new(
                "kpi-rate",
                "Research EXP/hr",
                format::rate(rate.total.max(0.0)),
            )
            .caption(format!(
                "{} · observations × multipliers",
                format::count(rate.total.max(0.0).round() as u64)
            ))
            .large(),
        )
        .child(
            Statistic::new("kpi-multi", "Multiplier", grouped(rate.multi, 2))
                .caption("additive and grid bonuses"),
        )
        .child(
            Statistic::new("kpi-obs", "Observation EXP", grouped(rate.obs_base, 1))
                .caption("before multipliers"),
        )
        .child(
            Statistic::new(
                "kpi-points",
                "Free grid points",
                format::count(points.free as u64),
            )
            .caption(format!("{} spent of {}", points.spent, points.earned)),
        )
        .child(
            Statistic::new(
                "kpi-shapes",
                "Shapes",
                format::count(state.simulation.placements.len() as u64),
            )
            .caption(format!("{} unlocked", state.shapes_owned())),
        )
        .child(
            Statistic::new(
                "kpi-mags",
                "Magnifiers",
                format::count(state.simulation.mags.len() as u64),
            )
            .caption(format!("{} owned", state.magnifiers_owned())),
        );
    let cards = div()
        .id("kpi-row")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Research EXP/hr {} · multiplier {:.2}",
            format::rate(rate.total),
            rate.multi
        ))
        .child(cards);

    div()
        .id("dashboard")
        .flex()
        .flex_col()
        .gap_3()
        .size_full()
        .min_h_0()
        .child(cards)
        .child(layout::split_pane(
            "rates-body",
            breakpoint,
            div()
                .id("observations-column")
                .flex()
                .flex_col()
                .min_w_0()
                .when(!breakpoint.is_compact(), |column| column.flex_1().min_h_0())
                .child(observations_card(state, rows, table_height, cx)),
            div()
                .id("supplements-column")
                .flex()
                .flex_col()
                .min_h_0()
                .gap_3()
                .child(formula_card(tree))
                .child(supplements_card(state, window, cx)),
            px(340.0),
        ))
}

/// The shared formula tree, collapsed to a fixed height inside the side rail.
fn formula_card(tree: Entity<idleon_ui::formula::FormulaTree>) -> impl IntoElement {
    Card::new()
        .header(
            CardHeader::new("EXP formula")
                .description("Search every factor and copy the full tree"),
        )
        .child(
            div()
                .id("formula-card-body")
                .h(px(220.0))
                .min_h_0()
                .child(tree),
        )
}

/// One save-derived supplement the Rates pane edits: its element id, label,
/// range, step and decimals, where it lives in the context and the setter
/// that clamps and stores it.
pub(super) struct Supplement {
    id: &'static str,
    label: &'static str,
    range: (f64, f64),
    step: f64,
    precision: usize,
    get: fn(&SimContext) -> f64,
    set: fn(&mut AppState, f64, &mut Context<AppState>),
}

pub(super) const SUPPLEMENTS: [Supplement; 7] = [
    Supplement {
        id: "supp-svrxp",
        label: "Server EXP scale",
        range: (1.0, 10.0),
        step: 0.05,
        precision: 2,
        get: |context| context.server_var_res_xp,
        set: AppState::set_server_var_res_xp,
    },
    Supplement {
        id: "supp-abm",
        label: "All-bonus multiplier",
        range: (1.0, 100.0),
        step: 0.05,
        precision: 2,
        get: |context| context.all_bonus_multi,
        set: AppState::set_all_bonus_multi,
    },
    Supplement {
        id: "supp-extra-pct",
        label: "Extra account %",
        range: (0.0, 10_000.0),
        step: 5.0,
        precision: 0,
        get: |context| context.extra_pct,
        set: AppState::set_extra_pct,
    },
    Supplement {
        id: "supp-extra-multi",
        label: "Extra account ×",
        range: (0.0, 100.0),
        step: 0.05,
        precision: 2,
        get: |context| context.extra_multiplier,
        set: AppState::set_extra_multiplier,
    },
    Supplement {
        id: "supp-points",
        label: "Bonus grid points",
        range: (0.0, 100_000.0),
        step: 10.0,
        precision: 0,
        get: |context| f64::from(context.bonus_grid_points),
        set: AppState::set_bonus_grid_points,
    },
    Supplement {
        id: "supp-shapes",
        label: "Extra shapes",
        range: (0.0, 100.0),
        step: 1.0,
        precision: 0,
        get: |context| f64::from(context.extra_shapes),
        set: AppState::set_extra_shapes,
    },
    Supplement {
        id: "supp-mags",
        label: "Extra magnifiers",
        range: (0.0, 200.0),
        step: 1.0,
        precision: 0,
        get: |context| f64::from(context.extra_magnifiers),
        set: AppState::set_extra_magnifiers,
    },
];

/// The number fields behind [`SUPPLEMENTS`], built once with the section
/// state. An edit flows through the supplement's setter, which clamps it.
pub(super) fn supplement_fields(
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Vec<Entity<InputState>> {
    SUPPLEMENTS
        .iter()
        .map(|supplement| {
            let (min, max) = supplement.range;
            let field = number_state(min, max, supplement.step, window, cx);
            let set = supplement.set;
            subscriptions.push(cx.subscribe(&field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    set(state, value, cx);
                }
            }));
            field
        })
        .collect()
}

/// One supplement row: its name, its value, and the field that edits it.
fn supplement_row(state: &AppState, index: usize, window: &mut Window, cx: &mut App) -> AnyElement {
    let supplement = &SUPPLEMENTS[index];
    let value = (supplement.get)(&state.context);
    let precision = supplement.precision;
    let field = &state.research.supplement_fields[index];
    show_number(field, value, precision, window, cx);
    let shown = format!("{value:.precision$}");
    div()
        .id(supplement.id)
        .role(gpui::Role::Label)
        .aria_label(format!("{}: {shown}", supplement.label))
        .aria_value(shown)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .h(px(30.0))
        .child(Caption::new(supplement.label))
        .child(
            crate::ui::number_field(
                SharedString::from(format!("{}-stepper", supplement.id)),
                field,
                supplement.label,
            )
            .w(px(140.0)),
        )
        .into_any_element()
}

fn supplements_card(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let open = state.supplements_open;
    let entity = cx.entity();
    let toggle = entity.clone();
    let reset = entity.clone();
    let header = CardHeader::new("Supplements")
        .description("Save-derived multipliers that live outside the grid")
        .action(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new("supp-reset")
                        .label("Reset")
                        .small()
                        .disabled(!open)
                        .on_click(move |_event, _window, cx| {
                            reset.update(cx, |state, cx| state.reset_supplements(cx));
                        }),
                )
                .child(
                    Button::new("supp-toggle")
                        .label(if open { "Hide" } else { "Show" })
                        .small()
                        .on_click(move |_event, _window, cx| {
                            toggle.update(cx, |state, cx| state.set_supplements_open(!open, cx));
                        }),
                ),
        );
    let card = Card::new().header(header);
    if !open {
        return card.child(Caption::new(
            "Hidden; the simulated multipliers keep their values.",
        ));
    }
    card.child(
        div()
            .id("supplements-card")
            .flex()
            .flex_col()
            .gap_1()
            .children((0..SUPPLEMENTS.len()).map(|index| supplement_row(state, index, window, cx))),
    )
}

/// One observation row, formatted when the layout last changed.
pub(super) struct ObservationRow {
    obs: usize,
    name: SharedString,
    level: SharedString,
    mags: SharedString,
    insight_rate: SharedString,
}

/// The usable observations and their insight rates for this layout, rebuilt
/// only when the simulation or the context changes.
fn observation_rows(state: &mut AppState) -> Rc<Vec<ObservationRow>> {
    let (simulation, context) = (&state.simulation, &state.context);
    state.research.observation_rows.get_by(
        |(cached_simulation, cached_context)| {
            cached_simulation == simulation && cached_context == context
        },
        || (simulation.clone(), context.clone()),
        || {
            let level = simulation.research_level;
            let to_find = sim::observations_to_find(level, &simulation.observations.found);
            let overlay = simulation.overlay();
            let kal_map = sim::kaleidoscope_map(&simulation.mags, game().observations.len());
            let kal_base = sim::kalei_base(&simulation.grid, &overlay, context);
            (0..to_find.min(game().observations.len()))
                .filter(|obs| sim::observation_usable(*obs, level, &simulation.observations))
                .map(|obs| {
                    let insight_rate =
                        sim::insight_rate(obs, simulation, &overlay, context, &kal_map, kal_base);
                    let mags = simulation
                        .mags
                        .iter()
                        .filter(|mag| mag.slot == Some(obs))
                        .count();
                    ObservationRow {
                        obs,
                        name: game().observations[obs].name.replace('_', " ").into(),
                        level: format!("Lv {}", simulation.observations.insight_level[obs]).into(),
                        mags: mags.to_string().into(),
                        insight_rate: format::rate(insight_rate).into(),
                    }
                })
                .collect()
        },
    )
}

fn observations_card(
    state: &mut AppState,
    rows: Rc<Vec<ObservationRow>>,
    table_height: gpui::Pixels,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let theme = cx.theme();
    let (muted, accent, sunken) = (theme.muted_foreground, theme.primary, theme.muted);
    let level = state.simulation.research_level;
    let to_find = sim::observations_to_find(level, &state.simulation.observations.found);
    let shown = rows.len();
    let entity = cx.entity();
    let selected = state
        .research
        .selected_observation
        .filter(|obs| *obs < game().observations.len());
    let head = |label: &'static str| TableHead::new().child(label);
    let narrow = px(40.0);
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(head("#").w(narrow))
                    .child(head("Observation"))
                    .child(head("Insight"))
                    .child(head("+").w(narrow))
                    .child(head("Mags").text_right())
                    .child(head("Ins/hr").text_right())
                    .child(head("").w(narrow)),
            ),
        )
        .child(TableBody::new().children(rows.iter().map(|row| {
            let obs = row.obs;
            let bump = entity.clone();
            let toggle = entity.clone();
            let pick = entity.clone();
            TableRow::new()
                .child(
                    TableCell::new()
                        .w(narrow)
                        .child(div().id(("obs-row", obs)).child(format!("{}", obs + 1))),
                )
                .child(
                    TableCell::new().child(
                        div()
                            .id(("obs-name", obs))
                            .min_w_0()
                            .cursor_pointer()
                            .when(selected == Some(obs), |name| name.text_color(accent))
                            .on_click(move |_event, window, cx| {
                                pick.update(cx, |state, cx| {
                                    state.select_observation(obs, cx);
                                    window.focus(&state.focus, cx);
                                });
                            })
                            .child(row.name.clone()),
                    ),
                )
                .child(TableCell::new().child(row.level.clone()))
                .child(
                    TableCell::new().w(narrow).child(
                        Button::new(("obs-plus", obs))
                            .icon(IconName::Plus)
                            .ghost()
                            .xsmall()
                            .tooltip("Raise insight")
                            .on_click(move |_event, _window, cx| {
                                bump.update(cx, |state, cx| state.bump_insight(obs, 1, cx));
                            }),
                    ),
                )
                .child(TableCell::new().text_right().child(row.mags.clone()))
                .child(
                    TableCell::new()
                        .text_right()
                        .child(row.insight_rate.clone()),
                )
                .child(
                    TableCell::new().w(narrow).child(
                        Button::new(("obs-toggle", obs))
                            .icon(IconName::RefreshCw)
                            .ghost()
                            .xsmall()
                            .tooltip("Toggle found")
                            .on_click(move |_event, _window, cx| {
                                toggle.update(cx, |state, cx| state.toggle_found(obs, cx));
                            }),
                    ),
                )
        })));

    let selection = selected
        .map(|obs| {
            format!(
                "Selected: {} (#{})",
                game().observations[obs].name.replace('_', " "),
                obs + 1
            )
        })
        .unwrap_or_else(|| "No observation selected".into());
    Card::new()
        .header(CardHeader::new("Observations").description(format!(
            "{shown} usable of {to_find} found at research level {level}"
        )))
        .child(div().h(px(1.0)).bg(sunken))
        .child(
            div()
                .min_w_0()
                .child(table)
                .overflow_y_scrollbar()
                .id("observations-scroll")
                .h(table_height),
        )
        .child(div().text_size(TEXT_CAPTION).text_color(muted).child(
            "Insight/hr counts monocles only; magnifiers marked here sit on that observation.",
        ))
        .child(
            div()
                .id("observations-selected")
                .role(gpui::Role::Label)
                .aria_label(selection.clone())
                .text_size(TEXT_CAPTION)
                .text_color(accent)
                .child(selection),
        )
}

/// `value` with `decimals` places and its whole part grouped in thousands,
/// as in `30,808.96`.
fn grouped(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text.split_once('.') {
        Some((whole, fraction)) => {
            format!("{}.{fraction}", format::exact(whole.parse().unwrap_or(0.0)))
        }
        None => format::exact(value),
    }
}
