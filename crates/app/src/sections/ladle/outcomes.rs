//! The forecast comparison and the meal outcomes table (LD-03).
//!
//! The comparison runs the same horizon under the saved assignments, the
//! optimized assignments and the donation policy, then lists every discovered
//! meal with the levels each source bought and the target the `Selected meal
//! targets` objective spends toward. The numbers come from
//! [`idleon_ladle::kitchen::forecast`] and its plans; the meal list runs to the
//! whole menu, so it is a virtualized kit `DataTable` whose rows
//! [`super::State::sync`] rebuilds when the forecast, the scenario or the sort
//! change.

use super::State;
use super::kitchens::{ChartPoint, Figures};
use super::summary::count;
use super::table::{table, text};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_ladle::kitchen::{self, Forecast, Objective, Plan, Scenario};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, Statistic, TEXT_MONO, TableColumn, show_number,
};

/// How the meal outcomes table orders its rows (`k.outcomeSort`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OutcomeSort {
    /// Most ranks gained over the horizon.
    #[default]
    Gain,
    /// Highest projected level.
    Level,
    /// Meal name.
    Name,
}

impl OutcomeSort {
    pub(crate) const ALL: [OutcomeSort; 3] =
        [OutcomeSort::Gain, OutcomeSort::Level, OutcomeSort::Name];

    pub(crate) fn label(self) -> &'static str {
        match self {
            OutcomeSort::Gain => "Most ranks gained",
            OutcomeSort::Level => "Projected level",
            OutcomeSort::Name => "Name",
        }
    }
}

/// The objective's name, as the comparison labels it.
pub(crate) fn objective_label(objective: Objective) -> &'static str {
    match objective {
        Objective::Ranks => "Most meal ranks",
        Objective::Targets => "Selected meal targets",
    }
}

/// The scenario comparison, its extra-rank figures and the two reset charts.
pub(crate) fn comparison(
    state: &State,
    forecast: &Forecast,
    figures: &Figures,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let objective = state.kitchen.objective;
    let cols = [
        TableColumn::text("Scenario"),
        TableColumn::number("Meal ranks"),
        TableColumn::number("Donated"),
        TableColumn::number("Final daily grant"),
        TableColumn::number("Ladles spent"),
    ];
    let rows = kitchen::SCENARIOS.into_iter().map(|scenario| {
        let plan = forecast.plan(scenario);
        vec![
            text(scenario.label()),
            text(format::number(forecast.ranks(scenario))),
            text(format::number(plan.jelly.donated)),
            text(format!("{} h", format::number(plan.jelly.daily_hours_end))),
            text(format::number(plan.ladles_used)),
        ]
    });
    let first_donation = forecast
        .recommended
        .timeline
        .iter()
        .find(|day| day.jelly_donation > 0.0)
        .map(|day| format!("Before reset {}", day.day))
        .unwrap_or_else(|| "No new donation in this horizon".to_owned());
    let extra = KpiStrip::new()
        .child(
            Statistic::new(
                "kitchens-extra-layout-ranks",
                "Extra ranks from layouts",
                count(forecast.ranks(Scenario::Layout) - forecast.ranks(Scenario::Current)),
            )
            .caption("without new donations"),
        )
        .child(
            Statistic::new(
                "kitchens-extra-donation-ranks",
                "Extra ranks from donations",
                count(forecast.ranks(Scenario::Recommended) - forecast.ranks(Scenario::Layout)),
            )
            .caption("beyond the optimized assignments"),
        );
    let select = &state.controls.scenario;
    select.show(
        super::controls::scenario_choices(),
        Some(&state.scenario),
        window,
        cx,
    );
    Card::new()
        .header(
            CardHeader::new("Forecast comparison")
                .description(objective_label(objective))
                .action(
                    div().flex_none().w(px(240.0)).child(
                        Select::new(select.state())
                            .id("kitchens-scenario")
                            .accessibility_label("Inspect scenario")
                            .small(),
                    ),
                ),
        )
        .child(
            div()
                .id("kitchens-comparison-objective")
                .role(gpui::Role::Label)
                .aria_label(objective_label(objective)),
        )
        .child(table("kitchens-comparison", &cols, rows, cx))
        .child(div().id("kitchens-comparison-cards").child(extra))
        .child(Caption::new(format!(
            "Direct spending is planned in all three scenarios; only the reset assignments and \
             new donations differ. {first_donation}."
        )))
        .child(charts(figures, cx))
        .into_any_element()
}

/// The daily grant hours and the cumulative meal ranks the comparison draws.
fn charts(figures: &Figures, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let chart = |id: &'static str,
                 line: &'static str,
                 caption: &'static str,
                 name: &'static str,
                 points: &[ChartPoint],
                 stroke| {
        div()
            .id(id)
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(Caption::new(caption))
            .child(
                div().h(px(180.0)).child(
                    LineChart::new(points.to_vec())
                        .id(line)
                        .name(name)
                        .x(|point: &ChartPoint| point.label.clone())
                        .y(|point: &ChartPoint| point.value)
                        .y_tick_format(format::number)
                        .stroke(stroke),
                ),
            )
    };
    div()
        .id("kitchens-comparison-charts")
        .flex()
        .gap_3()
        .child(chart(
            "kitchens-grant-chart",
            "kitchens-grant-line",
            "Daily grant hours per kitchen",
            "Daily grant hours",
            &figures.grants,
            theme.chart_1,
        ))
        .child(chart(
            "kitchens-ranks-chart",
            "kitchens-ranks-line",
            "Cumulative meal ranks gained",
            "Cumulative meal ranks",
            &figures.ranks,
            theme.chart_2,
        ))
        .into_any_element()
}

/// The meal outcomes table, its sort and the per-meal target inputs.
///
/// When the forecast refused its inputs the table still draws the target inputs
/// so the player can fix them, and the projected column is then `-`.
pub(crate) fn meal_outcomes(
    state: &State,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let select = &state.controls.sort;
    select.show(
        super::controls::sort_choices(),
        Some(&state.outcome_sort),
        window,
        cx,
    );
    Card::new()
        .header(
            CardHeader::new("Meal outcomes & targets")
                .description(
                    "Targets drive the Selected meal targets objective; 0 means the meal has none",
                )
                .action(
                    div().flex_none().w(px(200.0)).child(
                        Select::new(select.state())
                            .id("kitchens-outcomes-sort")
                            .accessibility_label("Sort meals")
                            .small(),
                    ),
                ),
        )
        .child(
            div()
                .id("kitchens-outcomes")
                .h(px(360.0))
                .child(DataTable::new(&state.controls.outcomes)),
        )
        .child(Caption::new(
            "Direct is what the claimed ladles and passive kitchen time bought, Jellyfish the \
             grant's batch stock and NMLB No Meal Left Behind's daily hit.",
        ))
        .into_any_element()
}

/// One discovered meal's row, formatted once.
pub(crate) struct Row {
    meal: usize,
    name: SharedString,
    current: SharedString,
    projected: SharedString,
    direct: SharedString,
    jelly: SharedString,
    nmlb: SharedString,
    target: f64,
}

/// The rows of the meal outcomes table and the target fields their cells draw.
pub(crate) struct Rows {
    pub(super) rows: Vec<Row>,
    /// One target field per meal, owned by the section's controls.
    pub(super) targets: Vec<Entity<InputState>>,
}

/// The discovered meals' rows in the order the sort names.
pub(super) fn rows(
    levels: &[f64],
    plan: Option<&Plan>,
    sort: OutcomeSort,
    targets: &[f64],
) -> Vec<Row> {
    sorted(levels, plan, sort)
        .into_iter()
        .map(|meal| {
            let outcome = plan.and_then(|plan| plan.per_meal.get(meal).copied());
            let levels_from = |pick: fn(&kitchen::MealOutcome) -> f64| {
                format::number(outcome.as_ref().map_or(0.0, pick)).into()
            };
            Row {
                meal,
                name: kitchen::meal_name(meal).into(),
                current: format::number(levels[meal]).into(),
                projected: plan
                    .map_or_else(|| "-".to_owned(), |plan| format::number(plan.levels[meal]))
                    .into(),
                direct: levels_from(|row| row.ladle_levels),
                jelly: levels_from(|row| row.jelly_levels),
                nmlb: levels_from(|row| row.nmlb_levels),
                target: targets.get(meal).copied().unwrap_or(0.0),
            }
        })
        .collect()
}

/// The table state over the meal rows: no selection, sorting or column moves.
pub(super) fn state(
    targets: Vec<Entity<InputState>>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TableState<Rows>> {
    cx.new(|cx| {
        TableState::new(
            Rows {
                rows: Vec::new(),
                targets,
            },
            window,
            cx,
        )
        .row_selectable(false)
        .col_selectable(false)
        .col_movable(false)
        .sortable(false)
    })
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &App) -> usize {
        7
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("meal", "Meal").width(px(200.0)),
            1 => Column::new("current", "Current").text_right(),
            2 => Column::new("projected", "Projected").text_right(),
            3 => Column::new("direct", "Direct").text_right(),
            4 => Column::new("jelly", "Jellyfish").text_right(),
            5 => Column::new("nmlb", "NMLB").text_right(),
            _ => Column::new("target", "Target").width(px(140.0)),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let number = |value: &SharedString, cx: &App| {
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(TEXT_MONO)
                .child(value.clone())
                .into_any_element()
        };
        match col_ix {
            0 => div().child(row.name.clone()).into_any_element(),
            1 => number(&row.current, cx),
            2 => number(&row.projected, cx),
            3 => number(&row.direct, cx),
            4 => number(&row.jelly, cx),
            5 => number(&row.nmlb, cx),
            _ => match self.targets.get(row.meal) {
                Some(field) => {
                    show_number(field, row.target, 0, window, cx);
                    number_field(
                        SharedString::from(format!("kitchens-target-{}", row.meal)),
                        field,
                        format!("{} target level", row.name),
                    )
                    .into_any_element()
                }
                None => div().into_any_element(),
            },
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match col_ix {
            0 => row.name.clone(),
            1 => row.current.clone(),
            2 => row.projected.clone(),
            3 => row.direct.clone(),
            4 => row.jelly.clone(),
            5 => row.nmlb.clone(),
            _ => format::number(row.target).into(),
        })
    }
}

/// The discovered meals in the order the sort names.
pub(crate) fn sorted(levels: &[f64], plan: Option<&Plan>, sort: OutcomeSort) -> Vec<usize> {
    let mut meals: Vec<usize> = levels
        .iter()
        .enumerate()
        .filter(|(_, level)| **level >= 1.0)
        .map(|(meal, _)| meal)
        .collect();
    meals.sort_by(|a, b| match sort {
        OutcomeSort::Gain => {
            let gained = |meal: usize| plan.map_or(0.0, |plan| plan.gained[meal]);
            gained(*b).total_cmp(&gained(*a)).then_with(|| a.cmp(b))
        }
        OutcomeSort::Level => {
            let level = |meal: usize| plan.map_or(levels[meal], |plan| plan.levels[meal]);
            level(*b).total_cmp(&level(*a)).then_with(|| a.cmp(b))
        }
        OutcomeSort::Name => kitchen::meal_name(*a).cmp(kitchen::meal_name(*b)),
    });
    meals
}
