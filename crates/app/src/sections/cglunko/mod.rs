//! The Cglunko section: the save-derived overview, the upgrade list, the upgrade
//! plan and the stone focus.
//!
//! CL-01 builds the Overview tab; CL-02 adds the Upgrades and Optimizer tabs
//! with a page selector that drives both; CL-03 adds the Stones tab, whose blue
//! and purple pages the same selector picks.
//!
//! The section draws with GPUI Kit; what the tabs list from the save is worked
//! out in [`figures`] when the save or the page changes, never per frame.

mod actions;

mod figures;
mod optimizer;
mod overview;
mod stones;
mod table;
mod upgrades;

pub(crate) use figures::Figures;

use crate::state::AppState;
use crate::ui::layout::tab_bar;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};
use gpui_kit::component::tag::Tag;
use idleon_cglunko::Page;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Tone};

/// The Cglunko tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// Summary, the DR cards and the formula tree.
    #[default]
    Overview,
    /// Every upgrade on the selected page, in the game's order.
    Upgrades,
    /// The ranked priority and the plan the saved stones buy.
    Optimizer,
    /// The selected page's stones, their digit impact and the focus target.
    Stones,
}

impl Tab {
    pub(crate) const ALL: [Tab; 4] = [Tab::Overview, Tab::Upgrades, Tab::Optimizer, Tab::Stones];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Upgrades => "upgrades",
            Tab::Optimizer => "optimizer",
            Tab::Stones => "stones",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Upgrades => "Upgrades",
            Tab::Optimizer => "Optimizer",
            Tab::Stones => "Stones",
        }
    }
}

/// The section: the tab strip, the page selector, the summary and the pane.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.cglunko_figures.sync(
        state.save_revision,
        &state.cglunko,
        state.cglunko_page,
        &state.cglunko_breakdown,
        cx,
    );
    let tab = state.cglunko_tab;
    let page = state.cglunko_page;
    let cglunko = &state.cglunko;
    let summary = format!(
        "DR ×{:.2} · {} blue digits · {} purple digits · {} Glunko kills · normal DR {}",
        cglunko.dr(),
        cglunko.blue_digits(),
        cglunko.purple_digits(),
        format::fmt_val(cglunko.kills),
        format::fmt_val(cglunko.normal_dr)
    );
    let entity = cx.entity();
    let selected = Tab::ALL
        .iter()
        .position(|entry| *entry == tab)
        .unwrap_or_default();
    let tabs = tab_bar(
        "cglunko-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        selected,
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_cglunko_tab(Tab::ALL[index], cx));
        },
    )
    .flex_1();
    let page_entity = cx.entity();
    // Page `n` keeps Ely's `cglunko-page-toggle-<n>` id, which the harness
    // clicks. A click on the checked page clears every flag: keep the page.
    let pages = ToggleGroup::new("cglunko-page")
        .single()
        .outline()
        .small()
        .children(Page::ALL.into_iter().enumerate().map(|(index, option)| {
            Toggle::new(SharedString::from(format!("cglunko-page-toggle-{index}")))
                .label(option.label())
                .checked(option == page)
        }))
        .on_click(move |checked, _window, cx| {
            if let Some(index) = checked.iter().position(|checked| *checked) {
                let page = Page::ALL[index];
                page_entity.update(cx, |state, cx| state.set_cglunko_page(page, cx));
            }
        });
    let header = div()
        .id("cglunko-header")
        .flex()
        .items_center()
        .px_3()
        .gap_3()
        .child(tabs)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap_2()
                .child(Caption::new("Page"))
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
        .child(Caption::new(summary));

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
        section = section.child(
            div().px_3().child(
                Alert::new(
                    "cglunko-missing",
                    "This save carries no Cglunko data: OptLacc[630..669] is empty, so every value below reads zero.",
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            ),
        );
    }
    let entity = cx.entity();
    let state: &AppState = state;
    let cx: &App = cx;
    section
        .child(match tab {
            Tab::Overview => overview::overview_pane(state),
            Tab::Upgrades => upgrades::upgrades_pane(state, cx),
            Tab::Optimizer => optimizer::optimizer_pane(state, entity, cx),
            Tab::Stones => stones::stones_pane(state, cx),
        })
        .into_any_element()
}

/// `value` with `decimals` places and its whole part grouped in thousands,
/// as Ely's statistic drew it: `30,808.96`.
pub(super) fn grouped(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text.split_once('.') {
        Some((whole, fraction)) => {
            format!("{}.{fraction}", format::exact(whole.parse().unwrap_or(0.0)))
        }
        None => format::exact(value),
    }
}

/// A small outlined tag.
pub(super) fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// A tab's scrolling pane under its Ely scroll id.
pub(super) fn pane(id: &'static str, body: impl IntoElement) -> AnyElement {
    use gpui_kit::component::scroll::ScrollableElement as _;
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id(id)
        .into_any_element()
}
