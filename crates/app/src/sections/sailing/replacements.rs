//! The Replacements tab: the strongest-source replacement matrix.
//!
//! A banked chest stores no source boat or captain, so the matrix answers a
//! hypothetical: if the near-full scan reached a chest of this boat's value,
//! what would replace it? The reference is the fleet's strongest source, and the
//! sources are ranked strongest first so the order never depends on the input.

use super::captain_code;
use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, div, px};
use idleon_sailing::{
    ReplacementOutcome, RouteMetrics, chest_loot_multiplier, ranked, replacement_outcome, strongest,
};
use idleon_ui::format;

/// The tiers the matrix compares, upstream's ordinary chest tiers.
const TIERS: usize = 5;

/// The replacements tab: the controls, the reference KPIs and the matrix.
pub(crate) fn replacements_tab(
    state: &AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let sources = ranked(&plan.routes);
    let reference = strongest(&plan.routes).cloned();
    div()
        .id("replacements-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(controls(state, cx))
        .child(kpis(state, &sources, reference.as_ref(), cx))
        .child(table(state, &sources, reference.as_ref(), table_height, cx))
        .into_any_element()
}

fn controls(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let talent = state.sailing.unending_loot_search;
    let input = NumberInput::new("replacements-uls", talent)
        .range(0.0, 100.0)
        .step(1.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Unending Loot Search")
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_unending_loot_search(value, cx);
            });
        });
    Card::new()
        .header(CardHeader::new("Replacement threshold").description(
            "Unending Loot Search (talent 325) drives the near-full scan; without it no ordinary chest is replaced.",
        ))
        .child(
            div()
                .id("replacements-controls")
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
                        .min_w(px(160.0))
                        .child(Caption::new("Unending Loot Search %"))
                        .child(input),
                ),
        )
        .into_any_element()
}

fn kpis(
    state: &AppState,
    sources: &[&RouteMetrics],
    reference: Option<&RouteMetrics>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let (name, value) = match reference {
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
    let talent = state.sailing.unending_loot_search;
    div()
        .id("replacements-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!("{} sources · strongest {name}", sources.len()))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "replacements-sources",
                    "Ranked sources",
                    sources.len() as f64,
                )
                .decimals(0),
            )
            .caption("strongest Tier 0 value first"),
        )
        .child(
            div()
                .id("replacements-strongest")
                .role(gpui::Role::Label)
                .aria_label(format!("{name} · {value}"))
                .child(
                    Card::new().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_3()
                            .child(Caption::new("Strongest source").text_color(muted))
                            .child(div().text_size(px(20.0)).child(name))
                            .child(
                                Caption::new(format!("{value} Tier 0 value per payout"))
                                    .text_color(muted),
                            ),
                    ),
                ),
        )
        .child(
            KpiCard::new(
                Statistic::new("replacements-talent", "Unending Loot Search", talent)
                    .decimals(0)
                    .suffix("%")
                    .size(TextSize::Lg),
            )
            .caption("talent 325; the save does not model talents"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "replacements-rate",
                    "Replacements/hr",
                    reference.map_or(0.0, |reference| {
                        sources
                            .iter()
                            .map(|source| {
                                replacement_outcome(source, reference, 0, &state.sailing)
                                    .total_rate_per_hour
                            })
                            .sum()
                    }),
                )
                .decimals(2),
            )
            .caption("Tier 0 targets, either ordinary path"),
        )
        .into_any_element()
}

fn table(
    state: &AppState,
    sources: &[&RouteMetrics],
    reference: Option<&RouteMetrics>,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(reference) = reference else {
        return div().into_any_element();
    };
    let mut headers = vec![
        "Rank".to_string(),
        "Boat".to_string(),
        "Captain".to_string(),
        "Tier 0 value".to_string(),
        "% max".to_string(),
    ];
    for tier in 0..TIERS {
        headers.push(match tier {
            0 => "Strongest Tier 0".to_string(),
            4 => "Strongest Tier 4".to_string(),
            tier => format!("Strongest Tier {tier}"),
        });
    }
    headers.push("Miracle find".into());
    let mut table = Table::new(headers);
    for (rank, source) in sources.iter().enumerate() {
        let captain = source
            .captain
            .map(captain_code)
            .unwrap_or_else(|| "—".into());
        let mut cells: Vec<AnyElement> = vec![
            div()
                .id(("replacements-rank", source.boat))
                .child((rank + 1).to_string())
                .into_any_element(),
            div()
                .child(format!("Boat {}", source.boat + 1))
                .into_any_element(),
            div().child(captain).into_any_element(),
            div()
                .child(format::rate(source.loot.value))
                .into_any_element(),
            div()
                .child(format!(
                    "{}%",
                    if reference.loot.value > 0.0 {
                        (source.loot.value / reference.loot.value * 100.0).round()
                    } else {
                        0.0
                    }
                ))
                .into_any_element(),
        ];
        for tier in 0..TIERS {
            cells.push(cell(state, source, reference, tier as u32));
        }
        let miracle = replacement_outcome(source, reference, 0, &state.sailing);
        cells.push(
            div()
                .id(("replacements-miracle", source.boat))
                .tooltip(Tooltip::text(format!(
                    "Natural Miracle generation per incoming chest\nExpected finds: {} /hr\n3+ empty slots add a chest; 0-2 replace the first scanned non-Miracle.\nUnending Loot Search cannot promote Tier 4 to Miracle.",
                    format::value(miracle.miracle_generation_rate_per_hour)
                )))
                .child(chance(miracle.miracle_generation_chance))
                .into_any_element(),
        );
        table = table.row(cells);
    }

    let table = ScrollArea::new("replacements-table-scroll")
        .vertical()
        .h(table_height)
        .child(div().min_w_0().child(table));
    Card::new()
        .header(
            CardHeader::new("Strongest-Source Replacement Matrix").description(
                "Same-tier promotion plus the natural higher tier an incoming chest can roll on its own island.",
            ),
        )
        .child(
            div()
                .id("replacements-table")
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
                    "Cells are exact chance × the conditional Unending Loot Search roll, or the chance of a natural tier that beats the reference. Both need the same island and exactly one empty slot, and assume the scan reaches this target. Miracle generation is independent of island and value; pile order and future evolution arrive with SL-03.",
                ),
        )
        .into_any_element()
}

/// One target-tier cell: the replacement chance and which path produced it.
fn cell(
    state: &AppState,
    source: &RouteMetrics,
    reference: &RouteMetrics,
    tier: u32,
) -> AnyElement {
    let outcome = replacement_outcome(source, reference, tier, &state.sailing);
    let path = match outcome.threshold.required_tier {
        Some(required) if required <= 4 => Some(format!("T{required}-4")),
        _ if outcome.same_tier_upgrade_chance > 0.0 => Some("Unending Loot Search".to_string()),
        _ => None,
    };
    let body = match path {
        Some(_) => chance(outcome.total_chance),
        None if outcome.total_chance <= 0.0 => "Never".to_string(),
        None => chance(outcome.total_chance),
    };
    div()
        .id(format!("replacements-cell-{}-{tier}", source.boat))
        .tooltip(Tooltip::text(breakdown(source, reference, &outcome, tier)))
        .child(match path {
            Some(path) => format!("{body} · {path}"),
            None => body,
        })
        .into_any_element()
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
