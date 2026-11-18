//! Upgrade Path: spend the saved currency on the most cost-effective +1 each
//! step.

use super::{boss_name, run_inputs};
use crate::sections::research::sushi::columns::{self, Column};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_minehead::Path;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

impl AppState {
    /// Find the affordable, cost-effective upgrade order.
    pub(crate) fn run_minehead_path(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = run_inputs(self, 50, 2000) else {
            return;
        };
        self.minehead_path.update(cx, |runner, cx| {
            runner.start(cx, inputs, |inputs, report| {
                idleon_minehead::cost_path(
                    &inputs.options,
                    &inputs.params,
                    &inputs.path,
                    inputs.trials,
                    inputs.seed,
                    report,
                )
            });
        });
    }

    fn cancel_minehead_path(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.minehead_path.clone(),
            "Cancelling the upgrade path…",
            cx,
        );
    }
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.minehead.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let (floor, currency) = (inputs.floor, inputs.currency);
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let running = state.minehead_path.read(cx).running();
    let run_inputs = run_inputs(state, 50, 2000);
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_path.read(cx).state(inputs));
    let params_label = super::optimize::strategy_label(&state.research.minehead.params());
    let checked = state.research.minehead.use_inferred;
    let toggle_entity = entity.clone();
    let use_inferred = Checkbox::new("mh-path-inferred")
        .label("Your Strategy")
        .checked(checked)
        .on_click(move |on, _window, cx| {
            let on = *on;
            toggle_entity.update(cx, |state, cx| {
                state.research.minehead.use_inferred = on;
                cx.notify();
            });
        });
    let run = entity.clone();
    let cancel = entity.clone();
    let controls = Card::new()
        .header(
            CardHeader::new("Affordable upgrade path")
                .description(format!(
                    "Spends the saved currency on {} one step at a time.",
                    boss_name(floor)
                ))
                .action(
                    Tag::new()
                        .with_variant(Tone::Info.into())
                        .outline()
                        .small()
                        .child(params_label),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("mh-path-run").label(if running {
                            "Finding path…"
                        } else {
                            "Find Upgrade Path"
                        })
                    .primary()
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move |_event, _window, cx| {
                        run.update(cx, |state, cx| state.run_minehead_path(cx));
                    }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-path-cancel").label("Cancel").on_click(
                        move |_event, _window, cx| {
                            cancel.update(cx, |state, cx| state.cancel_minehead_path(cx));
                        },
                    ))
                })
                .child(use_inferred)
                .child(
                    Caption::new(format!("{} saved", format::value(currency)))
                        .text_color(muted),
                ),
        );

    let mut body = div().flex().flex_col().gap_3().min_h_0();
    if let Some(task) = task.as_ref() {
        if task.running {
            if let Some(progress) = task.progress.clone() {
                let cancel = entity.clone();
                body = body.child(idleon_ui::run_chrome::progress_card(
                    "mh-path",
                    "Finding the path",
                    "Each step evaluates every affordable +1 with 2,000 trials.",
                    progress,
                    move |_window, cx| {
                        cancel.update(cx, |state, cx| state.cancel_minehead_path(cx));
                    },
                ));
            }
        } else if task.cancelled {
            body = body.child(idleon_ui::run_chrome::cancelled_banner(
                "mh-path-cancelled",
                task.result.is_some(),
            ));
        }
    }
    if task.as_ref().is_some_and(|task| task.stale) {
        body = body.child(idleon_ui::run_chrome::stale_banner("mh-path-stale"));
    }
    if let Some(result) = task.as_ref().and_then(|task| task.result.as_ref()) {
        body = body.child(result_card(result, muted, cx.theme().success));
    }

    div()
        .id("minehead-path")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(controls)
        .child(body)
        .into_any_element()
}

/// The path table's columns.
const COLUMNS: [Column; 6] = [
    ("Step", Some(56.0), true),
    ("Buy upgrade", None, false),
    ("Cost", None, true),
    ("Win%", None, true),
    ("Avg Dmg", None, true),
    ("Turns", None, true),
];

fn result_card(result: &Path, muted: gpui::Hsla, success: gpui::Hsla) -> AnyElement {
    let cell = |index| columns::cell(&COLUMNS, index).min_w(px(56.0));
    let body = TableBody::new().children(result.steps.iter().map(|step| {
        let name = idleon_minehead::game()
            .upgrade(step.upg_idx)
            .map(|upgrade| upgrade.name.replace('_', " "))
            .unwrap_or_default();
        TableRow::new()
            .child(cell(0).child(format!("{}", step.step)))
            .child(cell(1).child(div().id(("mh-path-name", step.upg_idx)).child(name)))
            .child(cell(2).child(format::value(step.cost)))
            .child(
                cell(3)
                    .text_color(if step.result.win_rate > 0.8 {
                        success
                    } else {
                        muted
                    })
                    .child(format!("{:.1}%", step.result.win_rate * 100.0)),
            )
            .child(cell(4).child(format::value(step.result.avg_dmg)))
            .child(cell(5).child(format!("{:.1}", step.result.avg_turns)))
    }));
    let table = div().h(px(420.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("mh-path-table")
            .size_full()
            .child(
                Table::new()
                    .small()
                    .child(columns::header(&COLUMNS))
                    .child(body),
            ),
    );
    Card::new()
        .header(
            CardHeader::new(format!(
                "{} upgrades · {} spent",
                result.steps.len(),
                format::value(result.spent)
            ))
            .description(format!(
                "Baseline {} avg dmg · {:.1}% win · {} left",
                format::value(result.baseline.avg_dmg),
                result.baseline.win_rate * 100.0,
                format::value(result.remaining_currency)
            )),
        )
        .child(table)
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(format!(
                    "After {} upgrades: {} avg dmg, {:.1}% win (was {} · {:.1}%).",
                    result.steps.len(),
                    format::value(result.final_result.avg_dmg),
                    result.final_result.win_rate * 100.0,
                    format::value(result.baseline.avg_dmg),
                    result.baseline.win_rate * 100.0,
                )),
        )
        .into_any_element()
}
