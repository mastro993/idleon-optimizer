//! The simulated run: its cards, cell contribution and critical timeline.

use crate::sections::jelly_operator::estimate::{stat, text_kpi};
use crate::state::AppState;
use crate::ui::format;
use crate::ui::task;
use ely_gpui_component::data_display::{KpiCard, Statistic, Timeline, TimelineItem, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::jelly::Trial;

/// The simulated run: its outcome, cell contribution and critical timeline.
pub fn card(trial: &Trial, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let mut header = CardHeader::new("Simulated run").description(format!(
        "{} · {:.2}s · {} cell type(s) fired",
        status(trial),
        trial.time,
        trial
            .contribution
            .iter()
            .filter(|row| row.shots > 0.0 || row.hits > 0.0)
            .count()
    ));
    if stale {
        header = header.action(task::stale_badge("jelly-sim-result"));
    }
    let card = Card::new().header(header);
    if !trial.valid {
        return card
            .child(div().p_3().child(Banner::new(
                "jelly-sim-invalid",
                Severity::Warning,
                format!("The sandbox board is not valid: {}", trial.reason),
            )))
            .into_any_element();
    }
    card.child(
        div()
            .id("jelly-sim-result")
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(cards(trial, cx))
            .child(contribution(trial))
            .child(timeline(trial)),
    )
    .into_any_element()
}

fn status(trial: &Trial) -> &'static str {
    if trial.success {
        "Cleared"
    } else if trial.ended_by == "simulation-cap" {
        "Simulation cap reached"
    } else {
        "Failed"
    }
}

/// `roidTimingText`: how one attempt describes its Stronkroid activation.
fn roid_label(trial: &Trial) -> String {
    match trial.roid_start {
        None => "Not used".to_string(),
        Some(seconds) if seconds <= 0.0 => "Used immediately".to_string(),
        Some(seconds) if (seconds - trial.standard_time).abs() < 0.01 => {
            format!("Used at Critical start ({:.0}s)", trial.standard_time)
        }
        Some(seconds) => format!("Used {seconds:.0}s after start"),
    }
}

fn cards(trial: &Trial, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("jelly-sim-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(text_kpi(
            "jelly-sim-outcome",
            "Outcome",
            status(trial).to_string(),
            format!("{:.2}s · {}", trial.time, trial.ended_by.replace('-', " ")),
            cx,
        ))
        .child(
            KpiCard::new(stat("jelly-sim-damage", "Damage", trial.damage)).caption(format!(
                "{} of {} boss HP · {:.1}%",
                format::value(trial.damage),
                format::value(trial.hp),
                100.0 * trial.damage_pct
            )),
        )
        .child(
            KpiCard::new(stat("jelly-sim-peak-dps", "Peak DPS", trial.peak_dps)).caption(format!(
                "Best DPS {} → {}",
                format::value(trial.best_dps_before),
                format::value(trial.best_dps_after)
            )),
        )
        .child(
            KpiCard::new(stat(
                "jelly-sim-bloodcells",
                "Bloodcells earned",
                trial.bloodcells,
            ))
            .caption(format!("Attempt change: {:+.0}", trial.attempt_delta)),
        )
        .child(
            KpiCard::new(stat(
                "jelly-sim-cell-exp",
                "Cell EXP earned",
                trial.cell_exp,
            ))
            .caption(format!("{} hits from {} shots", trial.hits, trial.shots)),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "jelly-sim-critical-time",
                    "Critical survival",
                    trial.critical_time,
                )
                .decimals(2)
                .suffix("s"),
            )
            .caption(format!(
                "{} deaths · {} revives used",
                trial.deaths, trial.revives_used
            )),
        )
        .child(
            KpiCard::new(stat(
                "jelly-sim-amoeba",
                "Amoeba weakening",
                trial.amoeba_stacks,
            ))
            .caption(format!(
                "{} at Critical start",
                trial.amoeba_stacks_at_critical_start
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "jelly-sim-immunoid",
                    "Immunoid focus",
                    trial.immunoid_focus_time,
                )
                .decimals(2)
                .suffix("s"),
            )
            .caption(format!("{} focused attacks", trial.immunoid_hits)),
        )
        .child(text_kpi(
            "jelly-sim-roid-card",
            "Stronkroid",
            roid_label(trial),
            format!("{}s normal phase", trial.standard_time),
            cx,
        ))
        .into_any_element()
}

fn contribution(trial: &Trial) -> AnyElement {
    let mut table = Table::new(["Cell", "Shots", "Hits", "Damage", "Bloodcells", "EXP"]);
    for row in trial
        .contribution
        .iter()
        .filter(|row| row.shots > 0.0 || row.hits > 0.0)
    {
        table = table.row([
            row.name.clone(),
            format::number(row.shots),
            format::number(row.hits),
            format::value(row.damage),
            format::value(row.bloodcells),
            format::value(row.exp),
        ]);
    }
    div()
        .id("jelly-sim-contribution")
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new("Cell contribution"))
        .child(table)
        .into_any_element()
}

fn timeline(trial: &Trial) -> AnyElement {
    let mut items = Timeline::new();
    for death in &trial.death_log {
        let tone = if death.revived {
            Tone::Success
        } else if death.focused {
            Tone::Warning
        } else {
            Tone::Neutral
        };
        items = items.item(
            TimelineItem::new(format!(
                "{}{} slot {}",
                if death.focused { "Focused " } else { "" },
                if death.disabled_anchor {
                    "anchor"
                } else {
                    "footprint"
                },
                death.slot
            ))
            .time(format!("{:.2}s", death.time))
            .tone(tone)
            .child(Caption::new(if death.revived {
                "Revived by the automatic revive policy."
            } else {
                "Lost for the rest of the run."
            })),
        );
    }
    let mut body = div()
        .id("jelly-sim-timeline")
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new("Critical target timeline"));
    if trial.death_log.is_empty() {
        body = body.child(Caption::new("The run never reached the critical phase."));
    } else {
        body = body.child(items);
    }
    body.into_any_element()
}
