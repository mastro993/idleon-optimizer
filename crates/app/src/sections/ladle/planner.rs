//! The Blue Jellyfish donation planner and the No Meal Left Behind steering
//! panel.
//!
//! The planner is the upstream `optimizeBlueJellyfish` search: it compares
//! donation splits against direct spending off the UI thread through the shared
//! runner, so a cancel keeps the result the tab already shows. The steering panel
//! is `nmlbFloorPlan`: the meal split, tonight's pick, the floor's blockers and the
//! rotation holding it predicts.

use super::summary;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_ladle::kitchen::steer::{Floor, RotationDay, Target};
use idleon_ladle::kitchen::{self, Donation, Policy, Schedule};
use idleon_ui::format;
use idleon_ui::runner::TaskState;

/// The inputs the donation search plans: the reset state and the section's own
/// settings.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LadlePlanRequest {
    pub(crate) reset: kitchen::Reset,
    pub(crate) inputs: kitchen::Inputs,
}

/// The planner's stored result.
pub(crate) type LadleDonation = Donation;

impl AppState {
    /// The donation search's progress, result and staleness for these inputs.
    pub(crate) fn ladle_plan_task(
        &self,
        request: &LadlePlanRequest,
        cx: &gpui::App,
    ) -> TaskState<LadleDonation> {
        self.ladle_plan.read(cx).state(request)
    }

    /// Compare donation splits against direct spending off the UI thread.
    pub(crate) fn run_ladle_plan(&mut self, cx: &mut Context<Self>) {
        let Some(ladle) = self.ladle.clone() else {
            self.set_status("Donation planner: import a save first.", cx);
            return;
        };
        let Some(reset) = ladle.kitchen.clone() else {
            self.set_status("Donation planner: the save carries no kitchens.", cx);
            return;
        };
        let stats = summary::all(&ladle, self.ladle_state.panel, self.ladle_state.claim_hours);
        let inputs = super::kitchens::inputs(
            &self.ladle_state.kitchen,
            &self.ladle_state.assigned,
            self.ladle_state.panel,
            self.ladle_state.use_owned,
            &ladle,
            &stats,
        );
        self.start_run(
            self.ladle_plan.clone(),
            "Comparing donation schedules…",
            LadlePlanRequest { reset, inputs },
            move |request, report| {
                kitchen::donation::optimize(&request.reset, &request.inputs, report)
            },
            cx,
        );
    }

    /// Ask a running donation search to stop at its next comparison.
    pub(crate) fn cancel_ladle_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.ladle_plan.clone(),
            "Cancelling the donation search…",
            cx,
        );
    }
}

/// The donation planner card: the search, its progress and its recommendation.
pub(crate) fn pane(
    state: &AppState,
    reset: &kitchen::Reset,
    inputs: &kitchen::Inputs,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let request = LadlePlanRequest {
        reset: reset.clone(),
        inputs: inputs.clone(),
    };
    let task = state.ladle_plan_task(&request, cx);
    let muted = cx.theme().colors.fg_muted;
    let unlocked = reset.jelly.unlocked;
    let run_entity = cx.entity();
    let run = Button::new(
        "ladle-plan-start",
        match task.running {
            true => "Comparing…",
            false => "Compare donation vs spending",
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(task.running || !unlocked)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_ladle_plan(cx));
    });
    let mut body = div()
        .id("ladle-plan-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_3()
        .pb_3()
        .child(run)
        .child(
            Caption::new(
                "Compares an up-front share of the owned ladles and a share of the guaranteed \
                 income until a cutoff reset, then refines the best split.",
            )
            .text_color(muted),
        );
    if !unlocked {
        body = body.child(
            div().px_3().pb_3().child(
                Callout::new(Severity::Warning)
                    .title("Blue Jellyfish is locked")
                    .child(Caption::new(
                        "Unlock it through Menagerie rank 6 to compare new donations. The saved \
                         grant still applies.",
                    )),
            ),
        );
    }
    let objective = objective_label(inputs.objective);
    let mut card = Card::new()
        .header(
            CardHeader::new("Blue Jellyfish donation planner").description(format!(
                "Objective: {objective} after {} resets · kitchen speed is a fixed entered rate \
                 here, so the search ranks by the section objective",
                inputs.days
            )),
        )
        .child(body);
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        card = card.child(idleon_ui::run_chrome::progress_card(
            "ladle-plan-run",
            "Comparing donation schedules",
            "Runs off the UI thread; cancelling keeps the recommendation this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_ladle_plan(cx));
            },
        ));
    }
    if task.cancelled {
        card = card.child(idleon_ui::run_chrome::cancelled_banner(
            "ladle-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(donation) = task.result.clone() {
        card = card.child(result(state, &donation, objective, task.stale, cx));
    }
    div()
        .id("ladle-plan")
        .flex()
        .flex_col()
        .gap_3()
        .child(card)
        .child(
            div().px_1().child(
                Caption::new(
                    "Both plans include the saved donations, reset timing, meal costs, Dream \
                     discounts and NMLB. Jellyfish time arrives as one batch after NMLB. The \
                     search is a bounded comparison, not a global-optimum proof.",
                )
                .text_color(muted),
            ),
        )
        .into_any_element()
}

/// The stored recommendation: what it donates, what it reaches and its schedule.
fn result(
    state: &AppState,
    donation: &Donation,
    objective: &'static str,
    stale: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut header = CardHeader::new("Recommendation")
        .description(format!("{} schedules compared", donation.evaluated));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("ladle-plan-stale"));
    }
    let mut body = div()
        .id("ladle-plan-result")
        .flex()
        .flex_col()
        .gap_3()
        .p_3();
    if stale {
        body = body.child(idleon_ui::run_chrome::stale_banner("ladle-plan-stale"));
    }
    body = body.child(
        div()
            .id("ladle-plan-cards")
            .flex()
            .flex_wrap()
            .gap_3()
            .child(
                KpiCard::new(
                    Statistic::new(
                        "ladle-plan-donated",
                        "Donate over the plan",
                        donation.plan.jelly.donated,
                    )
                    .decimals(0),
                )
                .caption(format!(
                    "{} owned up front + the schedule's daily share",
                    format::number(donation.schedule.owned)
                )),
            )
            .child(
                KpiCard::new(
                    Statistic::new(
                        "ladle-plan-grant",
                        "Permanent daily grant",
                        donation.plan.jelly.daily_hours_end,
                    )
                    .decimals(1)
                    .suffix("h"),
                )
                .caption("per kitchen after the last planned donation"),
            )
            .child(
                KpiCard::new(
                    Statistic::new("ladle-plan-gain", objective, donation.score).decimals(0),
                )
                .caption(match donation.improved {
                    true => format!(
                        "{} without new donations",
                        format::number(donation.baseline_score)
                    ),
                    false => "no tested split improves on spending the ladles".to_owned(),
                }),
            ),
    );
    body = body.child(schedule(&donation.schedule, muted));
    if donation.improved {
        let entity = cx.entity();
        let applied = state.ladle_state.kitchen.donations.is_some();
        let schedule = donation.schedule.clone();
        body = body.child(
            Checkbox::new("ladle-plan-apply", applied)
                .label("Use this schedule in the meal plan")
                .on_change(move |on, _window, cx| {
                    let schedule = match on {
                        true => Some(schedule.clone()),
                        false => None,
                    };
                    entity.update(cx, |state, cx| {
                        state.ladle_state.set_kitchen(|inputs| {
                            inputs.donations = schedule;
                            if on {
                                inputs.policy = Policy::None;
                            }
                        });
                        cx.notify();
                    });
                }),
        );
    }
    Card::new().header(header).child(body).into_any_element()
}

/// The objective the search ranks by, as the section names it.
fn objective_label(objective: kitchen::Objective) -> &'static str {
    match objective {
        kitchen::Objective::Ranks => "meal levels",
        kitchen::Objective::Targets => "meal target progress",
    }
}

/// The whole-ladle schedule, one row per reset that donates.
fn schedule(schedule: &Schedule, muted: gpui::Hsla) -> AnyElement {
    let mut table = Table::new(["When", "Ladles"]);
    if schedule.owned > 0.0 {
        table = table.row([
            div().child("Owned ladles, up front").into_any_element(),
            div()
                .child(format::number(schedule.owned))
                .into_any_element(),
        ]);
    }
    for (index, ladles) in schedule.daily.iter().enumerate() {
        if *ladles <= 0.0 {
            continue;
        }
        table = table.row([
            div()
                .child(format!("Before reset {}", index + 1))
                .into_any_element(),
            div().child(format::number(*ladles)).into_any_element(),
        ]);
    }
    let empty = schedule.owned <= 0.0 && schedule.daily.iter().all(|ladles| *ladles <= 0.0);
    div()
        .id("ladle-plan-schedule")
        .flex()
        .flex_col()
        .gap_2()
        .child(match empty {
            true => Caption::new("No new donations: keep spending the ladles on meals.")
                .text_color(muted)
                .into_any_element(),
            false => table.into_any_element(),
        })
        .into_any_element()
}

/// The No Meal Left Behind steering panel: the split, the floor and the rotation.
pub(crate) fn steering(
    reset: &kitchen::Reset,
    inputs: &kitchen::Inputs,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if reset.nmlb <= 0.0 {
        return div().into_any_element();
    }
    let muted = cx.theme().colors.fg_muted;
    let floor = kitchen::steer::floor_plan(&steer_input(reset, inputs));
    let mut body = div()
        .id("ladle-steer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(cards(&floor, reset.nmlb, muted));
    if !floor.rotation.is_empty() {
        body = body
            .child(
                div()
                    .text_sm()
                    .child("Predicted NMLB rotation while the floor is held"),
            )
            .child(
                div()
                    .id("ladle-steer-rotation")
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(floor.rotation.iter().map(rotation_chip)),
            )
            .child(
                Caption::new(
                    "Assumes only the floor's blockers are raised. The meal plan also levels meals \
                     with leftover ladles, which reorders later resets.",
                )
                .text_color(muted),
            );
    }
    body = body
        .child(
            div()
                .text_sm()
                .child("NMLB targets by ladles saved per hit"),
        )
        .child(targets_table(&floor))
        .child(div().text_sm().child(match floor.floor_meal {
            Some(meal) => format!(
                "Ladleable blockers below {} Lv {}",
                kitchen::meal_name(meal),
                format::number(floor.floor_level.unwrap_or(0.0))
            ),
            None => "Ladleable blockers".to_owned(),
        }))
        .child(blockers_table(&floor, muted));
    Card::new()
        .header(CardHeader::new("No Meal Left Behind steering").description(
            "NMLB raises the lowest meal at Lv 2+ below max, up to Lv 150; ties go to the highest \
             meal number. Ladleable meals cost at most one day of kitchen time per level, so \
             holding them above the floor keeps the daily hit on a target.",
        ))
        .child(body)
        .into_any_element()
}

/// The rates the steering plan reads.
fn steer_input<'a>(
    reset: &'a kitchen::Reset,
    inputs: &kitchen::Inputs,
) -> kitchen::steer::Input<'a> {
    let passive = match inputs.passive {
        true => 24.0,
        false => 0.0,
    };
    kitchen::steer::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        max_level: inputs.max_level,
        cost: inputs.cost,
        speed: reset.speed(inputs.base_speed),
        ladle_multi: inputs.ladle_multi,
        nmlb: reset.nmlb,
        day_hours: passive + inputs.ladles_per_day * inputs.ladle_multi,
        rotation_days: kitchen::steer::ROTATION_DAYS,
    }
}

/// The split, tonight's pick and the floor's cost.
fn cards(floor: &Floor, gain: f64, muted: gpui::Hsla) -> AnyElement {
    let current = match floor.current_pick {
        Some(meal) => format!(
            "{} · Lv {}",
            kitchen::meal_name(meal),
            format::number(floor.classes[meal].level)
        ),
        None => "—".to_owned(),
    };
    let floor_meal = match floor.floor_meal {
        Some(meal) => format!(
            "{} · Lv {}",
            kitchen::meal_name(meal),
            format::number(floor.floor_level.unwrap_or(0.0))
        ),
        None => "—".to_owned(),
    };
    div()
        .id("ladle-steer-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "ladle-steer-classes",
                    "Meal classes",
                    floor.ladleable_count as f64,
                )
                .decimals(0),
            )
            .caption(format!(
                "ladleable of {} · split at {} of kitchen time a day",
                floor.target_count + floor.ladleable_count,
                format::hours(floor.day_hours)
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new("ladle-steer-tonight", "Tonight's NMLB", floor.current_saved)
                    .decimals(0)
                    .suffix(" ladles"),
            )
            .caption(current),
        )
        .child(
            KpiCard::new(
                Statistic::new("ladle-steer-floor", "NMLB floor", floor.floor_saved)
                    .decimals(0)
                    .suffix(" ladles"),
            )
            .caption(floor_meal),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "ladle-steer-blockers",
                    "Ladles to hold the floor",
                    floor.blocker_ladles,
                )
                .decimals(0),
            )
            .caption(format!(
                "{} ladleable blocker(s) · {}",
                floor.blockers.len(),
                match floor.fits_in_day {
                    true => "fits in one day".to_owned(),
                    false => format!("needs {}", format::hours(floor.blocker_hours)),
                }
            )),
        )
        .child(
            Caption::new(format!(
                "NMLB awards +{} level(s) a reset to the lowest eligible meal.",
                format::number(gain)
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// One rotation step as a chip.
fn rotation_chip(day: &RotationDay) -> AnyElement {
    Badge::new(format!(
        "Day {}: {} {}→{}",
        day.day,
        kitchen::meal_name(day.meal),
        format::number(day.from),
        format::number(day.to)
    ))
    .tone(Tone::Warning)
    .into_any_element()
}

/// The target table, most ladles saved first.
fn targets_table(floor: &Floor) -> AnyElement {
    let mut table = Table::new([
        "Target meal",
        "Lv",
        "Ladles → next",
        "Saved per hit",
        "In rotation",
        "Blockers",
        "Ladles to steer",
        "Status",
    ]);
    for target in &floor.targets {
        table = table.row([
            div()
                .child(kitchen::meal_name(target.meal))
                .into_any_element(),
            div().child(format::number(target.level)).into_any_element(),
            div()
                .child(format::number(target.next_ladles))
                .into_any_element(),
            div()
                .child(format::number(target.ladles_saved))
                .into_any_element(),
            div()
                .child(match target.rotation_day {
                    Some(day) => format!("Day {day}"),
                    None => "later".to_owned(),
                })
                .into_any_element(),
            div().child(blocker_count(target)).into_any_element(),
            div()
                .child(match target.feasible {
                    true => format::number(target.steer_ladles),
                    false => "—".to_owned(),
                })
                .into_any_element(),
            status(target).into_any_element(),
        ]);
    }
    div()
        .id("ladle-steer-targets")
        .child(table)
        .into_any_element()
}

/// The target's blockers, split by whether the ladles can raise them.
fn blocker_count(target: &Target) -> String {
    match target.feasible {
        true => format!("{} ladleable", target.ladleable_blockers),
        false => format!(
            "{} + {} target",
            target.ladleable_blockers, target.target_blockers
        ),
    }
}

/// The target's steering status.
fn status(target: &Target) -> AnyElement {
    match (target.floor, target.feasible, target.fits_in_day) {
        (true, _, _) => Badge::new("floor").tone(Tone::Warning).into_any_element(),
        (false, true, true) => Badge::new("steerable today")
            .tone(Tone::Success)
            .into_any_element(),
        (false, true, false) => Badge::new("steer over days")
            .tone(Tone::Info)
            .into_any_element(),
        (false, false, _) => Badge::new(format!("after {} target(s)", target.target_blockers))
            .tone(Tone::Neutral)
            .into_any_element(),
    }
}

/// The floor's ladleable blockers and their total.
fn blockers_table(floor: &Floor, muted: gpui::Hsla) -> AnyElement {
    if floor.floor_meal.is_none() {
        return div()
            .child(
                Caption::new("No NMLB target meals: every open meal is ladleable within a day.")
                    .text_color(muted),
            )
            .into_any_element();
    }
    if floor.blockers.is_empty() {
        return div()
            .child(
                Caption::new(format!(
                    "Already held: NMLB lands on {} tonight.",
                    kitchen::meal_name(floor.floor_meal.unwrap_or(0))
                ))
                .text_color(muted),
            )
            .into_any_element();
    }
    let mut table = Table::new(["Blocker", "Raise to", "Levels", "Ladles"]);
    let mut levels = 0.0;
    for blocker in &floor.blockers {
        levels += blocker.levels;
        table = table.row([
            div()
                .child(kitchen::meal_name(blocker.meal))
                .into_any_element(),
            div()
                .child(format!(
                    "Lv {} → {}",
                    format::number(blocker.level),
                    format::number(blocker.need)
                ))
                .into_any_element(),
            div()
                .child(format::number(blocker.levels))
                .into_any_element(),
            div()
                .child(format::number(blocker.ladles))
                .into_any_element(),
        ]);
    }
    table = table.row([
        div().child("Total").into_any_element(),
        div().into_any_element(),
        div().child(format::number(levels)).into_any_element(),
        div()
            .child(format::number(floor.blocker_ladles))
            .into_any_element(),
    ]);
    div()
        .id("ladle-steer-blockers")
        .child(table)
        .into_any_element()
}
