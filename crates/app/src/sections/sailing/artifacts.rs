//! The Artifacts tab: the island ranking and the selected island's artifacts.
//!
//! The tab ranks every unlocked island by the selected boat and captain's
//! artifact finds per hour, then lists the selected island's artifacts with
//! their next-tier denominator and the chance one generated chest finds them.
//! The ranking only counts islands the save can reach, and a capped artifact
//! stays capped until Verisimilitude unlocks the seventh tier.
//!
//! Both tables are bounded by the game: one row per island, and the island's
//! own artifacts.

use super::controls::cached;
use super::table;
use super::{artifacts as finds, captain_short, chance, trip};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_sailing::{
    ArtifactOutcome, RouteMetrics, incomplete_artifacts, max_artifact_tier, route_metrics,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, TableColumn, Tone};

/// One island of the ranking, formatted.
struct IslandRow {
    island: usize,
    incomplete: SharedString,
    trip: SharedString,
    per_chest: SharedString,
    finds: SharedString,
}

/// One of the selected island's artifacts, formatted.
struct ArtifactRow {
    artifact: usize,
    name: SharedString,
    tip: SharedString,
    tier: SharedString,
    odds: SharedString,
    chance: SharedString,
    status: (&'static str, Tone),
}

/// The figures the tab shows, read once per key and boat.
pub(crate) struct View {
    summary: SharedString,
    multiplier: SharedString,
    chance: SharedString,
    finds: SharedString,
    cap: u32,
    island: usize,
    ranking: Vec<IslandRow>,
    artifacts: Vec<ArtifactRow>,
    note: SharedString,
}

/// The artifacts tab: the KPIs, the island ranking and the island's own artifacts.
pub(crate) fn artifacts_tab(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let key = (state.sailing_key(), state.sailing_state.breakdown_boat);
    let view = cached(state, |controls| &mut controls.artifacts, key, cx, build);
    let Some(view) = view.as_ref() else {
        return div().into_any_element();
    };
    let muted = cx.theme().muted_foreground;
    let kpis = div()
        .id("artifacts-kpis")
        .role(gpui::Role::Label)
        .aria_label(view.summary.clone())
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "artifacts-kpi-multiplier",
                        "Boat artifact multiplier",
                        view.multiplier.clone(),
                    )
                    .caption("before the chest tier's own multiplier"),
                )
                .child(
                    Statistic::new(
                        "artifacts-kpi-chance",
                        "Selected island",
                        view.chance.clone(),
                    )
                    .caption("per generated chest, kept tiers only"),
                )
                .child(
                    Statistic::new("artifacts-kpi-finds", "Direct finds/hr", view.finds.clone())
                        .caption("natural tiers and the minimum chest filter included"),
                )
                .child(
                    Statistic::new(
                        "artifacts-kpi-cap",
                        "Artifact cap",
                        format!("Tier {}", view.cap),
                    )
                    .caption("Verisimilitude (Menagerie unlock 0) raises it to Tier 7"),
                ),
        );
    let ranking = Card::new()
        .header(
            CardHeader::new("Artifact route ranking")
                .description("Every unlocked island at the selected boat and captain, best first"),
        )
        .child(div().id("artifacts-ranking").child(table::table(
            "artifacts-ranking-scroll",
            &[
                TableColumn::text("Island"),
                TableColumn::number("Incomplete"),
                TableColumn::number("Trip"),
                TableColumn::number("Per chest"),
                TableColumn::number("Direct finds/hr"),
            ],
            view.ranking.iter().enumerate().map(|(index, row)| {
                table::Row::new(("artifacts-row", row.island))
                    .child(table::cell(
                        h_flex()
                            .id(("artifacts-island", row.island))
                            .gap_2()
                            .child(format!("Island {}", row.island + 1))
                            .when(index == 0, |cell| {
                                cell.child(table::tag("Best", Tone::Success))
                            }),
                    ))
                    .child(table::cell(
                        div()
                            .id(("artifacts-incomplete", row.island))
                            .child(row.incomplete.clone()),
                    ))
                    .child(table::cell(row.trip.clone()))
                    .child(table::cell(row.per_chest.clone()))
                    .child(table::cell(row.finds.clone()))
            }),
        )));
    let island = Card::new()
        .header(
            CardHeader::new(format!("Selected island · Island {}", view.island + 1)).description(
                format!(
                    "{} artifacts · next tier odds and the chance one kept chest finds each",
                    view.artifacts.len()
                ),
            ),
        )
        .child(div().id("artifacts-island-table").child(table::table(
            "artifacts-island-scroll",
            &[
                TableColumn::text("Artifact"),
                TableColumn::number("Current tier"),
                TableColumn::number("Next odds"),
                TableColumn::number("Per generated chest"),
                TableColumn::text("Status"),
            ],
            view.artifacts.iter().map(|row| {
                let tip = row.tip.clone();
                table::Row::new(("artifact-row", row.artifact))
                    .child(table::cell(
                        div()
                            .id(("artifact-name", row.artifact))
                            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                            .child(row.name.clone()),
                    ))
                    .child(table::cell(row.tier.clone()))
                    .child(table::cell(
                        div()
                            .id(("artifact-odds", row.artifact))
                            .child(row.odds.clone()),
                    ))
                    .child(table::cell(
                        div()
                            .id(("artifact-chance", row.artifact))
                            .child(row.chance.clone()),
                    ))
                    .child(table::cell(
                        div()
                            .id(("artifact-status", row.artifact))
                            .child(table::tag(row.status.0, row.status.1)),
                    ))
            }),
        )))
        .child(Caption::new(view.note.clone()).text_color(muted));
    v_flex()
        .id("artifacts-body")
        .gap_3()
        .child(kpis)
        .child(ranking)
        .child(island)
        .into_any_element()
}

/// Rank the islands for the selected boat once.
fn build(state: &AppState, _: &mut App) -> Option<View> {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let boat = state
        .sailing_state
        .breakdown_boat
        .min(state.fleet.boats.len().saturating_sub(1));
    let route = plan.routes.iter().find(|route| route.boat == boat)?;
    let mut ranking: Vec<RouteMetrics> = (0..state.fleet.available_islands().max(1))
        .map(|island| {
            route_metrics(
                &state.fleet,
                route.boat,
                island,
                route.captain,
                &state.sailing,
            )
        })
        .collect();
    ranking.sort_by(|left, right| {
        right
            .artifact_finds_per_hour
            .total_cmp(&left.artifact_finds_per_hour)
    });
    let cap = max_artifact_tier(state.sailing.unlocks);
    let captain = route
        .captain
        .map(|captain| captain_short(state, captain))
        .unwrap_or_else(|| "—".into());
    let min_tier = state.sailing.minimum_tier;
    Some(View {
        summary: format!(
            "Boat {} captain {} · {} artifact finds/hr",
            route.boat + 1,
            captain,
            finds(route.artifact_finds_per_hour)
        )
        .into(),
        multiplier: format!("{:.2}x", route.artifact_base.value).into(),
        chance: chance(route.artifact_chance_per_trip).into(),
        finds: finds(route.artifact_finds_per_hour).into(),
        cap,
        island: route.island,
        ranking: ranking
            .iter()
            .map(|route| IslandRow {
                island: route.island,
                incomplete: incomplete_artifacts(&state.fleet, route.island, state.sailing.unlocks)
                    .to_string()
                    .into(),
                trip: trip(route.timing.trip_seconds).into(),
                per_chest: chance(route.artifact_chance_per_trip).into(),
                finds: finds(route.artifact_finds_per_hour).into(),
            })
            .collect(),
        artifacts: route
            .artifact_outcomes
            .iter()
            .map(|outcome| ArtifactRow {
                artifact: outcome.artifact,
                name: outcome.name.clone().into(),
                tip: format!(
                    "Artifact {} · tier {} of {cap}",
                    outcome.artifact + 1,
                    outcome.current_tier
                )
                .into(),
                tier: outcome.current_tier.to_string().into(),
                odds: next_odds(outcome).into(),
                chance: chance(outcome.chance_per_trip).into(),
                status: status(outcome, cap),
            })
            .collect(),
        note: format!(
            "Next odds is the tier's own denominator: the artifact multiplier divided by it is \
             one check's chance, so a bigger multiplier finds the same artifact sooner. The per \
             chest column weights that chance by every chest tier kept at Tier {min_tier}+, \
             multiplied by the chest tier's own artifact bonus. Locked rows need the next \
             unlock; max means the artifact is at the highest tier this save can reach \
             (Tier {cap})."
        )
        .into(),
    })
}

fn next_odds(outcome: &ArtifactOutcome) -> String {
    if !outcome.unlocked || !outcome.denominator.is_finite() {
        "—".into()
    } else {
        format!("1 in {}", format::fmt_val(outcome.denominator))
    }
}

fn status(outcome: &ArtifactOutcome, cap: u32) -> (&'static str, Tone) {
    if outcome.current_tier >= cap {
        ("Max", Tone::Success)
    } else if outcome.unlocked {
        ("Available", Tone::Info)
    } else {
        ("Locked", Tone::Warning)
    }
}
