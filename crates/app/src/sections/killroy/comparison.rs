//! The upgrade list, the selection preview, the trials and the comparison.
//!
//! Ports the `+1` half of `js/ui/killroy.js` and `buildKillroyPlan` from
//! `js/stats/systems/common/killroy-planner.js`: every upgrade the save unlocks
//! is compared separately against its character's baseline with the same seeds,
//! the preview says how many attempts that is, and the summary highlights the
//! best option by collected skulls.

use crate::sections::killroy::Mode;
use crate::sections::killroy::forecast_view::{metric, week_label};
use crate::state::AppState;
use crate::task::TaskState;
use crate::ui::{layout, task};
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_core::killroy::{Plan, Row, RowSummary, UpgradeChoice};
use idleon_core::save::Save;

/// The `+1` list: every upgrade the save lists, its level and its availability.
pub fn upgrade_card(state: &AppState, save: &Save, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let choices = idleon_core::killroy::upgrade_choices(save);
    let entity = cx.entity();
    let rows = choices.iter().fold(
        Table::new(["Upgrade", "Level", "Runs needed", "Availability", ""]),
        |table, choice| {
            table.row([
                div()
                    .child(SharedString::from(choice.name.clone()))
                    .into_any_element(),
                div()
                    .child(match choice.level {
                        Some(level) => crate::ui::format::count(level as u64),
                        None => "—".into(),
                    })
                    .into_any_element(),
                div()
                    .child(match choice.id {
                        0 => "—".to_string(),
                        _ => crate::ui::format::count(choice.runs_required as u64),
                    })
                    .into_any_element(),
                availability(choice),
                upgrade_check(state, choice, entity.clone()),
            ])
        },
    );

    Card::new()
        .header(CardHeader::new("Compare +1 upgrades").description(
            "Each selected +1 is tested separately against that character's baseline.",
        ))
        .child(
            div().id("killroy-upgrades").p_3().child(
                ScrollArea::new("killroy-upgrades-scroll")
                    .vertical()
                    .h(px(240.0))
                    .child(rows),
            ),
        )
        .child(
            div()
                .id("killroy-upgrade-notes")
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "These are free post-run choices, not skull purchases or sequential upgrades.",
                )
                .child(
                    "A locked upgrade keeps its baseline only; it needs the listed number of \
                     Killroy runs and a Dream level that reaches it.",
                ),
        )
        .into_any_element()
}

/// One upgrade's availability: unlocked, locked, or unknown until the save says.
fn availability(choice: &UpgradeChoice) -> AnyElement {
    let (text, tone) = match choice.unlocked {
        Some(true) => ("Unlocked", Tone::Success),
        Some(false) => ("Locked", Tone::Warning),
        None => ("Availability unknown", Tone::Neutral),
    };
    div()
        .id(("killroy-availability", choice.id))
        .role(gpui::Role::Label)
        .aria_label(format!("{}: {text}", choice.name))
        .child(Badge::new(text).tone(tone))
        .into_any_element()
}

/// One upgrade's check for the comparison.
fn upgrade_check(
    state: &AppState,
    choice: &UpgradeChoice,
    entity: gpui::Entity<AppState>,
) -> AnyElement {
    let id = choice.id;
    let enabled = choice.unlocked == Some(true) && choice.level.is_some();
    let checked = state.killroy.compares(id);
    let check = ely_gpui_component::forms::Checkbox::new(format!("killroy-upgrade-{id}"), checked)
        .label(SharedString::from(format!("+1 {}", choice.name)))
        .disabled(!enabled)
        .on_change(move |on, _window, cx| {
            entity.update(cx, |state, cx| state.toggle_killroy_upgrade(id, on, cx));
        });
    check.into_any_element()
}

/// The trials: the settings, the queue preview, the run and its progress.
pub fn run_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let comparing = state.killroy.mode == Mode::Upgrades;
    let plan = state.killroy_plan();
    let entity = cx.entity();
    let trials_entity = entity.clone();
    let kps_entity = entity.clone();
    let seed_entity = entity.clone();
    let run_entity = entity.clone();
    let trials = state.killroy.trials;
    let kills = state.killroy.kills_per_second;
    let seed = state.killroy.seed;
    let running = state.killroy_task(cx).running;

    let trials_input = NumberInput::new("killroy-trials", trials as f64)
        .range(1.0, 20.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Trials")
        .on_change(move |value, _window, cx| {
            trials_entity.update(cx, |state, cx| state.set_killroy_trials(value, cx));
        });
    let kills_input = NumberInput::new("killroy-kills", kills)
        .range(0.0, 100.0)
        .step(1.0)
        .precision(1)
        .size(ControlSize::Sm)
        .label("Kills per second")
        .on_change(move |value, _window, cx| {
            kps_entity.update(cx, |state, cx| {
                state.set_killroy_kills_per_second(value, cx)
            });
        });
    let seed_input = NumberInput::new("killroy-seed", seed as f64)
        .range(1.0, 999_999.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Seed")
        .on_change(move |value, _window, cx| {
            seed_entity.update(cx, |state, cx| state.set_killroy_seed(value, cx));
        });

    let (label, attempts) = match &plan {
        Ok(plan) => (
            format!(
                "{}: {} attempts",
                if comparing {
                    "Compare upgrades"
                } else {
                    "Simulate baseline"
                },
                crate::ui::format::count(plan.attempts() as u64)
            ),
            plan.attempts(),
        ),
        Err(_) => ("Select characters to simulate".to_string(), 0),
    };
    let run = Button::new("killroy-run", SharedString::from(label))
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .disabled(plan.is_err() || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_killroy(cx));
        });

    let task_state = state.killroy_task(cx);
    let mut body = div()
        .id("killroy-trials")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(labelled("Trials", trials_input))
                .child(labelled("Kills / second", kills_input))
                .child(labelled("Seed", seed_input)),
        )
        .child(preview(state, &plan, attempts, cx))
        .child(run);
    if let TaskState {
        progress: Some(progress),
        ..
    } = &task_state
    {
        let cancel_entity = cx.entity();
        body = body.child(task::progress_card(
            "killroy-run",
            "Running the trials",
            "Each attempt starts from the imported save with the trial's own seed.",
            progress.clone(),
            move |_window, cx| {
                cancel_entity.update(cx, |state, cx| state.cancel_killroy(cx));
            },
        ));
    }
    if state.killroy_task(cx).cancelled {
        body = body.child(task::cancelled_banner(
            "killroy-cancelled",
            state.killroy_task(cx).result.is_some(),
        ));
    }
    body = body.child(div().text_size(px(11.0)).text_color(muted).child(
        "Average per attempt over the chosen seeds; sampled differences are not confirmed wins.",
    ));

    Card::new()
        .header(CardHeader::new("Trials").description(
            "Every attempt starts independently from the imported save; no weekly progression is applied.",
        ))
        .child(body)
        .into_any_element()
}

fn labelled(label: &'static str, input: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(input)
        .into_any_element()
}

/// What the run would cover: the weeks, the configurations and the attempts.
fn preview(
    state: &AppState,
    plan: &Result<Plan, String>,
    attempts: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    match plan {
        Err(error) => div()
            .id("killroy-preview-error")
            .role(gpui::Role::Label)
            .aria_label(error.clone())
            .child(Caption::new(error.clone()).text_color(cx.theme().colors.warning))
            .into_any_element(),
        Ok(plan) => {
            let summary = format!(
                "Queue: {} {} · {} {} × {} {} · {} attempts",
                plan.weeks.len(),
                if plan.weeks.len() == 1 {
                    "week"
                } else {
                    "weeks"
                },
                plan.rows.len(),
                if plan.rows.len() == 1 {
                    "configuration"
                } else {
                    "configurations"
                },
                state.killroy.trials,
                if state.killroy.trials == 1 {
                    "trial"
                } else {
                    "trials"
                },
                crate::ui::format::count(attempts as u64)
            );
            let rows = plan.rows.iter().fold(
                Table::new(["Week", "Map / class", "Character", "Configurations"]),
                |table, row| table.row(queue_row(plan, row)),
            );
            div()
                .id("killroy-preview")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .flex_col()
                .gap_2()
                .child(Caption::new(summary).text_color(muted))
                .child(
                    ScrollArea::new("killroy-preview-scroll")
                        .vertical()
                        .h(px(150.0))
                        .child(rows),
                )
                .into_any_element()
        }
    }
}

/// One configuration in the queue preview.
fn queue_row(plan: &Plan, row: &Row) -> [AnyElement; 4] {
    let configurations: Vec<String> = if row.upgrade.is_some() {
        Vec::new()
    } else {
        plan.rows
            .iter()
            .filter(|other| other.baseline == row.key && other.upgrade.is_some())
            .filter_map(|other| other.upgrade.as_ref())
            .map(|upgrade| format!("+1 {}", upgrade.name))
            .collect()
    };
    [
        div().child(week_label(row.week_offset)).into_any_element(),
        div()
            .flex()
            .flex_col()
            .child(div().child(SharedString::from(row.encounter.map_name.clone())))
            .child(Caption::new(format!(
                "{} · Encounter {}{}",
                row.encounter.family_name,
                row.slot + 1,
                if row.encounter.used == Some(true) {
                    " (practice)"
                } else {
                    ""
                }
            )))
            .into_any_element(),
        div()
            .child(SharedString::from(row.character.clone()))
            .into_any_element(),
        div()
            .child(Caption::new(if configurations.is_empty() {
                if row.upgrade.is_some() {
                    format!(
                        "+1 {}",
                        row.upgrade
                            .as_ref()
                            .map(|upgrade| upgrade.name.as_str())
                            .unwrap_or_default()
                    )
                } else {
                    String::new()
                }
            } else {
                configurations.join(" · ")
            }))
            .into_any_element(),
    ]
}

/// The comparison: every configuration's average, its delta and the best option.
pub fn results_card(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task_state = state.killroy_task(cx);
    let Some(rows) = task_state.result.clone() else {
        return div().into_any_element();
    };
    let height = layout::table_height(f32::from(window.viewport_size().height));
    let muted = cx.theme().colors.fg_muted;
    let comparing = rows.iter().any(RowSummary::comparing);
    let runs = rows.iter().map(|row| row.runs).max().unwrap_or(0);
    let table = ordered(&rows).into_iter().fold(
        Table::new([
            "Character",
            "Configuration",
            "Kills",
            "Skulls",
            "Δ skulls",
            "Talents",
            "Credits",
            "Pearls",
        ]),
        |table, row| table.row(result_row(row)),
    );

    let mut card = Card::new().header(
        CardHeader::new(if comparing {
            "Upgrade comparison results"
        } else {
            "Baseline results"
        })
        .description(format!(
            "{} · average per attempt · imported save",
            match runs {
                1 => "1 trial".to_string(),
                runs => format!("{runs} trials"),
            }
        )),
    );
    if task_state.stale {
        card = card.child(
            div()
                .px_3()
                .pt_3()
                .child(task::stale_banner("killroy-stale")),
        );
    }
    card.child(
        div().id("killroy-comparison").p_3().child(
            ScrollArea::new("killroy-comparison-scroll")
                .vertical()
                .h(height.min(px(320.0)))
                .child(table),
        ),
    )
    .child(
        div()
            .id("killroy-comparison-notes")
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .pb_3()
            .text_size(px(11.0))
            .text_color(muted)
            .child(
                "The best option per character and encounter is marked. A delta is the difference \
                 against that configuration's own baseline over the same seeds.",
            )
            .child(
                "The combat simulator is not built yet, so the kill rate is the trials' stand-in \
                 input; the encounter, respawn and drop maths are the game's own.",
            ),
    )
    .into_any_element()
}

/// The rows in reading order: each baseline, then its options best first.
fn ordered(rows: &[RowSummary]) -> Vec<&RowSummary> {
    let mut ordered: Vec<&RowSummary> = Vec::new();
    for base in rows.iter().filter(|row| !row.comparing()) {
        ordered.push(base);
        let mut options: Vec<&RowSummary> = rows
            .iter()
            .filter(|row| row.comparing() && row.row.baseline == base.row.key)
            .collect();
        options.sort_by(|a, b| {
            let key = |row: &RowSummary| {
                let mean = row.mean.unwrap_or_default();
                (
                    std::cmp::Reverse(ordered_key(mean.skulls_picked)),
                    row.row.key.clone(),
                )
            };
            key(a).cmp(&key(b))
        });
        ordered.extend(options);
    }
    ordered
}

fn ordered_key(value: f64) -> u64 {
    (value.max(0.0) * 1000.0).round() as u64
}

/// One configuration row: its mean gains, its delta and the best marker.
fn result_row(row: &RowSummary) -> [AnyElement; 8] {
    let mean = row.mean.unwrap_or_default();
    let muted_caption = row.row.encounter.map_name.clone();
    let configuration: AnyElement = match &row.row.upgrade {
        None => Caption::new("Baseline reference").into_any_element(),
        Some(upgrade) => div()
            .id(SharedString::from(format!(
                "killroy-config-{}",
                row.row.key
            )))
            .flex()
            .items_center()
            .gap_2()
            .child(div().child(SharedString::from(format!(
                "+1 {} ({} → {})",
                upgrade.name,
                crate::ui::format::count(upgrade.level as u64),
                crate::ui::format::count(upgrade.next as u64),
            ))))
            .children(row.best.then(|| {
                div()
                    .id(SharedString::from(format!("killroy-best-{}", row.row.key)))
                    .role(gpui::Role::Label)
                    .aria_label("Best option")
                    .child(Badge::new("Best").tone(Tone::Success))
            }))
            .into_any_element(),
    };
    [
        div()
            .flex()
            .flex_col()
            .child(div().child(SharedString::from(row.row.character.clone())))
            .child(Caption::new(muted_caption))
            .into_any_element(),
        configuration,
        number(mean.kills),
        number(mean.skulls_picked),
        match row.delta {
            Some(delta) => number(delta.skulls_picked),
            None => Caption::new("—").into_any_element(),
        },
        number(mean.talent_points),
        number(mean.dungeon_credits),
        number(mean.pearls),
    ]
}

/// A mean, rounded the way the upstream tables print it.
fn number(value: f64) -> AnyElement {
    let text = if value.fract().abs() < 0.005 {
        crate::ui::format::count(value.round() as u64)
    } else {
        metric(Some(value))
    };
    Caption::new(SharedString::from(text)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::killroy::{
        Comparisons, Delta, Means, PlanInput, Selection, plan as build, trial_seeds,
    };
    use idleon_core::save::import;
    use serde_json::json;

    fn save() -> Save {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        import(&json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string())
            .expect("the wrapped fixture imports")
            .save
    }

    fn rows() -> Vec<RowSummary> {
        let plan = build(
            &save(),
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds: trial_seeds(1, 3),
                comparisons: Comparisons::Only(vec![0, 2]),
                kills_per_second: 3.0,
            },
        )
        .expect("the plan builds");
        idleon_core::killroy::run(&plan, |_, _| false).expect("the run finishes")
    }

    #[test]
    fn the_reading_order_puts_each_baseline_before_its_options() {
        let rows = rows();
        let ordered = ordered(&rows);
        assert_eq!(ordered.len(), rows.len());
        let baseline = ordered.first().expect("the plan has a baseline");
        assert!(!baseline.comparing());
        assert!(ordered[1..].iter().all(|row| row.comparing()));
        assert!(
            ordered[1..]
                .iter()
                .all(|row| row.row.baseline == baseline.row.key)
        );
    }

    #[test]
    fn the_best_option_sorts_first_and_is_marked() {
        let rows = rows();
        let ordered = ordered(&rows);
        let best = ordered
            .iter()
            .find(|row| row.best)
            .expect("a comparison marks its best option");
        assert!(
            ordered[1..]
                .iter()
                .all(|row| row.mean.unwrap().skulls_picked <= best.mean.unwrap().skulls_picked)
        );
    }

    #[test]
    fn a_delta_prints_without_a_sign_for_zero() {
        let base = RowSummary {
            row: rows()[0].row.clone(),
            runs: 1,
            complete: true,
            totals: Means::default(),
            mean: Some(Means::default()),
            delta: Some(Delta {
                kills: 0.0,
                skulls_picked: 0.0,
            }),
            best: false,
        };
        let cell = result_row(&base);
        assert_eq!(cell.len(), 8);
    }

    #[test]
    fn the_queue_preview_counts_the_options() {
        let plan = build(
            &save(),
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds: trial_seeds(1, 2),
                comparisons: Comparisons::All,
                kills_per_second: 3.0,
            },
        )
        .expect("the plan builds");
        let baseline = plan
            .rows
            .iter()
            .find(|row| row.upgrade.is_none())
            .expect("a baseline row");
        let options = plan
            .rows
            .iter()
            .filter(|row| row.baseline == baseline.key && row.upgrade.is_some())
            .count();
        assert_eq!(options, idleon_core::killroy::UPGRADES);
        assert_eq!(plan.attempts(), (options + 1) * 2);
    }
}
