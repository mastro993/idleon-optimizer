//! The operation estimate: the run controls, its aggregate and its outlook.

use super::outlook;
use super::parts::{field_row, figure, info, number_field, run_button, stat, strips};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::component::alert::Alert;
use idleon_jelly::Estimate;
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, Tone};

pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.jelly_task(cx);
    let running = task.running;
    let trials = number_field(
        "jelly-trials",
        "Trials",
        &state.jelly_controls.trials.clone(),
        f64::from(state.jelly_state.trials),
        window,
        cx,
    );
    let seed = number_field(
        "jelly-seed",
        "Seed",
        &state.jelly_controls.seed.clone(),
        f64::from(state.jelly_state.seed),
        window,
        cx,
    );
    let run = run_button(
        "jelly-run-estimate",
        "Run estimate",
        running,
        |state, cx| state.run_jelly_estimate(cx),
        cx,
    );
    let stale = task.stale;

    let mut body = div()
        .id("jelly-estimate")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            field_row()
                .id("jelly-estimate-controls")
                .child(trials)
                .child(seed)
                .child(run)
                .child(Caption::new(
                    "One seeded 60 FPS operation per trial, aggregated across the trials.",
                )),
        );
    if running && let Some(progress) = task.progress.clone() {
        let entity = cx.entity();
        body = body.child(run_chrome::progress_card(
            "jelly",
            "Estimating the operation",
            "Seeded trials of the saved board.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_jelly_estimate(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(run_chrome::cancelled_banner("jelly-cancelled", stale));
    }
    let (Some(result), Some(insights)) = (task.result, state.jelly_state.results.insights.clone())
    else {
        if !running && !task.cancelled {
            body = body.child(info(
                "jelly-estimate-empty",
                "Run the estimate to see the clear probability, the Bloodcell gain, the cell contributions and the outlook for the next obstructions.",
            ));
        }
        return body.into_any_element();
    };
    if stale {
        body = body.child(run_chrome::stale_banner("jelly-estimate-stale"));
    }
    body.child(
        div()
            .id("jelly-estimate-result")
            .flex()
            .flex_col()
            .gap(PANEL_GAP)
            .child(result_cards(&result, stale))
            .child(outlook::insights_card(&insights)),
    )
    .child(outlook::contribution_card(&result))
    .child(outlook::timeline_card(&result, cx))
    .into_any_element()
}

fn result_cards(result: &Estimate, stale: bool) -> AnyElement {
    if !result.valid {
        return Card::new()
            .header(CardHeader::new("Estimate result"))
            .child(
                Alert::new(
                    "jelly-estimate-invalid",
                    format!("The board is not a valid layout: {}", result.reason),
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            )
            .into_any_element();
    }
    let clear = 100.0 * result.clear_probability;
    let mut header = CardHeader::new("Estimate result").description(format!(
        "{} / {} seeds cleared · mean {:.0}% boss damage",
        result.success_count,
        result.trials,
        100.0 * result.mean_damage / result.representative.hp.max(1.0)
    ));
    if stale {
        header = header.action(run_chrome::stale_badge("jelly-estimate"));
    }
    Card::new()
        .header(header)
        .child(strips(
            "jelly-estimate-cards",
            vec![
                figure(
                    "jelly-estimate-clear-probability",
                    "Clear probability",
                    clear,
                    1,
                )
                .suffix("%")
                .caption(format!(
                    "{} to {:.1}% Wilson interval",
                    100.0 * result.clear_probability_low,
                    100.0 * result.clear_probability_high
                )),
                stat(
                    "jelly-estimate-clear-time",
                    "Median clear",
                    result.median_time.unwrap_or(0.0),
                )
                .caption(match result.p90_time {
                    Some(p90) => format!("{} · 90th {:.2}s", format::duration(p90), p90),
                    None => "No clears observed".to_string(),
                }),
                stat("jelly-estimate-damage", "Mean damage", result.mean_damage).caption(format!(
                    "Normal {} · critical {}",
                    format::value(result.mean_normal_damage),
                    format::value(result.mean_critical_damage)
                )),
                stat(
                    "jelly-estimate-bloodcells",
                    "Bloodcells / attempt",
                    result.mean_bloodcells,
                )
                .caption(format!(
                    "Median {} · max {}",
                    format::value(result.median_bloodcells.unwrap_or(0.0)),
                    format::value(result.max_bloodcells)
                )),
                stat(
                    "jelly-estimate-cell-exp",
                    "Cell EXP / attempt",
                    result.mean_cell_exp,
                )
                .caption("Spread over the cells the shots hit"),
                figure(
                    "jelly-estimate-critical",
                    "Critical time",
                    result.mean_critical_time,
                    2,
                )
                .suffix("s")
                .caption(format!(
                    "{} deaths · {} revives",
                    format::number(result.mean_deaths),
                    format::number(result.mean_revives)
                )),
                stat(
                    "jelly-estimate-peak-dps",
                    "Mean peak DPS",
                    result.mean_peak_dps,
                )
                .caption(format!(
                    "Best DPS ×{:.3} Bloodcells",
                    result.mean_best_dps_multiplier
                )),
                figure(
                    "jelly-estimate-live-slots",
                    "Live slots",
                    result.mean_live_slots,
                    0,
                )
                .caption(format!(
                    "{} boss attacks · {} hits",
                    format::number(result.mean_boss_attacks),
                    format::number(result.mean_hits)
                )),
            ],
        ))
        .into_any_element()
}
