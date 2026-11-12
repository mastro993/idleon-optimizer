//! The Kitchens & Jellyfish tab: the reset investment, the daily layout and the
//! copyable reset instructions.
//!
//! The plan itself lives in [`idleon_ladle::kitchen`]: this tab only gathers
//! the inputs the save and the section's selectors give it, then draws the cards
//! and hands each reset to [`super::forecast`].

use super::summary;
use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::{Callout, EmptyState};
use ely_gpui_component::forms::{Checkbox, Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_ladle::kitchen::{self, Policy};
use idleon_ladle::{self as ladle, Ladle};
use idleon_ui::format;

/// The whole Kitchens & Jellyfish tab.
pub(crate) fn pane(
    state: &mut AppState,
    ladle: &Ladle,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(reset) = ladle.kitchen.clone() else {
        return no_kitchens(cx);
    };
    let stats = summary::all(
        ladle,
        state.ladle_state.panel,
        state.ladle_state.claim_hours,
    );
    let inputs = inputs(
        &state.ladle_state.kitchen,
        &state.ladle_state.assigned,
        state.ladle_state.panel,
        state.ladle_state.use_owned,
        ladle,
        &stats,
    );
    state.ladle_state.plan(&reset, &inputs);
    let cached = state
        .ladle_state
        .kitchen_plan
        .as_ref()
        .expect("the plan was just computed");
    let plan = match &cached.plan {
        Ok(plan) => plan,
        Err(problem) => return refused(state, *problem, cx),
    };
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
            ScrollArea::new("kitchens-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(controls(state, cx))
                        .child(cards(state, &inputs, plan))
                        .child(cost_inputs(state, cx))
                        .child(super::forecast::pane(state, &reset, plan, cx))
                        .child(
                            div().px_1().child(
                                Caption::new(
                                    "The plan spends the guaranteed claim income only: the Post Office \
                                     19c roll and the Prayer 2 cap already shape the rates on the Ladles \
                                     tab. Kitchens are fungible, so every ladle adds its hours to every \
                                     kitchen, and the Blue Jellyfish grant cooks one batch in all of them \
                                     after the reset. Layouts and donation shares are bounded best-found \
                                     recommendations, not global-optimum proofs.",
                                )
                                .text_color(cx.theme().colors.fg_muted),
                            ),
                        ),
                ),
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
fn controls(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let inputs = &state.ladle_state.kitchen;
    let entity = cx.entity();
    let policy = entity.clone();
    let share = entity.clone();
    let fixed = entity.clone();
    let owned = entity.clone();
    let days = entity.clone();
    let speed = entity.clone();
    let max_level = entity.clone();
    let use_owned = entity.clone();
    let passive = entity.clone();
    let ladleable = entity.clone();
    let policy_choices: Vec<Choice> = [
        (Policy::None, "none", "No new donations"),
        (Policy::All, "all", "Donate all daily income"),
        (Policy::Share, "share", "Donate a share of daily income"),
        (Policy::Fixed, "fixed", "Donate a fixed amount daily"),
    ]
    .into_iter()
    .map(|(_, id, label)| Choice::new(id, label))
    .collect();
    let mut row = div()
        .id("kitchens-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            field("Donation policy", muted).child(
                Select::new("kitchens-policy", policy_choices)
                    .label("Donation policy")
                    .selected(policy_id(inputs.policy))
                    .on_change(move |value, _window, cx| {
                        let chosen = parse_policy(value.as_ref());
                        policy.update(cx, |state, cx| {
                            state
                                .ladle_state
                                .set_kitchen(|inputs| inputs.policy = chosen);
                            cx.notify();
                        });
                    }),
            ),
        );
    if inputs.policy == Policy::Share {
        row = row.child(
            field("Income donated (%)", muted).child(
                Stepper::new("kitchens-share", inputs.income_fraction * 100.0)
                    .range(0.0, 100.0)
                    .step(5.0)
                    .precision(0)
                    .on_change(move |value, _window, cx| {
                        share.update(cx, |state, cx| {
                            state.ladle_state.set_kitchen(|inputs| {
                                inputs.income_fraction = (value / 100.0).clamp(0.0, 1.0);
                            });
                            cx.notify();
                        });
                    }),
            ),
        );
    }
    if inputs.policy == Policy::Fixed {
        row = row.child(
            field("Ladles donated / day", muted).child(
                Stepper::new("kitchens-fixed", inputs.donation_per_day)
                    .range(0.0, 100_000.0)
                    .step(1.0)
                    .precision(0)
                    .on_change(move |value, _window, cx| {
                        fixed.update(cx, |state, cx| {
                            state.ladle_state.set_kitchen(|inputs| {
                                inputs.donation_per_day = value.max(0.0);
                            });
                            cx.notify();
                        });
                    }),
            ),
        );
    }
    if inputs.policy != Policy::None {
        row = row.child(
            field("Owned ladles donated (%)", muted).child(
                Stepper::new("kitchens-owned", inputs.owned_fraction * 100.0)
                    .range(0.0, 100.0)
                    .step(5.0)
                    .precision(0)
                    .on_change(move |value, _window, cx| {
                        owned.update(cx, |state, cx| {
                            state.ladle_state.set_kitchen(|inputs| {
                                inputs.owned_fraction = (value / 100.0).clamp(0.0, 1.0);
                            });
                            cx.notify();
                        });
                    }),
            ),
        );
    }
    row.child(
        field("Resets", muted).child(
            Stepper::new("kitchens-days", f64::from(inputs.days))
                .range(1.0, 365.0)
                .step(1.0)
                .precision(0)
                .on_change(move |value, _window, cx| {
                    days.update(cx, |state, cx| {
                        state.ladle_state.set_kitchen(|inputs| {
                            inputs.days = value.clamp(1.0, 365.0).round() as u32;
                        });
                        // The forecast's selected reset must stay inside the horizon.
                        state.ladle_state.day = state
                            .ladle_state
                            .day
                            .clamp(1, state.ladle_state.kitchen.days as usize);
                        cx.notify();
                    });
                }),
        ),
    )
    .child(
        field("Cooking speed /hr (Speed Lv 0)", muted).child(
            Stepper::new("kitchens-speed", inputs.base_speed)
                .range(0.0, 1e12)
                .step(10.0)
                .precision(0)
                .on_change(move |value, _window, cx| {
                    speed.update(cx, |state, cx| {
                        state.ladle_state.set_kitchen(|inputs| {
                            inputs.base_speed = value.max(0.0);
                        });
                        cx.notify();
                    });
                }),
        ),
    )
    .child(
        field("Meal max level", muted).child(
            Stepper::new("kitchens-max-level", inputs.max_level)
                .range(2.0, 1000.0)
                .step(1.0)
                .precision(0)
                .on_change(move |value, _window, cx| {
                    max_level.update(cx, |state, cx| {
                        state.ladle_state.set_kitchen(|inputs| {
                            inputs.max_level = value.max(2.0);
                        });
                        cx.notify();
                    });
                }),
        ),
    )
    .child(
        Checkbox::new("kitchens-use-owned", state.ladle_state.use_owned)
            .label("Spend the ladles the save carries")
            .on_change(move |on, _window, cx| {
                use_owned.update(cx, |state, cx| {
                    state.ladle_state.use_owned = on;
                    cx.notify();
                });
            }),
    )
    .child(
        Checkbox::new("kitchens-passive", inputs.passive)
            .label("24h passive kitchens")
            .on_change(move |on, _window, cx| {
                passive.update(cx, |state, cx| {
                    state.ladle_state.set_kitchen(|inputs| inputs.passive = on);
                    cx.notify();
                });
            }),
    )
    .child(
        Checkbox::new("kitchens-ladleable", inputs.ladleable_only)
            .label("Only level meals a day of kitchen time can finish")
            .on_change(move |on, _window, cx| {
                ladleable.update(cx, |state, cx| {
                    state
                        .ladle_state
                        .set_kitchen(|inputs| inputs.ladleable_only = on);
                    cx.notify();
                });
            }),
    )
    .into_any_element()
}

/// The donation policy's id, as the select carries it.
pub(crate) fn policy_id(policy: Policy) -> &'static str {
    match policy {
        Policy::None => "none",
        Policy::All => "all",
        Policy::Share => "share",
        Policy::Fixed => "fixed",
    }
}

/// The donation policy a select value names.
pub(crate) fn parse_policy(value: &str) -> Policy {
    match value {
        "all" => Policy::All,
        "share" => Policy::Share,
        "fixed" => Policy::Fixed,
        _ => Policy::None,
    }
}

/// The summary cards the reset plan answers with.
fn cards(state: &AppState, inputs: &kitchen::Inputs, plan: &kitchen::Plan) -> AnyElement {
    let checked = state.ladle_state.assigned.len();
    let row = div()
        .id("kitchens-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-income",
                    "Daily ladle income",
                    inputs.ladles_per_day,
                )
                .decimals(0),
            )
            .caption(format!("guaranteed · {checked} checked characters")),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-grant",
                    "Saved reset grant",
                    plan.jelly.daily_hours,
                )
                .decimals(1)
                .suffix("h"),
            )
            .caption("per kitchen at every reset"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-multi",
                    "Direct-use multiplier",
                    plan.ladle_multi,
                )
                .decimals(3)
                .suffix("×"),
            )
            .caption("Talent 148 · not applied to Jellyfish grants"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-donated",
                    "Donated over the plan",
                    plan.jelly.donated,
                )
                .decimals(0),
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
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-grant-end",
                    "Final daily grant",
                    plan.jelly.daily_hours_end,
                )
                .decimals(1)
                .suffix("h"),
            )
            .caption(format!(
                "{} hours used across the plan",
                format::hours(plan.jelly.total_hours)
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kitchens-kpi-ranks",
                    "Meal levels gained",
                    plan.gained.iter().sum::<f64>(),
                )
                .decimals(0),
            )
            .caption(format!(
                "{} from ladles · {} from the grant · {} from NMLB",
                format::number(plan.by_source.ladle),
                format::number(plan.by_source.jelly),
                format::number(plan.by_source.nmlb)
            )),
        );
    row.into_any_element()
}

/// The meal cost terms the stats engine would otherwise supply.
fn cost_inputs(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let cost = state.ladle_state.kitchen.cost;
    let fields: [(&'static str, &'static str, f64, f64, f64, f64); 5] = [
        (
            "Cost companion 162",
            "companion",
            cost.companion_162,
            0.0,
            1e6,
            1.0,
        ),
        (
            "Achievement 233",
            "achievement",
            cost.achievement_233,
            0.0,
            1.0,
            1.0,
        ),
        ("Cloud 33", "cloud", cost.cloud_33, 0.0, 1.0, 0.05),
        (
            "Dream days banked",
            "discount",
            cost.discount_days,
            0.0,
            3650.0,
            1.0,
        ),
        ("Dream days cap", "dream", cost.dream_11, 0.0, 3650.0, 1.0),
    ];
    let mut row = div().id("kitchens-cost-inputs").flex().flex_wrap().gap_3();
    for (label, id, value, min, max, step) in fields {
        let entity = cx.entity();
        row = row.child(
            field(label, muted).child(
                Stepper::new(format!("kitchens-cost-{id}"), value)
                    .range(min, max)
                    .step(step)
                    .precision(if step < 0.1 { 2 } else { 0 })
                    .on_change(move |value, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.ladle_state.set_kitchen(|inputs| {
                                let cost = &mut inputs.cost;
                                match id {
                                    "companion" => cost.companion_162 = value,
                                    "achievement" => cost.achievement_233 = value,
                                    "cloud" => cost.cloud_33 = value,
                                    "discount" => cost.discount_days = value,
                                    _ => cost.dream_11 = value,
                                }
                            });
                            cx.notify();
                        });
                    }),
            ),
        );
    }
    Card::new()
        .header(CardHeader::new("Meal cost inputs").description(
            "The Cooking menu's discount terms · the defaults are the neutral values a save without \
             them carries",
        ))
        .child(row)
        .child(
            div().px_3().pb_3().child(
                Caption::new(
                    "Meal levels cost CookingR(\"CookingMenuMealCosts\"): the companion, achievement, cloud \
                     and Dream terms below scale every price, and Dream days discount exactly one purchase.",
                )
                .text_color(cx.theme().colors.fg_muted),
            ),
        )
        .into_any_element()
}

/// The plan refused its inputs: say which.
fn refused(state: &AppState, problem: kitchen::Problem, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("kitchens-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .p_3()
        .child(
            div().id("kitchens-error").child(
                Callout::new(Severity::Warning)
                    .title("The reset plan cannot run")
                    .child(Caption::new(problem.message())),
            ),
        )
        .child(controls(state, cx))
        .into_any_element()
}

/// A save without `Cooking` or `Meals` rows has no reset to plan.
fn no_kitchens(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("kitchens-pane")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("kitchens-empty", IconName::Map, "No kitchen rows")
                .body(
                    "The reset plan reads each kitchen's unlock state, the meal levels and stocks and the \
                     Blue Jellyfish grant from a save. Import one to see the daily layout and its \
                     instructions.",
                )
                .action(
                    ely_gpui_component::buttons::Button::new("kitchens-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

/// A labelled stepper column.
fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}
