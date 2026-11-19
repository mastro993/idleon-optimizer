//! The Kitchens & Jellyfish tab: the reset investment, the daily layout and the
//! copyable reset instructions.
//!
//! The plan itself lives in [`idleon_ladle::kitchen`]: this tab only gathers
//! the inputs the save and the section's selectors give it, then draws the cards
//! and hands each reset to [`super::forecast`]. [`Figures`] runs the forecast
//! and what it feeds once per change of the save or the inputs.

use super::controls::{COST, Pin, cost_value, objective_choices, policy_choices};
use super::planner::LadlePlanRequest;
use super::summary::{self, count, figure};
use super::{State, field, import_prompt};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use idleon_ladle::kitchen::{self, Forecast, Policy, Problem, steer::Floor};
use idleon_ladle::{self as ladle, Ladle};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, Choice, KpiStrip, Statistic, Tone, show_number,
};
use std::rc::Rc;

/// One point of a reset chart.
#[derive(Clone, Debug)]
pub(super) struct ChartPoint {
    pub(super) label: SharedString,
    pub(super) value: f64,
}

/// The reset forecast for one set of inputs and everything the tab derives
/// from it.
pub(crate) struct Figures {
    /// The donation search's request: the save's reset and these inputs.
    pub(super) request: LadlePlanRequest,
    /// Shared with the forecast list, whose items draw from it.
    pub(super) forecast: Rc<Result<Forecast, Problem>>,
    /// The NMLB steering plan, when the save has NMLB.
    pub(super) floor: Option<Floor>,
    pub(super) grants: Vec<ChartPoint>,
    pub(super) ranks: Vec<ChartPoint>,
    /// One choice per reset of the recommended plan.
    pub(super) resets: Vec<Choice<usize>>,
    /// The discovered meals the pins and exclusions offer.
    pub(super) discovered: Vec<usize>,
    pub(super) pins: Vec<Choice<Pin>>,
}

impl Figures {
    pub(super) fn new(reset: &kitchen::Reset, inputs: &kitchen::Inputs) -> Self {
        let forecast = kitchen::forecast(reset, inputs);
        let floor = (reset.nmlb > 0.0)
            .then(|| kitchen::steer::floor_plan(&super::planner::steer_input(reset, inputs)));
        let (grants, ranks) = forecast.as_ref().map(charts).unwrap_or_default();
        let resets = forecast
            .as_ref()
            .map(|forecast| {
                forecast
                    .recommended
                    .timeline
                    .iter()
                    .map(|day| Choice::new(day.day as usize, format!("Reset {}", day.day)))
                    .collect()
            })
            .unwrap_or_default();
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
        let pins = [
            Choice::new(None, "Optimizer chooses"),
            Choice::new(Some(-1), "Keep idle"),
        ]
        .into_iter()
        .chain(
            discovered
                .iter()
                .map(|meal| Choice::new(Some(*meal as i32), kitchen::meal_name(*meal))),
        )
        .collect();
        Self {
            request: LadlePlanRequest {
                reset: reset.clone(),
                inputs: inputs.clone(),
            },
            forecast: Rc::new(forecast),
            floor,
            grants,
            ranks,
            resets,
            discovered,
            pins,
        }
    }
}

/// The daily grant hours and the cumulative meal ranks the comparison draws.
fn charts(forecast: &Forecast) -> (Vec<ChartPoint>, Vec<ChartPoint>) {
    let plan = &forecast.recommended;
    let mut grants = vec![ChartPoint {
        label: "Now".into(),
        value: plan.jelly.daily_hours,
    }];
    let mut ranks = vec![ChartPoint {
        label: "Now".into(),
        value: 0.0,
    }];
    let mut total = 0.0;
    for day in &plan.timeline {
        let label: SharedString = format!("Reset {}", day.day).into();
        grants.push(ChartPoint {
            label: label.clone(),
            value: day.jelly_hours,
        });
        total += day.levels_bought as f64 + day.nmlb_to - day.nmlb_from;
        ranks.push(ChartPoint {
            label,
            value: total,
        });
    }
    (grants, ranks)
}

/// The whole Kitchens & Jellyfish tab.
pub(crate) fn pane(
    state: &AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let (Some(reset), Some(figures)) =
        (ladle.kitchen.as_ref(), ladle_state.derived.kitchen.as_ref())
    else {
        return no_kitchens(cx);
    };
    let inputs = &figures.request.inputs;
    let forecast = match figures.forecast.as_ref() {
        Ok(forecast) => forecast,
        Err(problem) => return refused(ladle_state, *problem, window, cx),
    };
    let plan = forecast.plan(ladle_state.scenario);
    let message = format!(
        "{} resets, {} ladles donated, {} meal levels gained",
        plan.days,
        format::number(plan.jelly.donated),
        format::number(plan.gained.iter().sum::<f64>())
    );
    div()
        .id("kitchens-pane")
        .role(Role::Label)
        .aria_label(message)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(controls(ladle_state, window, cx))
                .child(cards(ladle_state, inputs, plan))
                .child(cost_inputs(ladle_state, window, cx))
                .child(super::planner::pane(state, reset, inputs, cx))
                .child(super::planner::steering(reset, figures.floor.as_ref(), cx))
                .child(super::outcomes::comparison(
                    ladle_state,
                    forecast,
                    figures,
                    window,
                    cx,
                ))
                .child(super::forecast::pane(
                    ladle_state,
                    reset,
                    figures,
                    plan,
                    window,
                    cx,
                ))
                .child(super::outcomes::meal_outcomes(ladle_state, window, cx))
                .child(div().px_1().child(Caption::new(
                    "The plan spends the guaranteed claim income only: the Post Office 19c roll \
                     and the Prayer 2 cap already shape the rates on the Ladles tab. Kitchens are \
                     fungible, so every ladle adds its hours to every kitchen, and the Blue \
                     Jellyfish grant cooks one batch in all of them after the reset. Layouts and \
                     donation shares are bounded best-found recommendations, not global-optimum \
                     proofs.",
                )))
                .overflow_y_scrollbar()
                .id("kitchens-scroll"),
        )
        .into_any_element()
}

/// The plan's inputs: the entered settings plus what the save and the rates say.
pub(crate) fn inputs(
    settings: &kitchen::Inputs,
    assigned: &[usize],
    panel: ladle::Panel,
    use_owned: bool,
    ladle: &Ladle,
    stats: &[ladle::Stats],
) -> kitchen::Inputs {
    let mut inputs = settings.clone();
    inputs.ladles_per_day = summary::total(stats, assigned, |row| row.min_per_day);
    inputs.hours_to_reset = ladle.reset_hours.unwrap_or(24.0);
    inputs.start_ladles = match use_owned {
        true => ladle.ladles.value,
        false => 0.0,
    };
    inputs.ladle_multi = panel.ladle_hours();
    inputs
}

/// The reset's own controls: the horizon, the donation policy and the model.
fn controls(state: &State, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let inputs = &state.kitchen;
    let fields = &state.controls;
    fields
        .objective
        .show(objective_choices(), Some(&inputs.objective), window, cx);
    fields
        .policy
        .show(policy_choices(), Some(&inputs.policy), window, cx);
    show_number(&fields.share, inputs.income_fraction * 100.0, 0, window, cx);
    show_number(&fields.fixed, inputs.donation_per_day, 0, window, cx);
    show_number(&fields.owned, inputs.owned_fraction * 100.0, 0, window, cx);
    show_number(&fields.days, f64::from(inputs.days), 0, window, cx);
    show_number(&fields.speed, inputs.base_speed, 0, window, cx);
    show_number(&fields.max_level, inputs.max_level, 0, window, cx);
    let mut first = div()
        .flex()
        .items_end()
        .gap_3()
        .child(
            field("Objective").child(
                div().flex_none().w(px(200.0)).child(
                    Select::new(fields.objective.state())
                        .id("kitchens-objective")
                        .accessibility_label("Objective")
                        .small(),
                ),
            ),
        )
        .child(
            field("Donation policy").child(
                div().flex_none().w(px(240.0)).child(
                    Select::new(fields.policy.state())
                        .id("kitchens-policy")
                        .accessibility_label("Donation policy")
                        .small(),
                ),
            ),
        );
    if inputs.policy == Policy::Share {
        first = first.child(field("Income donated (%)").child(number_field(
            "kitchens-share",
            &fields.share,
            "Income donated (%)",
        )));
    }
    if inputs.policy == Policy::Fixed {
        first = first.child(field("Ladles donated / day").child(number_field(
            "kitchens-fixed",
            &fields.fixed,
            "Ladles donated / day",
        )));
    }
    if inputs.policy != Policy::None {
        first = first.child(field("Owned ladles donated (%)").child(number_field(
            "kitchens-owned",
            &fields.owned,
            "Owned ladles donated (%)",
        )));
    }
    let second = div()
        .flex()
        .items_end()
        .gap_3()
        .child(field("Resets").child(number_field("kitchens-days", &fields.days, "Resets")))
        .child(field("Cooking speed /hr (Speed Lv 0)").child(number_field(
            "kitchens-speed",
            &fields.speed,
            "Cooking speed /hr",
        )))
        .child(field("Meal max level").child(number_field(
            "kitchens-max-level",
            &fields.max_level,
            "Meal max level",
        )));
    let use_owned = cx.entity();
    let passive = cx.entity();
    let ladleable = cx.entity();
    let third = div()
        .flex()
        .items_center()
        .gap_4()
        .child(
            Checkbox::new("kitchens-use-owned")
                .label("Spend the ladles the save carries")
                .checked(state.use_owned)
                .on_click(move |on, _window, cx| {
                    let on = *on;
                    use_owned.update(cx, |state, cx| {
                        state.ladle_state.use_owned = on;
                        cx.notify();
                    });
                }),
        )
        .child(
            Checkbox::new("kitchens-passive")
                .label("24h passive kitchens")
                .checked(inputs.passive)
                .on_click(move |on, _window, cx| {
                    let on = *on;
                    passive.update(cx, |state, cx| {
                        state.ladle_state.set_kitchen(|inputs| inputs.passive = on);
                        cx.notify();
                    });
                }),
        )
        .child(
            Checkbox::new("kitchens-ladleable")
                .label("Only level meals a day of kitchen time can finish")
                .checked(inputs.ladleable_only)
                .on_click(move |on, _window, cx| {
                    let on = *on;
                    ladleable.update(cx, |state, cx| {
                        state
                            .ladle_state
                            .set_kitchen(|inputs| inputs.ladleable_only = on);
                        cx.notify();
                    });
                }),
        );
    // Rows of controls that never wrap (`docs/ui-kit/display.md`).
    div()
        .id("kitchens-controls")
        .flex()
        .flex_col()
        .gap_3()
        .child(first)
        .child(second)
        .child(third)
        .into_any_element()
}

/// The summary figures the reset plan answers with.
fn cards(state: &State, inputs: &kitchen::Inputs, plan: &kitchen::Plan) -> AnyElement {
    let checked = state.assigned.len();
    let first = KpiStrip::new()
        .child(
            Statistic::new(
                "kitchens-kpi-income",
                "Daily ladle income",
                count(inputs.ladles_per_day),
            )
            .caption(format!("guaranteed · {checked} checked characters")),
        )
        .child(
            Statistic::new(
                "kitchens-kpi-grant",
                "Saved reset grant",
                figure(plan.jelly.daily_hours, 1),
            )
            .suffix("h")
            .caption("per kitchen at every reset"),
        )
        .child(
            Statistic::new(
                "kitchens-kpi-multi",
                "Direct-use multiplier",
                figure(plan.ladle_multi, 3),
            )
            .suffix("×")
            .caption("Talent 148 · not applied to Jellyfish grants"),
        );
    let second = KpiStrip::new()
        .child(
            Statistic::new(
                "kitchens-kpi-donated",
                "Donated over the plan",
                count(plan.jelly.donated),
            )
            .caption(match (plan.jelly.unlocked, plan.jelly.donated > 0.0) {
                (false, _) => "Jellyfish locked · the saved grant still applies".to_owned(),
                (true, true) => format!(
                    "{} owned up front + the policy's daily share",
                    format::number(plan.jelly.owned_donation)
                ),
                (true, false) => "No new donations · the saved grant still applies".to_owned(),
            }),
        )
        .child(
            Statistic::new(
                "kitchens-kpi-grant-end",
                "Final daily grant",
                figure(plan.jelly.daily_hours_end, 1),
            )
            .suffix("h")
            .caption(format!(
                "{} hours used across the plan",
                format::hours(plan.jelly.total_hours)
            )),
        )
        .child(
            Statistic::new(
                "kitchens-kpi-ranks",
                "Meal levels gained",
                count(plan.gained.iter().sum::<f64>()),
            )
            .caption(format!(
                "{} from ladles · {} from the grant · {} from NMLB",
                format::number(plan.by_source.ladle),
                format::number(plan.by_source.jelly),
                format::number(plan.by_source.nmlb)
            )),
        );
    div()
        .id("kitchens-cards")
        .flex()
        .flex_col()
        .gap_2()
        .child(first)
        .child(second)
        .into_any_element()
}

/// The meal cost terms the stats engine would otherwise supply.
fn cost_inputs(state: &State, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let mut inputs = state.kitchen.clone();
    let mut row = div().id("kitchens-cost-inputs").flex().items_end().gap_3();
    for (index, spec) in COST.iter().enumerate() {
        let field_state = &state.controls.cost[index];
        show_number(
            field_state,
            *cost_value(&mut inputs, index),
            spec.precision,
            window,
            cx,
        );
        row = row.child(field(spec.label).child(number_field(
            format!("kitchens-cost-{}", spec.id),
            field_state,
            spec.label,
        )));
    }
    Card::new()
        .header(CardHeader::new("Meal cost inputs").description(
            "The Cooking menu's discount terms · the defaults are the neutral values a save without \
             them carries",
        ))
        .child(row)
        .child(Caption::new(
            "Meal levels cost CookingR(\"CookingMenuMealCosts\"): the companion, achievement, cloud \
             and Dream terms below scale every price, and Dream days discount exactly one purchase.",
        ))
        .into_any_element()
}

/// The forecast refused its inputs: say which, and keep the target inputs open so
/// a target objective without a target can be fixed from the same tab.
fn refused(
    state: &State,
    problem: Problem,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    div()
        .id("kitchens-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(
                    div().id("kitchens-error").child(
                        Callout::new(Tone::Warning, "The reset forecast cannot run")
                            .child(Caption::new(problem.message())),
                    ),
                )
                .child(controls(state, window, cx))
                .child(super::outcomes::meal_outcomes(state, window, cx))
                .overflow_y_scrollbar()
                .id("kitchens-scroll"),
        )
        .into_any_element()
}

/// A save without `Cooking` or `Meals` rows has no reset to plan.
fn no_kitchens(cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("kitchens-pane")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(import_prompt(
            "kitchens-empty",
            "kitchens-import",
            "No kitchen rows",
            "The reset plan reads each kitchen's unlock state, the meal levels and stocks and the \
             Blue Jellyfish grant from a save. Import one to see the daily layout and its \
             instructions.",
            cx,
        ))
        .into_any_element()
}
