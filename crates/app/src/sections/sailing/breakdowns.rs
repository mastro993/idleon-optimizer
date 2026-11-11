//! The Breakdowns tab: the loot, trip, artifact and chest formula trees.
//!
//! One selector picks the boat, as upstream's breakdown tab does, and every tree
//! reads that boat's route from the active assignment. The trees themselves come
//! from `crates/core`, so the searched and copied numbers are the ones the other
//! tabs report.

use super::{captain_short, trip};
use crate::state::AppState;
use crate::ui::formula::FormulaTree;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Pixels, Styled, div, px};
use idleon_core::sailing::{RouteMetrics, artifact_tree, chest_tree, loot_tree, speed_tree};

/// The breakdowns tab: the boat selector and the four trees.
pub fn breakdowns_tab(
    state: &mut AppState,
    _table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let boat = state
        .sailing_state
        .breakdown_boat
        .min(state.fleet.boats.len().saturating_sub(1));
    let Some(route) = plan.routes.iter().find(|route| route.boat == boat).cloned() else {
        return div().into_any_element();
    };
    state.sailing_state.breakdown_boat = route.boat;
    let context = state.sailing.clone();
    state.sailing_trees.loot.update(cx, |tree, cx| {
        tree.set_root(loot_tree(&route, &context), cx)
    });
    state.sailing_trees.speed.update(cx, |tree, cx| {
        tree.set_root(speed_tree(&route, &context), cx)
    });
    state
        .sailing_trees
        .artifact
        .update(cx, |tree, cx| tree.set_root(artifact_tree(&route), cx));
    state
        .sailing_trees
        .chest
        .update(cx, |tree, cx| tree.set_root(chest_tree(&route), cx));
    let loot = state.sailing_trees.loot.clone();
    let speed = state.sailing_trees.speed.clone();
    let artifact = state.sailing_trees.artifact.clone();
    let chest = state.sailing_trees.chest.clone();
    div()
        .id("breakdowns-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(controls(state, &plan.routes, &route, cx))
        .child(
            div()
                .id("sailing-breakdowns")
                .flex()
                .flex_wrap()
                .gap_3()
                .items_start()
                .child(panel(
                    "sailing-breakdown-loot",
                    "Loot value",
                    "Base loot × captain × Davey Jones × the loot multiplier",
                    &loot,
                ))
                .child(panel(
                    "sailing-breakdown-speed",
                    "Route speed and trip time",
                    "Raw speed against the minimum-time cap, then trips per hour",
                    &speed,
                ))
                .child(panel(
                    "sailing-breakdown-artifact",
                    "Artifact find",
                    "Chests per hour × the chance one chest finds an artifact",
                    &artifact,
                ))
                .child(panel(
                    "sailing-breakdown-chest",
                    "Chest opening",
                    "Tier chances × the value and payouts each tier opens",
                    &chest,
                )),
        )
        .into_any_element()
}

fn controls(
    state: &AppState,
    routes: &[RouteMetrics],
    route: &RouteMetrics,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let boats: Vec<Choice> = (0..state.fleet.boats.len())
        .map(|boat| {
            let captain = routes
                .iter()
                .find(|route| route.boat == boat)
                .and_then(|route| route.captain);
            let label = match captain {
                Some(captain) => format!("Boat {} · {}", boat + 1, captain_short(state, captain)),
                None => format!("Boat {} · no captain", boat + 1),
            };
            Choice::new(boat.to_string(), label)
        })
        .collect();
    let select = Select::new("breakdown-boat", boats)
        .label("Break down boat")
        .selected(route.boat.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(boat) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_sailing_breakdown_boat(boat, cx));
            }
        });
    Card::new()
        .header(CardHeader::new("Interactive formula breakdowns").description(
            "Click any row with an arrow to expand it, hover for the exact stacking details, or search the nodes",
        ))
        .child(
            div()
                .id("breakdowns-controls")
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
                        .min_w(px(240.0))
                        .child(Caption::new("Break down boat"))
                        .child(select),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Assignment"))
                        .child(
                            div()
                                .id("breakdowns-assignment")
                                .role(gpui::Role::Label)
                                .aria_label(format!(
                                    "Island {} · trip {}",
                                    route.island + 1,
                                    trip(route.timing.trip_seconds)
                                ))
                                .child(format!(
                                    "Island {} · trip {}",
                                    route.island + 1,
                                    trip(route.timing.trip_seconds)
                                )),
                        ),
                )
                .child(div().flex_1())
                .child(
                    Caption::new(
                        "Stateful near-full pile processing is excluded from every tree.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// One breakdown card around the shared formula tree.
fn panel(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    tree: &Entity<FormulaTree>,
) -> AnyElement {
    Card::new()
        .header(CardHeader::new(title).description(description))
        .child(
            div()
                .id(id)
                .h(px(360.0))
                .min_h_0()
                .flex_1()
                .child(tree.clone()),
        )
        .into_any_element()
}
