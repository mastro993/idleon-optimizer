//! The Resources tab: the world's resource nodes, what drains them, the banked
//! hours and the daily reset projection.
//!
//! The inspector is driven by a node selector so the harness can read it
//! without a pointer, and the drain staffing is the stream table under it.

use super::overview::{figure_text, kpi, number_cell, rate_cell, tag};
use super::table::{Col, table, text, text_id};
use super::world_rail;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div, px};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{
    self as royal_guardian, Allocation, NodeAllocation, Rate, ResourceNode,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, Tone};

/// The Resources pane.
pub(crate) fn resources(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let nodes = royal.world_resources(world);
    let derived = &state.royal_controls.derived;
    let selected = selected_resource(
        royal,
        derived.streamed(royal),
        world,
        state.royal_state.resource,
    );
    let allocation = derived.allocation(royal);
    let banked = derived.banked(royal);
    v_flex()
        .id("royal-resources")
        .gap(PANEL_GAP)
        .child(world_rail(world, cx))
        .child(time_cards(allocation, banked, cx))
        .child(node_picker(state, &nodes, world, selected, window, cx))
        .child(inspector(royal, allocation, selected, cx))
        .child(node_table(&nodes, allocation, selected, cx))
        .child(rate_card(&royal.global_collection_rate(), cx))
        .into_any_element()
}

/// The banked hours and the daily reset.
fn time_cards(
    allocation: &Allocation,
    banked: &royal_guardian::Banked,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let reset = &allocation.reset;
    let reset_caption = match (reset.available, reset.hours_remaining) {
        (true, Some(hours)) => format!("{} until the daily reset", format::hours(hours)),
        _ => "The save carries no TimeAway.ShopRestock".to_owned(),
    };
    let refills = allocation
        .projected_refills_at_reset
        .map(|refills| format::count(refills as u64))
        .unwrap_or_else(|| "—".to_owned());
    let emptied = allocation
        .nodes_emptying_by_reset
        .map(|nodes| format::count(nodes as u64))
        .unwrap_or_else(|| "—".to_owned());
    let collectable = allocation
        .collectable_before_reset
        .map(format::value)
        .unwrap_or_else(|| "—".to_owned());
    let grades = allocation
        .projected_grade_gains_at_reset
        .map(format::value)
        .unwrap_or_else(|| "—".to_owned());
    let reset_body = v_flex()
        .id("royal-reset-projection")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Drain projection: {collectable} collectable before the reset, {emptied} nodes \
             emptying, {refills} refilling and {grades} grade gains"
        ))
        .gap_1()
        .child(format!(
            "Drain projection: {collectable} collectable before the reset"
        ))
        .child(format!(
            "{emptied} nodes empty before it · {refills} refill · {grades} grade gains"
        ))
        .child(Caption::new(reset_caption.clone()).text_color(muted));
    let strip = KpiStrip::new()
        .child(kpi(
            "royal-kpi-banked",
            "Banked hours",
            banked.hours,
            "h",
            if banked.banks_time {
                "Armory 30 banks time while the menu is closed"
            } else {
                "Armory 30 is not bought, so no time banks"
            },
        ))
        .child(kpi(
            "royal-kpi-banked-gain",
            "Banked collection",
            banked.gain,
            "",
            format!(
                "{} currency from the banked time",
                format::value(banked.currency_gain)
            ),
        ))
        .child(kpi(
            "royal-kpi-reset",
            "Hours to reset",
            reset.hours_remaining.unwrap_or(0.0),
            "h",
            reset_caption.clone(),
        ))
        .child(
            v_flex()
                .id("royal-reset-card")
                .gap_1()
                .child(Caption::new("Daily reset"))
                .child(reset_body),
        );
    div()
        .id("royal-resource-time")
        .child(strip)
        .into_any_element()
}

/// The node selector that drives the inspector.
fn node_picker(
    state: &AppState,
    nodes: &[ResourceNode],
    world: usize,
    selected: usize,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let choices: Vec<Choice<usize>> = nodes
        .iter()
        .map(|node| {
            Choice::new(
                node.resource,
                format!(
                    "Resource {} · {}",
                    node.resource + 1,
                    if node.currency_slot < 0 {
                        "no currency".to_owned()
                    } else {
                        format!("currency slot {}", node.currency_slot)
                    }
                ),
            )
        })
        .collect();
    let select = &state.royal_controls.resource;
    select.show(choices, Some(&selected), window, cx);
    h_flex()
        .id("royal-resource-picker")
        .gap_3()
        .child(
            div().flex_none().w(px(240.0)).child(
                Select::new(select.state())
                    .id("royal-resource")
                    .accessibility_label("Resource node")
                    .small(),
            ),
        )
        .child(Caption::new(format!(
            "World {} lists {} nodes",
            world + 1,
            nodes.len()
        )))
        .into_any_element()
}

/// The selected node: its state, its cycle and the outposts that drain it.
fn inspector(
    royal: &RoyalGuardian,
    allocation: &Allocation,
    resource: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let node = royal.resource_node(resource);
    let detail = allocation
        .details
        .iter()
        .find(|detail| detail.resource == resource);
    let cycle = royal.resource_cycle(
        resource,
        detail.map(|detail| detail.rate).unwrap_or_default(),
    );
    let mut body = v_flex()
        .id("royal-resource-detail")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Resource {}: {} of {} remaining, grade {}",
            resource + 1,
            format::value(node.remaining),
            format::value(node.capacity),
            format::value(node.grade)
        ))
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(15.0))
                        .child(format!("Resource {}", resource + 1)),
                )
                .child(tag(status(&node, detail), tone(&node, detail))),
        )
        .child(Caption::new(format!(
            "World {} · grade {} · {} · saved progress {}",
            node.world + 1,
            format::value(node.grade),
            if node.currency_slot < 0 {
                "no currency".to_owned()
            } else {
                format!("currency slot {}", node.currency_slot)
            },
            format::value(node.raw_progress)
        )))
        .child(format!(
            "{} of {} remaining · {} collected",
            format::value(node.remaining),
            format::value(node.capacity),
            format::value(node.collected)
        ))
        .child(format!(
            "Drain {} /hr · currency {} /hr · {}",
            format::value(detail.map(|detail| detail.rate).unwrap_or_default()),
            format::value(
                detail
                    .map(|detail| detail.currency_rate)
                    .unwrap_or_default()
            ),
            match detail.map(|detail| detail.drain_hours) {
                Some(hours) if hours.is_finite() && hours > 0.0 => {
                    format!("{} to empty", format::hours(hours))
                }
                _ => "no drain".to_owned(),
            }
        ))
        .child(reset_line(detail))
        .child(cycle_line(&cycle));
    if let Some(detail) = detail {
        body = body.child(staffing_table(royal, detail, cx));
    }
    Card::new()
        .header(
            CardHeader::new("Resource node")
                .description("What the node holds, what drains it and what the reset refills"),
        )
        .child(body)
        .into_any_element()
}

/// What the reset does to the selected node.
fn reset_line(detail: Option<&NodeAllocation>) -> AnyElement {
    let text = match detail {
        Some(detail) if detail.collectable_before_reset.is_some() => {
            let grade = detail
                .projected_grade_at_reset
                .map(format::value)
                .unwrap_or_else(|| "—".to_owned());
            let remaining = detail
                .projected_remaining_at_reset
                .map(format::value)
                .unwrap_or_else(|| "—".to_owned());
            match detail.refill_boundary {
                "saved-drained" => {
                    format!("Drained and saved: refills to {remaining} at grade {grade}")
                }
                "collect-before-reset" => format!(
                    "Empties before the reset: collect it to refill to {remaining} at grade {grade}"
                ),
                _ => format!("No refill at the reset: {remaining} left at grade {grade}"),
            }
        }
        _ => "No reset projection: the save carries no reset countdown".to_owned(),
    };
    div()
        .id("royal-resource-reset")
        .child(text)
        .into_any_element()
}

/// The next cycle at the current drain.
fn cycle_line(cycle: &royal_guardian::NodeCycle) -> AnyElement {
    let text = if cycle.next_cycle_available {
        format!(
            "Next cycle: grade {} · {} capacity · {} /hr · {} to empty{}",
            format::value(cycle.next_grade),
            format::value(cycle.next_capacity.unwrap_or_default()),
            format::value(cycle.next_rate.unwrap_or_default()),
            format::hours(cycle.next_drain_hours.unwrap_or_default()),
            if cycle.grade_increase_unlocked {
                " · Armory 0 adds a grade"
            } else {
                ""
            }
        )
    } else {
        format!(
            "Resource Replenish (Armory 70) is locked: a full node at grade {} takes {} to drain",
            format::value(cycle.grade),
            format::hours(cycle.full_drain_hours)
        )
    };
    div()
        .id("royal-resource-cycle")
        .child(text)
        .into_any_element()
}

/// The outposts draining the selected node.
fn staffing_table(
    royal: &RoyalGuardian,
    detail: &NodeAllocation,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if detail.streams.is_empty() {
        return div()
            .id("royal-resource-staffing-empty")
            .child(Caption::new("No outpost links this node."))
            .into_any_element();
    }
    let rows = detail.streams.iter().map(|stream| {
        vec![
            text_id(
                SharedString::from(format!("royal-stream-{}-{}", stream.map, stream.slot)),
                royal.outpost(stream.map).name.replace('_', " "),
            ),
            number_cell(stream.slot as f64 + 1.0),
            text(if stream.savage { "Savage" } else { "Normal" }),
            number_cell(stream.rate),
            number_cell(stream.drain_rate),
            number_cell(stream.currency_rate),
        ]
    });
    v_flex()
        .id("royal-resource-staffing")
        .gap_1()
        .child(Caption::new("Drain staffing"))
        .child(table(
            "royal-stream",
            &[
                Col::Text("Outpost"),
                Col::Number("Slot"),
                Col::Text("Kind"),
                Col::Number("Rate /hr"),
                Col::Number("Drain /hr"),
                Col::Number("Currency /hr"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// Every node the world lists, with its rate and the reset's effect.
fn node_table(
    nodes: &[ResourceNode],
    allocation: &Allocation,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let accent = cx.theme().primary;
    let rows = nodes.iter().map(|node| {
        let detail = allocation
            .details
            .iter()
            .find(|detail| detail.resource == node.resource);
        let pick = cx.entity();
        let resource = node.resource;
        vec![
            v_flex()
                .id(SharedString::from(format!("royal-node-row-{resource}")))
                .role(gpui::Role::Button)
                .cursor_pointer()
                .gap_1()
                .child(
                    div()
                        .when(resource == selected, |row| row.text_color(accent))
                        .child(format!("Resource {}", resource + 1)),
                )
                .child(Caption::new(status(node, detail)))
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.set_royal_resource(resource, cx));
                })
                .into_any_element(),
            number_cell(node.grade),
            text(format!(
                "{} / {}",
                format::value(node.remaining),
                format::value(node.capacity)
            )),
            rate_cell(detail.map(|detail| detail.rate).unwrap_or_default()),
            text(match detail.map(|detail| detail.drain_hours) {
                Some(hours) if hours.is_finite() => format::hours(hours),
                _ => "—".to_owned(),
            }),
            text(if node.currency_slot < 0 {
                "—".to_owned()
            } else {
                format!("Slot {}", node.currency_slot)
            }),
            text(detail.map(reset_chip).unwrap_or_else(|| "—".to_owned())),
        ]
    });
    let table = table(
        "royal-node",
        &[
            Col::Text("Node"),
            Col::Number("Grade"),
            Col::Number("Remaining"),
            Col::Number("Rate /hr"),
            Col::Number("Drain"),
            Col::Text("Currency"),
            Col::Text("Reset"),
        ],
        rows,
        cx,
    );
    Card::new()
        .header(
            CardHeader::new("Resource nodes")
                .description("Every node the world page lists, with its saved drain"),
        )
        .child(div().id("royal-node-table").child(table))
        .into_any_element()
}

/// The reset's one-word effect on a node.
fn reset_chip(detail: &NodeAllocation) -> String {
    match detail.refill_boundary {
        "saved-drained" => "Refills".to_owned(),
        "collect-before-reset" => "Collect first".to_owned(),
        _ => "—".to_owned(),
    }
}

/// The collection rate's factors, and the sources this build leaves out.
fn rate_card(rate: &Rate, cx: &mut Context<AppState>) -> AnyElement {
    let rows = rate.factors.iter().map(|factor| {
        vec![
            text_id(
                SharedString::from(format!("royal-rate-factor-{}", factor.name)),
                factor.label,
            ),
            number_cell(factor.value),
        ]
    });
    let factors = table(
        "royal-rate-factor",
        &[Col::Text("Factor"), Col::Number("Value")],
        rows,
        cx,
    );
    Card::new()
        .header(
            CardHeader::new("Collection rate")
                .description("What every outpost's collection rate multiplies in"),
        )
        .child(
            v_flex()
                .id("royal-collection-factors")
                .gap_2()
                .child(figure_line(rate.value))
                .child(factors),
        )
        .when(rate.partial, |card| {
            card.child(
                v_flex()
                    .gap_1()
                    .child(Caption::new("Sources this build does not read yet:"))
                    .child(
                        v_flex()
                            .id("royal-collection-missing")
                            .gap_1()
                            .children(rate.missing.iter().cloned().map(Caption::new)),
                    ),
            )
        })
        .into_any_element()
}

/// The rate as a headline line.
fn figure_line(value: f64) -> AnyElement {
    div()
        .id("royal-collection-rate")
        .role(gpui::Role::Label)
        .aria_label(format!("Collection rate: {}", figure_text(value)))
        .child(format!("×{} kingdom multiplier", format::value(value)))
        .into_any_element()
}

/// The node the inspector explains: the saved one while it is in this world,
/// else the first node that a stream drains, else the world's first node.
/// `streamed` lists the nodes the saved streams drain, in stream order.
pub(super) fn selected_resource(
    royal: &RoyalGuardian,
    streamed: &[usize],
    world: usize,
    saved: Option<usize>,
) -> usize {
    let nodes = royal.world_resources(world);
    let in_world = |resource: usize| nodes.iter().any(|node| node.resource == resource);
    if let Some(saved) = saved
        && in_world(saved)
    {
        return saved;
    }
    streamed
        .iter()
        .copied()
        .find(|resource| in_world(*resource))
        .or_else(|| nodes.first().map(|node| node.resource))
        .unwrap_or(0)
}

/// A node's one-word state.
fn status(node: &ResourceNode, detail: Option<&NodeAllocation>) -> String {
    if node.drained {
        return "Ready to reset".to_owned();
    }
    if node.remaining == 0.0 {
        return "Empty".to_owned();
    }
    match detail {
        Some(detail) if detail.rate > 0.0 => "Draining".to_owned(),
        _ => "No stream".to_owned(),
    }
}

/// The badge tone that goes with the state.
fn tone(node: &ResourceNode, detail: Option<&NodeAllocation>) -> Tone {
    if node.drained {
        return Tone::Warning;
    }
    if node.remaining == 0.0 {
        return Tone::Neutral;
    }
    match detail {
        Some(detail) if detail.rate > 0.0 => Tone::Success,
        _ => Tone::Danger,
    }
}
