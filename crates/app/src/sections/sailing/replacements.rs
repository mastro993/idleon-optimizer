//! The Replacements tab: the strongest-source replacement matrix.
//!
//! A banked chest stores no source boat or captain, so the matrix answers a
//! hypothetical: if the near-full scan reached a chest of this boat's value,
//! what would replace it? The reference is the fleet's strongest source, and the
//! sources are ranked strongest first so the order never depends on the input.

use super::controls::cached;
use super::table::{self, Cell};
use super::{captain_code, field};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div,
};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_sailing::{
    ReplacementOutcome, RouteMetrics, chest_loot_multiplier, ranked, replacement_outcome, strongest,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, show_number};

/// The tiers the matrix compares, the ordinary chest tiers.
const TIERS: usize = 5;

/// The figures the tab shows, read once per key.
pub(crate) struct View {
    sources: usize,
    strongest: SharedString,
    strongest_value: SharedString,
    rate: SharedString,
}

/// One target-tier cell: its text and the factors behind it.
pub(crate) struct Chance {
    text: SharedString,
    tip: SharedString,
}

/// One ranked source, formatted.
pub(crate) struct Row {
    boat: usize,
    rank: SharedString,
    captain: SharedString,
    value: SharedString,
    share: SharedString,
    tiers: [Chance; TIERS],
    miracle: Chance,
}

pub(crate) fn columns() -> Vec<Cell<Row>> {
    vec![
        Cell::element(
            "Rank",
            60.0,
            |row, _| {
                div()
                    .id(("replacements-rank", row.boat))
                    .child(row.rank.clone())
                    .into_any_element()
            },
            |row| row.rank.clone(),
        ),
        Cell::text("Boat", 70.0, |row| format!("Boat {}", row.boat + 1).into()),
        Cell::text("Captain", 70.0, |row| row.captain.clone()),
        Cell::number("Tier 0 value", 100.0, |row| row.value.clone()),
        Cell::number("% max", 70.0, |row| row.share.clone()),
        Cell::element("Strongest Tier 0", 190.0, tier_cell::<0>, |row| {
            row.tiers[0].text.clone()
        }),
        Cell::element("Strongest Tier 1", 190.0, tier_cell::<1>, |row| {
            row.tiers[1].text.clone()
        }),
        Cell::element("Strongest Tier 2", 190.0, tier_cell::<2>, |row| {
            row.tiers[2].text.clone()
        }),
        Cell::element("Strongest Tier 3", 190.0, tier_cell::<3>, |row| {
            row.tiers[3].text.clone()
        }),
        Cell::element("Strongest Tier 4", 190.0, tier_cell::<4>, |row| {
            row.tiers[4].text.clone()
        }),
        Cell::element(
            "Miracle find",
            110.0,
            |row, _| {
                table::tip(
                    ("replacements-miracle", row.boat),
                    row.miracle.tip.clone(),
                    row.miracle.text.clone(),
                )
            },
            |row| row.miracle.text.clone(),
        ),
    ]
}

fn tier_cell<const TIER: usize>(row: &Row, _: &App) -> AnyElement {
    let cell = &row.tiers[TIER];
    table::tip(
        SharedString::from(format!("replacements-cell-{}-{TIER}", row.boat)),
        cell.tip.clone(),
        cell.text.clone(),
    )
}

/// The replacements tab: the controls, the reference KPIs and the matrix.
pub(crate) fn replacements_tab(
    state: &mut AppState,
    table_height: Pixels,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = state.sailing_key();
    let view = cached(state, |controls| &mut controls.replacements, key, cx, build);
    let muted = cx.theme().muted_foreground;
    let talent = state.sailing.unending_loot_search;
    let uls = &state.sailing_controls.unending_loot_search;
    show_number(uls, talent, 0, window, cx);
    let controls = Card::new()
        .header(CardHeader::new("Replacement threshold").description(
            "Unending Loot Search (talent 325) drives the near-full scan; without it no ordinary chest is replaced.",
        ))
        .child(div().id("replacements-controls").child(field(
            "Unending Loot Search %",
            number_field("replacements-uls", uls, "Unending Loot Search"),
        )));
    let kpis = div()
        .id("replacements-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{} sources · strongest {}",
            view.sources, view.strongest
        ))
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "replacements-sources",
                        "Ranked sources",
                        view.sources.to_string(),
                    )
                    .caption("strongest Tier 0 value first"),
                )
                .child(
                    div()
                        .id("replacements-strongest")
                        .role(gpui::Role::Label)
                        .aria_label(format!("{} · {}", view.strongest, view.strongest_value))
                        .child(
                            Statistic::new(
                                "replacements-strongest-source",
                                "Strongest source",
                                view.strongest.clone(),
                            )
                            .caption(format!("{} Tier 0 value per payout", view.strongest_value)),
                        ),
                )
                .child(
                    Statistic::new(
                        "replacements-talent",
                        "Unending Loot Search",
                        format!("{talent:.0}"),
                    )
                    .suffix("%")
                    .caption("talent 325; the save does not model talents"),
                )
                .child(
                    Statistic::new("replacements-rate", "Replacements/hr", view.rate.clone())
                        .caption("Tier 0 targets, either ordinary path"),
                ),
        );
    let mut body = v_flex()
        .id("replacements-body")
        .gap_3()
        .child(controls)
        .child(kpis);
    if view.sources > 0 {
        body = body.child(
            Card::new()
                .header(CardHeader::new("Strongest-Source Replacement Matrix").description(
                    "Same-tier promotion plus the natural higher tier an incoming chest can roll on its own island.",
                ))
                .child(
                    div()
                        .id("replacements-table")
                        .h(table::data_height(view.sources, 33.0, table_height))
                        .child(DataTable::new(&state.sailing_controls.replacements_table)),
                )
                .child(
                    Caption::new(
                        "Cells are exact chance × the conditional Unending Loot Search roll, or the chance of a natural tier that beats the reference. Both need the same island and exactly one empty slot, and assume the scan reaches this target. Miracle generation is independent of island and value; pile order and future evolution arrive with SL-03.",
                    )
                    .text_color(muted),
                ),
        );
    }
    body.into_any_element()
}

/// Rank the sources once and fill the matrix.
fn build(state: &AppState, cx: &mut App) -> View {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let sources = ranked(&plan.routes);
    let reference = strongest(&plan.routes).cloned();
    let (strongest, strongest_value) = match &reference {
        Some(reference) => (
            format!(
                "Boat {} / Captain {}",
                reference.boat + 1,
                reference
                    .captain
                    .map(captain_code)
                    .unwrap_or_else(|| "—".into())
            ),
            format::rate(reference.loot.value),
        ),
        None => ("—".into(), "—".into()),
    };
    let rate: f64 = reference.as_ref().map_or(0.0, |reference| {
        sources
            .iter()
            .map(|source| {
                replacement_outcome(source, reference, 0, &state.sailing).total_rate_per_hour
            })
            .sum()
    });
    let rows = match &reference {
        Some(reference) => sources
            .iter()
            .enumerate()
            .map(|(rank, source)| row(state, rank, source, reference))
            .collect(),
        None => Vec::new(),
    };
    let view = View {
        sources: if reference.is_some() {
            sources.len()
        } else {
            0
        },
        strongest: strongest.into(),
        strongest_value: strongest_value.into(),
        rate: format!("{rate:.2}").into(),
    };
    table::set_rows(&state.sailing_controls.replacements_table, rows, cx);
    view
}

fn row(state: &AppState, rank: usize, source: &RouteMetrics, reference: &RouteMetrics) -> Row {
    let miracle = replacement_outcome(source, reference, 0, &state.sailing);
    Row {
        boat: source.boat,
        rank: (rank + 1).to_string().into(),
        captain: source
            .captain
            .map(captain_code)
            .unwrap_or_else(|| "—".into())
            .into(),
        value: format::rate(source.loot.value).into(),
        share: format!(
            "{}%",
            if reference.loot.value > 0.0 {
                (source.loot.value / reference.loot.value * 100.0).round()
            } else {
                0.0
            }
        )
        .into(),
        tiers: std::array::from_fn(|tier| cell(state, source, reference, tier as u32)),
        miracle: Chance {
            text: chance(miracle.miracle_generation_chance).into(),
            tip: format!(
                "Natural Miracle generation per incoming chest\nExpected finds: {} /hr\n3+ empty slots add a chest; 0-2 replace the first scanned non-Miracle.\nUnending Loot Search cannot promote Tier 4 to Miracle.",
                format::value(miracle.miracle_generation_rate_per_hour)
            )
            .into(),
        },
    }
}

/// One target-tier cell: the replacement chance and which path produced it.
fn cell(state: &AppState, source: &RouteMetrics, reference: &RouteMetrics, tier: u32) -> Chance {
    let outcome = replacement_outcome(source, reference, tier, &state.sailing);
    let path = match outcome.threshold.required_tier {
        Some(required) if required <= 4 => Some(format!("T{required}-4")),
        _ if outcome.same_tier_upgrade_chance > 0.0 => Some("Unending Loot Search".to_string()),
        _ => None,
    };
    let body = chance(outcome.total_chance);
    Chance {
        text: match path {
            Some(path) => format!("{body} · {path}"),
            None => body,
        }
        .into(),
        tip: breakdown(source, reference, &outcome, tier).into(),
    }
}

/// The cell tooltip: every factor behind the chance.
fn breakdown(
    source: &RouteMetrics,
    reference: &RouteMetrics,
    outcome: &ReplacementOutcome,
    tier: u32,
) -> String {
    let threshold = &outcome.threshold;
    let mut lines = vec![
        format!(
            "Boat {} / Captain {}",
            source.boat + 1,
            source
                .captain
                .map(captain_code)
                .unwrap_or_else(|| "—".into())
        ),
        format!(
            "Base boat value: {} per payout at Tier 0",
            format::rate(source.loot.value)
        ),
        String::new(),
        format!(
            "Reference: Boat {} / Captain {} Tier {tier}",
            reference.boat + 1,
            reference
                .captain
                .map(captain_code)
                .unwrap_or_else(|| "—".into())
        ),
        format!(
            "Reference value: {} per payout",
            format::rate(threshold.reference_value)
        ),
        format!(
            "Incoming at Tier {tier}: {} per payout",
            format::rate(source.loot.value * chest_loot_multiplier(tier as usize))
        ),
        String::new(),
        format!(
            "Exact Tier {tier} drop chance: {}",
            format::percent(outcome.exact_same_tier_chance)
        ),
    ];
    if threshold.same_tier_eligible {
        lines.push(format!(
            "Conditional Unending Loot Search chance: {}",
            format::percent(outcome.uls_chance)
        ));
        lines.push(format!(
            "Same-tier path: {}",
            format::percent(outcome.same_tier_upgrade_chance)
        ));
    } else {
        lines.push("Same tier is short of the reference value".into());
    }
    match threshold.required_tier {
        Some(required) => {
            lines.push(String::new());
            lines.push(format!(
                "Natural replacement needs Tier {required} or better"
            ));
            lines.push(format!(
                "Chance: {}",
                format::percent(outcome.natural_replacement_chance)
            ));
        }
        None => {
            lines.push(String::new());
            lines.push("No ordinary natural replacement".into());
            lines.push(format!(
                "Best ordinary value: {} ({} short)",
                format::rate(threshold.max_value),
                format::percent(threshold.shortfall)
            ));
        }
    }
    if !outcome.active {
        lines.push(String::new());
        lines.push("Unending Loot Search is off, so no near-full replacement runs".into());
    }
    lines.push(String::new());
    lines.push(format!(
        "Combined chance: {}",
        format::percent(outcome.total_chance)
    ));
    lines.join("\n")
}

/// A chance as a percentage, reading as a reciprocal once it is tiny.
fn chance(value: f64) -> String {
    if value <= 0.0 {
        "Never".into()
    } else if value >= 0.0001 {
        format::percent(value)
    } else {
        format!("1 in {}", format::rate(1.0 / value))
    }
}
