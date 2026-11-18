//! Rank Upgrades: what one more level of each unlocked upgrade buys.

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
use idleon_minehead::RankOutcome;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

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
    let Some(inputs) = state.research.minehead.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let (floor, currency) = (inputs.floor, inputs.currency);
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let running = state.minehead_rank.read(cx).running();
    let run_inputs = run_inputs(state, 0, 10_000);
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_rank.read(cx).state(inputs));
    let params_label = super::optimize::strategy_label(&state.research.minehead.params());
    let checked = state.research.minehead.use_inferred;
    let toggle_entity = entity.clone();
    let use_inferred = Checkbox::new("mh-rank-inferred")
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
            CardHeader::new("Rank upgrades")
                .description(format!(
                    "Simulates each unlocked upgrade +1 on {} with 10,000 trials.",
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
                    Button::new("mh-rank-run")
                        .label(if running {
                            "Ranking…"
                        } else {
                            "Rank Upgrades"
                        })
                        .primary()
                        .icon(IconName::Play)
                        .disabled(running)
                        .on_click(move |_event, _window, cx| {
                            run.update(cx, |state, cx| state.run_minehead_rank(cx));
                        }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-rank-cancel").label("Cancel").on_click(
                        move |_event, _window, cx| {
                            cancel.update(cx, |state, cx| state.cancel_minehead_rank(cx));
                        },
                    ))
                })
                .child(use_inferred)
                .child(Caption::new(format::value(currency)).text_color(muted)),
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
    if let Some(result) = task.as_ref().and_then(|task| task.result.as_ref()) {
        body = body.child(result_card(result, muted, cx.theme().success));
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

/// The ranking table's columns.
const COLUMNS: [Column; 9] = [
    ("#", Some(36.0), true),
    ("Upgrade", Some(150.0), false),
    ("Affordable", Some(80.0), false),
    ("Dmg%", None, true),
    ("Cost", None, true),
    ("Dmg/Cost", None, true),
    ("Win%", Some(110.0), true),
    ("Avg Dmg", None, true),
    ("Turns", None, true),
];

fn result_card(result: &RankOutcome, muted: gpui::Hsla, success: gpui::Hsla) -> AnyElement {
    let cell = |index| columns::cell(&COLUMNS, index).min_w(px(56.0));
    let body = TableBody::new().children(result.rows.iter().enumerate().map(|(rank, row)| {
        TableRow::new()
            .child(cell(0).child(format!("{}", rank + 1)))
            .child(
                cell(1).child(
                    div()
                        .id(("mh-rank-name", row.idx))
                        .child(row.name.replace('_', " ")),
                ),
            )
            .child(
                cell(2)
                    .text_color(if row.can_afford { success } else { muted })
                    .child(if row.can_afford { "Yes" } else { "No" }),
            )
            .child(cell(3).child(format!("{:+.1}%", row.dmg_pct)))
            .child(cell(4).child(format::value(row.cost)))
            .child(cell(5).child(format::value(row.dmg_per_cost)))
            .child(cell(6).child(format!(
                "{:.1}% ({:+.1})",
                row.result.win_rate * 100.0,
                row.win_delta
            )))
            .child(cell(7).child(format::value(row.result.avg_dmg)))
            .child(cell(8).child(format!("{:.1}", row.result.avg_turns)))
    }));
    let table = div().h(px(420.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("mh-rank-table")
            .size_full()
            .child(
                Table::new()
                    .small()
                    .child(columns::header(&COLUMNS))
                    .child(body),
            ),
    );
    Card::new()
        .header(CardHeader::new("Upgrade rankings").description(format!(
            "Baseline {} avg dmg · {:.1}% win · {} trials",
            format::value(result.baseline.avg_dmg),
            result.baseline.win_rate * 100.0,
            result.trials
        )))
        .child(table)
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child("Sorted by damage gained per currency spent."),
        )
        .into_any_element()
}
