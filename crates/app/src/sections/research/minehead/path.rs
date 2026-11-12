//! Upgrade Path: spend the saved currency on the most cost-effective +1 each
//! step, as `_renderPathTab` in `js/ui/minehead-ui.js`.

use super::{boss_name, run_inputs};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_minehead::Path;
use idleon_ui::format;

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
    let Some(inputs) = state.research.minehead.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let running = state.minehead_path.read(cx).running();
    let run_inputs = run_inputs(state, 50, 2000);
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_path.read(cx).state(inputs));
    let params_label = super::optimize::strategy_label(&state.research.minehead.params());
    let checked = state.research.minehead.use_inferred;
    let toggle_entity = entity.clone();
    let use_inferred = Checkbox::new("mh-path-inferred", checked)
        .label("Your Strategy")
        .on_change(move |on, _window, cx| {
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
                    boss_name(inputs.floor)
                ))
                .action(Badge::new(params_label).tone(Tone::Info)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pb_3()
                .child(
                    Button::new(
                        "mh-path-run",
                        if running {
                            "Finding path…"
                        } else {
                            "Find Upgrade Path"
                        },
                    )
                    .primary()
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move |_event, _window, cx| {
                        run.update(cx, |state, cx| state.run_minehead_path(cx));
                    }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-path-cancel", "Cancel").on_click(
                        move |_event, _window, cx| {
                            cancel.update(cx, |state, cx| state.cancel_minehead_path(cx));
                        },
                    ))
                })
                .child(use_inferred)
                .child(
                    Caption::new(format!("{} saved", format::value(inputs.currency)))
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
    if let Some(result) = task.as_ref().and_then(|task| task.result.clone()) {
        body = body.child(result_card(&result, muted, cx.theme().colors.success));
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

fn result_card(result: &Path, muted: gpui::Hsla, success: gpui::Hsla) -> AnyElement {
    let mut table = Table::new(["Step", "Buy upgrade", "Cost", "Win%", "Avg Dmg", "Turns"]);
    for step in &result.steps {
        let name = idleon_minehead::game()
            .upgrade(step.upg_idx)
            .map(|upgrade| upgrade.name.replace('_', " "))
            .unwrap_or_default();
        table = table.row([
            div().child(format!("{}", step.step)).into_any_element(),
            div()
                .id(("mh-path-name", step.upg_idx))
                .child(name)
                .into_any_element(),
            div().child(format::value(step.cost)).into_any_element(),
            div()
                .text_color(if step.result.win_rate > 0.8 {
                    success
                } else {
                    muted
                })
                .child(format!("{:.1}%", step.result.win_rate * 100.0))
                .into_any_element(),
            div()
                .child(format::value(step.result.avg_dmg))
                .into_any_element(),
            div()
                .child(format!("{:.1}", step.result.avg_turns))
                .into_any_element(),
        ]);
    }
    let table = ScrollArea::new("mh-path-table")
        .vertical()
        .h(px(420.0))
        .child(div().min_w_0().child(table));
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
        .child(div().min_h_0().overflow_hidden().p_3().child(table))
        .child(
            div()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
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
