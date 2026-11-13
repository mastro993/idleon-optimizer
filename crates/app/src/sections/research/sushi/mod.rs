//! The Sushi Station tab: the station overview, the upgrade table, the
//! currency breakdown, the best-upgrade ranking and the sushi tier table.
//!
//! The formulas and the save decode live in `idleon-sushi`; this module owns
//! the tab state the window keeps and renders the upstream subtabs.

mod best;
mod currency;
mod overview;
mod tiers;
mod upgrades;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::theme::ControlSize;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use idleon_research::model::Simulation;
use idleon_save::Save;
use idleon_sushi::{Inputs, inputs};

/// The tab's own subtabs, in the upstream order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    #[default]
    Overview,
    Upgrades,
    Currency,
    Best,
    Sushi,
}

impl Tab {
    pub(crate) const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Upgrades,
        Tab::Currency,
        Tab::Best,
        Tab::Sushi,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "su-overview",
            Tab::Upgrades => "su-upgrades",
            Tab::Currency => "su-currency",
            Tab::Best => "su-best-upg",
            Tab::Sushi => "su-sushi",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Upgrades => "Upgrades",
            Tab::Currency => "Currency/hr",
            Tab::Best => "Best Upgrade",
            Tab::Sushi => "Sushi",
        }
    }
}

/// The Sushi Station tab's state, kept while the window is open.
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The save-derived inputs, or `None` before a save loads.
    pub(crate) inputs: Option<Inputs>,
    pub(crate) warnings: Vec<String>,
    /// Whether the upgrade table shows the upgrades past the next unlock.
    pub(crate) show_spoiler_upgrades: bool,
    /// Whether the sushi table shows the tiers past the next Ring of Glory.
    pub(crate) show_spoiler_rog: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            tab: Tab::default(),
            inputs: None,
            warnings: Vec::new(),
            show_spoiler_upgrades: false,
            show_spoiler_rog: false,
        }
    }

    /// Decode the tab's inputs from a newly applied save.
    pub(crate) fn load(&mut self, save: Option<&Save>, simulation: &Simulation) {
        match save {
            Some(save) => {
                let (decoded, warnings) = inputs(save, simulation);
                self.inputs = Some(decoded);
                self.warnings = warnings;
            }
            None => {
                self.inputs = None;
                self.warnings.clear();
            }
        }
    }
}

/// The Sushi Station pane: the subtab strip and the active subtab.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tab = state.research.sushi.tab;
    let mut strip = div().id("sushi-tabs").flex().items_center().gap_1();
    for candidate in Tab::ALL {
        let entity = cx.entity();
        let mut button = Button::new(candidate.id(), candidate.label()).size(ControlSize::Sm);
        if candidate == tab {
            button = button.primary();
        }
        strip = strip.child(button.on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.research.sushi.tab = candidate;
                cx.notify();
            });
        }));
    }

    div()
        .id("sushi-tab")
        .flex()
        .flex_col()
        .gap_3()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id("sushi-header")
                .flex()
                .items_center()
                .gap_3()
                .child(strip),
        )
        .child(match tab {
            Tab::Overview => overview::pane(state, window, cx),
            Tab::Upgrades => upgrades::pane(state, cx),
            Tab::Currency => currency::pane(state, window, cx),
            Tab::Best => best::pane(state, cx),
            Tab::Sushi => tiers::pane(state, cx),
        })
        .into_any_element()
}

/// A rate as the cards print it: scaled with its suffix, two decimals.
pub(crate) fn rate_text(value: f64) -> String {
    let (scaled, suffix) = idleon_ui::format::scale(value);
    match suffix {
        "" => format!("{scaled:.0}"),
        "K" => format!("{scaled:.1}{suffix}"),
        _ => format!("{scaled:.2}{suffix}"),
    }
}
