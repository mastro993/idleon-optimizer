//! The Jar Calculator section: the upstream Overview and Production tabs.
//!
//! The upstream `jar-calc.html` has nine tabs. JR-01 builds the first two; the
//! rest arrive with JR-02 to JR-04 and are added to [`Tab::ALL`] then, so the
//! strip never offers a tab that has nothing behind it.

mod overview;
mod production;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div};

/// The jar page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// The jar summary, rupie balances and the cavern's status.
    #[default]
    Overview,
    /// The shared production rate, its multipliers and every slot.
    Production,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Overview, Tab::Production];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Production => "production",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Production => "Production",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The jar section: the tab strip over a scrolling pane, or the import prompt
/// when the loaded save carries no cavern.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    if state.jar.is_none() {
        return prompt(cx);
    }
    let tab = state.jar_tab;
    div()
        .id("jar-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(tabs(state, cx))
        .child(
            ScrollArea::new("jar-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(match tab {
                    Tab::Overview => overview::overview(state, cx),
                    Tab::Production => production::production(state, cx),
                }),
        )
        .into_any_element()
}

/// The upstream tab strip, driven by the state's own tab.
fn tabs(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("jar-tabs").selected(SharedString::from(state.jar_tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_jar_tab(tab, cx));
        }
    })
}

/// A save without `Holes` has no jar state: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("jar-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("jar-empty", IconName::Map, "No jar state")
                .body(
                    "The jar calculator reads the cavern from a save. Import one to see the jar \
                     types, rupie balances and production slots.",
                )
                .action(
                    Button::new("jar-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
