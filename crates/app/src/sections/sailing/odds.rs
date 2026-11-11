//! The Chest Odds tab: fleet chest outcomes, per-boat tier chances and the
//! near-full loot pile transitions.
//!
//! Every chest picks its tier when it is generated, so the fleet row weights each
//! captain's odds by that boat's exact-tier chest rate. The payout columns then
//! turn one tier into stored value, guaranteed payouts and the expected amount of
//! each treasure currency. The near-full block repeats one ordinary promotion at
//! a time: what Unending Loot Search rolls and what the promoted chest is worth.
//!
//! The collection-window recommendation needs the pile's capacity model, so it
//! arrives with the pile simulation; nothing on this tab depends on how full the
//! pile is.

use super::{captain_short, chance, chest_odds, percent, text_kpi};
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
    ChestPayoutBreakdown, FleetChestOutcomes, RouteMetrics, aggregate_fleet_tiers, currency,
    fleet_chest_payout_breakdown, near_full_pile,
};

/// The odds tab: the KPI row, the fleet table, the per-boat chances and the pile.
pub fn odds_tab(state: &AppState, table_height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let outcomes = aggregate_fleet_tiers(&plan.routes);
    let breakdowns: Vec<ChestPayoutBreakdown> = outcomes
        .tiers
        .iter()
        .map(|tier| fleet_chest_payout_breakdown(&plan.routes, tier.tier, &state.sailing))
        .collect();
    div()
        .id("odds-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(kpis(state, &plan.routes, &outcomes, &breakdowns, cx))
        .child(fleet_table(state, &outcomes, &breakdowns, table_height, cx))
        .child(boat_table(state, &plan.routes, table_height, cx))
        .child(pile(state, cx))
        .into_any_element()
}

fn kpis(
    state: &AppState,
    routes: &[RouteMetrics],
    outcomes: &FleetChestOutcomes,
    breakdowns: &[ChestPayoutBreakdown],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let island = state.sailing_state.island;
    let primary = currency(1 + 2 * island);
    let rare = currency(2 + 2 * island);
    let shares = routes
        .first()
        .map_or_else(Default::default, |route| route.treasure_shares);
    let mut weighted = 0.0;
    let mut minimum = f64::INFINITY;
    let mut maximum: f64 = 0.0;
    let mut generated = 0.0;
    for route in routes {
        let scale = route.rarity.rarity_scale;
        weighted += route.generated_chests_per_hour * scale;
        generated += route.generated_chests_per_hour;
        minimum = minimum.min(scale);
        maximum = maximum.max(scale);
    }
    let weighted = if generated > 0.0 {
        weighted / generated
    } else {
        0.0
    };
    let minimum = if minimum.is_finite() { minimum } else { 0.0 };
    let payouts: Vec<String> = breakdowns
        .iter()
        .map(|breakdown| breakdown.payouts.to_string())
        .collect();
    div()
        .id("odds-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{:.2} chests/hr · rare-chest scale {weighted:.3}x · tier {}+ kept",
            outcomes.generated_per_hour, state.sailing.minimum_tier
        ))
        .flex()
        .flex_wrap()
        .items_stretch()
        .gap_2()
        .child(text_kpi(
            "odds-kpi-tier",
            "Chest tier",
            "Random once",
            "selected when each chest is generated",
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-scale",
            "Rare-chest scale",
            &format!("{weighted:.3}x"),
            &format!("{minimum:.3}x to {maximum:.3}x across captains"),
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-payouts",
            "Guaranteed payouts",
            &payouts.join(" / "),
            "Tier 0 through Tier 5",
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-payout",
            "Each payout",
            "Full chest value",
            "the currency is selected independently",
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-currencies",
            "Currency chances",
            &format!(
                "{} / {} / {}",
                percent(shares.gold),
                percent(shares.primary),
                percent(shares.rare)
            ),
            "gold / primary / rare per payout",
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-filter",
            "Current filter",
            &format!("Tier {}+", state.sailing.minimum_tier),
            "discarded chests are not rerolled",
            muted,
        ))
        .child(text_kpi(
            "odds-kpi-names",
            "Island currencies",
            &format!("{} · {}", primary.name(), rare.name()),
            "paid by the selected island",
            muted,
        ))
        .into_any_element()
}

fn fleet_table(
    state: &AppState,
    outcomes: &FleetChestOutcomes,
    breakdowns: &[ChestPayoutBreakdown],
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let island = state.sailing_state.island;
    let primary = currency(1 + 2 * island);
    let rare = currency(2 + 2 * island);
    let mut table = Table::new([
        "Tier".to_string(),
        "Exact chance".to_string(),
        "Tier+ chance".to_string(),
        "Chests/hr".to_string(),
        "Value / payout ×".to_string(),
        "Guaranteed payouts".to_string(),
        "Average value / payout".to_string(),
        format!("{} / opened chest", currency(0).name()),
        format!("{} / opened chest", primary.name()),
        format!("{} / opened chest", rare.name()),
        "Artifact chance ×".to_string(),
        "Filter".to_string(),
    ]);
    for (tier, breakdown) in outcomes.tiers.iter().zip(breakdowns) {
        let kept = tier.tier >= state.sailing.minimum_tier as usize;
        let currencies = breakdown.currencies;
        table = table.row([
            div()
                .id(("odds-tier", tier.tier))
                .child(format!("Tier {}", tier.tier))
                .into_any_element(),
            chance_cell(tier.probability, tier.tier, "exact"),
            chance_cell(tier.cumulative, tier.tier, "cumulative"),
            div()
                .child(format!("{:.2}", tier.expected_per_hour))
                .into_any_element(),
            div()
                .child(format!("{:.3}x", tier.loot_multiplier))
                .into_any_element(),
            div()
                .child(breakdown.payouts.to_string())
                .into_any_element(),
            div()
                .id(("odds-average", tier.tier))
                .tooltip(Tooltip::text(format!(
                    "{} opened per chest across all guaranteed payouts",
                    format::value(breakdown.total_value_per_chest)
                )))
                .child(format::value(breakdown.average_stored_value))
                .into_any_element(),
            currency_cell(("odds-gold", tier.tier), currencies.gold),
            currency_cell(("odds-primary", tier.tier), currencies.primary),
            currency_cell(("odds-rare", tier.tier), currencies.rare),
            div()
                .child(format!("{:.3}", tier.artifact_multiplier))
                .into_any_element(),
            filter_badge(kept, tier.tier),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Fleet chest outcomes").description(format!(
                "{:.2} generated chests/hr · each tier weighted by the boats that roll it",
                outcomes.generated_per_hour
            )),
        )
        .child(
            div()
                .id("odds-fleet-table")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(
                    ScrollArea::new("odds-fleet-scroll")
                        .vertical()
                        .h(table_height)
                        .child(div().min_w_0().child(table)),
                ),
        )
        .child(callout(
            "odds-fleet-callout",
            cx,
            "The six exact chances above sum to one: every generated chest lands in exactly one tier. \
             Stored value per payout = boat value × the tier multiplier, and a tier also fixes how \
             many payouts it rolls; every payout picks its currency and pays the full stored value in \
             it. Artifact odds are one separate check per chest.",
        ))
        .into_any_element()
}

fn boat_table(
    state: &AppState,
    routes: &[RouteMetrics],
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut table = Table::new([
        "Boat",
        "Captain",
        "Chests/hr",
        "Rare chest bonus",
        "Tier scale",
        "Tier 0",
        "Tier 1",
        "Tier 2",
        "Tier 3",
        "Tier 4",
        "Miracle (Tier 5)",
    ]);
    for route in routes {
        let captain = route
            .captain
            .map(|captain| captain_short(state, captain))
            .unwrap_or_else(|| "—".into());
        let mut cells: Vec<AnyElement> = vec![
            div()
                .child(format!("Boat {}", route.boat + 1))
                .into_any_element(),
            div().child(captain).into_any_element(),
            div()
                .child(format!("{:.2}", route.generated_chests_per_hour))
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(route.rarity.captain)))
                .into_any_element(),
            div()
                .child(format!("{:.3}x", route.rarity.rarity_scale))
                .into_any_element(),
        ];
        for tier in &route.rarity.tiers {
            cells.push(chance_cell(tier.probability, tier.tier, "boat"));
        }
        table = table.row(cells);
    }
    Card::new()
        .header(
            CardHeader::new("Per-boat tier chances").description(
                "Exact odds per generated chest, never tier-or-higher; the fleet table above is their arrival-weighted average",
            ),
        )
        .child(
            div()
                .id("odds-boat-table")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(
                    ScrollArea::new("odds-boat-scroll")
                        .vertical()
                        .h(table_height)
                        .child(div().min_w_0().child(table)),
                ),
        )
        .child(callout(
            "odds-boat-callout",
            cx,
            "Captain rare-chest chance shifts generated chests into higher tiers. The ship changes \
             the chest rate and the stored value, not the tier odds.",
        ))
        .into_any_element()
}

fn pile(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let rows = near_full_pile(&state.sailing);
    let mut table = Table::new([
        "Transition",
        "Promotion chance",
        "Value / payout",
        "Value on success",
        "Artifact multiplier",
    ]);
    for row in &rows {
        table = table.row([
            div()
                .id(("pile-transition", row.tier))
                .child(format!("Tier {} → Tier {}", row.tier, row.next_tier))
                .into_any_element(),
            div().child(chance(row.chance)).into_any_element(),
            div()
                .child(format!(
                    "{:.3}x → {:.3}x",
                    row.loot_multiplier, row.next_loot_multiplier
                ))
                .into_any_element(),
            div()
                .child(format!("{:.3}x", row.value_ratio))
                .into_any_element(),
            div()
                .child(format!("{:.1}x", row.artifact_multiplier))
                .into_any_element(),
        ]);
    }
    let talent = state.sailing.unending_loot_search;
    Card::new()
        .header(
            CardHeader::new("Near-full loot pile").description(format!(
                "Unending Loot Search {talent}% · Tier 4 cannot promote to Miracle"
            )),
        )
        .child(div().id("odds-pile-table").p_3().child(table))
        .child(callout(
            "odds-pile-callout",
            cx,
            "With the talent active and exactly one empty slot, each incoming ordinary chest scans the \
             banked chests from the first one onward. The first same-island, same-tier, \
             sufficient-value match rolls its promotion chance and stops the scan either way. \
             Otherwise the first same-island chest with both a strictly higher tier and a strictly \
             greater stored value replaces the banked chest. A stored chest has no captain identity, \
             so this stateful processing is not part of direct treasure/hr; grouping boats on one \
             island raises the number of ordinary matches.",
        ))
        .into_any_element()
}

fn chance_cell(probability: f64, tier: usize, kind: &'static str) -> AnyElement {
    let text = chest_odds(probability);
    let tooltip = format!("Tier {tier} {kind} probability: {}", chance(probability));
    let id = if kind == "boat" {
        "odds-boat"
    } else if kind == "cumulative" {
        "odds-cumulative"
    } else {
        "odds-exact"
    };
    div()
        .id((id, tier))
        .tooltip(Tooltip::text(tooltip))
        .child(text)
        .into_any_element()
}

fn currency_cell(
    id: (&'static str, usize),
    payout: idleon_core::sailing::CurrencyPayout,
) -> AnyElement {
    div()
        .id(id)
        .child(format::value(payout.expected_value_per_chest))
        .tooltip(Tooltip::text(format!(
            "{} value per opened chest · {:.2} payouts avg · {} per payout · {}/hr",
            format::value(payout.expected_value_per_chest),
            payout.expected_payouts_per_chest,
            percent(payout.chance_per_payout),
            format::rate(payout.expected_value_per_hour),
        )))
        .into_any_element()
}

fn filter_badge(kept: bool, tier: usize) -> AnyElement {
    let badge = if kept {
        Badge::new("Kept").tone(Tone::Success)
    } else {
        Badge::new("Discarded")
    };
    div()
        .id((if kept { "odds-kept" } else { "odds-discarded" }, tier))
        .child(badge)
        .into_any_element()
}

fn callout(id: &'static str, cx: &mut Context<AppState>, text: &str) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id(id)
        .px_3()
        .pb_2()
        .child(Caption::new(text.to_string()).text_color(muted))
        .into_any_element()
}
