//! The Ships tab: loot and speed breakpoints, their costs and ETAs.
//!
//! Upstream targets the island that pays each upgrade's currency, prices every
//! level up to the breakpoint and reads what the route gains once the boat is
//! there. The plan for that island always optimises direct loot value, as the
//! upstream levels tab does.

use super::{gain, hours_or_ready, text_kpi};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, SharedString, Styled, div};
use idleon_core::sailing::{
    Currency, FleetPlan, Goal, UpgradeBreakpoint, UpgradeStat, currency, upgrade_breakpoint,
    upgrade_cost_type, upgrade_eta,
};
use std::collections::BTreeMap;

/// One boat/stat row: its breakpoint and the island its currency is paid on.
struct Row {
    breakpoint: UpgradeBreakpoint,
    island: usize,
}

/// The ships tab: the breakpoint KPI row and the table.
pub fn ships_tab(state: &AppState, table_height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    // One plan per currency island, exactly as `activeFleet(targetIsland)`
    // resolves it upstream: the currency decides the route, not the selector.
    let mut plans: BTreeMap<usize, FleetPlan> = BTreeMap::new();
    for (boat, stat) in stats(state) {
        let island = currency(upgrade_cost_type(boat, stat)).island.unwrap_or(0);
        plans
            .entry(island)
            .or_insert_with(|| state.fleet_plan(island, Goal::Treasure));
    }
    let rows: Vec<Row> = stats(state)
        .into_iter()
        .map(|(boat, stat)| {
            let island = currency(upgrade_cost_type(boat, stat)).island.unwrap_or(0);
            Row {
                breakpoint: upgrade_breakpoint(
                    &state.fleet,
                    boat,
                    stat,
                    &plans[&island].routes,
                    &state.sailing,
                ),
                island,
            }
        })
        .collect();
    div()
        .id("ships-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(kpis(state, &rows, cx))
        .child(table(state, &rows, &plans, table_height, cx))
        .into_any_element()
}

/// Every boat's loot and speed upgrade, in boat order.
fn stats(state: &AppState) -> Vec<(usize, UpgradeStat)> {
    let mut rows = Vec::with_capacity(state.fleet.boats.len() * UpgradeStat::ALL.len());
    for boat in 0..state.fleet.boats.len() {
        for stat in UpgradeStat::ALL {
            rows.push((boat, stat));
        }
    }
    rows
}

fn kpis(state: &AppState, rows: &[Row], cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let ready = rows
        .iter()
        .filter(|row| row.breakpoint.remaining <= 0.0)
        .count();
    let fastest = rows
        .iter()
        .map(|row| &row.breakpoint)
        .min_by(|left, right| left.hours.total_cmp(&right.hours));
    let (fastest_value, fastest_caption) = match fastest {
        Some(fastest) => (
            hours_or_ready(fastest.hours),
            format!(
                "Boat {} {} · {}",
                fastest.boat + 1,
                fastest.stat.label(),
                fastest.currency.name()
            ),
        ),
        None => ("—".into(), "no boats".into()),
    };
    let entity = cx.entity();
    let sorting = state.sailing_state.sort_by_eta;
    let sort_checkbox = Checkbox::new("ships-sort-eta", sorting)
        .label(SharedString::from("Sort boats by lowest breakpoint ETA"))
        .on_change(move |on, _window, cx| {
            entity.update(cx, |state, cx| state.set_sailing_sort_by_eta(on, cx));
        });
    div()
        .id("ships-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{} breakpoints · {ready} ready · fastest {fastest_value}",
            rows.len()
        ))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(text_kpi(
                    "ships-ready",
                    "Breakpoints ready",
                    &ready.to_string(),
                    &format!("{} loot and speed breakpoints checked", rows.len()),
                    muted,
                ))
                .child(text_kpi(
                    "ships-fastest",
                    "Fastest next breakpoint",
                    &fastest_value,
                    &fastest_caption,
                    muted,
                ))
                .child(text_kpi(
                    "ships-assignment",
                    "ETA assignment",
                    state.sailing_state.mode.label(),
                    "each fleet targets the required resource island",
                    muted,
                )),
        )
        .child(sort_checkbox)
        .into_any_element()
}

fn table(
    state: &AppState,
    rows: &[Row],
    plans: &BTreeMap<usize, FleetPlan>,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut ordered: Vec<&Row> = rows.iter().collect();
    if state.sailing_state.sort_by_eta {
        // A paid breakpoint is ready now, so it sorts before every ETA.
        ordered.sort_by(|left, right| {
            let key = |row: &Row| {
                if row.breakpoint.remaining <= 0.0 {
                    0.0
                } else {
                    row.breakpoint.hours
                }
            };
            key(left)
                .total_cmp(&key(right))
                .then_with(|| left.breakpoint.boat.cmp(&right.breakpoint.boat))
                .then_with(|| {
                    (left.breakpoint.stat as usize).cmp(&(right.breakpoint.stat as usize))
                })
        });
    }

    let mut table = Table::new([
        "Boat",
        "Upgrade",
        "Next level",
        "Breakpoint",
        "Levels away",
        "Base gain",
        "Route gain",
        "Resource",
        "Target route",
        "Breakpoint cost",
        "Needed",
        "Breakpoint ETA",
    ]);
    for row in ordered {
        let breakpoint = &row.breakpoint;
        let stat = breakpoint.stat;
        let routes = &plans[&row.island].routes;
        let route_gain = if stat.is_loot() {
            breakpoint.treasure_gain_pct
        } else {
            breakpoint.chest_gain_pct
        };
        let route_text = if breakpoint.speed_capped {
            "capped · 0%".to_string()
        } else {
            gain(route_gain)
        };
        let breakpoint_tooltip = match breakpoint.davey_level {
            Some(level) => format!(
                "{} needs level {level} for the level-400 Davey gate",
                stat.label()
            ),
            None => format!(
                "{} level divisible by {}",
                stat.label(),
                breakpoint.interval
            ),
        };
        let next = upgrade_eta(&state.fleet, breakpoint.boat, stat, routes);
        table = table.row([
            div()
                .child(format!("Boat {}", breakpoint.boat + 1))
                .into_any_element(),
            div().child(stat.label()).into_any_element(),
            div()
                .id(format!("ships-next-{}-{}", breakpoint.boat, stat.is_loot()))
                .tooltip(Tooltip::text(format!(
                    "One level: {} {}\nNext level ETA: {}",
                    format::rate(next.cost),
                    next.currency.name(),
                    hours_or_ready(next.hours)
                )))
                .child(format!(
                    "{} → {}",
                    format::rate(breakpoint.level as f64),
                    format::rate(breakpoint.level as f64 + 1.0)
                ))
                .into_any_element(),
            div()
                .id(format!(
                    "ships-target-{}-{}",
                    breakpoint.boat,
                    stat.is_loot()
                ))
                .tooltip(Tooltip::text(breakpoint_tooltip))
                .child(format!(
                    "{} / {}",
                    breakpoint.level, breakpoint.target_level
                ))
                .into_any_element(),
            div()
                .child(breakpoint.levels.to_string())
                .into_any_element(),
            div()
                .child(format!("+{:.2}%", breakpoint.base_gain_pct))
                .into_any_element(),
            div()
                .id(format!(
                    "ships-route-gain-{}-{}",
                    breakpoint.boat,
                    stat.is_loot()
                ))
                .child(route_text)
                .into_any_element(),
            div().child(breakpoint.currency.name()).into_any_element(),
            div()
                .child(island_label(breakpoint.currency))
                .into_any_element(),
            div()
                .child(format::rate(breakpoint.total_cost))
                .into_any_element(),
            div()
                .child(format::rate(breakpoint.remaining))
                .into_any_element(),
            div()
                .child(if breakpoint.remaining <= 0.0 {
                    "Ready".to_string()
                } else {
                    hours_or_ready(breakpoint.hours)
                })
                .into_any_element(),
        ]);
    }

    let table = ScrollArea::new("ships-table-scroll")
        .vertical()
        .h(table_height)
        .child(div().min_w_0().child(table));
    Card::new()
        .header(
            CardHeader::new("Ship Upgrade Breakpoints").description(
                "The next loot level divisible by 8 or speed level divisible by 7, where the ship formula's squared term rises.",
            ),
        )
        .child(
            div()
                .id("ships-table")
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
                    "Costs and ETAs cover every level through the breakpoint and assume the whole fleet sails the currency's island. A capped speed breakpoint still raises raw speed but generates no more chests. The event-shop discount is not modelled, so prices stay an upper bound.",
                ),
        )
        .into_any_element()
}

/// The island the currency's upgrades are paid on.
fn island_label(currency: Currency) -> String {
    match currency.island {
        None => "Island 1".into(),
        Some(island) => format!("Island {}", island + 1),
    }
}
