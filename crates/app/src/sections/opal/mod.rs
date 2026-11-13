//! The Opal Optimizer (W5): the current villagers, their EXP rates, the account
//! bonuses behind them, the distribution optimizer and the manual simulator.
//!
//! `Current state` is the upstream page's first tab: the villager overview and
//! table, the current opal state and the shared formula tree, with a villager
//! picker that switches the tree between the total and one villager. `Optimizer`
//! splits a total across the unlocked villagers for the selected target, and
//! `Manual simulator` reads a hand-typed split against the save.
//!
//! Every save-derived figure comes from the import; the section keeps only its
//! visible selections.

mod actions;

mod bonuses;
mod optimizer;
mod overview;
mod simulate;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use idleon_opal::Opal;
use idleon_ui::formula::FormulaTree;

/// The largest opal count a manual or planned input takes; a save above it
/// raises the ceiling.
const MAX_OPALS: f64 = 1_000_000.0;

/// The Opal Optimizer tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// The villagers, their rates, the bonus terms and the formula tree.
    #[default]
    Current,
    /// The distribution optimizer.
    Optimizer,
    /// A hand-typed distribution read against the save.
    Manual,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Current, Tab::Optimizer, Tab::Manual];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Current => "opal-tab-current",
            Tab::Optimizer => "opal-tab-optimizer",
            Tab::Manual => "opal-tab-manual",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Current => "Current state",
            Tab::Optimizer => "Optimizer",
            Tab::Manual => "Manual simulator",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section: the tab strip and the selected pane under it.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.clone() else {
        return prompt(cx);
    };
    let tab = state.opal_tab;
    let tree = state.opal_breakdown.clone();
    tree.update(cx, |tree, cx| {
        tree.set_root(opal.breakdown(state.opal_selected), cx)
    });
    let entity = cx.entity();
    let mut tabs = TabBar::new("opal-tabs").selected(SharedString::from(tab.id()));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(entry.id(), entry.label()));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_opal_tab(tab, cx));
        }
    });
    div()
        .id("opal-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id("opal-header")
                .flex()
                .flex_none()
                .items_center()
                .gap_3()
                .px_3()
                .pt_2()
                .child(tabs),
        )
        .child(match tab {
            Tab::Current => current_pane(state, &opal, tree, cx),
            Tab::Optimizer => optimizer::optimizer_pane(state, window, cx),
            Tab::Manual => simulate::pane(state, window, cx),
        })
        .into_any_element()
}

/// The current state pane: the overview, the villagers, the terms and the tree.
fn current_pane(
    state: &AppState,
    opal: &Opal,
    tree: Entity<FormulaTree>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    ScrollArea::new("opal-pane")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("opal-overview")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(overview::current_state(opal, cx))
                .child(overview::villagers(state, opal, cx))
                .child(bonuses::terms(opal, cx))
                .child(bonuses::formula(state, tree)),
        )
        .into_any_element()
}

/// A save without `Holes` has no villagers: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("opal-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("opal-empty", IconName::Map, "No villager state")
                .body(
                    "The opal optimizer reads the five villagers, their levels and their invested \
                     opals from a save. Import one to see the EXP rates and every bonus term.",
                )
                .action(
                    Button::new("opal-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_ids_match_the_strip() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
        assert_eq!(Tab::from_id("optimizer"), None);
    }
}
