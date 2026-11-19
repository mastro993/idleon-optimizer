//! The Fleet tab: current versus optimized pairings on the selected island.

use super::controls::cached;
use super::table::{self, Cell};
use super::{artifacts, captain_details, captain_short, trip};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Pixels, SharedString, Styled};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_sailing::{AssignmentMode, FleetPlan, Goal, RouteMetrics, route_metrics};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, fit_columns};

/// The figures the tab shows, read once per [`Key`](super::controls::Key).
pub(crate) struct View {
    title: SharedString,
    description: SharedString,
    pairings: usize,
    summary: SharedString,
    mode: SharedString,
    generated: SharedString,
    kept: SharedString,
    treasure: SharedString,
    artifacts: SharedString,
    gain: SharedString,
    minimum: SharedString,
}

/// One boat's pairing.
pub(crate) struct Row {
    boat: usize,
    captain: SharedString,
    details: SharedString,
    route: SharedString,
    trip: SharedString,
    chests: SharedString,
    value: SharedString,
    treasure: SharedString,
    artifacts: SharedString,
    best: SharedString,
}

pub(crate) fn columns() -> Vec<Cell<Row>> {
    vec![
        Cell::text("Boat", 80.0, |row| format!("Boat {}", row.boat + 1).into()),
        Cell::element(
            "Captain",
            110.0,
            |row, _| {
                table::tip(
                    ("fleet-captain", row.boat),
                    row.details.clone(),
                    row.captain.clone(),
                )
            },
            |row| row.captain.clone(),
        ),
        Cell::text("Route", 130.0, |row| row.route.clone()),
        Cell::number("Trip", 80.0, |row| row.trip.clone()),
        Cell::number("Chest/hr", 90.0, |row| row.chests.clone()),
        Cell::number("Base chest", 100.0, |row| row.value.clone()),
        Cell::number("Treasure/hr", 110.0, |row| row.treasure.clone()),
        Cell::number("Artifact/hr", 120.0, |row| row.artifacts.clone()),
        Cell::text("Best route", 170.0, |row| row.best.clone()),
    ]
}

/// The fleet tab: the KPI row over the assignment table.
pub(crate) fn fleet_tab(
    state: &mut AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = state.sailing_key();
    let view = cached(state, |controls| &mut controls.fleet, key, cx, build);
    let muted = cx.theme().muted_foreground;
    let kpis = KpiStrip::new()
        .child(
            Statistic::new("fleet-mode", "Fleet assignment", view.mode.clone())
                .caption(format!("{} active boat/captain pairings", view.pairings)),
        )
        .child(
            Statistic::new(
                "kpi-generated",
                "Generated chests/hr",
                view.generated.clone(),
            )
            .caption(view.kept.clone()),
        )
        .child(
            Statistic::new("kpi-treasure", "Direct treasure/hr", view.treasure.clone())
                .caption("generated kept tiers"),
        )
        .child(
            Statistic::new("kpi-artifacts", "Artifact finds/hr", view.artifacts.clone())
                .caption("expected successful chest opens"),
        )
        .child(
            gpui::div().id("fleet-gain").child(
                Statistic::new("kpi-gain", "Optimized gain", view.gain.clone())
                    .caption("optimizer versus the save"),
            ),
        )
        .child(
            Statistic::new("kpi-minimum", "Minimum trip", view.minimum.clone())
                .suffix("min")
                .caption("family/shiny travel reductions not modelled"),
        );
    let table = Card::new()
        .header(CardHeader::new(view.title.clone()).description(view.description.clone()))
        .child(
            gpui::div()
                .id("fleet-table")
                .h(table::data_height(view.pairings, 33.0, table_height))
                .child(fit_columns(DataTable::new(&state.sailing_controls.fleet_table), &state.sailing_controls.fleet_table)),
        )
        .child(
            Caption::new(
                "Current save keeps the stored captain-to-boat pairings. Optimized assigns each captain once to maximise the selected goal. Account-wide loot, speed and artifact multipliers outside the save model are excluded, so absolute rates are a lower bound.",
            )
            .text_color(muted),
        );
    v_flex()
        .id("fleet-body")
        .gap_3()
        .child(
            gpui::div()
                .id("fleet-kpis")
                .role(gpui::Role::Label)
                .aria_label(view.summary.clone())
                .child(kpis),
        )
        .child(table)
        .into_any_element()
}

/// Compare the pairings once and fill the table.
fn build(state: &AppState, cx: &mut App) -> View {
    let comparison = state.fleet_comparison();
    let plan = match state.sailing_state.mode {
        AssignmentMode::Optimized => &comparison.optimized,
        AssignmentMode::Saved => &comparison.saved,
    };
    let gain = comparison.gain_pct();
    let rows = plan
        .routes
        .iter()
        .map(|route| row(state, plan, route))
        .collect();
    table::set_rows(&state.sailing_controls.fleet_table, rows, cx);

    let mode = plan.mode.label();
    let pairings = plan.routes.len();
    let gain_label = if plan.mode == AssignmentMode::Saved && pairings == 0 {
        "—".to_string()
    } else if gain.abs() < 0.005 {
        "0.00%".to_string()
    } else {
        format!("{gain:+.2}%")
    };
    let minimum = plan
        .routes
        .first()
        .map(|route| route.timing.minimum_minutes)
        .unwrap_or(0.0);
    View {
        title: format!("{mode} Fleet Assignment").into(),
        description: format!(
            "{pairings} pairings · trip rate, boat value, natural tier odds, payouts and artifact odds are included."
        )
        .into(),
        pairings,
        summary: format!(
            "{mode} fleet · {pairings} pairings · {} treasure/hr · {} artifacts/hr · {gain_label} gain",
            format::rate(plan.treasure_per_hour()),
            artifacts(plan.artifact_finds_per_hour()),
        )
        .into(),
        mode: mode.into(),
        generated: format!("{:.2}", plan.generated_chests_per_hour()).into(),
        kept: format!(
            "{} kept at tier {}+",
            format::rate(plan.kept_chests_per_hour()),
            plan.routes
                .first()
                .map(|route| route.minimum_tier)
                .unwrap_or(0)
        )
        .into(),
        treasure: format::count(plan.treasure_per_hour().max(0.0).round() as u64).into(),
        artifacts: artifacts(plan.artifact_finds_per_hour()).into(),
        gain: gain_label.into(),
        minimum: format!("{minimum:.0}").into(),
    }
}

fn row(state: &AppState, plan: &FleetPlan, route: &RouteMetrics) -> Row {
    let best = best_island(state, route);
    Row {
        boat: route.boat,
        captain: route
            .captain
            .map(|captain| captain_short(state, captain))
            .unwrap_or_else(|| "—".to_string())
            .into(),
        details: route
            .captain
            .map(|captain| captain_details(state, captain))
            .unwrap_or_else(|| "No captain".to_string())
            .into(),
        route: format!(
            "Island {} · {}",
            route.island + 1,
            format::rate(route.timing.distance)
        )
        .into(),
        trip: trip(route.timing.trip_seconds).into(),
        chests: format!("{:.2}", route.generated_chests_per_hour).into(),
        value: format::rate(route.loot.value).into(),
        treasure: format::rate(route.treasure_per_hour).into(),
        artifacts: artifacts(route.artifact_finds_per_hour).into(),
        best: match best {
            Some(route) => {
                let score = route.score(plan.goal);
                let value = if plan.goal == Goal::Artifact {
                    artifacts(score)
                } else {
                    format::rate(score)
                };
                format!("Island {} · {}", route.island + 1, value)
            }
            None => "—".into(),
        }
        .into(),
    }
}

/// The best island for one pairing under the plan's goal.
fn best_island(state: &AppState, route: &RouteMetrics) -> Option<RouteMetrics> {
    let available = state.fleet.available_islands().max(1);
    (0..available)
        .map(|island| {
            route_metrics(
                &state.fleet,
                route.boat,
                island,
                route.captain,
                &state.sailing,
            )
        })
        .max_by(|a, b| {
            a.score(state.sailing_state.goal)
                .total_cmp(&b.score(state.sailing_state.goal))
        })
}
