//! The Clamworks section: the overview, the upgrade tables, the AFK claim, the
//! plan and the promotion.
//!
//! Summary, pearl value, the upgrade priority, the claim, the budgeted plan, the
//! promotion tracker and the Scurge panels are ports of the upstream
//! `clamworks-calc.html` tabs, `js/stats/systems/w7/clamworks.js` and
//! `js/stats/systems/w7/clamworks-scurge.js`. The save-derived inputs (pearl
//! balance, upgrade and promotion levels, Scurge level and upgrades) come from the
//! shared import; the multikill total, the plan's budget, and the claim's kills
//! per hour and drop rate are manual inputs until the combat simulator runs Clam
//! Work, exactly like the upstream's own supplement fields.

mod actions;

pub(crate) mod claim;
pub(crate) mod compare;
pub(crate) mod optimizer;
pub(crate) mod overview;
pub(crate) mod promotion;
pub(crate) mod scurge;
pub(crate) mod state;
pub(crate) mod upgrades;

pub(crate) use state::State;

use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};
use idleon_ui::format;

/// The Clamworks tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// Summary, rates and the pearl value breakdown.
    #[default]
    Overview,
    /// Every upgrade, in the game's order.
    Upgrades,
    /// One AFK claim and the `+1` comparison.
    Claim,
    /// The budgeted plan and the upstream priority ranking.
    Optimizer,
    /// The promotion tracker and the worker-class rewards.
    Promotion,
    /// The Self Affliction comparison and the Scurge upgrade allocation.
    Scurge,
}

impl Tab {
    pub(crate) const ALL: [Tab; 6] = [
        Tab::Overview,
        Tab::Upgrades,
        Tab::Claim,
        Tab::Optimizer,
        Tab::Promotion,
        Tab::Scurge,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Upgrades => "upgrades",
            Tab::Claim => "claim",
            Tab::Optimizer => "optimizer",
            Tab::Promotion => "promotion",
            Tab::Scurge => "scurge",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Upgrades => "Upgrades",
            Tab::Claim => "AFK claim",
            Tab::Optimizer => "Optimizer",
            Tab::Promotion => "Promotion",
            Tab::Scurge => "Scurge",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The Clamworks section: the tab strip, the pearl-value summary and the pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tab = state.clamworks_tab;
    let clamworks = state.clamworks_view();
    let projected = state.clamworks_projection.is_some();
    let value = clamworks.pearl_value(state.clamworks_multikill);
    let colors = {
        let palette = &cx.theme().colors;
        (palette.accent, palette.fg_muted)
    };
    let summary = format!(
        "×{value:.2} per pearl · {} pearls",
        format::rate(clamworks.pearl_balance)
    );
    let entity = cx.entity();
    let mut tabs = TabBar::new("clamworks-tabs").selected(SharedString::from(tab.id()));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(entry.id(), entry.label()));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
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
                    Caption::new(format!("{} pearls", format::rate(clamworks.pearl_balance)))
                        .text_color(colors.1),
                ),
        );

    let mut section = div()
        .id("clamworks-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header);
    if projected {
        section = section.child(projection_banner(cx));
    }
    section
        .child(match tab {
            Tab::Overview => overview::overview_pane(state, window, cx),
            Tab::Upgrades => upgrades::upgrades_pane(state, window, cx),
            Tab::Claim => claim::claim_pane(state, window, cx),
            Tab::Optimizer => optimizer::optimizer_pane(state, window, cx),
            Tab::Promotion => promotion::promotion_pane(state, window, cx),
            Tab::Scurge => scurge::scurge_pane(state, window, cx),
        })
        .into_any_element()
}

/// The bar that says the panes read the applied plan's levels, not the save's.
fn projection_banner(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Banner::new(
        "clamworks-projection",
        Severity::Info,
        "Showing the applied plan's upgrade levels. The saved levels are unchanged.",
    )
    .action("Show saved levels", move |_window, cx| {
        entity.update(cx, |state, cx| {
            state.show_saved_clamworks_levels(cx);
        });
    })
    .into_any_element()
}

/// One summary card: label, figure and a line of context.
pub(super) fn kpi(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    suffix: Option<&'static str>,
    caption: &str,
) -> AnyElement {
    let mut statistic = Statistic::new(id, label, value)
        .decimals(decimals)
        .size(TextSize::Xl);
    if let Some(suffix) = suffix {
        statistic = statistic.suffix(suffix);
    }
    KpiCard::new(statistic).caption(caption).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_ids_match_the_strip() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
        assert_eq!(Tab::from_id("scurge"), Some(Tab::Scurge));
        assert_eq!(Tab::from_id("combat"), None);
    }
}
