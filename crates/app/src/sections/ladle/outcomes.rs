//! The forecast comparison and the meal outcomes table (LD-03).
//!
//! Upstream compares the same horizon under the saved assignments, the optimized
//! assignments and the donation policy, then lists every discovered meal with the
//! levels each source bought and the target the `Selected meal targets` objective
//! spends toward. The tables and the two charts reuse the shared components; the
//! numbers come from [`idleon_ladle::kitchen::forecast`] and its plans.

use crate::state::AppState;
use ely_gpui_component::charts::{LineChart, Series};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_ladle::kitchen::{self, Forecast, Objective, Plan, Scenario};
use idleon_ui::format;

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

    pub(crate) fn id(self) -> &'static str {
        match self {
            OutcomeSort::Gain => "gain",
            OutcomeSort::Level => "level",
            OutcomeSort::Name => "name",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            OutcomeSort::Gain => "Most ranks gained",
            OutcomeSort::Level => "Projected level",
            OutcomeSort::Name => "Name",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sort| sort.id() == id)
    }
}

/// The objective's name, as upstream labels it in the comparison.
pub(crate) fn objective_label(objective: Objective) -> &'static str {
    match objective {
        Objective::Ranks => "Most meal ranks",
        Objective::Targets => "Selected meal targets",
    }
}

/// The scenario comparison, its extra-rank cards and the two reset charts.
pub(crate) fn comparison(
    state: &AppState,
    forecast: &Forecast,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let pick = entity.clone();
    let objective = state.ladle_state.kitchen.objective;
    let mut table = Table::new([
        "Scenario",
        "Meal ranks",
        "Donated",
        "Final daily grant",
        "Ladles spent",
    ]);
    for scenario in kitchen::SCENARIOS {
        let plan = forecast.plan(scenario);
        table = table.row([
            div().child(scenario.label()).into_any_element(),
            div()
                .child(format::number(forecast.ranks(scenario)))
                .into_any_element(),
            div()
                .child(format::number(plan.jelly.donated))
                .into_any_element(),
            div()
                .child(format!("{} h", format::number(plan.jelly.daily_hours_end)))
                .into_any_element(),
            div()
                .child(format::number(plan.ladles_used))
                .into_any_element(),
        ]);
    }
    let first_donation = forecast
        .recommended
        .timeline
        .iter()
        .find(|day| day.jelly_donation > 0.0)
        .map(|day| format!("Before reset {}", day.day))
        .unwrap_or_else(|| "No new donation in this horizon".to_owned());
    let cards = div()
        .id("kitchens-comparison-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-extra-layout-ranks",
                    "Extra ranks from layouts",
                    forecast.ranks(Scenario::Layout) - forecast.ranks(Scenario::Current),
                )
                .decimals(0),
            )
            .caption("without new donations"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-extra-donation-ranks",
                    "Extra ranks from donations",
                    forecast.ranks(Scenario::Recommended) - forecast.ranks(Scenario::Layout),
                )
                .decimals(0),
            )
            .caption("beyond the optimized assignments"),
        );
    let scenarios: Vec<Choice> = kitchen::SCENARIOS
        .into_iter()
        .map(|scenario| Choice::new(scenario.id(), scenario.label()))
        .collect();
    let select = Select::new("kitchens-scenario", scenarios)
        .label("Inspect scenario")
        .selected(state.ladle_state.scenario.id())
        .on_change(move |value, _window, cx| {
            if let Some(scenario) = Scenario::from_id(value.as_ref()) {
                pick.update(cx, |state, cx| {
                    state.ladle_state.scenario = scenario;
                    cx.notify();
                });
            }
        });
    Card::new()
        .header(
            CardHeader::new("Forecast comparison")
                .description(objective_label(objective))
                .action(select),
        )
        .child(div().id("kitchens-comparison-objective").role(gpui::Role::Label).aria_label(objective_label(objective)))
        .child(div().px_3().pb_3().child(table))
        .child(div().px_3().pb_3().child(cards))
        .child(div().px_3().pb_3().child(
            Caption::new(format!(
                "Direct spending is planned in all three scenarios; only the reset assignments and \
                 new donations differ. {first_donation}."
            ))
            .text_color(muted),
        ))
        .child(charts(forecast))
        .into_any_element()
}

/// The daily grant hours and the cumulative meal ranks the comparison draws.
fn charts(forecast: &Forecast) -> AnyElement {
    let plan = &forecast.recommended;
    let mut labels = vec!["Now".to_owned()];
    let mut grants = vec![plan.jelly.daily_hours];
    let mut ranks = vec![0.0];
    let mut total = 0.0;
    for day in &plan.timeline {
        labels.push(format!("Reset {}", day.day));
        grants.push(day.jelly_hours);
        total += day.levels_bought as f64 + day.nmlb_to - day.nmlb_from;
        ranks.push(total);
    }
    let grants = LineChart::new("kitchens-grant-chart", labels.clone())
        .series(Series::new("Daily grant hours", grants))
        .h(px(180.0));
    let ranks = LineChart::new("kitchens-ranks-chart", labels)
        .series(Series::new("Cumulative meal ranks", ranks))
        .h(px(180.0));
    div()
        .id("kitchens-comparison-charts")
        .flex()
        .flex_wrap()
        .gap_3()
        .px_3()
        .pb_3()
        .child(
            div()
                .id("kitchens-grant-chart")
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(280.0))
                .gap_1()
                .child(Caption::new("Daily grant hours per kitchen"))
                .child(grants),
        )
        .child(
            div()
                .id("kitchens-ranks-chart")
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(280.0))
                .gap_1()
                .child(Caption::new("Cumulative meal ranks gained"))
                .child(ranks),
        )
        .into_any_element()
}

/// The meal outcomes table, its sort and the per-meal target inputs.
///
/// `plan` is `None` when the forecast refused its inputs, as upstream still draws
/// the target inputs so the player can fix them; the projected column is then `-`.
pub(crate) fn meal_outcomes(
    state: &AppState,
    levels: &[f64],
    plan: Option<&Plan>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let sort_pick = entity.clone();
    let max_level = plan
        .map_or(state.ladle_state.kitchen.max_level, |plan| plan.max_level)
        .max(1.0);
    let sort = state.ladle_state.outcome_sort;
    let mut table = Table::new([
        "Meal",
        "Current",
        "Projected",
        "Direct",
        "Jellyfish",
        "NMLB",
        "Target",
    ]);
    for meal in sorted(levels, plan, sort) {
        let outcome = plan.and_then(|plan| plan.per_meal.get(meal).copied());
        let target = state
            .ladle_state
            .kitchen
            .targets
            .get(meal)
            .copied()
            .unwrap_or(0.0);
        let input = entity.clone();
        let name = kitchen::meal_name(meal).to_owned();
        table = table.row([
            div().child(name.clone()).into_any_element(),
            div().child(format::number(levels[meal])).into_any_element(),
            div()
                .child(
                    plan.map_or_else(|| "-".to_owned(), |plan| format::number(plan.levels[meal])),
                )
                .into_any_element(),
            div()
                .child(format::number(outcome.map_or(0.0, |row| row.ladle_levels)))
                .into_any_element(),
            div()
                .child(format::number(outcome.map_or(0.0, |row| row.jelly_levels)))
                .into_any_element(),
            div()
                .child(format::number(outcome.map_or(0.0, |row| row.nmlb_levels)))
                .into_any_element(),
            div()
                .child(
                    NumberInput::new(format!("kitchens-target-{meal}"), target)
                        .range(0.0, max_level)
                        .label(format!("{name} target level"))
                        .on_commit(move |value, _window, cx| {
                            input.update(cx, |state, cx| {
                                state.ladle_state.set_target(meal, value);
                                cx.notify();
                            });
                        }),
                )
                .into_any_element(),
        ]);
    }
    let choices: Vec<Choice> = OutcomeSort::ALL
        .into_iter()
        .map(|sort| Choice::new(sort.id(), sort.label()))
        .collect();
    let select = Select::new("kitchens-outcomes-sort", choices)
        .label("Sort meals")
        .selected(sort.id())
        .on_change(move |value, _window, cx| {
            if let Some(sort) = OutcomeSort::from_id(value.as_ref()) {
                sort_pick.update(cx, |state, cx| {
                    state.ladle_state.outcome_sort = sort;
                    cx.notify();
                });
            }
        });
    Card::new()
        .header(
            CardHeader::new("Meal outcomes & targets")
                .description(
                    "Targets drive the Selected meal targets objective; 0 means the meal has none",
                )
                .action(select),
        )
        .child(div().id("kitchens-outcomes").px_3().pb_3().child(table))
        .child(div().px_3().pb_3().child(
            Caption::new(
                "Direct is what the claimed ladles and passive kitchen time bought, Jellyfish the \
                 grant's batch stock and NMLB No Meal Left Behind's daily hit.",
            )
            .text_color(muted),
        ))
        .into_any_element()
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
