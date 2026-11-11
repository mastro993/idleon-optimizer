//! The Artifacts tab: the island ranking and the selected island's artifacts.
//!
//! The upstream page ranks every unlocked island by the selected boat and
//! captain's artifact finds per hour, then lists the selected island's artifacts
//! with their next-tier denominator and the chance one generated chest finds
//! them. The ranking only counts islands the save can reach, and a capped
//! artifact stays capped until Verisimilitude unlocks the seventh tier.

use super::{artifacts as finds, captain_short, chance, text_kpi, trip};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, div};
use idleon_core::sailing::{
    ArtifactOutcome, RouteMetrics, incomplete_artifacts, max_artifact_tier, route_metrics,
};

/// The artifacts tab: the KPIs, the island ranking and the island's own artifacts.
pub fn artifacts_tab(
    state: &AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let boat = state
        .sailing_state
        .breakdown_boat
        .min(state.fleet.boats.len().saturating_sub(1));
    let Some(selected) = plan.routes.iter().find(|route| route.boat == boat).cloned() else {
        return div().into_any_element();
    };
    let mut ranking: Vec<RouteMetrics> = (0..state.fleet.available_islands().max(1))
        .map(|island| {
            route_metrics(
                &state.fleet,
                selected.boat,
                island,
                selected.captain,
                &state.sailing,
            )
        })
        .collect();
    ranking.sort_by(|left, right| {
        right
            .artifact_finds_per_hour
            .total_cmp(&left.artifact_finds_per_hour)
    });
    div()
        .id("artifacts-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(kpis(state, &selected, cx))
        .child(ranking_table(state, &ranking, table_height))
        .child(island_artifacts(state, &selected, table_height, cx))
        .into_any_element()
}

fn kpis(state: &AppState, route: &RouteMetrics, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let cap = max_artifact_tier(state.sailing.unlocks);
    let captain = route
        .captain
        .map(|captain| captain_short(state, captain))
        .unwrap_or_else(|| "—".into());
    div()
        .id("artifacts-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Boat {} captain {} · {} artifact finds/hr",
            route.boat + 1,
            captain,
            finds(route.artifact_finds_per_hour)
        ))
        .flex()
        .flex_wrap()
        .gap_2()
        .child(text_kpi(
            "artifacts-kpi-multiplier",
            "Boat artifact multiplier",
            &format!("{:.2}x", route.artifact_base.value),
            "before the chest tier's own multiplier",
            muted,
        ))
        .child(text_kpi(
            "artifacts-kpi-chance",
            "Selected island",
            &chance(route.artifact_chance_per_trip),
            "per generated chest, kept tiers only",
            muted,
        ))
        .child(text_kpi(
            "artifacts-kpi-finds",
            "Direct finds/hr",
            &finds(route.artifact_finds_per_hour),
            "natural tiers and the minimum chest filter included",
            muted,
        ))
        .child(text_kpi(
            "artifacts-kpi-cap",
            "Artifact cap",
            &format!("Tier {cap}"),
            "Verisimilitude (Menagerie unlock 0) raises it to Tier 7",
            muted,
        ))
        .into_any_element()
}

fn ranking_table(state: &AppState, ranking: &[RouteMetrics], table_height: Pixels) -> AnyElement {
    let mut table = Table::new([
        "Island",
        "Incomplete",
        "Trip",
        "Per chest",
        "Direct finds/hr",
    ]);
    for (index, route) in ranking.iter().enumerate() {
        let incomplete = incomplete_artifacts(&state.fleet, route.island, state.sailing.unlocks);
        let island = div()
            .id(("artifacts-island", route.island))
            .child(if index == 0 {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(format!("Island {}", route.island + 1))
                    .child(Badge::new("Best").tone(Tone::Success))
                    .into_any_element()
            } else {
                div()
                    .child(format!("Island {}", route.island + 1))
                    .into_any_element()
            })
            .into_any_element();
        table = table.row([
            island,
            div()
                .id(("artifacts-incomplete", route.island))
                .child(incomplete.to_string())
                .into_any_element(),
            div()
                .child(trip(route.timing.trip_seconds))
                .into_any_element(),
            div()
                .child(chance(route.artifact_chance_per_trip))
                .into_any_element(),
            div()
                .child(finds(route.artifact_finds_per_hour))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Artifact route ranking")
                .description("Every unlocked island at the selected boat and captain, best first"),
        )
        .child(
            div()
                .id("artifacts-ranking")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(
                    ScrollArea::new("artifacts-ranking-scroll")
                        .vertical()
                        .h(table_height)
                        .child(div().min_w_0().child(table)),
                ),
        )
        .into_any_element()
}

fn island_artifacts(
    state: &AppState,
    route: &RouteMetrics,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let cap = max_artifact_tier(state.sailing.unlocks);
    let mut table = Table::new([
        "Artifact",
        "Current tier",
        "Next odds",
        "Per generated chest",
        "Status",
    ]);
    for outcome in &route.artifact_outcomes {
        table = table.row([
            div()
                .id(("artifact-name", outcome.artifact))
                .tooltip(Tooltip::text(format!(
                    "Artifact {} · tier {} of {cap}",
                    outcome.artifact + 1,
                    outcome.current_tier
                )))
                .child(outcome.name.clone())
                .into_any_element(),
            div()
                .child(outcome.current_tier.to_string())
                .into_any_element(),
            div()
                .id(("artifact-odds", outcome.artifact))
                .child(next_odds(outcome))
                .into_any_element(),
            div()
                .id(("artifact-chance", outcome.artifact))
                .child(chance(outcome.chance_per_trip))
                .into_any_element(),
            status_badge(outcome, cap),
        ]);
    }
    let min_tier = state.sailing.minimum_tier;
    Card::new()
        .header(
            CardHeader::new(format!("Selected island · Island {}", route.island + 1)).description(
                format!(
                    "{} artifacts · next tier odds and the chance one kept chest finds each",
                    route.artifact_outcomes.len()
                ),
            ),
        )
        .child(
            div()
                .id("artifacts-island-table")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(
                    ScrollArea::new("artifacts-island-scroll")
                        .vertical()
                        .h(table_height)
                        .child(div().min_w_0().child(table)),
                ),
        )
        .child(callout(
            cx,
            format!(
                "Next odds is the tier's own denominator: the artifact multiplier divided by it is \
                 one check's chance, so a bigger multiplier finds the same artifact sooner. The per \
                 chest column weights that chance by every chest tier kept at Tier {min_tier}+, \
                 multiplied by the chest tier's own artifact bonus. Locked rows need the next \
                 unlock; max means the artifact is at the highest tier this save can reach \
                 (Tier {cap})."
            ),
        ))
        .into_any_element()
}

fn next_odds(outcome: &ArtifactOutcome) -> String {
    if !outcome.unlocked || !outcome.denominator.is_finite() {
        "—".into()
    } else {
        format!("1 in {}", format::fmt_val(outcome.denominator))
    }
}

fn status_badge(outcome: &ArtifactOutcome, cap: u32) -> AnyElement {
    let (label, tone) = if outcome.current_tier >= cap {
        ("Max", Tone::Success)
    } else if outcome.unlocked {
        ("Available", Tone::Info)
    } else {
        ("Locked", Tone::Warning)
    };
    div()
        .id(("artifact-status", outcome.artifact))
        .child(Badge::new(label).tone(tone))
        .into_any_element()
}

fn callout(cx: &mut Context<AppState>, text: String) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .px_3()
        .pb_2()
        .child(Caption::new(text).text_color(muted))
        .into_any_element()
}
