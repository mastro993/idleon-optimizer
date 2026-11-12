//! The Fountain Calculator: the upstream Overview, Currency and Upgrades tabs.
//!
//! Ported from `fountain-calc.html` and `js/stats/systems/w5/fountain.js`. FN-01
//! builds the overview (the fountain summary, fill and income rates, lucky
//! coins and ducks, the ground coin stacks and the cross-system bonuses); FN-02
//! adds the currency values, the desired-currency simulator and the upgrade
//! list with its cost curve; FN-03 adds the upgrade planner and the greedy
//! optimizer. The marble tab arrives with FN-04.

mod actions;

mod bonuses;
mod currency;
mod overview;
mod planner;
pub(crate) mod state;
mod upgrades;

pub(crate) use state::{STEP_CHOICES, State, Tab};

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};

/// The fountain section: the tab strip and the selected pane, or the import
/// prompt when the loaded save carries no cavern.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let fountain = state.fountain.as_ref();
    let muted = cx.theme().colors.fg_muted;
    let Some(fountain) = fountain else {
        return prompt(cx);
    };
    let tab = state.fountain_state.tab;
    let entity = cx.entity();
    let mut tabs = TabBar::new("fountain-tabs")
        .selected(SharedString::from(format!("fountain-tab-{}", tab.id())));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("fountain-tab-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let tab = id
            .as_ref()
            .strip_prefix("fountain-tab-")
            .and_then(Tab::from_id);
        if let Some(tab) = tab {
            entity.update(cx, |state, cx| state.set_fountain_tab(tab, cx));
        }
    });
    let desired = state.fountain_desired();
    let saved = fountain.desired_currency();
    let wanted = idleon_fountain::currency_label(desired);
    let summary = format!(
        "Fountain level {} · {} · {}× currency",
        overview::num(fountain.level()),
        if desired == saved {
            format!("{wanted} desired")
        } else {
            format!("simulating {wanted}")
        },
        overview::num(fountain.currency_all_multi())
    );
    let header = div()
        .id("fountain-header")
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(div().flex_1())
        .child(
            div()
                .id("fountain-summary-line")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new(summary).text_color(muted)),
        );
    let pane = match tab {
        Tab::Overview => overview::overview(state, cx),
        Tab::Currency => currency::currency_pane(state, cx),
        Tab::Upgrades => upgrades::upgrades_pane(state, cx),
        Tab::Planner => planner::planner_pane(state, cx),
    };
    div()
        .id("fountain-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(
            ScrollArea::new("fountain-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(pane),
        )
        .into_any_element()
}

/// A save without `Holes` has no fountain state: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("fountain-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("fountain-empty", IconName::Map, "No fountain state")
                .body(
                    "The fountain calculator reads the cavern from a save. Import one to see the \
                     fountain level, currency values, upgrades and income rates.",
                )
                .action(
                    Button::new("fountain-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests;
