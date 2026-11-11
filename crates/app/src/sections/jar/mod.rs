//! The Jar Calculator section: the upstream Overview, Production, Rupie Value
//! and Chances tabs.
//!
//! The upstream `jar-calc.html` has nine tabs. JR-01 and JR-02 build the first
//! four; the rest arrive with JR-03 and JR-04 and are added to [`Tab::ALL`]
//! then, so the strip never offers a tab that has nothing behind it.

mod chances;
mod overview;
mod production;
mod rupies;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px,
};

/// The jar page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// The jar summary, rupie balances and the cavern's status.
    #[default]
    Overview,
    /// The shared production rate, its multipliers and every slot.
    Production,
    /// The multiplier every rupie is worth and what a break pays.
    #[serde(rename = "rupie")]
    RupieValue,
    /// The opal, collectible and enchant chances and their tier scaling.
    Chances,
}

impl Tab {
    pub const ALL: [Tab; 4] = [
        Tab::Overview,
        Tab::Production,
        Tab::RupieValue,
        Tab::Chances,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Production => "production",
            Tab::RupieValue => "rupie",
            Tab::Chances => "chances",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Production => "Production",
            Tab::RupieValue => "Rupie Value",
            Tab::Chances => "Chances",
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
                    Tab::RupieValue => rupies::rupies(state, cx),
                    Tab::Chances => chances::chances(state, cx),
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

/// A KPI figure with its value scaled to `K`/`M`/`B`, so a cavern-sized
/// multiplier still fits the card.
pub(super) fn stat(id: &'static str, label: &'static str, value: f64) -> Statistic {
    let (scaled, suffix) = crate::ui::format::scaled(value);
    let statistic = Statistic::new(id, label, scaled).decimals(2);
    match suffix.is_empty() {
        true => statistic,
        false => statistic.suffix(suffix),
    }
}

/// A labelled figure: the automation bridge and screen readers read the
/// headline as one value instead of walking the card's children.
fn labelled(
    id: &'static str,
    title: &'static str,
    display: String,
    body: AnyElement,
) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{title}: {display}"))
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(200.0))
        .child(body)
        .into_any_element()
}

/// A KPI card wearing its own label, so its figure reads as one value.
pub(super) fn kpi(
    id: &'static str,
    title: &'static str,
    display: String,
    statistic: Statistic,
    caption: impl Into<SharedString>,
) -> AnyElement {
    labelled(
        id,
        title,
        display,
        KpiCard::new(statistic).caption(caption).into_any_element(),
    )
}

/// A card whose headline is a formatted string: a chance reads `1 in 1.09M`,
/// which the numeric statistic cannot show.
pub(super) fn metric(
    id: &'static str,
    title: &'static str,
    display: String,
    caption: String,
    tone: Tone,
    cx: &Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let body = Card::new()
        .header(CardHeader::new(title).action(Badge::new(display.clone()).tone(tone)))
        .child(Caption::new(caption).text_color(muted))
        .into_any_element();
    labelled(id, title, display, body)
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
