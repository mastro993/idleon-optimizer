//! The Cglunko section: the save-derived overview, the upgrade list and the
//! upgrade plan.
//!
//! CL-01 builds the Overview tab of `cglunko-calc.html`; CL-02 adds the
//! Upgrades and Optimizer tabs with a page selector that drives both. The
//! stone focus (CL-03) arrives later and adds its own tab to a strip that never
//! offers a page with nothing behind it, the way the Jar calculator grew.

mod optimizer;
mod overview;
mod upgrades;

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{ToggleGroup, ToggleItem};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};
use idleon_core::cglunko::Page;

/// The Cglunko tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// Summary, the DR cards and the formula tree.
    #[default]
    Overview,
    /// Every upgrade on the selected page, in the game's order.
    Upgrades,
    /// The ranked priority and the plan the saved stones buy.
    Optimizer,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Overview, Tab::Upgrades, Tab::Optimizer];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Upgrades => "upgrades",
            Tab::Optimizer => "optimizer",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Upgrades => "Upgrades",
            Tab::Optimizer => "Optimizer",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section: the tab strip, the page selector, the summary and the pane.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let tab = state.cglunko_tab;
    let page = state.cglunko_page;
    let cglunko = state.cglunko.clone();
    let summary = format!(
        "DR ×{:.2} · {} blue digits · {} purple digits · {} Glunko kills · normal DR {}",
        cglunko.dr(),
        cglunko.blue_digits(),
        cglunko.purple_digits(),
        format::fmt_val(cglunko.kills),
        format::fmt_val(cglunko.normal_dr)
    );
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let mut tabs = TabBar::new("cglunko-tabs").selected(SharedString::from(tab.id()));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(entry.id(), entry.label()));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_cglunko_tab(tab, cx));
        }
    });
    let page_entity = cx.entity();
    let pages = Page::ALL.into_iter().fold(
        ToggleGroup::new("cglunko-page").size(ControlSize::Sm),
        |control, option| control.item(ToggleItem::new(option.id()).label(option.label())),
    );
    let pages = pages
        .selected([page.id()])
        .on_change(move |selected, _window, cx| {
            let Some(page) = selected
                .first()
                .and_then(|value| Page::from_id(value.as_ref()))
            else {
                return;
            };
            page_entity.update(cx, |state, cx| state.set_cglunko_page(page, cx));
        });
    let header = div()
        .id("cglunko-header")
        .flex()
        .flex_wrap()
        .items_center()
        .px_3()
        .gap_3()
        .child(tabs)
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new("Page").text_color(muted))
                .child(pages),
        );
    // The summary carries the kills and normal drop rate the DR formula reads,
    // so it gets its own line and stays readable at narrow widths.
    let summary_line = div()
        .id("cglunko-summary")
        .role(gpui::Role::Label)
        .aria_label(summary.clone())
        .px_3()
        .min_w_0()
        .child(Caption::new(summary).text_color(muted));

    let mut section = div()
        .id("cglunko-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(summary_line);
    // The overview is the save alone, so a save without the Cove rows can only
    // read zero; say why instead of pretending the account is fresh.
    if state.save_loaded && !state.cglunko.saved {
        section = section.child(Banner::new(
            "cglunko-missing",
            Severity::Warning,
            "This save carries no Cglunko data: OptLacc[630..669] is empty, so every value below reads zero.",
        ));
    }
    section
        .child(match tab {
            Tab::Overview => overview::overview_pane(state, window, cx),
            Tab::Upgrades => upgrades::upgrades_pane(state, window, cx),
            Tab::Optimizer => optimizer::optimizer_pane(state, window, cx),
        })
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
        assert_eq!(Tab::from_id("stones"), None);
    }
}
