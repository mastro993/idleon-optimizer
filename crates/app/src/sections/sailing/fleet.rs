//! The Fleet tab: current versus optimized pairings on the selected island.

use super::{artifacts, captain_details, captain_short, trip};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, div};
use idleon_core::sailing::{AssignmentMode, FleetPlan, Goal, RouteMetrics, route_metrics};

/// The fleet tab: the KPI row over the assignment table.
pub fn fleet_tab(state: &AppState, table_height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    let comparison = state.fleet_comparison();
    let plan = match state.sailing_state.mode {
        AssignmentMode::Optimized => &comparison.optimized,
        AssignmentMode::Saved => &comparison.saved,
    };
    div()
        .id("fleet-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(kpis(plan, comparison.gain_pct(), cx))
        .child(table(state, plan, table_height, cx))
        .into_any_element()
}

fn kpis(plan: &FleetPlan, gain: f64, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
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
    div()
        .id("fleet-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{} fleet · {} pairings · {} treasure/hr · {} artifacts/hr · {} gain",
            mode,
            pairings,
            format::rate(plan.treasure_per_hour()),
            artifacts(plan.artifact_finds_per_hour()),
            gain_label
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(text_kpi(
            "fleet-mode",
            "Fleet assignment",
            mode,
            &format!("{pairings} active boat/captain pairings"),
            muted,
        ))
        .child(
            KpiCard::new(
                Statistic::new(
                    "kpi-generated",
                    "Generated chests/hr",
                    plan.generated_chests_per_hour(),
                )
                .decimals(2),
            )
            .caption(format!(
                "{} kept at tier {}+",
                format::rate(plan.kept_chests_per_hour()),
                plan.routes
                    .first()
                    .map(|route| route.minimum_tier)
                    .unwrap_or(0)
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kpi-treasure",
                    "Direct treasure/hr",
                    plan.treasure_per_hour(),
                )
                .decimals(0),
            )
            .caption("generated kept tiers"),
        )
        .child(text_kpi(
            "kpi-artifacts",
            "Artifact finds/hr",
            &artifacts(plan.artifact_finds_per_hour()),
            "expected successful chest opens",
            muted,
        ))
        .child(
            div().id("fleet-gain").child(
                KpiCard::new(
                    Statistic::new("kpi-gain", "Optimized gain", gain)
                        .decimals(2)
                        .suffix("%")
                        .size(TextSize::Lg),
                )
                .caption("optimizer versus the save"),
            ),
        )
        .child(
            KpiCard::new(Statistic::new("kpi-minimum", "Minimum trip", minimum).decimals(0))
                .caption("minutes; family/shiny travel reductions not modelled"),
        )
        .into_any_element()
}

fn text_kpi(
    id: &'static str,
    label: &'static str,
    value: &str,
    caption: &str,
    muted: gpui::Hsla,
) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(gpui::Role::Label)
                .aria_label(value.to_string())
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .child(Caption::new(label).text_color(muted))
                .child(div().text_size(gpui::px(20.0)).child(value.to_string()))
                .child(Caption::new(caption.to_string()).text_color(muted)),
        )
        .into_any_element()
}

fn table(
    state: &AppState,
    plan: &FleetPlan,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut table = Table::new([
        "Boat",
        "Captain",
        "Route",
        "Trip",
        "Chest/hr",
        "Base chest",
        "Treasure/hr",
        "Artifact/hr",
        "Best route",
    ]);
    for route in &plan.routes {
        let best = best_island(state, route);
        let captain = route
            .captain
            .map(|captain| captain_short(state, captain))
            .unwrap_or_else(|| "—".to_string());
        let details = route
            .captain
            .map(|captain| captain_details(state, captain))
            .unwrap_or_else(|| "No captain".to_string());
        table = table.row([
            div()
                .child(format!("Boat {}", route.boat + 1))
                .into_any_element(),
            div()
                .id(("fleet-captain", route.boat))
                .tooltip(Tooltip::text(details))
                .child(captain)
                .into_any_element(),
            div()
                .child(format!(
                    "Island {} · {}",
                    route.island + 1,
                    format::rate(route.timing.distance)
                ))
                .into_any_element(),
            div()
                .child(trip(route.timing.trip_seconds))
                .into_any_element(),
            div()
                .child(format!("{:.2}", route.generated_chests_per_hour))
                .into_any_element(),
            div()
                .child(format::rate(route.loot.value))
                .into_any_element(),
            div()
                .child(format::rate(route.treasure_per_hour))
                .into_any_element(),
            div()
                .child(artifacts(route.artifact_finds_per_hour))
                .into_any_element(),
            div()
                .child(match best {
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
                })
                .into_any_element(),
        ]);
    }

    let table = ScrollArea::new("fleet-table-scroll")
        .vertical()
        .h(table_height)
        .child(div().min_w_0().child(table));
    Card::new()
        .header(
            CardHeader::new(format!("{} Fleet Assignment", plan.mode.label())).description(
                format!(
                    "{} pairings · trip rate, boat value, natural tier odds, payouts and artifact odds are included.",
                    plan.routes.len()
                ),
            ),
        )
        .child(
            div()
                .id("fleet-table")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(table),
        )
        .child(
            div()
                .px_3()
                .pb_2()
                .text_size(gpui::px(11.0))
                .text_color(cx.theme().colors.fg_muted)
                .child(
                    "Current save keeps the stored captain-to-boat pairings. Optimized assigns each captain once to maximise the selected goal. Account-wide loot, speed and artifact multipliers outside the save model are excluded, so absolute rates are a lower bound.",
                ),
        )
        .into_any_element()
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
