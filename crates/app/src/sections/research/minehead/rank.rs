//! Rank Upgrades: what one more level of each unlocked upgrade buys, as
//! `_renderRankTab` in `js/ui/minehead-ui.js`.

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
use idleon_minehead::RankOutcome;
use idleon_ui::format;

impl AppState {
    /// Rank every unlocked upgrade's +1 against a baseline run.
    pub(crate) fn run_minehead_rank(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = run_inputs(self, 0, 10_000) else {
            return;
        };
        self.minehead_rank.update(cx, |runner, cx| {
            runner.start(cx, inputs, |inputs, report| {
                idleon_minehead::rank_upgrades(
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

    fn cancel_minehead_rank(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.minehead_rank.clone(),
            "Cancelling the upgrade ranking…",
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
    let running = state.minehead_rank.read(cx).running();
    let run_inputs = run_inputs(state, 0, 10_000);
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_rank.read(cx).state(inputs));
    let params_label = super::optimize::strategy_label(&state.research.minehead.params());
    let checked = state.research.minehead.use_inferred;
    let toggle_entity = entity.clone();
    let use_inferred = Checkbox::new("mh-rank-inferred", checked)
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
            CardHeader::new("Rank upgrades")
                .description(format!(
                    "Simulates each unlocked upgrade +1 on {} with 10,000 trials.",
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
                        "mh-rank-run",
                        if running {
                            "Ranking…"
                        } else {
                            "Rank Upgrades"
                        },
                    )
                    .primary()
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move |_event, _window, cx| {
                        run.update(cx, |state, cx| state.run_minehead_rank(cx));
                    }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-rank-cancel", "Cancel").on_click(
                        move |_event, _window, cx| {
                            cancel.update(cx, |state, cx| state.cancel_minehead_rank(cx));
                        },
                    ))
                })
                .child(use_inferred)
                .child(Caption::new(format::value(inputs.currency)).text_color(muted)),
        );

    let mut body = div().flex().flex_col().gap_3().min_h_0();
    if let Some(task) = task.as_ref() {
        if task.running {
            if let Some(progress) = task.progress.clone() {
                let cancel = entity.clone();
                body = body.child(idleon_ui::run_chrome::progress_card(
                    "mh-rank",
                    "Ranking upgrades",
                    "One baseline, then one 10,000-trial run per unlocked upgrade.",
                    progress,
                    move |_window, cx| {
                        cancel.update(cx, |state, cx| state.cancel_minehead_rank(cx));
                    },
                ));
            }
        } else if task.cancelled {
            body = body.child(idleon_ui::run_chrome::cancelled_banner(
                "mh-rank-cancelled",
                task.result.is_some(),
            ));
        }
    }
    if task.as_ref().is_some_and(|task| task.stale) {
        body = body.child(idleon_ui::run_chrome::stale_banner("mh-rank-stale"));
    }
    if let Some(result) = task.as_ref().and_then(|task| task.result.clone()) {
        body = body.child(result_card(&result, muted, cx.theme().colors.success));
    }

    div()
        .id("minehead-rank")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(controls)
        .child(body)
        .into_any_element()
}

fn result_card(result: &RankOutcome, muted: gpui::Hsla, success: gpui::Hsla) -> AnyElement {
    let mut table = Table::new([
        "#",
        "Upgrade",
        "Affordable",
        "Dmg%",
        "Cost",
        "Dmg/Cost",
        "Win%",
        "Avg Dmg",
        "Turns",
    ]);
    for (rank, row) in result.rows.iter().enumerate() {
        table = table.row([
            div().child(format!("{}", rank + 1)).into_any_element(),
            div()
                .id(("mh-rank-name", row.idx))
                .child(row.name.replace('_', " "))
                .into_any_element(),
            div()
                .text_color(if row.can_afford { success } else { muted })
                .child(if row.can_afford { "Yes" } else { "No" })
                .into_any_element(),
            div()
                .child(format!("{:+.1}%", row.dmg_pct))
                .into_any_element(),
            div().child(format::value(row.cost)).into_any_element(),
            div()
                .child(format::value(row.dmg_per_cost))
                .into_any_element(),
            div()
                .child(format!(
                    "{:.1}% ({:+.1})",
                    row.result.win_rate * 100.0,
                    row.win_delta
                ))
                .into_any_element(),
            div()
                .child(format::value(row.result.avg_dmg))
                .into_any_element(),
            div()
                .child(format!("{:.1}", row.result.avg_turns))
                .into_any_element(),
        ]);
    }
    let table = ScrollArea::new("mh-rank-table")
        .vertical()
        .h(px(420.0))
        .child(div().min_w_0().child(table));
    Card::new()
        .header(CardHeader::new("Upgrade rankings").description(format!(
            "Baseline {} avg dmg · {:.1}% win · {} trials",
            format::value(result.baseline.avg_dmg),
            result.baseline.win_rate * 100.0,
            result.trials
        )))
        .child(div().min_h_0().overflow_hidden().p_3().child(table))
        .child(
            div()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child("Sorted by damage gained per currency spent."),
        )
        .into_any_element()
}
