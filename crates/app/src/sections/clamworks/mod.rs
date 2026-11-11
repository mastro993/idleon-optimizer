//! The Clamworks section: the overview and the upgrade tables.
//!
//! Summary, pearl value and the upgrade priority are ports of the upstream
//! `clamworks-calc.html` tabs and `js/stats/systems/w7/clamworks.js`. The two
//! save-derived inputs (pearl balance and upgrade levels) come from the shared
//! import; the multikill total is a manual input until the combat simulator
//! lands, exactly like the upstream's own supplement fields.

pub mod overview;
pub mod upgrades;

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};

/// The Clamworks tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// Summary, rates and the pearl value breakdown.
    #[default]
    Overview,
    /// Every upgrade and the priority ranking.
    Upgrades,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Overview, Tab::Upgrades];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Upgrades => "upgrades",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Upgrades => "Upgrades",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The Clamworks section: the tab strip, the pearl-value summary and the pane.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let tab = state.clamworks_tab;
    let value = state.clamworks.pearl_value(state.clamworks_multikill);
    let colors = {
        let palette = &cx.theme().colors;
        (palette.accent, palette.fg_muted)
    };
    let summary = format!(
        "×{value:.2} per pearl · {} pearls",
        format::rate(state.clamworks.pearl_balance)
    );
    let entity = cx.entity();
    let tabs = TabBar::new("clamworks-tabs")
        .selected(SharedString::from(tab.id()))
        .tab(WindowTab::new(Tab::Overview.id(), Tab::Overview.label()))
        .tab(WindowTab::new(Tab::Upgrades.id(), Tab::Upgrades.label()))
        .on_select(move |id, _window, cx| {
            if let Some(tab) = Tab::from_id(id.as_ref()) {
                entity.update(cx, |state, cx| state.set_clamworks_tab(tab, cx));
            }
        });
    let header = div()
        .id("clamworks-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(div().flex_1())
        .child(
            div()
                .id("clamworks-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new(format!("×{value:.2} per pearl")).text_color(colors.0))
                .child(
                    Caption::new(format!(
                        "{} pearls",
                        format::rate(state.clamworks.pearl_balance)
                    ))
                    .text_color(colors.1),
                ),
        );

    div()
        .id("clamworks-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(match tab {
            Tab::Overview => overview::overview_pane(state, window, cx),
            Tab::Upgrades => upgrades::upgrades_pane(state, window, cx),
        })
        .into_any_element()
}
