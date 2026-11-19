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

use super::controls::cached;
use super::table::{self, Cell};
use super::{captain_short, chance, chest_odds, percent};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, ElementId, IntoElement, ParentElement, Pixels, SharedString, Styled,
    div,
};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, v_flex};
use idleon_sailing::{
    ChestPayoutBreakdown, CurrencyPayout, RouteMetrics, aggregate_fleet_tiers, currency,
    fleet_chest_payout_breakdown, near_full_pile,
};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, Statistic, TableColumn, Tone, fit_columns,
};

/// A value with the text its tooltip shows.
#[derive(Clone)]
struct Tipped {
    text: SharedString,
    tip: SharedString,
}

/// One fleet tier, formatted.
struct TierRow {
    tier: usize,
    exact: Tipped,
    cumulative: Tipped,
    per_hour: SharedString,
    multiplier: SharedString,
    payouts: SharedString,
    average: Tipped,
    currencies: [Tipped; 3],
    artifact: SharedString,
    kept: bool,
}

/// One near-full promotion, formatted.
struct PileRow {
    tier: usize,
    transition: SharedString,
    chance: SharedString,
    value: SharedString,
    success: SharedString,
    artifact: SharedString,
}

/// The figures the tab shows, read once per key.
pub(crate) struct View {
    summary: SharedString,
    kpis: [(&'static str, &'static str, SharedString, SharedString); 7],
    headers: Vec<TableColumn>,
    generated: SharedString,
    tiers: Vec<TierRow>,
    boats: usize,
    pile: Vec<PileRow>,
    talent: SharedString,
}

/// One boat's tier chances, formatted.
pub(crate) struct BoatRow {
    boat: usize,
    captain: SharedString,
    chests: SharedString,
    bonus: SharedString,
    scale: SharedString,
    tiers: Vec<Tipped>,
}

pub(crate) fn boat_columns() -> Vec<Cell<BoatRow>> {
    vec![
        Cell::text("Boat", 70.0, |row| format!("Boat {}", row.boat + 1).into()),
        Cell::text("Captain", 90.0, |row| row.captain.clone()),
        Cell::number("Chests/hr", 80.0, |row| row.chests.clone()),
        Cell::number("Rare chest bonus", 120.0, |row| row.bonus.clone()),
        Cell::number("Tier scale", 80.0, |row| row.scale.clone()),
        Cell::element("Tier 0", 90.0, boat_tier::<0>, |row| boat_text(row, 0)),
        Cell::element("Tier 1", 90.0, boat_tier::<1>, |row| boat_text(row, 1)),
        Cell::element("Tier 2", 90.0, boat_tier::<2>, |row| boat_text(row, 2)),
        Cell::element("Tier 3", 90.0, boat_tier::<3>, |row| boat_text(row, 3)),
        Cell::element("Tier 4", 90.0, boat_tier::<4>, |row| boat_text(row, 4)),
        Cell::element("Miracle (Tier 5)", 120.0, boat_tier::<5>, |row| {
            boat_text(row, 5)
        }),
    ]
}

fn boat_text(row: &BoatRow, tier: usize) -> SharedString {
    row.tiers
        .get(tier)
        .map_or_else(|| "—".into(), |cell| cell.text.clone())
}

fn boat_tier<const TIER: usize>(row: &BoatRow, _: &App) -> AnyElement {
    match row.tiers.get(TIER) {
        Some(cell) => table::tip(("odds-boat", TIER), cell.tip.clone(), cell.text.clone()),
        None => div().child("—").into_any_element(),
    }
}

/// The odds tab: the KPI row, the fleet table, the per-boat chances and the pile.
pub(crate) fn odds_tab(
    state: &mut AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = state.sailing_key();
    let view = cached(state, |controls| &mut controls.odds, key, cx, build);
    let muted = cx.theme().muted_foreground;
    let kpis = div()
        .id("odds-kpis")
        .role(gpui::Role::Label)
        .aria_label(view.summary.clone())
        .flex()
        .flex_col()
        .gap_3()
        .children(view.kpis.chunks(4).map(|figures| {
            KpiStrip::new().children(figures.iter().map(|(id, label, value, caption)| {
                Statistic::new(*id, *label, value.clone())
                    .caption(caption.clone())
                    .small()
            }))
        }));
    let fleet = Card::new()
        .header(CardHeader::new("Fleet chest outcomes").description(view.generated.clone()))
        .child(div().id("odds-fleet-table").child(table::table("odds-fleet-scroll", &view.headers, view.tiers.iter().map(tier_row))))
        .child(callout(
            "odds-fleet-callout",
            muted,
            "The six exact chances above sum to one: every generated chest lands in exactly one tier. \
             Stored value per payout = boat value × the tier multiplier, and a tier also fixes how \
             many payouts it rolls; every payout picks its currency and pays the full stored value in \
             it. Artifact odds are one separate check per chest.",
        ));
    let boats = Card::new()
        .header(CardHeader::new("Per-boat tier chances").description(
            "Exact odds per generated chest, never tier-or-higher; the fleet table above is their arrival-weighted average",
        ))
        .child(
            div()
                .id("odds-boat-table")
                .h(table::data_height(view.boats, 33.0, table_height))
                .child(fit_columns(DataTable::new(&state.sailing_controls.odds_boats_table), &state.sailing_controls.odds_boats_table)),
        )
        .child(callout(
            "odds-boat-callout",
            muted,
            "Captain rare-chest chance shifts generated chests into higher tiers. The ship changes \
             the chest rate and the stored value, not the tier odds.",
        ));
    let pile = Card::new()
        .header(CardHeader::new("Near-full loot pile").description(view.talent.clone()))
        .child(div().id("odds-pile-table").child(table::table("odds-pile-scroll", 
            &[
                TableColumn::text("Transition"),
                TableColumn::number("Promotion chance"),
                TableColumn::number("Value / payout"),
                TableColumn::number("Value on success"),
                TableColumn::number("Artifact multiplier"),
            ],
            view.pile.iter().map(|row| {
                table::Row::new(("pile-row", row.tier))
                    .child(table::cell(
                        div()
                            .id(("pile-transition", row.tier))
                            .child(row.transition.clone()),
                    ))
                    .child(table::cell(row.chance.clone()))
                    .child(table::cell(row.value.clone()))
                    .child(table::cell(row.success.clone()))
                    .child(table::cell(row.artifact.clone()))
            }),
        )))
        .child(callout(
            "odds-pile-callout",
            muted,
            "With the talent active and exactly one empty slot, each incoming ordinary chest scans the \
             banked chests from the first one onward. The first same-island, same-tier, \
             sufficient-value match rolls its promotion chance and stops the scan either way. \
             Otherwise the first same-island chest with both a strictly higher tier and a strictly \
             greater stored value replaces the banked chest. A stored chest has no captain identity, \
             so this stateful processing is not part of direct treasure/hr; grouping boats on one \
             island raises the number of ordinary matches.",
        ));
    v_flex()
        .id("odds-body")
        .gap_3()
        .child(kpis)
        .child(fleet)
        .child(boats)
        .child(pile)
        .into_any_element()
}

fn tier_row(row: &TierRow) -> table::Row {
    let tipped = |id: ElementId, cell: &Tipped| {
        table::cell(table::tip(id, cell.tip.clone(), cell.text.clone()))
    };
    let [gold, primary, rare] = &row.currencies;
    table::Row::new(("odds-row", row.tier))
        .child(table::cell(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .id(("odds-tier", row.tier))
                        .child(format!("Tier {}", row.tier)),
                )
                .child(
                    div()
                        .id((
                            if row.kept {
                                "odds-kept"
                            } else {
                                "odds-discarded"
                            },
                            row.tier,
                        ))
                        .child(if row.kept {
                            table::tag("Kept", Tone::Success)
                        } else {
                            table::tag("Discarded", Tone::Neutral)
                        }),
                ),
        ))
        .child(tipped(("odds-exact", row.tier).into(), &row.exact))
        .child(tipped(
            ("odds-cumulative", row.tier).into(),
            &row.cumulative,
        ))
        .child(table::cell(row.per_hour.clone()))
        .child(table::cell(row.multiplier.clone()))
        .child(table::cell(row.payouts.clone()))
        .child(tipped(("odds-average", row.tier).into(), &row.average))
        .child(tipped(("odds-gold", row.tier).into(), gold))
        .child(tipped(("odds-primary", row.tier).into(), primary))
        .child(tipped(("odds-rare", row.tier).into(), rare))
        .child(table::cell(row.artifact.clone()))
}

/// Weigh the fleet's tiers once and fill the per-boat table.
fn build(state: &AppState, cx: &mut App) -> View {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let routes = &plan.routes;
    let outcomes = aggregate_fleet_tiers(routes);
    let breakdowns: Vec<ChestPayoutBreakdown> = outcomes
        .tiers
        .iter()
        .map(|tier| fleet_chest_payout_breakdown(routes, tier.tier, &state.sailing))
        .collect();
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
    let minimum_tier = state.sailing.minimum_tier;

    let boats: Vec<BoatRow> = routes.iter().map(|route| boat_row(state, route)).collect();
    let boat_count = boats.len();
    table::set_rows(&state.sailing_controls.odds_boats_table, boats, cx);

    let tiers = outcomes
        .tiers
        .iter()
        .zip(&breakdowns)
        .map(|(tier, breakdown)| {
            let currencies = breakdown.currencies;
            TierRow {
                tier: tier.tier,
                exact: odds(tier.probability, tier.tier, "exact"),
                cumulative: odds(tier.cumulative, tier.tier, "cumulative"),
                per_hour: format!("{:.2}", tier.expected_per_hour).into(),
                multiplier: format!("{:.3}x", tier.loot_multiplier).into(),
                payouts: breakdown.payouts.to_string().into(),
                average: Tipped {
                    text: format::value(breakdown.average_stored_value).into(),
                    tip: format!(
                        "{} opened per chest across all guaranteed payouts",
                        format::value(breakdown.total_value_per_chest)
                    )
                    .into(),
                },
                currencies: [currencies.gold, currencies.primary, currencies.rare].map(payout),
                artifact: format!("{:.3}", tier.artifact_multiplier).into(),
                kept: tier.tier >= minimum_tier as usize,
            }
        })
        .collect();
    let pile = near_full_pile(&state.sailing)
        .iter()
        .map(|row| PileRow {
            tier: row.tier,
            transition: format!("Tier {} → Tier {}", row.tier, row.next_tier).into(),
            chance: chance(row.chance).into(),
            value: format!(
                "{:.3}x → {:.3}x",
                row.loot_multiplier, row.next_loot_multiplier
            )
            .into(),
            success: format!("{:.3}x", row.value_ratio).into(),
            artifact: format!("{:.1}x", row.artifact_multiplier).into(),
        })
        .collect();
    let headers = vec![
        // The tier carries its filter tag, which saves a column.
        TableColumn::text("Tier").width(104.),
        TableColumn::number("Exact chance").width(76.),
        TableColumn::number("Tier+ chance").width(76.),
        TableColumn::number("Chests/hr").width(64.),
        TableColumn::number("Value / payout ×").width(72.),
        TableColumn::number("Guaranteed payouts").width(96.),
        TableColumn::number("Average value / payout").width(80.),
        TableColumn::titled(format!("{} / opened chest", currency(0).name())).numeric(),
        TableColumn::titled(format!("{} / opened chest", primary.name())).numeric(),
        TableColumn::titled(format!("{} / opened chest", rare.name())).numeric(),
        TableColumn::number("Artifact chance ×").width(72.),
    ];
    View {
        summary: format!(
            "{:.2} chests/hr · rare-chest scale {weighted:.3}x · tier {minimum_tier}+ kept",
            outcomes.generated_per_hour
        )
        .into(),
        kpis: [
            (
                "odds-kpi-tier",
                "Chest tier",
                "Random once".into(),
                "selected when each chest is generated".into(),
            ),
            (
                "odds-kpi-scale",
                "Rare-chest scale",
                format!("{weighted:.3}x").into(),
                format!("{minimum:.3}x to {maximum:.3}x across captains").into(),
            ),
            (
                "odds-kpi-payouts",
                "Guaranteed payouts",
                payouts.join(" / ").into(),
                "Tier 0 through Tier 5".into(),
            ),
            (
                "odds-kpi-payout",
                "Each payout",
                "Full chest value".into(),
                "the currency is selected independently".into(),
            ),
            (
                "odds-kpi-currencies",
                "Currency chances",
                format!(
                    "{} / {} / {}",
                    percent(shares.gold),
                    percent(shares.primary),
                    percent(shares.rare)
                )
                .into(),
                "gold / primary / rare per payout".into(),
            ),
            (
                "odds-kpi-filter",
                "Current filter",
                format!("Tier {minimum_tier}+").into(),
                "discarded chests are not rerolled".into(),
            ),
            (
                "odds-kpi-names",
                "Island currencies",
                format!("{} · {}", primary.name(), rare.name()).into(),
                "paid by the selected island".into(),
            ),
        ],
        headers,
        generated: format!(
            "{:.2} generated chests/hr · each tier weighted by the boats that roll it",
            outcomes.generated_per_hour
        )
        .into(),
        tiers,
        boats: boat_count,
        pile,
        talent: format!(
            "Unending Loot Search {}% · Tier 4 cannot promote to Miracle",
            state.sailing.unending_loot_search
        )
        .into(),
    }
}

fn boat_row(state: &AppState, route: &RouteMetrics) -> BoatRow {
    BoatRow {
        boat: route.boat,
        captain: route
            .captain
            .map(|captain| captain_short(state, captain))
            .unwrap_or_else(|| "—".into())
            .into(),
        chests: format!("{:.2}", route.generated_chests_per_hour).into(),
        bonus: format!("+{}%", format::value(route.rarity.captain)).into(),
        scale: format!("{:.3}x", route.rarity.rarity_scale).into(),
        tiers: route
            .rarity
            .tiers
            .iter()
            .map(|tier| odds(tier.probability, tier.tier, "boat"))
            .collect(),
    }
}

/// A tier probability and its exact value on hover.
fn odds(probability: f64, tier: usize, kind: &str) -> Tipped {
    Tipped {
        text: chest_odds(probability).into(),
        tip: format!("Tier {tier} {kind} probability: {}", chance(probability)).into(),
    }
}

/// One currency's value per opened chest and how it is paid.
fn payout(payout: CurrencyPayout) -> Tipped {
    Tipped {
        text: format::value(payout.expected_value_per_chest).into(),
        tip: format!(
            "{} value per opened chest · {:.2} payouts avg · {} per payout · {}/hr",
            format::value(payout.expected_value_per_chest),
            payout.expected_payouts_per_chest,
            percent(payout.chance_per_payout),
            format::rate(payout.expected_value_per_hour),
        )
        .into(),
    }
}

fn callout(id: &'static str, muted: gpui::Hsla, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .child(Caption::new(text).text_color(muted))
        .into_any_element()
}
