//! The Royal Guardian Calculator: the Overview, Outposts, Resources,
//! Currencies, Upgrades, Breakdowns and Forecast tabs.
//!
//! RG-01 builds the overview: the counters, the world progress cards, the raw
//! clearing values every outpost reports and the range target behind an
//! outpost's Advanced Logistics plan. RG-02 adds the outpost map, the resource
//! nodes, the currencies, the drain staffing and the banked hours. RG-03 adds
//! the upgrade list with its cost plans, the global breakdown tree and the
//! forecast. RG-04 adds the outpost optimizer and the unit EXP plan with the
//! shelf steps toward the next Armory shelf.

mod actions;
mod breakdowns;
mod controls;
mod currencies;
mod forecast;
mod optimizer;
mod outposts;
mod overview;
mod resources;
mod table;
mod unit_exp;
mod upgrades;

pub(crate) use controls::Controls;
pub(crate) use upgrades::View as UpgradeView;

#[cfg(test)]
mod tests;

use crate::state::AppState;
use crate::ui::layout::tab_bar;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon};
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
    /// The Armory shelves and the Orblet market, with their cost plans.
    Upgrades,
    /// The account-wide rules as the shared formula tree.
    Breakdowns,
    /// The projection over the collects the player places.
    Forecast,
    /// The outpost and unit optimizer: a bounded search over legal moves.
    Optimizer,
    /// The unit EXP moves and the steps toward the next Armory shelf.
    #[serde(rename = "unit-exp")]
    UnitExp,
}

impl Tab {
    pub(crate) const ALL: [Tab; 9] = [
        Tab::Overview,
        Tab::Outposts,
        Tab::Resources,
        Tab::Currencies,
        Tab::Upgrades,
        Tab::Breakdowns,
        Tab::Forecast,
        Tab::Optimizer,
        Tab::UnitExp,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Outposts => "outposts",
            Tab::Resources => "resources",
            Tab::Currencies => "currencies",
            Tab::Upgrades => "upgrades",
            Tab::Breakdowns => "breakdowns",
            Tab::Forecast => "forecast",
            Tab::Optimizer => "optimizer",
            Tab::UnitExp => "unit-exp",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Outposts => "Outposts",
            Tab::Resources => "Resources",
            Tab::Currencies => "Currencies",
            Tab::Upgrades => "Upgrades",
            Tab::Breakdowns => "Breakdowns",
            Tab::Forecast => "Forecast",
            Tab::Optimizer => "Optimizer",
            Tab::UnitExp => "Unit EXP",
        }
    }
}

/// The section's own state, kept across dashboard visits.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The upgrade list the Upgrades pane shows.
    pub(crate) upgrade_view: upgrades::View,
    /// The shelf whose cost plan the Upgrades pane prices.
    pub(crate) armory_plan: Option<usize>,
    /// The global rule the Breakdowns pane expands.
    pub(crate) breakdown: idleon_royal_guardian::Category,
    /// The forecast horizon, in hours.
    pub(crate) forecast_hours: f64,
    /// The hours the forecast's collects sit at.
    pub(crate) collects: Vec<f64>,
    /// The forecast's menu-open windows, as `(start, end)` hours.
    pub(crate) windows: Vec<(f64, f64)>,
    /// The objective the outpost optimizer maximizes.
    pub(crate) optimizer: idleon_royal_guardian::Objective,
    /// The outpost the unit EXP planner plans for.
    pub(crate) unit_exp_map: Option<usize>,
    /// The rank targets the unit EXP planner reaches for.
    pub(crate) unit_exp_targets: idleon_royal_guardian::UnitTargets,
    /// Whether the unit EXP plan moves same-world assignments.
    pub(crate) unit_exp_move_transients: bool,
    /// How many levels the shelf plan may buy.
    pub(crate) shelf_purchases: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            world: 0,
            outpost: None,
            resource: None,
            currency: None,
            upgrade_view: upgrades::View::default(),
            armory_plan: None,
            breakdown: idleon_royal_guardian::Category::default(),
            forecast_hours: idleon_royal_guardian::HORIZON.2,
            collects: Vec::new(),
            windows: Vec::new(),
            optimizer: idleon_royal_guardian::Objective::default(),
            unit_exp_map: None,
            unit_exp_targets: idleon_royal_guardian::UnitTargets::default(),
            unit_exp_move_transients: false,
            shelf_purchases: 10,
        }
    }
}

impl State {
    /// The forecast inputs as the engine reads them.
    pub(crate) fn forecast_options(&self) -> idleon_royal_guardian::ForecastOptions {
        forecast::options(self.forecast_hours, &self.collects, &self.windows)
    }
}

/// The Royal Guardian section: the tab strip and its pane, or the import prompt
/// when the loaded save carries no Royal Guardian rows.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if state.royal.is_none() {
        return prompt(cx);
    }
    let tab = state.royal_state.tab;
    if tab == Tab::Forecast {
        state.ensure_royal_forecast();
    }
    state.sync_royal(cx);
    let entity = cx.entity();
    let selected = Tab::ALL.iter().position(|entry| *entry == tab).unwrap_or(0);
    let tabs = Tab::ALL.map(|entry| {
        (
            SharedString::from(format!("royal-tab-{}", entry.id())),
            SharedString::from(entry.label()),
        )
    });
    let header = div().id("royal-tab-strip").flex().pt_2().child(
        tab_bar("royal-tabs", tabs, selected, move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_royal_tab(Tab::ALL[index], cx));
        })
        .flex_1(),
    );
    let pane = match tab {
        Tab::Overview => overview::overview(state, window, cx),
        Tab::Outposts => outposts::outposts(state, cx),
        Tab::Resources => resources::resources(state, window, cx),
        Tab::Currencies => currencies::currencies(state, window, cx),
        Tab::Upgrades => upgrades::upgrades(state, window, cx),
        Tab::Breakdowns => breakdowns::breakdowns(state, cx),
        Tab::Forecast => forecast::forecast(state, window, cx),
        Tab::Optimizer => optimizer::optimizer(state, window, cx),
        Tab::UnitExp => unit_exp::unit_exp(state, window, cx),
    };
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(header)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(div().py_3().child(pane))
                .overflow_y_scrollbar()
                .id("royal-pane"),
        )
        .into_any_element()
}

/// The world rail every pane shares: which world its tables list.
pub(super) fn world_rail(world: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let tabs = (0..royal_guardian::WORLDS).map(|index| {
        (
            SharedString::from(format!("royal-world-{index}")),
            SharedString::from(format!("W{}", index + 1)),
        )
    });
    div()
        .id("royal-world-rail")
        .flex()
        .child(
            tab_bar(
                "royal-worlds-tabs",
                tabs,
                world,
                move |world, _window, cx| {
                    entity.update(cx, |state, cx| state.set_royal_world(world, cx));
                },
            )
            .flex_1(),
        )
        .into_any_element()
}

/// A save without `RoyalG` has no outposts: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div().id("royal-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No Royal Guardian state"))
                            .description(EmptyDescription::new().child(
                                "The Royal Guardian calculator reads the outposts from a save. \
                                 Import one to see the counters, the world progress, the raw \
                                 clearing values and the range targets.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("royal-import")
                                .label("Import save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    import.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}
