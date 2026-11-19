//! The Clamworks section: the overview, the upgrade tables, the AFK claim, the
//! plan and the promotion.
//!
//! The tabs cover the summary, pearl value, the upgrade priority, the claim,
//! the budgeted plan, the promotion tracker and the Scurge panels. The
//! save-derived inputs (pearl balance, upgrade and promotion levels, Scurge
//! level and upgrades) come from the shared import; the multikill total, the
//! plan's budget, and the claim's kills per hour and drop rate are manual
//! inputs until the combat simulator runs Clam Work.

mod actions;

pub(crate) mod claim;
pub(crate) mod compare;
mod controls;
pub(crate) mod optimizer;
pub(crate) mod overview;
pub(crate) mod promotion;
pub(crate) mod scurge;
pub(crate) mod state;
mod table;
pub(crate) mod upgrades;

pub(crate) use controls::Controls;
pub(crate) use state::State;

use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, PANEL_GAP, Statistic, Tone};

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
    /// The budgeted plan and the priority ranking.
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
}

/// The Clamworks section: the tab strip, the pearl-value summary and the pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tab = state.clamworks_tab;
    let clamworks = state.clamworks_shown();
    let projected = state.clamworks_projection.is_some();
    let value = clamworks.pearl_value(state.clamworks_multikill);
    let (accent, muted) = (cx.theme().primary, cx.theme().muted_foreground);
    let summary = format!(
        "×{value:.2} per pearl · {} pearls",
        format::rate(clamworks.pearl_balance)
    );
    let entity = cx.entity();
    let tabs = layout::tab_bar(
        "clamworks-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        Tab::ALL.iter().position(|entry| *entry == tab).unwrap_or(0),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_clamworks_tab(Tab::ALL[index], cx));
        },
    );
    let header = h_flex()
        .id("clamworks-header")
        .flex_none()
        .gap_3()
        .child(tabs.flex_1())
        .child(
            h_flex()
                .id("clamworks-summary")
                .role(gpui::Role::Label)
                .aria_label(summary)
                .flex_none()
                .gap_2()
                .child(Caption::new(format!("×{value:.2} per pearl")).text_color(accent))
                .child(
                    Caption::new(format!("{} pearls", format::rate(clamworks.pearl_balance)))
                        .text_color(muted),
                ),
        );

    let pane = match tab {
        Tab::Overview => overview::overview_pane(state, window, cx),
        Tab::Upgrades => upgrades::upgrades_pane(state, cx),
        Tab::Claim => claim::claim_pane(state, window, cx),
        Tab::Optimizer => optimizer::optimizer_pane(state, window, cx),
        Tab::Promotion => promotion::promotion_pane(state, cx),
        Tab::Scurge => scurge::scurge_pane(state, window, cx),
    };
    v_flex()
        .id("clamworks-section")
        .size_full()
        .min_h_0()
        .gap(PANEL_GAP)
        .child(header)
        .when(projected, |section| section.child(projection_banner(cx)))
        .child(pane)
        .into_any_element()
}

/// A tab's body in its own vertical scroll area.
pub(super) fn pane(id: &'static str, body: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id(id)
        .into_any_element()
}

/// The bar that says the panes read the applied plan's levels, not the save's.
fn projection_banner(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    h_flex()
        .flex_none()
        .gap_2()
        .child(
            div().flex_1().min_w_0().child(
                Alert::new(
                    "clamworks-projection",
                    "Showing the applied plan's upgrade levels. The saved levels are unchanged.",
                )
                .with_variant(Tone::Info.into())
                .banner(),
            ),
        )
        .child(
            Button::new("clamworks-projection-saved")
                .label("Show saved levels")
                .small()
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.show_saved_clamworks_levels(cx));
                }),
        )
        .into_any_element()
}

/// One summary figure: label, value and a line of context, for a `KpiStrip`.
pub(super) fn kpi(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    suffix: Option<&'static str>,
    caption: &str,
) -> Statistic {
    let shown = if decimals == 0 {
        format::count(value.round().max(0.0) as u64)
    } else {
        format!("{value:.decimals$}")
    };
    let statistic = Statistic::new(id, label, shown).caption(caption.to_string());
    match suffix {
        Some(suffix) => statistic.suffix(suffix),
        None => statistic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_ids_match_the_strip() {
        assert_eq!(
            Tab::ALL.map(Tab::id),
            [
                "overview",
                "upgrades",
                "claim",
                "optimizer",
                "promotion",
                "scurge"
            ]
        );
    }
}
