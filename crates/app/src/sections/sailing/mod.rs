//! Sailing fleet assignment: current versus optimized pairings on one route.

use crate::state::AppState;
use crate::ui::format;
use crate::ui::layout;
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Tooltip};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::sailing::{
    AssignmentMode, FleetPlan, Goal, RouteMetrics, captain_bonus_name, island_artifact_count,
    island_distance, route_metrics,
};

/// The section's own state, kept across dashboard visits.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub mode: AssignmentMode,
    pub island: usize,
    pub goal: Goal,
}

impl Default for State {
    fn default() -> Self {
        Self {
            mode: AssignmentMode::Optimized,
            island: 0,
            goal: Goal::Treasure,
        }
    }
}

/// The sailing section: controls, the KPI row and the assignment table.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let comparison = state.fleet_comparison();
    let plan = match state.sailing_state.mode {
        AssignmentMode::Optimized => &comparison.optimized,
        AssignmentMode::Saved => &comparison.saved,
    };
    let gain = comparison.gain_pct();
    let empty = state.fleet.boats.is_empty();

    div()
        .id("sailing-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header(state, cx))
        .child(kpis(plan, gain, empty, cx))
        .child(controls(state, cx))
        .child(
            div()
                .id("fleet-body")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(table(state, plan, table_height, empty, cx)),
        )
        .into_any_element()
}

fn header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let island = state.sailing_state.island;
    let summary = format!(
        "Island {} · {} boats · {} captains",
        island + 1,
        state.fleet.boats.len(),
        state.fleet.captains.len()
    );
    div()
        .id("sailing-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(Badge::new("Fleet Assignment").tone(Tone::Accent))
        .child(
            Caption::new(
                "Ship levels, chest odds, artifacts and the pile simulation follow in SL-02/SL-03.",
            )
            .text_color(muted),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("sailing-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .child(Caption::new(summary).text_color(muted)),
        )
        .into_any_element()
}

fn kpis(plan: &FleetPlan, gain: f64, empty: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mode = plan.mode.label();
    let pairings = plan.routes.len();
    let gain_label = if empty || plan.mode == AssignmentMode::Saved && pairings == 0 {
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
                .child(div().text_size(px(20.0)).child(value.to_string()))
                .child(Caption::new(caption.to_string()).text_color(muted)),
        )
        .into_any_element()
}

fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mode = state.sailing_state.mode;
    let optimized = entity.clone();
    let saved = entity.clone();
    let mode_control = div()
        .id("fleet-mode-control")
        .flex()
        .items_center()
        .gap_1()
        .child(
            Button::new("fleet-mode-optimized", "Optimized")
                .variant(if mode == AssignmentMode::Optimized {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    optimized.update(cx, |state, cx| {
                        state.set_fleet_mode(AssignmentMode::Optimized, cx);
                    });
                }),
        )
        .child(
            Button::new("fleet-mode-saved", "Current save")
                .variant(if mode == AssignmentMode::Saved {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    saved.update(cx, |state, cx| {
                        state.set_fleet_mode(AssignmentMode::Saved, cx);
                    });
                }),
        );

    let island_entity = entity.clone();
    let available = state.fleet.available_islands().max(1);
    let islands: Vec<Choice> = (0..available)
        .map(|island| {
            Choice::new(
                island.to_string(),
                format!("Island {} · {} dist", island + 1, island_distance(island)),
            )
            .note(format!("{} artifacts", island_artifact_count(island)))
        })
        .collect();
    let island_select = Select::new("fleet-island", islands)
        .label("Fleet destination")
        .selected(state.sailing_state.island.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(island) = value.parse::<usize>() {
                island_entity.update(cx, |state, cx| state.set_fleet_island(island, cx));
            }
        });

    let goal_entity = entity.clone();
    let goal_control = SegmentedControl::new("fleet-goal", state.sailing_state.goal.id())
        .size(ControlSize::Sm)
        .segment("treasure", "Loot value", None)
        .segment("artifact", "Artifacts", None)
        .segment("chests", "Chest rate", None)
        .on_change(move |value, _window, cx| {
            if let Some(goal) = Goal::from_id(value.as_ref()) {
                goal_entity.update(cx, |state, cx| state.set_fleet_goal(goal, cx));
            }
        });

    let tier_entity = entity.clone();
    let tiers: Vec<Choice> = (0..5)
        .map(|tier| Choice::new(tier.to_string(), format!("Tier {tier}+")))
        .collect();
    let tier_select = Select::new("fleet-min-tier", tiers)
        .label("Minimum chest")
        .selected(state.sailing.minimum_tier.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(tier) = value.parse::<u32>() {
                tier_entity.update(cx, |state, cx| state.set_fleet_minimum_tier(tier, cx));
            }
        });

    Card::new()
        .header(CardHeader::new("Fleet controls").description(
            "The selected island applies to every pairing. Current save keeps each boat's captain, not its stored destination.",
        ))
        .child(
            div()
                .id("fleet-controls-row")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .p_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Fleet assignment"))
                        .child(mode_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(px(220.0))
                        .child(Caption::new("Fleet destination"))
                        .child(island_select),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Optimize for"))
                        .child(goal_control),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(px(140.0))
                        .child(Caption::new("Minimum chest"))
                        .child(tier_select),
                ),
        )
        .into_any_element()
}

fn table(
    state: &AppState,
    plan: &FleetPlan,
    table_height: gpui::Pixels,
    empty: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if empty {
        let entity = cx.entity();
        return Card::new()
            .header(CardHeader::new("Fleet assignment"))
            .child(
                EmptyState::new("fleet-empty", IconName::Map, "No sailing fleet loaded")
                    .body("Import a save with boats and captains to compare the current pairings with the optimizer.")
                    .action(
                        Button::new("fleet-import", "Import a save")
                            .icon(IconName::Upload)
                            .on_click(move |_event, window, cx| {
                                entity.update(cx, |state, cx| state.open_import(window, cx));
                            }),
                    ),
            )
            .into_any_element();
    }

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
        .header(CardHeader::new(format!(
            "{} Fleet Assignment",
            plan.mode.label()
        ))
        .description(format!(
            "{} pairings · trip rate, boat value, natural tier odds, payouts and artifact odds are included.",
            plan.routes.len()
        )))
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
                .text_size(px(11.0))
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

fn captain_short(state: &AppState, captain: usize) -> String {
    let Some(entry) = state.fleet.captains.get(captain) else {
        return "—".into();
    };
    format!("{} · Lv{}", captain_code(captain), entry.level)
}

fn captain_details(state: &AppState, captain: usize) -> String {
    let Some(entry) = state.fleet.captains.get(captain) else {
        return "No captain".into();
    };
    let bonuses: Vec<String> = entry
        .bonus_types
        .iter()
        .zip(entry.rolls)
        .filter(|(kind, _)| **kind >= 0)
        .map(|(kind, roll)| format!("{} +{}%", captain_bonus_name(*kind), roll))
        .collect();
    let code = captain_code(captain);
    if bonuses.is_empty() {
        format!("Captain {code} · Lv{}", entry.level)
    } else {
        format!(
            "Captain {code} · Lv{} · {}",
            entry.level,
            bonuses.join(" / ")
        )
    }
}

/// A, B, … Z, AA, AB, … as upstream labels captains.
fn captain_code(index: usize) -> String {
    let mut value = index + 1;
    let mut code = String::new();
    while value > 0 {
        value -= 1;
        code.insert(0, (b'A' + (value % 26) as u8) as char);
        value /= 26;
    }
    code
}

fn artifacts(value: f64) -> String {
    if value <= 0.0 {
        "—".into()
    } else if value >= 0.01 {
        format!("{value:.2}")
    } else if value >= 1e-4 {
        format!("{value:.4}")
    } else {
        format!("1 in {}", format::rate(1.0 / value))
    }
}

fn trip(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "—".into();
    }
    if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.2}h", seconds / 3600.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captain_codes_match_the_upstream_sequence() {
        assert_eq!(captain_code(0), "A");
        assert_eq!(captain_code(25), "Z");
        assert_eq!(captain_code(26), "AA");
        assert_eq!(captain_code(27), "AB");
    }

    #[test]
    fn trip_times_read_in_seconds_minutes_and_hours() {
        assert_eq!(trip(45.0), "45s");
        assert_eq!(trip(90.0), "1.5m");
        assert_eq!(trip(7200.0), "2.00h");
        assert_eq!(trip(f64::INFINITY), "—");
    }
}
