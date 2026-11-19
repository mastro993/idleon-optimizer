//! The Fountain Calculator: the Overview, Currency and Upgrades tabs.
//!
//! FN-01 builds the overview (the fountain summary, fill and income rates,
//! lucky coins and ducks, the ground coin stacks and the cross-system bonuses);
//! FN-02 adds the currency values, the desired-currency simulator and the
//! upgrade list with its cost curve; FN-03 adds the upgrade planner and the
//! greedy optimizer; FN-04 adds the marble planner, its optimizer and the
//! simulated run.

mod actions;

mod bonuses;
mod controls;
mod currency;
mod marble;
mod overview;
mod planner;
mod sim;
pub(crate) mod state;
mod table;
mod upgrades;

pub(crate) use actions::{MarblePlanInputs, SimInputs};
pub(crate) use controls::{Controls, Runs};
pub(crate) use state::{SIM_DEFAULT_HOURS, SIM_MAX_HOURS, SIM_MIN_HOURS, State, Tab};

use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, h_flex, v_flex};
use idleon_ui::widgets::{Caption, PANEL_PADDING};

/// The fountain section: the tab strip and the selected pane, or the import
/// prompt when the loaded save carries no cavern.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let Some(fountain) = state.fountain.as_ref() else {
        return prompt(cx);
    };
    let revision = state.save_revision;
    if state.fountain_controls.tree_revision != Some(revision) {
        let root = fountain.multi_tree();
        state
            .fountain_breakdown
            .update(cx, |tree, cx| tree.set_root(root, cx));
        state.fountain_controls.tree_revision = Some(revision);
    }
    let tab = state.fountain_state.tab;
    let entity = cx.entity();
    let tabs = layout::tab_bar(
        "fountain-tabs",
        Tab::ALL.map(|tab| {
            (
                format!("fountain-tab-{}", tab.id()).into(),
                tab.label().into(),
            )
        }),
        Tab::ALL.iter().position(|entry| *entry == tab).unwrap_or(0),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_fountain_tab(Tab::ALL[index], cx));
        },
    );
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
    let header = h_flex()
        .id("fountain-header")
        .flex_none()
        .gap_3()
        .child(tabs.flex_1())
        .child(
            div()
                .id("fountain-summary-line")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex_none()
                .child(Caption::new(summary).text_color(muted)),
        );
    let pane = match tab {
        Tab::Overview => overview::overview(state, cx),
        Tab::Currency => currency::currency_pane(state, window, cx),
        Tab::Upgrades => upgrades::upgrades_pane(state, window, cx),
        Tab::Planner => planner::planner_pane(state, window, cx),
        Tab::Marble => marble::marble_pane(state, window, cx),
        Tab::Sim => sim::sim_pane(state, window, cx),
    };
    v_flex()
        .id("fountain-section")
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(pane)
                .overflow_y_scrollbar()
                .id("fountain-pane"),
        )
        .into_any_element()
}

/// A save without `Holes` has no fountain state: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    v_flex()
        .id("fountain-section")
        .size_full()
        .min_h_0()
        .p(PANEL_PADDING)
        .child(
            div().id("fountain-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No fountain state"))
                            .description(EmptyDescription::new().child(
                                "The fountain calculator reads the cavern from a save. Import one \
                                 to see the fountain level, currency values, upgrades and income \
                                 rates.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("fountain-import")
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

#[cfg(test)]
mod tests;
