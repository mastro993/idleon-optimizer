//! The Royal Guardian Calculator: the upstream Overview, Outposts, Resources
//! and Currencies tabs.
//!
//! Ported from `royal-guardian-calc.html`,
//! `js/stats/systems/w7/royal-guardian.js` and
//! `js/stats/systems/w7/royal-guardian-optimizer.js`. RG-01 builds the overview:
//! the counters, the world progress cards, the raw clearing values every outpost
//! reports and the range target behind an outpost's Advanced Logistics plan.
//! RG-02 adds the outpost map, the resource nodes, the currencies, the drain
//! staffing and the banked hours. The upgrades arrive with RG-03 and the
//! optimizer with RG-04, so the strip lists only the panes behind it.

mod actions;

mod currencies;
mod outposts;
mod overview;
mod resources;

#[cfg(test)]
mod tests;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div};
use idleon_royal_guardian as royal_guardian;

/// The Royal Guardian tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// The counters, world progress, clearing values and range targets.
    #[default]
    Overview,
    /// The outpost map and every outpost the selected world lists.
    Outposts,
    /// The resource nodes, the drain staffing and the banked hours.
    Resources,
    /// The currency balances and the nodes that pay them.
    Currencies,
}

impl Tab {
    pub(crate) const ALL: [Tab; 4] = [
        Tab::Overview,
        Tab::Outposts,
        Tab::Resources,
        Tab::Currencies,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Outposts => "outposts",
            Tab::Resources => "resources",
            Tab::Currencies => "currencies",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Outposts => "Outposts",
            Tab::Resources => "Resources",
            Tab::Currencies => "Currencies",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section's own state, kept across dashboard visits.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// The pane the strip shows.
    pub(crate) tab: Tab,
    /// The world the outpost tables list.
    pub(crate) world: usize,
    /// The built outpost whose next range target the panel explains.
    pub(crate) outpost: Option<usize>,
    /// The resource node the resources pane explains.
    pub(crate) resource: Option<usize>,
    /// The currency slot the currencies pane explains.
    pub(crate) currency: Option<i64>,
}

/// The Royal Guardian section: the tab strip and its pane, or the import prompt
/// when the loaded save carries no Royal Guardian rows.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if state.royal.is_none() {
        return prompt(cx);
    }
    let tab = state.royal_state.tab;
    let entity = cx.entity();
    let mut tabs =
        TabBar::new("royal-tabs").selected(SharedString::from(format!("royal-tab-{}", tab.id())));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("royal-tab-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let tab = id
            .as_ref()
            .strip_prefix("royal-tab-")
            .and_then(Tab::from_id);
        if let Some(tab) = tab {
            entity.update(cx, |state, cx| state.set_royal_tab(tab, cx));
        }
    });
    let header = div()
        .id("royal-tab-strip")
        .flex()
        .items_center()
        .px_3()
        .pt_2()
        .child(tabs);
    let pane = match tab {
        Tab::Overview => overview::overview(state, cx),
        Tab::Outposts => outposts::outposts(state, cx),
        Tab::Resources => resources::resources(state, cx),
        Tab::Currencies => currencies::currencies(state, cx),
    };
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(header)
        .child(
            ScrollArea::new("royal-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(pane),
        )
        .into_any_element()
}

/// The world rail every pane shares: which world its tables list.
pub(super) fn world_rail(world: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("royal-worlds-tabs").selected(format!("royal-world-{world}"));
    for index in 0..royal_guardian::WORLDS {
        tabs = tabs.tab(WindowTab::new(
            format!("royal-world-{index}"),
            format!("W{}", index + 1),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        if let Some(world) = id
            .strip_prefix("royal-world-")
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            entity.update(cx, |state, cx| state.set_royal_world(world, cx));
        }
    });
    div()
        .id("royal-world-rail")
        .flex()
        .items_center()
        .gap_2()
        .child(tabs)
        .into_any_element()
}

/// A save without `RoyalG` has no outposts: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("royal-empty", IconName::Map, "No Royal Guardian state")
                .body(
                    "The Royal Guardian calculator reads the outposts from a save. Import one to \
                     see the counters, the world progress, the raw clearing values and the range \
                     targets.",
                )
                .action(
                    Button::new("royal-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
