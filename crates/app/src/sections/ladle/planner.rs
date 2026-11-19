//! The Blue Jellyfish donation planner and the No Meal Left Behind steering
//! panel.
//!
//! The planner is a search that compares donation splits against direct
//! spending off the UI thread through the shared runner, so a cancel keeps the
//! result the tab already shows. Its schedule runs to one row per reset, so it
//! is a virtualized table filled when a result is stored. The steering panel is
//! [`idleon_ladle::kitchen::steer::floor_plan`], worked out with the forecast:
//! the meal split, tonight's pick, the floor's blockers and the rotation
//! holding it predicts.

use super::summary::{self, count, figure, tag};
use super::table::{blank, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::table::DataTable;
use idleon_ladle::kitchen::steer::{Floor, RotationDay, Target};
use idleon_ladle::kitchen::{self, Donation, Policy, Schedule};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, RowColumn, RowsDelegate, Statistic, TEXT_BODY,
    TableColumn, Tone,
};

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
    let figures = state.ladle_state.derived.kitchen.as_ref();
    let runner = state.ladle_plan.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = figures.is_some_and(|figures| runner.is_stale(&figures.request));
    let stored = runner.result().is_some();
    let unlocked = reset.jelly.unlocked;
    let run_entity = cx.entity();
    let run = Button::new("ladle-plan-start")
        .label(match running {
            true => "Comparing…",
            false => "Compare donation vs spending",
        })
        .primary()
        .icon(IconName::Play)
        .disabled(running || !unlocked)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_ladle_plan(cx));
        });
    let mut body = div()
        .id("ladle-plan-controls")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(run)
                .child(Caption::new(
                    "Compares an up-front share of the owned ladles and a share of the guaranteed \
                 income until a cutoff reset, then refines the best split.",
                )),
        );
    if !unlocked {
        body = body.child(
            Callout::new(Tone::Warning, "Blue Jellyfish is locked").child(Caption::new(
                "Unlock it through Menagerie rank 6 to compare new donations. The saved grant \
                 still applies.",
            )),
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
    if running && let Some(progress) = progress {
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
    if cancelled {
        card = card.child(idleon_ui::run_chrome::cancelled_banner(
            "ladle-plan-cancelled",
            stored,
        ));
    }
    let entity = cx.entity();
    if let Some(donation) = state.ladle_plan.read(cx).result() {
        card = card.child(result(state, donation, objective, stale, entity));
    }
    div()
        .id("ladle-plan")
        .flex()
        .flex_col()
        .gap_3()
        .child(card)
        .child(div().px_1().child(Caption::new(
            "Both plans include the saved donations, reset timing, meal costs, Dream discounts \
             and NMLB. Jellyfish time arrives as one batch after NMLB. The search is a bounded \
             comparison, not a global-optimum proof.",
        )))
        .into_any_element()
}

/// The stored recommendation: what it donates, what it reaches and its schedule.
fn result(
    state: &AppState,
    donation: &Donation,
    objective: &'static str,
    stale: bool,
    entity: gpui::Entity<AppState>,
) -> AnyElement {
    let mut header = CardHeader::new("Recommendation")
        .description(format!("{} schedules compared", donation.evaluated));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("ladle-plan-stale"));
    }
    let mut body = div().id("ladle-plan-result").flex().flex_col().gap_3();
    if stale {
        body = body.child(idleon_ui::run_chrome::stale_banner("ladle-plan-stale"));
    }
    body = body.child(
        div().id("ladle-plan-cards").child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "ladle-plan-donated",
                        "Donate over the plan",
                        count(donation.plan.jelly.donated),
                    )
                    .caption(format!(
                        "{} owned up front + the schedule's daily share",
                        format::number(donation.schedule.owned)
                    )),
                )
                .child(
                    Statistic::new(
                        "ladle-plan-grant",
                        "Permanent daily grant",
                        figure(donation.plan.jelly.daily_hours_end, 1),
                    )
                    .suffix("h")
                    .caption("per kitchen after the last planned donation"),
                )
                .child(
                    Statistic::new("ladle-plan-gain", objective, count(donation.score)).caption(
                        match donation.improved {
                            true => format!(
                                "{} without new donations",
                                format::number(donation.baseline_score)
                            ),
                            false => "no tested split improves on spending the ladles".to_owned(),
                        },
                    ),
                ),
        ),
    );
    body = body.child(schedule(state, &donation.schedule));
    if donation.improved {
        let applied = state.ladle_state.kitchen.donations.is_some();
        let schedule = donation.schedule.clone();
        body = body.child(
            Checkbox::new("ladle-plan-apply")
                .label("Use this schedule in the meal plan")
                .checked(applied)
                .on_click(move |on, _window, cx| {
                    let on = *on;
                    let schedule = on.then(|| schedule.clone());
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

/// One donating reset of the schedule, formatted once per stored result.
pub(crate) struct ScheduleRow {
    when: SharedString,
    ladles: SharedString,
}

/// The schedule table's columns.
pub(super) fn schedule_table() -> RowsDelegate<ScheduleRow> {
    RowsDelegate::new(vec![
        RowColumn::text("when", "When", |row: &ScheduleRow| row.when.clone())
            .column(|column| column.width(px(220.0))),
        RowColumn::number("ladles", "Ladles", |row: &ScheduleRow| row.ladles.clone()),
    ])
}

/// The whole-ladle schedule, one row per reset that donates.
pub(super) fn schedule_rows(schedule: &Schedule) -> Vec<ScheduleRow> {
    let owned = (schedule.owned > 0.0).then(|| ScheduleRow {
        when: "Owned ladles, up front".into(),
        ladles: format::number(schedule.owned).into(),
    });
    owned
        .into_iter()
        .chain(
            schedule
                .daily
                .iter()
                .enumerate()
                .filter(|(_, ladles)| **ladles > 0.0)
                .map(|(index, ladles)| ScheduleRow {
                    when: format!("Before reset {}", index + 1).into(),
                    ladles: format::number(*ladles).into(),
                }),
        )
        .collect()
}

/// The schedule: its virtualized table, or a line when it donates nothing.
fn schedule(state: &AppState, schedule: &Schedule) -> AnyElement {
    let donating = schedule.owned > 0.0 || schedule.daily.iter().any(|ladles| *ladles > 0.0);
    let rows = schedule
        .daily
        .iter()
        .filter(|ladles| **ladles > 0.0)
        .count()
        + usize::from(schedule.owned > 0.0);
    div()
        .id("ladle-plan-schedule")
        .flex()
        .flex_col()
        .gap_2()
        .child(match donating {
            // A short schedule takes only the rows it has.
            true => div()
                .h(px((36.0 + 32.0 * rows as f32).min(320.0)))
                .child(DataTable::new(&state.ladle_state.controls.schedule))
                .into_any_element(),
            false => Caption::new("No new donations: keep spending the ladles on meals.")
                .into_any_element(),
        })
        .into_any_element()
}

/// The rates the steering plan reads.
pub(super) fn steer_input<'a>(
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

/// The No Meal Left Behind steering panel: the split, the floor and the rotation.
pub(crate) fn steering(reset: &kitchen::Reset, floor: Option<&Floor>, cx: &App) -> AnyElement {
    let Some(floor) = floor else {
        return div().into_any_element();
    };
    let mut body = div()
        .id("ladle-steer")
        .flex()
        .flex_col()
        .gap_3()
        .child(cards(floor, reset.nmlb));
    if !floor.rotation.is_empty() {
        body = body
            .child(
                div()
                    .text_size(TEXT_BODY)
                    .child("Predicted NMLB rotation while the floor is held"),
            )
            .child(rotation(floor))
            .child(Caption::new(
                "Assumes only the floor's blockers are raised. The meal plan also levels meals \
                 with leftover ladles, which reorders later resets.",
            ));
    }
    body = body
        .child(
            div()
                .text_size(TEXT_BODY)
                .child("NMLB targets by ladles saved per hit"),
        )
        .child(targets_table(floor, cx))
        .child(div().text_size(TEXT_BODY).child(match floor.floor_meal {
            Some(meal) => format!(
                "Ladleable blockers below {} Lv {}",
                kitchen::meal_name(meal),
                format::number(floor.floor_level.unwrap_or(0.0))
            ),
            None => "Ladleable blockers".to_owned(),
        }))
        .child(blockers_table(floor, cx));
    Card::new()
        .header(CardHeader::new("No Meal Left Behind steering").description(
            "NMLB raises the lowest meal at Lv 2+ below max, up to Lv 150; ties go to the highest \
             meal number. Ladleable meals cost at most one day of kitchen time per level, so \
             holding them above the floor keeps the daily hit on a target.",
        ))
        .child(body)
        .into_any_element()
}

/// The split, tonight's pick and the floor's cost.
fn cards(floor: &Floor, gain: f64) -> AnyElement {
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
        .flex_col()
        .gap_2()
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "ladle-steer-classes",
                        "Meal classes",
                        count(floor.ladleable_count as f64),
                    )
                    .caption(format!(
                        "ladleable of {} · split at {} of kitchen time a day",
                        floor.target_count + floor.ladleable_count,
                        format::hours(floor.day_hours)
                    )),
                )
                .child(
                    Statistic::new(
                        "ladle-steer-tonight",
                        "Tonight's NMLB",
                        count(floor.current_saved),
                    )
                    .suffix("ladles")
                    .caption(current),
                )
                .child(
                    Statistic::new("ladle-steer-floor", "NMLB floor", count(floor.floor_saved))
                        .suffix("ladles")
                        .caption(floor_meal),
                )
                .child(
                    Statistic::new(
                        "ladle-steer-blockers",
                        "Ladles to hold the floor",
                        count(floor.blocker_ladles),
                    )
                    .caption(format!(
                        "{} ladleable blocker(s) · {}",
                        floor.blockers.len(),
                        match floor.fits_in_day {
                            true => "fits in one day".to_owned(),
                            false => format!("needs {}", format::hours(floor.blocker_hours)),
                        }
                    )),
                ),
        )
        .child(Caption::new(format!(
            "NMLB awards +{} level(s) a reset to the lowest eligible meal.",
            format::number(gain)
        )))
        .into_any_element()
}

/// The rotation steps as tags, six to a row so a row never wraps.
fn rotation(floor: &Floor) -> AnyElement {
    div()
        .id("ladle-steer-rotation")
        .flex()
        .flex_col()
        .gap_2()
        .children(floor.rotation.chunks(6).map(|chunk| {
            div()
                .flex()
                .gap_2()
                .children(chunk.iter().map(rotation_chip))
        }))
        .into_any_element()
}

/// One rotation step as a chip.
fn rotation_chip(day: &RotationDay) -> AnyElement {
    tag(
        format!(
            "Day {}: {} {}→{}",
            day.day,
            kitchen::meal_name(day.meal),
            format::number(day.from),
            format::number(day.to)
        ),
        Tone::Warning,
    )
    .into_any_element()
}

/// The target table, most ladles saved first.
fn targets_table(floor: &Floor, cx: &App) -> AnyElement {
    let cols = [
        TableColumn::text("Target meal"),
        TableColumn::number("Lv"),
        TableColumn::number("Ladles → next"),
        TableColumn::number("Saved per hit"),
        TableColumn::text("In rotation"),
        TableColumn::text("Blockers"),
        TableColumn::number("Ladles to steer"),
        TableColumn::text("Status"),
    ];
    let rows = floor.targets.iter().map(|target| {
        vec![
            text(kitchen::meal_name(target.meal)),
            text(format::number(target.level)),
            text(format::number(target.next_ladles)),
            text(format::number(target.ladles_saved)),
            text(match target.rotation_day {
                Some(day) => format!("Day {day}"),
                None => "later".to_owned(),
            }),
            text(blocker_count(target)),
            text(match target.feasible {
                true => format::number(target.steer_ladles),
                false => "—".to_owned(),
            }),
            status(target),
        ]
    });
    table("ladle-steer-targets", &cols, rows, cx).into_any_element()
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
    let (label, tone) = match (target.floor, target.feasible, target.fits_in_day) {
        (true, _, _) => ("floor".to_owned(), Tone::Warning),
        (false, true, true) => ("steerable today".to_owned(), Tone::Success),
        (false, true, false) => ("steer over days".to_owned(), Tone::Info),
        (false, false, _) => (
            format!("after {} target(s)", target.target_blockers),
            Tone::Neutral,
        ),
    };
    tag(label, tone).into_any_element()
}

/// The floor's ladleable blockers and their total.
fn blockers_table(floor: &Floor, cx: &App) -> AnyElement {
    if floor.floor_meal.is_none() {
        return Caption::new("No NMLB target meals: every open meal is ladleable within a day.")
            .into_any_element();
    }
    if floor.blockers.is_empty() {
        return Caption::new(format!(
            "Already held: NMLB lands on {} tonight.",
            kitchen::meal_name(floor.floor_meal.unwrap_or(0))
        ))
        .into_any_element();
    }
    let cols = [
        TableColumn::text("Blocker"),
        TableColumn::text("Raise to"),
        TableColumn::number("Levels"),
        TableColumn::number("Ladles"),
    ];
    let levels: f64 = floor.blockers.iter().map(|blocker| blocker.levels).sum();
    let rows = floor
        .blockers
        .iter()
        .map(|blocker| {
            vec![
                text(kitchen::meal_name(blocker.meal)),
                text(format!(
                    "Lv {} → {}",
                    format::number(blocker.level),
                    format::number(blocker.need)
                )),
                text(format::number(blocker.levels)),
                text(format::number(blocker.ladles)),
            ]
        })
        .chain(std::iter::once(vec![
            text("Total"),
            blank(),
            text(format::number(levels)),
            text(format::number(floor.blocker_ladles)),
        ]));
    table("ladle-steer-blockers", &cols, rows, cx).into_any_element()
}
