//! The upgrade list, the selection preview, the trials and the comparison.
//!
//! Every upgrade the save unlocks is compared separately against its
//! character's baseline with the same seeds, the preview says how many attempts
//! that is, and the summary highlights the best option by collected skulls.
//! The rows come from [`super::Controls`], built when their inputs change.

use crate::sections::killroy::Mode;
use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{
    DataTable, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, Size, h_flex, v_flex};
use idleon_killroy::{Plan, UpgradeChoice};
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone, fit_columns, show_number};

/// A row tall enough for a name over its caption.
const TWO_LINE_ROW: Size = Size::Size(px(44.0));
/// The table head, at the Console row height.
const HEAD_ROW: gpui::Pixels = px(28.0);

/// A footnote under a card's body.
fn notes(id: &'static str, cx: &mut Context<AppState>) -> gpui::Stateful<gpui::Div> {
    v_flex()
        .id(id)
        .gap_1()
        .text_size(TEXT_CAPTION)
        .text_color(cx.theme().muted_foreground)
}

/// The `+1` list: every upgrade the save lists, its level and its availability.
pub(crate) fn upgrade_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let rows = state.killroy_controls.upgrades.iter().map(|choice| {
        TableRow::new()
            .id(("killroy-upgrade-row", choice.id))
            .child(TableCell::new().child(choice.name.clone()))
            .child(TableCell::new().text_right().child(match choice.level {
                Some(level) => idleon_ui::format::count(level as u64),
                None => "—".into(),
            }))
            .child(TableCell::new().text_right().child(match choice.id {
                0 => "—".to_string(),
                _ => idleon_ui::format::count(choice.runs_required as u64),
            }))
            .child(TableCell::new().child(availability(choice)))
            .child(TableCell::new().child(upgrade_check(state, choice, cx)))
    });
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .id("killroy-upgrades-head")
                    .child(TableHead::new().child("Upgrade"))
                    .child(TableHead::new().text_right().child("Level"))
                    .child(TableHead::new().text_right().child("Runs needed"))
                    .child(TableHead::new().child("Availability"))
                    .child(TableHead::new()),
            ),
        )
        .child(TableBody::new().children(rows));

    Card::new()
        .header(CardHeader::new("Compare +1 upgrades").description(
            "Each selected +1 is tested separately against that character's baseline.",
        ))
        .child(
            div().id("killroy-upgrades").child(
                div()
                    .h(px(240.0))
                    .child(table)
                    .overflow_y_scrollbar()
                    .id("killroy-upgrades-scroll"),
            ),
        )
        .child(
            notes("killroy-upgrade-notes", cx)
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
        .child(
            Tag::new()
                .with_variant(tone.into())
                .outline()
                .small()
                .child(text),
        )
        .into_any_element()
}

/// One upgrade's check for the comparison.
fn upgrade_check(
    state: &AppState,
    choice: &UpgradeChoice,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let id = choice.id;
    let enabled = choice.unlocked == Some(true) && choice.level.is_some();
    let entity = cx.entity();
    Checkbox::new(format!("killroy-upgrade-{id}"))
        .label(SharedString::from(format!("+1 {}", choice.name)))
        .checked(state.killroy.compares(id))
        .disabled(!enabled)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| state.toggle_killroy_upgrade(id, on, cx));
        })
        .into_any_element()
}

/// The trials: the settings, the queue preview, the run and its progress.
pub(crate) fn run_card(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let controls = &state.killroy_controls;
    let comparing = state.killroy.mode == Mode::Upgrades;
    let plan = &controls.plan;
    let entity = cx.entity();
    let (running, cancelled, progress, kept) = {
        let runner = state.killroy_run.read(cx);
        (
            runner.running(),
            runner.cancelled(),
            runner.progress().cloned(),
            runner.result().is_some(),
        )
    };
    // Values flow setting -> field; the fields' edits come back through the
    // subscriptions in `Controls::new`.
    show_number(&controls.trials, state.killroy.trials as f64, 0, window, cx);
    show_number(
        &controls.kills,
        state.killroy.kills_per_second,
        1,
        window,
        cx,
    );
    show_number(&controls.seed, state.killroy.seed as f64, 0, window, cx);

    let label = match plan {
        Ok(plan) => format!(
            "{}: {} attempts",
            if comparing {
                "Compare upgrades"
            } else {
                "Simulate baseline"
            },
            idleon_ui::format::count(plan.attempts() as u64)
        ),
        Err(_) => "Select characters to simulate".to_string(),
    };
    let run_entity = entity.clone();
    let run = Button::new("killroy-run")
        .label(label)
        .primary()
        .small()
        .disabled(plan.is_err() || running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_killroy(cx));
        });

    // Not `killroy-trials`: that is the trials field, and a duplicated id drops
    // out of the automation tree.
    let mut body = v_flex()
        .id("killroy-trials-card")
        .gap_3()
        .child(
            h_flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(labelled("Trials", "killroy-trials", &controls.trials))
                .child(labelled("Kills / second", "killroy-kills", &controls.kills))
                .child(labelled("Seed", "killroy-seed", &controls.seed)),
        )
        .child(options(state, cx))
        .child(preview(state, cx))
        .child(run);
    if let Some(progress) = progress {
        body = body.child(run_chrome::progress_card(
            "killroy-run",
            "Running the trials",
            "Each attempt starts from the imported save with the trial's own seed.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_killroy(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(run_chrome::cancelled_banner("killroy-cancelled", kept));
    }
    body = body.child(Caption::new(
        "Average per attempt over the chosen seeds; sampled differences are not confirmed wins.",
    ));

    Card::new()
        .header(CardHeader::new("Trials").description(
            "Every attempt starts independently from the imported save; no weekly progression is applied.",
        ))
        .child(body)
        .into_any_element()
}

/// A number field under its label. The kit `NumberInput` has no id, so a
/// wrapper keeps the field's.
fn labelled(
    label: &'static str,
    id: &'static str,
    field: &gpui::Entity<gpui_kit::component::input::InputState>,
) -> AnyElement {
    v_flex()
        .gap_1()
        .child(Caption::new(label))
        .child(crate::ui::number_field(id, field, label))
        .into_any_element()
}

/// The Options block: saved buffs and unlimited inventory.
fn options(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let keep_entity = cx.entity();
    let unlimited_entity = cx.entity();
    let keep = Checkbox::new("killroy-keep-buffs")
        .label("Keep saved buffs")
        .checked(state.killroy.keep_buffs)
        .on_click(move |on, _window, cx| {
            let on = *on;
            keep_entity.update(cx, |state, cx| state.set_killroy_keep_buffs(on, cx));
        });
    let unlimited = Checkbox::new("killroy-unlimited")
        .label("Unlimited inventory (hypothetical)")
        .checked(state.killroy.unlimited_inventory)
        .on_click(move |on, _window, cx| {
            let on = *on;
            unlimited_entity.update(cx, |state, cx| {
                state.set_killroy_unlimited_inventory(on, cx)
            });
        });
    v_flex()
        .id("killroy-options")
        .gap_1()
        .child(h_flex().flex_wrap().gap_4().child(keep).child(unlimited))
        .child(Caption::new(
            "A run starts as a fresh map entry: timed buffs are dropped unless kept, \
             the permanent form toggles always stay. Without unlimited inventory a \
             drop needs its own stack or a blank bag slot.",
        ))
        .into_any_element()
}

/// What the run would cover: the weeks, the configurations and the attempts.
fn preview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let controls = &state.killroy_controls;
    let plan = match &controls.plan {
        Err(error) => {
            return div()
                .id("killroy-preview-error")
                .role(gpui::Role::Label)
                .aria_label(error.clone())
                .child(Caption::new(error.clone()).text_color(cx.theme().warning))
                .into_any_element();
        }
        Ok(plan) => plan,
    };
    let summary = queue_summary(plan, state.killroy.trials);
    v_flex()
        .id("killroy-preview")
        .role(gpui::Role::Label)
        .aria_label(summary.clone())
        .gap_2()
        .child(Caption::new(summary))
        .child(
            div().h(px(150.0)).child(fit_columns(
                DataTable::new(&controls.queue)
                    .with_size(TWO_LINE_ROW)
                    .header_row_height(HEAD_ROW),
                &controls.queue,
            )),
        )
        .into_any_element()
}

/// The queue's one-line summary.
fn queue_summary(plan: &Plan, trials: usize) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    format!(
        "Queue: {} · {} × {} · {} attempts",
        plural(plan.weeks.len(), "week", "weeks"),
        plural(plan.rows.len(), "configuration", "configurations"),
        plural(trials, "trial", "trials"),
        idleon_ui::format::count(plan.attempts() as u64)
    )
}

/// The comparison: every configuration's average, its delta and the best option.
pub(crate) fn results_card(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let controls = &state.killroy_controls;
    let Some(run) = &controls.run else {
        return div().into_any_element();
    };
    let height = layout::table_height(f32::from(window.viewport_size().height));
    let export_entity = cx.entity();
    let export = Button::new("killroy-export")
        .label("Copy results JSON")
        .icon(IconName::Copy)
        .small()
        .on_click(move |_event, _window, cx| {
            export_entity.update(cx, |state, cx| state.copy_killroy_results(cx));
        });

    Card::new()
        .header(
            CardHeader::new(if run.comparing {
                "Upgrade comparison results"
            } else {
                "Baseline results"
            })
            .description(format!(
                "{} · average per attempt · imported save",
                match run.trials {
                    1 => "1 trial".to_string(),
                    runs => format!("{runs} trials"),
                }
            ))
            .action(export),
        )
        .when(controls.stale, |card| {
            card.child(run_chrome::stale_banner("killroy-stale"))
        })
        .child(
            div()
                .id("killroy-comparison")
                .h(height.min(px(320.0)))
                .child(fit_columns(
                    DataTable::new(&controls.results)
                        .with_size(TWO_LINE_ROW)
                        .header_row_height(HEAD_ROW),
                    &controls.results,
                )),
        )
        .child(
            notes("killroy-comparison-notes", cx)
                .child(
                    "The best option per character and encounter is marked. A delta is the \
                     difference against that configuration's own baseline over the same seeds.",
                )
                .child(
                    "The combat simulator is not built yet, so the kill rate is the trials' \
                     stand-in input; the encounter, respawn and drop maths are the game's own.",
                )
                .child(
                    div()
                        .id("killroy-result-details")
                        .role(gpui::Role::Label)
                        .aria_label(run.details.clone())
                        .child(run.details.clone()),
                )
                .children(run.warnings.iter().enumerate().map(|(index, warning)| {
                    div()
                        .id(SharedString::from(format!("killroy-warning-{index}")))
                        .child(SharedString::from(warning.clone()))
                })),
        )
        .into_any_element()
}

/// What the run's inputs were, as the Run details line reads them.
pub(crate) fn run_details(plan: &Plan) -> String {
    format!(
        "{} · {}",
        if plan.keep_buffs {
            "Saved buffs kept"
        } else {
            "Fresh map-entry buffs"
        },
        if plan.unlimited_inventory {
            "Unlimited inventory"
        } else {
            "Real inventory limits"
        }
    )
}

/// The saved buffs every selected character's run starts from.
pub(crate) fn buffs_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let controls = &state.killroy_controls;
    let keep = state.killroy.keep_buffs;
    let buffs = controls.buff_count;
    let body = if buffs == 0 {
        Caption::new("Check a character in the forecast to see the buffs its run starts with.")
            .into_any_element()
    } else {
        div()
            .id("killroy-buffs")
            .h(px(200.0))
            .child(fit_columns(
                DataTable::new(&controls.buffs)
                    .with_size(TWO_LINE_ROW)
                    .header_row_height(HEAD_ROW),
                &controls.buffs,
            ))
            .into_any_element()
    };

    Card::new()
        .header(CardHeader::new("Saved buffs").description(format!(
            "{buffs} {} from BuffsActive · {} · snapshot timers",
            if buffs == 1 { "row" } else { "rows" },
            if keep {
                "every saved buff kept"
            } else {
                "form toggles kept, timed buffs dropped"
            }
        )))
        .child(body)
        .child(notes("killroy-buffs-notes", cx).child(
            "These are the save's own rows: a run starts as a fresh map entry, so a timed \
             buff only starts active when Keep saved buffs is on. Form toggles always stay.",
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_import::import;
    use idleon_killroy::{Comparisons, PlanInput, Selection, plan as build, trial_seeds};
    use serde_json::json;

    fn plan(keep_buffs: bool, unlimited_inventory: bool) -> Plan {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        let save = import(&json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string())
            .expect("the wrapped fixture imports")
            .save;
        build(
            &save,
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds: trial_seeds(1, 1),
                comparisons: Comparisons::Off,
                kills_per_second: 3.0,
                keep_buffs,
                unlimited_inventory,
            },
        )
        .expect("the plan builds")
    }

    #[test]
    fn the_run_details_name_the_starting_buffs_and_the_bag() {
        assert_eq!(
            run_details(&plan(false, false)),
            "Fresh map-entry buffs · Real inventory limits"
        );
        assert_eq!(
            run_details(&plan(true, true)),
            "Saved buffs kept · Unlimited inventory"
        );
    }

    #[test]
    fn the_queue_summary_counts_weeks_rows_and_trials() {
        assert_eq!(
            queue_summary(&plan(false, false), 1),
            "Queue: 1 week · 1 configuration × 1 trial · 1 attempts"
        );
    }
}
