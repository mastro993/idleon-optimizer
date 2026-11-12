//! The Sneaking Optimizer: the upstream loadout and inventory workspace.
//!
//! Ported from `sneaking-calc.html` and `js/stats/systems/w6/sneaking-math.js`.
//! SN-01 builds the loadout half: the scenario strip, the saved floor
//! assignments, the per-twin loadout cards with their four slots and the 39-slot
//! inventory, all backed by the save's `Ninja` rows. The formula breakdowns
//! (SN-02), the placement optimizer (SN-03) and the floor × mastery table with
//! the door ETA (SN-04) arrive next, so the page renders its one built workspace
//! without a tab strip until then, as the fountain page does.
//!
//! The numbers a loadout card shows are the save-backed totals: the charm bonus
//! map with its symbol and Gold terms, action speed, KO recovery, the level,
//! charm, account, ally and flower stealth factors and the detection chance they
//! give. The Breakdowns workspace splits the same stack into the five upstream
//! formula trees.

mod breakdowns;
mod inventory;
mod loadout;
mod state;

pub(crate) use state::{State, Workspace};

use crate::state::AppState;
use crate::ui::section;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};

/// The sneaking section: the loadout or breakdowns workspace, or the import prompt.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(loadout) = &state.sneaking else {
        return prompt(state, cx);
    };
    let workspace = state.sneaking_state.workspace;
    let entity = cx.entity();
    let mut tabs = TabBar::new("sneaking-workspace").selected(SharedString::from(format!(
        "sneaking-tab-{}",
        workspace.id()
    )));
    for entry in Workspace::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("sneaking-tab-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let id = id.as_ref();
        if let Some(workspace) = id
            .strip_prefix("sneaking-tab-")
            .and_then(Workspace::from_id)
        {
            entity.update(cx, |state, cx| state.sneak_workspace(workspace, cx));
        }
    });

    let players = loadout.players();
    let body = match workspace {
        Workspace::Loadout => ScrollArea::new("sneaking-pane")
            .vertical()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .child(loadout::scenario(state, players, cx))
                    .child(loadout::saved_floors(state, loadout, cx))
                    .child(loadout::loadout_card(state, loadout, cx))
                    .child(inventory::card(state, loadout, cx)),
            )
            .into_any_element(),
        Workspace::Breakdowns => ScrollArea::new("sneaking-breakdown-pane")
            .vertical()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("sneaking-breakdown-pane")
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(breakdowns::pane(state, cx)),
            )
            .into_any_element(),
    };
    let header = div()
        .id("sneaking-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .px_4()
        .pt_4()
        .child(tabs)
        .child(div().flex_1());
    let page = div()
        .id("sneaking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .when(state.save.is_none(), |page| {
            page.child(section::no_save(cx))
        })
        .child(header)
        .child(body);
    page.into_any_element()
}

/// A save without `Ninja` rows has no loadout: say so and offer the import.
fn prompt(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let body = if state.save.is_some() {
        "The loaded save carries no Ninja rows, so there is no loadout, inventory or floor \
         assignment to read. Import a save from the Sneaking screen to edit its charms, weapons and \
         hats."
    } else {
        "The sneaking optimizer reads the ninja twins, their charms and the inventory from a save. \
         Import one to edit the loadout, see each twin's stealth, attempt and detection, and check \
         the saved floor assignments."
    };
    div()
        .id("sneaking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("sneaking-empty", IconName::Map, "No sneaking state")
                .body(body)
                .action(
                    Button::new("sneaking-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
