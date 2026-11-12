//! The Resources tab: the world's resource nodes, what drains them, the banked
//! hours and the daily reset projection.
//!
//! The upstream page's resource workspace is a list plus an inspector; here the
//! inspector is driven by a node selector so the harness can read it without a
//! pointer, and the drain staffing is the stream table under it.

use super::overview::{figure_text, kpi, number_cell, rate_cell};
use super::world_rail;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div, px};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{
    self as royal_guardian, Allocation, NodeAllocation, Rate, ResourceNode,
};
use idleon_ui::format;

/// The Resources pane.
pub(crate) fn resources(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let nodes = royal.world_resources(world);
    let selected = selected_resource(royal, world, state.royal_state.resource);
    let allocation = royal.allocation_to_reset();
    let banked = royal.banked();
    div()
        .id("royal-resources")
        .flex()
        .flex_col()
        .gap_3()
        .child(world_rail(world, cx))
        .child(time_cards(&allocation, &banked, cx))
        .child(node_picker(royal, world, selected, cx))
        .child(inspector(royal, &allocation, selected))
        .child(node_table(&nodes, &allocation, selected, cx))
        .child(rate_card(&royal.global_collection_rate(), cx))
        .into_any_element()
}

/// The banked hours and the daily reset.
fn time_cards(
    allocation: &Allocation,
    banked: &royal_guardian::Banked,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
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
    let reset_body = div()
        .id("royal-reset-projection")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Drain projection: {collectable} collectable before the reset, {emptied} nodes \
             emptying, {refills} refilling and {grades} grade gains"
        ))
        .flex()
        .flex_col()
        .gap_1()
        .child(format!(
            "Drain projection: {collectable} collectable before the reset"
        ))
        .child(format!(
            "{emptied} nodes empty before it · {refills} refill · {grades} grade gains"
        ))
        .child(Caption::new(reset_caption.clone()).text_color(muted));
    div()
        .id("royal-resource-time")
        .flex()
        .flex_wrap()
        .gap_3()
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
            Card::new()
                .header(CardHeader::new("Daily reset"))
                .child(div().id("royal-reset-card").p_3().child(reset_body)),
        )
        .into_any_element()
}

/// The node selector that drives the inspector.
fn node_picker(
    royal: &RoyalGuardian,
    world: usize,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let choices: Vec<Choice> = royal
        .world_resources(world)
        .into_iter()
        .map(|node| {
            Choice::new(
                node.resource.to_string(),
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
    let select = Select::new("royal-resource", choices)
        .label("Resource node")
        .selected(selected.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(resource) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_royal_resource(resource, cx));
            }
        });
    div()
        .id("royal-resource-picker")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(240.0))
                .child(select),
        )
        .child(
            Caption::new(format!(
                "World {} lists {} nodes",
                world + 1,
                royal.world_resources(world).len()
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// The selected node: its state, its cycle and the outposts that drain it.
fn inspector(royal: &RoyalGuardian, allocation: &Allocation, resource: usize) -> AnyElement {
    let node = royal.resource_node(resource);
    let detail = allocation
        .details
        .iter()
        .find(|detail| detail.resource == resource);
    let cycle = royal.resource_cycle(
        resource,
        detail.map(|detail| detail.rate).unwrap_or_default(),
    );
    let mut body = div()
        .id("royal-resource-detail")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Resource {}: {} of {} remaining, grade {}",
            resource + 1,
            format::value(node.remaining),
            format::value(node.capacity),
            format::value(node.grade)
        ))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(15.0))
                        .child(format!("Resource {}", resource + 1)),
                )
                .child(Badge::new(status(&node, detail)).tone(tone(&node, detail))),
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
        body = body.child(staffing_table(royal, detail));
    }
    Card::new()
        .header(
            CardHeader::new("Resource node")
                .description("What the node holds, what drains it and what the reset refills"),
        )
        .child(div().p_3().child(body))
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
fn staffing_table(royal: &RoyalGuardian, detail: &NodeAllocation) -> AnyElement {
    if detail.streams.is_empty() {
        return div()
            .id("royal-resource-staffing-empty")
            .child(Caption::new("No outpost links this node."))
            .into_any_element();
    }
    let mut table = Table::new([
        "Outpost",
        "Slot",
        "Kind",
        "Rate /hr",
        "Drain /hr",
        "Currency /hr",
    ]);
    for stream in &detail.streams {
        table = table.row([
            div()
                .id(format!("royal-stream-{}-{}", stream.map, stream.slot))
                .child(royal.outpost(stream.map).name.replace('_', " "))
                .into_any_element(),
            number_cell(stream.slot as f64 + 1.0),
            div()
                .child(if stream.savage { "Savage" } else { "Normal" })
                .into_any_element(),
            number_cell(stream.rate),
            number_cell(stream.drain_rate),
            number_cell(stream.currency_rate),
        ]);
    }
    div()
        .id("royal-resource-staffing")
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new("Drain staffing"))
        .child(table)
        .into_any_element()
}

/// Every node the world lists, with its rate and the reset's effect.
fn node_table(
    nodes: &[ResourceNode],
    allocation: &Allocation,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new([
        "Node",
        "Grade",
        "Remaining",
        "Rate /hr",
        "Drain",
        "Currency",
        "Reset",
    ]);
    for node in nodes {
        let detail = allocation
            .details
            .iter()
            .find(|detail| detail.resource == node.resource);
        let pick = cx.entity();
        let resource = node.resource;
        let picked = resource == selected;
        table = table.row([
            div()
                .id(format!("royal-node-row-{resource}"))
                .role(gpui::Role::Button)
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .when(picked, |row| row.text_color(cx.theme().colors.accent))
                        .child(format!("Resource {}", resource + 1)),
                )
                .child(Caption::new(status(node, detail)).text_color(muted))
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.set_royal_resource(resource, cx));
                })
                .into_any_element(),
            number_cell(node.grade),
            div()
                .flex()
                .flex_col()
                .items_end()
                .w_full()
                .child(format!(
                    "{} / {}",
                    format::value(node.remaining),
                    format::value(node.capacity)
                ))
                .into_any_element(),
            rate_cell(detail.map(|detail| detail.rate).unwrap_or_default()),
            div()
                .flex()
                .flex_col()
                .items_end()
                .w_full()
                .child(match detail.map(|detail| detail.drain_hours) {
                    Some(hours) if hours.is_finite() => format::hours(hours),
                    _ => "—".to_owned(),
                })
                .into_any_element(),
            div()
                .child(if node.currency_slot < 0 {
                    "—".to_owned()
                } else {
                    format!("Slot {}", node.currency_slot)
                })
                .into_any_element(),
            div()
                .child(detail.map(reset_chip).unwrap_or_else(|| "—".to_owned()))
                .into_any_element(),
        ]);
    }
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

/// The collection rate's factors, and the sources this port leaves out.
fn rate_card(rate: &Rate, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new(["Factor", "Value"]);
    for factor in &rate.factors {
        table = table.row([
            div()
                .id(format!("royal-rate-factor-{}", factor.name))
                .child(factor.label)
                .into_any_element(),
            number_cell(factor.value),
        ]);
    }
    let mut card = Card::new().header(
        CardHeader::new("Collection rate")
            .description("What every outpost's collection rate multiplies in"),
    );
    card = card.child(
        div()
            .id("royal-collection-factors")
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(figure_line(rate.value))
            .child(table),
    );
    if rate.partial {
        let mut lines = div()
            .id("royal-collection-missing")
            .flex()
            .flex_col()
            .gap_1();
        for source in &rate.missing {
            lines = lines.child(Caption::new(source.clone()).text_color(muted));
        }
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .pb_3()
                .child(Caption::new("Sources this port does not read yet:").text_color(muted))
                .child(lines),
        );
    }
    card.into_any_element()
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
pub(super) fn selected_resource(
    royal: &RoyalGuardian,
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
    let (streams, _) = royal.resource_streams();
    streams
        .iter()
        .map(|stream| stream.resource)
        .find(|resource| in_world(*resource))
        .or_else(|| nodes.first().map(|node| node.resource))
        .unwrap_or(0)
}

/// A node's one-word state, in the upstream page's words.
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
