//! The simulated run: its cards, cell contribution and critical timeline.

use crate::sections::jelly_operator::outlook::CONTRIBUTION;
use crate::sections::jelly_operator::parts::{figure, stat, strips, text_kpi, text_table};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::alert::Alert;
use idleon_jelly::Trial;
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, Timeline, TimelineItem, Tone};

/// The simulated run: its outcome, cell contribution and critical timeline.
pub(crate) fn card(trial: &Trial, stale: bool) -> AnyElement {
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
        header = header.action(run_chrome::stale_badge("jelly-sim-result"));
    }
    let card = Card::new().header(header);
    if !trial.valid {
        return card
            .child(
                Alert::new(
                    "jelly-sim-invalid",
                    format!("The sandbox board is not valid: {}", trial.reason),
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            )
            .into_any_element();
    }
    card.child(
        div()
            .id("jelly-sim-result")
            .flex()
            .flex_col()
            .gap(PANEL_GAP)
            .child(cards(trial))
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

/// How one attempt describes its Stronkroid activation.
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

fn cards(trial: &Trial) -> AnyElement {
    strips(
        "jelly-sim-cards",
        vec![
            text_kpi(
                "jelly-sim-outcome",
                "Outcome",
                status(trial).to_string(),
                format!("{:.2}s · {}", trial.time, trial.ended_by.replace('-', " ")),
            ),
            stat("jelly-sim-damage", "Damage", trial.damage).caption(format!(
                "{} of {} boss HP · {:.1}%",
                format::value(trial.damage),
                format::value(trial.hp),
                100.0 * trial.damage_pct
            )),
            stat("jelly-sim-peak-dps", "Peak DPS", trial.peak_dps).caption(format!(
                "Best DPS {} → {}",
                format::value(trial.best_dps_before),
                format::value(trial.best_dps_after)
            )),
            stat(
                "jelly-sim-bloodcells",
                "Bloodcells earned",
                trial.bloodcells,
            )
            .caption(format!("Attempt change: {:+.0}", trial.attempt_delta)),
            stat("jelly-sim-cell-exp", "Cell EXP earned", trial.cell_exp)
                .caption(format!("{} hits from {} shots", trial.hits, trial.shots)),
            figure(
                "jelly-sim-critical-time",
                "Critical survival",
                trial.critical_time,
                2,
            )
            .suffix("s")
            .caption(format!(
                "{} deaths · {} revives used",
                trial.deaths, trial.revives_used
            )),
            stat("jelly-sim-amoeba", "Amoeba weakening", trial.amoeba_stacks).caption(format!(
                "{} at Critical start",
                trial.amoeba_stacks_at_critical_start
            )),
            figure(
                "jelly-sim-immunoid",
                "Immunoid focus",
                trial.immunoid_focus_time,
                2,
            )
            .suffix("s")
            .caption(format!("{} focused attacks", trial.immunoid_hits)),
            text_kpi(
                "jelly-sim-roid-card",
                "Stronkroid",
                roid_label(trial),
                format!("{}s normal phase", trial.standard_time),
            ),
        ],
    )
}

fn contribution(trial: &Trial) -> AnyElement {
    let table = text_table(
        &CONTRIBUTION,
        trial
            .contribution
            .iter()
            .filter(|row| row.shots > 0.0 || row.hits > 0.0)
            .map(|row| {
                vec![
                    row.name.clone(),
                    format::number(row.shots),
                    format::number(row.hits),
                    format::value(row.damage),
                    format::value(row.bloodcells),
                    format::value(row.exp),
                ]
            }),
    );
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
