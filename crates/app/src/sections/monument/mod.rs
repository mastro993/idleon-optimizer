//! The Monument (W5) section: Bravery today, Justice and Wisdom when their
//! feature issues land, and the training sources both use.

pub mod bravery;

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled,
    Window, div, px,
};
use idleon_core::monument::Snapshot;

/// The upstream monument tab strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    #[default]
    Bravery,
    Justice,
    Wisdom,
    Sources,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Bravery, Tab::Justice, Tab::Wisdom, Tab::Sources];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Bravery => "bravery",
            Tab::Justice => "justice",
            Tab::Wisdom => "wisdom",
            Tab::Sources => "sources",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Bravery => "Bravery",
            Tab::Justice => "Justice",
            Tab::Wisdom => "Wisdom",
            Tab::Sources => "Sources",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The monument section's live inputs; the save supplies the rest.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub tab: Tab,
    /// The enemy number, 1 for the first enemy; the upstream wave is one less.
    pub wave: u32,
    /// Damage-boost stacks already earned this story.
    pub boosts: u32,
    /// Rethrows left; `None` uses the saved maximum.
    pub rerolls: Option<u32>,
    /// HP before the last sword, when the player knows it.
    pub remaining_hp: Option<f64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Bravery,
            wave: 1,
            boosts: 0,
            rerolls: None,
            remaining_hp: None,
        }
    }
}

/// The section: tab strip, training summary and the active pane.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let snapshot = state.monument_snapshot();
    let summary = snapshot
        .as_ref()
        .filter(|snapshot| snapshot.available)
        .map(|snapshot| {
            format!(
                "{}x training · {} hours per 24h claim",
                format::value(snapshot.training.rate),
                format::value(snapshot.training.per_day)
            )
        })
        .unwrap_or_else(|| "Import a save to calculate".into());
    let muted = cx.theme().colors.fg_muted;
    let header = div()
        .id("monument-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tab_bar(state, cx))
        .child(div().flex_1())
        .child(
            div()
                .id("monument-summary")
                .role(Role::Label)
                .aria_label(summary.clone())
                .child(Caption::new(summary).text_color(muted)),
        );

    div()
        .id("monument-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(pane(state, window, cx, snapshot.as_ref()))
        .into_any_element()
}

fn tab_bar(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs =
        TabBar::new("monument-tabs").selected(SharedString::from(state.monument.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_monument_tab(tab, cx));
        }
    })
}

fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    snapshot: Option<&Snapshot>,
) -> AnyElement {
    match state.monument.tab {
        Tab::Bravery => bravery::pane(state, snapshot, window, cx),
        Tab::Justice => planned_pane(
            "monument-pane-justice",
            "Justice",
            "Justice cases, effects, search and the response advisor arrive with MN-02.",
        ),
        Tab::Wisdom => planned_pane(
            "monument-pane-wisdom",
            "Wisdom",
            "Wisdom attempts, training and reward-charge effects arrive with MN-03.",
        ),
        Tab::Sources => sources_pane(snapshot),
    }
}

fn planned_pane(id: &'static str, title: &'static str, detail: &'static str) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            Card::new()
                .header(CardHeader::new(title).description(detail))
                .child(Caption::new("This tab is on the roadmap.")),
        )
        .into_any_element()
}

/// The additive training pool, which Bravery milestones and the other
/// monuments share.
fn sources_pane(snapshot: Option<&Snapshot>) -> AnyElement {
    let body = match snapshot.filter(|snapshot| snapshot.available) {
        Some(snapshot) => {
            let mut table = Table::new(["Source", "Bonus"]);
            for source in &snapshot.training.sources {
                table = table.row([
                    div()
                        .min_w_0()
                        .child(source.name.clone())
                        .into_any_element(),
                    div()
                        .child(format!("+{}%", format::value(source.value)))
                        .into_any_element(),
                ]);
            }
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Card::new()
                        .header(
                            CardHeader::new("Paying Respect: additive pool").description(format!(
                                "+{}% before the {}x Clutch Overtime Block",
                                format::value(snapshot.training.pct),
                                format::value(snapshot.training.multi)
                            )),
                        )
                        .child(table),
                )
                .into_any_element()
        }
        None => Card::new()
            .header(CardHeader::new("Bonus sources"))
            .child(Caption::new(
                "Import a save to see the training pool Bravery milestones use.",
            ))
            .into_any_element(),
    };
    div()
        .id("monument-pane-sources")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-sources-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

/// A metric card with a formatted value, matching the upstream cards.
pub(crate) fn metric_card(
    id: &'static str,
    title: &'static str,
    value: String,
    sub: String,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    div()
        .flex_1()
        .min_w(px(180.0))
        .child(
            Card::new().header(CardHeader::new(title)).child(
                div()
                    .id(id)
                    .role(Role::Label)
                    .aria_label(format!("{title}: {value}"))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(accent)
                            .child(value),
                    )
                    .child(Caption::new(sub).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// Upstream `clean`: underscores and pipes read as spaces.
pub(crate) fn clean_text(text: &str) -> String {
    text.replace(['_', '|'], " ")
}
