//! The Breakdowns tab: the loot, trip, artifact and chest formula trees.
//!
//! One selector picks the boat, and every tree reads that boat's route from the
//! active assignment. The trees themselves come from the domain crates, so the
//! searched and copied numbers are the ones the other tabs report. Their roots
//! are replaced only when the boat or the fleet changes.

use super::controls::cached;
use super::{captain_short, field, trip};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_sailing::{artifact_tree, chest_tree, loot_tree, speed_tree};
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice};

/// What the tab shows for one boat, read once per key and boat.
pub(crate) struct View {
    boat: usize,
    boats: Vec<Choice<usize>>,
    assignment: SharedString,
}

/// The breakdowns tab: the boat selector and the four trees.
pub(crate) fn breakdowns_tab(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = (state.sailing_key(), state.sailing_state.breakdown_boat);
    let view = cached(state, |controls| &mut controls.breakdowns, key, cx, build);
    let Some(view) = view.as_ref() else {
        return div().into_any_element();
    };
    let muted = cx.theme().muted_foreground;
    let select = &state.sailing_controls.breakdown_boat;
    select.show(view.boats.clone(), Some(&view.boat), window, cx);
    let controls = Card::new()
        .header(CardHeader::new("Interactive formula breakdowns").description(
            "Click any row with an arrow to expand it, hover for the exact stacking details, or search the nodes",
        ))
        .child(
            h_flex()
                .id("breakdowns-controls")
                .items_end()
                .gap_4()
                .child(field(
                    "Break down boat",
                    div().w(px(240.0)).child(
                        Select::new(select.state())
                            .id("breakdown-boat")
                            .accessibility_label("Break down boat")
                            .small(),
                    ),
                ))
                .child(field(
                    "Assignment",
                    div()
                        .id("breakdowns-assignment")
                        .role(gpui::Role::Label)
                        .aria_label(view.assignment.clone())
                        .child(view.assignment.clone()),
                ))
                .child(div().flex_1())
                .child(
                    Caption::new("Stateful near-full pile processing is excluded from every tree.")
                        .text_color(muted),
                ),
        );
    let trees = &state.sailing_trees;
    v_flex()
        .id("breakdowns-body")
        .gap_3()
        .child(controls)
        .child(
            div()
                .id("sailing-breakdowns")
                .grid()
                .grid_cols(2)
                .gap_3()
                .child(panel(
                    "sailing-breakdown-loot",
                    "Loot value",
                    "Base loot × captain × Davey Jones × the loot multiplier",
                    &trees.loot,
                ))
                .child(panel(
                    "sailing-breakdown-speed",
                    "Route speed and trip time",
                    "Raw speed against the minimum-time cap, then trips per hour",
                    &trees.speed,
                ))
                .child(panel(
                    "sailing-breakdown-artifact",
                    "Artifact find",
                    "Chests per hour × the chance one chest finds an artifact",
                    &trees.artifact,
                ))
                .child(panel(
                    "sailing-breakdown-chest",
                    "Chest opening",
                    "Tier chances × the value and payouts each tier opens",
                    &trees.chest,
                )),
        )
        .into_any_element()
}

/// Hand the four trees the selected boat's route, once.
fn build(state: &AppState, cx: &mut App) -> Option<View> {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let boat = state
        .sailing_state
        .breakdown_boat
        .min(state.fleet.boats.len().saturating_sub(1));
    let route = plan.routes.iter().find(|route| route.boat == boat)?;
    let context = &state.sailing;
    let trees = &state.sailing_trees;
    trees
        .loot
        .update(cx, |tree, cx| tree.set_root(loot_tree(route, context), cx));
    trees
        .speed
        .update(cx, |tree, cx| tree.set_root(speed_tree(route, context), cx));
    trees
        .artifact
        .update(cx, |tree, cx| tree.set_root(artifact_tree(route), cx));
    trees
        .chest
        .update(cx, |tree, cx| tree.set_root(chest_tree(route), cx));
    let boats = (0..state.fleet.boats.len())
        .map(|boat| {
            let captain = plan
                .routes
                .iter()
                .find(|route| route.boat == boat)
                .and_then(|route| route.captain);
            let label = match captain {
                Some(captain) => format!("Boat {} · {}", boat + 1, captain_short(state, captain)),
                None => format!("Boat {} · no captain", boat + 1),
            };
            Choice::new(boat, label)
        })
        .collect();
    Some(View {
        boat,
        boats,
        assignment: format!(
            "Island {} · trip {}",
            route.island + 1,
            trip(route.timing.trip_seconds)
        )
        .into(),
    })
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
        .child(div().id(id).h(px(360.0)).min_h_0().child(tree.clone()))
        .into_any_element()
}
