//! The Ships tab: loot and speed breakpoints, their costs and ETAs.
//!
//! Each upgrade targets the island that pays its currency, prices every level
//! up to the breakpoint and reads what the route gains once the boat is there.
//! The plan for that island always optimises direct loot value.

use super::controls::cached;
use super::table::{self, Cell};
use super::{gain, hours_or_ready};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, SharedString, Styled, div,
};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_sailing::{
    Currency, FleetPlan, Goal, UpgradeBreakpoint, UpgradeStat, currency, upgrade_breakpoint,
    upgrade_cost_type, upgrade_eta,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic};
use std::collections::BTreeMap;

/// The figures the tab shows, read once per key and sort order.
pub(crate) struct View {
    rows: usize,
    ready: usize,
    fastest: SharedString,
    fastest_caption: SharedString,
}

/// One boat/stat breakpoint, formatted.
pub(crate) struct Row {
    boat: usize,
    loot: bool,
    stat: &'static str,
    next: SharedString,
    next_tip: SharedString,
    target: SharedString,
    target_tip: SharedString,
    levels: SharedString,
    base_gain: SharedString,
    route_gain: SharedString,
    resource: SharedString,
    island: SharedString,
    cost: SharedString,
    needed: SharedString,
    eta: SharedString,
}

pub(crate) fn columns() -> Vec<Cell<Row>> {
    vec![
        Cell::text("Boat", 70.0, |row| format!("Boat {}", row.boat + 1).into()),
        Cell::text("Upgrade", 80.0, |row| row.stat.into()),
        Cell::element(
            "Next level",
            110.0,
            |row, _| {
                table::tip(
                    SharedString::from(format!("ships-next-{}-{}", row.boat, row.loot)),
                    row.next_tip.clone(),
                    row.next.clone(),
                )
            },
            |row| row.next.clone(),
        ),
        Cell::element(
            "Breakpoint",
            100.0,
            |row, _| {
                table::tip(
                    SharedString::from(format!("ships-target-{}-{}", row.boat, row.loot)),
                    row.target_tip.clone(),
                    row.target.clone(),
                )
            },
            |row| row.target.clone(),
        ),
        Cell::number("Levels away", 100.0, |row| row.levels.clone()),
        Cell::number("Base gain", 90.0, |row| row.base_gain.clone()),
        Cell::element(
            "Route gain",
            100.0,
            |row, cx| {
                div()
                    .id(SharedString::from(format!(
                        "ships-route-gain-{}-{}",
                        row.boat, row.loot
                    )))
                    .child(table::mono(row.route_gain.clone(), cx))
                    .into_any_element()
            },
            |row| row.route_gain.clone(),
        ),
        Cell::text("Resource", 130.0, |row| row.resource.clone()),
        Cell::text("Target route", 100.0, |row| row.island.clone()),
        Cell::number("Breakpoint cost", 120.0, |row| row.cost.clone()),
        Cell::number("Needed", 100.0, |row| row.needed.clone()),
        Cell::number("Breakpoint ETA", 120.0, |row| row.eta.clone()),
    ]
}

/// The ships tab: the breakpoint KPI row and the table.
pub(crate) fn ships_tab(
    state: &mut AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = (state.sailing_key(), state.sailing_state.sort_by_eta);
    let view = cached(state, |controls| &mut controls.ships, key, cx, build);
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let sort_checkbox = Checkbox::new("ships-sort-eta")
        .label("Sort boats by lowest breakpoint ETA")
        .checked(state.sailing_state.sort_by_eta)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| state.set_sailing_sort_by_eta(on, cx));
        });
    let kpis = div()
        .id("ships-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{} breakpoints · {} ready · fastest {}",
            view.rows, view.ready, view.fastest
        ))
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new("ships-ready", "Breakpoints ready", view.ready.to_string())
                        .caption(format!("{} loot and speed breakpoints checked", view.rows)),
                )
                .child(
                    Statistic::new(
                        "ships-fastest",
                        "Fastest next breakpoint",
                        view.fastest.clone(),
                    )
                    .caption(view.fastest_caption.clone()),
                )
                .child(
                    Statistic::new(
                        "ships-assignment",
                        "ETA assignment",
                        state.sailing_state.mode.label(),
                    )
                    .caption("each fleet targets the required resource island"),
                ),
        );
    let table = Card::new()
        .header(CardHeader::new("Ship Upgrade Breakpoints").description(
            "The next loot level divisible by 8 or speed level divisible by 7, where the ship formula's squared term rises.",
        ))
        .child(
            div()
                .id("ships-table")
                .h(table::data_height(view.rows, 33.0, table_height))
                .child(DataTable::new(&state.sailing_controls.ships_table)),
        )
        .child(
            Caption::new(
                "Costs and ETAs cover every level through the breakpoint and assume the whole fleet sails the currency's island. A capped speed breakpoint still raises raw speed but generates no more chests. The event-shop discount is not modelled, so prices stay an upper bound.",
            )
            .text_color(muted),
        );
    v_flex()
        .id("ships-body")
        .gap_3()
        .child(kpis)
        .child(sort_checkbox)
        .child(table)
        .into_any_element()
}

/// One breakpoint and the plan of the island its currency is paid on.
struct Breakpoint {
    breakpoint: UpgradeBreakpoint,
    island: usize,
}

/// Price every breakpoint once and fill the table in the selected order.
fn build(state: &AppState, cx: &mut App) -> View {
    // One plan per currency island: the currency decides the route, not the
    // selector.
    let mut plans: BTreeMap<usize, FleetPlan> = BTreeMap::new();
    let stats: Vec<(usize, UpgradeStat)> = (0..state.fleet.boats.len())
        .flat_map(|boat| UpgradeStat::ALL.map(|stat| (boat, stat)))
        .collect();
    for (boat, stat) in &stats {
        let island = currency(upgrade_cost_type(*boat, *stat))
            .island
            .unwrap_or(0);
        plans
            .entry(island)
            .or_insert_with(|| state.fleet_plan(island, Goal::Treasure));
    }
    let mut rows: Vec<Breakpoint> = stats
        .into_iter()
        .map(|(boat, stat)| {
            let island = currency(upgrade_cost_type(boat, stat)).island.unwrap_or(0);
            Breakpoint {
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

    let ready = rows
        .iter()
        .filter(|row| row.breakpoint.remaining <= 0.0)
        .count();
    let fastest = rows
        .iter()
        .map(|row| &row.breakpoint)
        .min_by(|left, right| left.hours.total_cmp(&right.hours));
    let (fastest, fastest_caption) = match fastest {
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

    if state.sailing_state.sort_by_eta {
        // A paid breakpoint is ready now, so it sorts before every ETA.
        let key = |row: &Breakpoint| {
            if row.breakpoint.remaining <= 0.0 {
                0.0
            } else {
                row.breakpoint.hours
            }
        };
        rows.sort_by(|left, right| {
            key(left)
                .total_cmp(&key(right))
                .then_with(|| left.breakpoint.boat.cmp(&right.breakpoint.boat))
                .then_with(|| {
                    (left.breakpoint.stat as usize).cmp(&(right.breakpoint.stat as usize))
                })
        });
    }
    let count = rows.len();
    let table_rows = rows
        .iter()
        .map(|row| table_row(state, &plans[&row.island], &row.breakpoint))
        .collect();
    table::set_rows(&state.sailing_controls.ships_table, table_rows, cx);
    View {
        rows: count,
        ready,
        fastest: fastest.into(),
        fastest_caption: fastest_caption.into(),
    }
}

fn table_row(state: &AppState, plan: &FleetPlan, breakpoint: &UpgradeBreakpoint) -> Row {
    let stat = breakpoint.stat;
    let route_gain = if stat.is_loot() {
        breakpoint.treasure_gain_pct
    } else {
        breakpoint.chest_gain_pct
    };
    let target_tip = match breakpoint.davey_level {
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
    let next = upgrade_eta(&state.fleet, breakpoint.boat, stat, &plan.routes);
    Row {
        boat: breakpoint.boat,
        loot: stat.is_loot(),
        stat: stat.label(),
        next: format!(
            "{} → {}",
            format::rate(breakpoint.level as f64),
            format::rate(breakpoint.level as f64 + 1.0)
        )
        .into(),
        next_tip: format!(
            "One level: {} {}\nNext level ETA: {}",
            format::rate(next.cost),
            next.currency.name(),
            hours_or_ready(next.hours)
        )
        .into(),
        target: format!("{} / {}", breakpoint.level, breakpoint.target_level).into(),
        target_tip: target_tip.into(),
        levels: breakpoint.levels.to_string().into(),
        base_gain: format!("+{:.2}%", breakpoint.base_gain_pct).into(),
        route_gain: if breakpoint.speed_capped {
            "capped · 0%".to_string()
        } else {
            gain(route_gain)
        }
        .into(),
        resource: breakpoint.currency.name().into(),
        island: island_label(breakpoint.currency).into(),
        cost: format::rate(breakpoint.total_cost).into(),
        needed: format::rate(breakpoint.remaining).into(),
        eta: if breakpoint.remaining <= 0.0 {
            "Ready".to_string()
        } else {
            hours_or_ready(breakpoint.hours)
        }
        .into(),
    }
}

/// The island the currency's upgrades are paid on.
fn island_label(currency: Currency) -> String {
    match currency.island {
        None => "Island 1".into(),
        Some(island) => format!("Island {}", island + 1),
    }
}
