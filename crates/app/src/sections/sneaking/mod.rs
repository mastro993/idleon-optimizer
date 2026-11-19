//! The Sneaking Optimizer: the loadout and inventory workspace.
//!
//! SN-01 builds the loadout half: the scenario strip, the saved floor
//! assignments, the per-twin loadout cards with their four slots and the
//! 39-slot inventory, all backed by the save's `Ninja` rows. SN-02 adds the
//! formula breakdowns, SN-03 the charm placement search and SN-04 the floor ×
//! mastery table with the active door, the next captive and their ETAs.
//!
//! The numbers a loadout card shows are the save-backed totals: the charm bonus
//! map with its symbol and Gold terms, action speed, KO recovery, the level,
//! charm, account, ally and flower stealth factors and the detection chance
//! they give. The Breakdowns workspace splits the same stack into the five
//! formula trees.

mod breakdowns;
mod controls;
mod floors;
mod inventory;
mod loadout;
mod optimizer;
mod state;

pub(crate) use controls::Controls;
pub(crate) use state::{State, Workspace};

use crate::state::AppState;
use crate::ui::layout::tab_bar;
use crate::ui::section;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;

/// The sneaking section: the loadout or breakdowns workspace, or the import prompt.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(loadout) = state.sneaking.as_ref() else {
        return prompt(state, cx);
    };
    let workspace = state.sneaking_state.workspace;
    let entity = cx.entity();
    let selected = Workspace::ALL
        .iter()
        .position(|entry| *entry == workspace)
        .unwrap_or_default();
    let tabs = tab_bar(
        "sneaking-workspace",
        Workspace::ALL.map(|entry| {
            (
                format!("sneaking-tab-{}", entry.id()).into(),
                entry.label().into(),
            )
        }),
        selected,
        move |index, _window, cx| {
            entity.update(cx, |state, cx| {
                state.sneak_workspace(Workspace::ALL[index], cx)
            });
        },
    );

    let (id, body) = match workspace {
        Workspace::Loadout => {
            let key = (
                state.save_revision,
                state.sneaking_controls.edits,
                state.sneaking_state.clone(),
            );
            let figures = state.sneaking_controls.figures(key, loadout);
            (
                "sneaking-pane",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .py_4()
                    .child(loadout::scenario(state, loadout, &figures, window, cx))
                    .child(loadout::saved_floors(state, loadout, &figures, cx))
                    .child(loadout::loadout_card(state, loadout, &figures, cx))
                    .child(inventory::card(state, loadout, &figures, cx))
                    .into_any_element(),
            )
        }
        Workspace::Optimizer => (
            "sneaking-optimizer-pane",
            pane(
                "sneaking-optimizer-pane",
                optimizer::pane(state, window, cx),
            ),
        ),
        Workspace::Floors => {
            let key = (state.save_revision, state.sneaking_controls.edits);
            let figures = state.sneaking_controls.floors(key, loadout);
            (
                "sneaking-floors-pane",
                pane("sneaking-floors-pane", floors::pane(state, &figures, cx)),
            )
        }
        Workspace::Breakdowns => {
            let players = loadout.players();
            let key = (
                state.save_revision,
                state.sneaking_controls.edits,
                state.sneaking_state.breakdown_twin(players),
                state.sneaking_state.formula_tab,
            );
            if let Some(root) = state.sneaking_controls.breakdown_root(key, loadout) {
                state
                    .sneaking_breakdown
                    .update(cx, |tree, cx| tree.set_root(root, cx));
            }
            let figures = state.sneaking_controls.figures(
                (
                    state.save_revision,
                    state.sneaking_controls.edits,
                    state.sneaking_state.clone(),
                ),
                loadout,
            );
            (
                "sneaking-breakdown-pane",
                pane(
                    "sneaking-breakdown-pane",
                    breakdowns::pane(state, loadout, &figures, window, cx),
                ),
            )
        }
    };
    let body = div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id(id);
    let header = div()
        .id("sneaking-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .pt_4()
        .child(tabs)
        .child(div().flex_1());
    div()
        .id("sneaking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .when(state.save.is_none(), |page| {
            page.child(section::no_save(cx))
        })
        .child(header)
        .child(body)
        .into_any_element()
}

/// A workspace's column inside its scroll area, under the old inner id.
fn pane(id: &'static str, content: AnyElement) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_3()
        .py_4()
        .child(content)
        .into_any_element()
}

/// A save without `Ninja` rows has no loadout: say so and offer the import.
fn prompt(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
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
            div().id("sneaking-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No sneaking state"))
                            .description(EmptyDescription::new().child(body)),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("sneaking-import")
                                .label("Import save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    import.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}
