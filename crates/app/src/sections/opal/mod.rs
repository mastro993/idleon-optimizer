//! The Opal Optimizer (W5): the current villagers, their EXP rates, the account
//! bonuses behind them, the distribution optimizer and the manual simulator.
//!
//! `Current state` is the first tab: the villager overview and table, the
//! current opal state and the shared formula tree, with a villager picker that
//! switches the tree between the total and one villager. `Optimizer` splits a
//! total across the unlocked villagers for the selected target, and `Manual
//! simulator` reads a hand-typed split against the save.
//!
//! Every save-derived figure comes from the import; the section keeps only its
//! visible selections, on its own [`State`].

mod actions;

mod bonuses;
mod optimizer;
mod overview;
mod simulate;
mod state;
mod table;

pub(crate) use state::{Config, State};

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
use gpui_kit::component::{ActiveTheme as _, Icon, v_flex};
use idleon_opal::Opal;
use idleon_ui::widgets::PANEL_GAP;

/// The largest opal count a manual or planned input takes; a save above it
/// raises the ceiling.
const MAX_OPALS: f64 = 1_000_000.0;

/// The Opal Optimizer tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// The villagers, their rates, the bonus terms and the formula tree.
    #[default]
    Current,
    /// The distribution optimizer.
    Optimizer,
    /// A hand-typed distribution read against the save.
    Manual,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Current, Tab::Optimizer, Tab::Manual];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Current => "opal-tab-current",
            Tab::Optimizer => "opal-tab-optimizer",
            Tab::Manual => "opal-tab-manual",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Current => "Current state",
            Tab::Optimizer => "Optimizer",
            Tab::Manual => "Manual simulator",
        }
    }
}

/// The section: the tab strip and the selected pane under it.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.save.clone() else {
        return prompt(cx);
    };
    state.opal.sync_tree(state.save_revision, cx);
    let tab = state.opal.tab;
    let entity = cx.entity();
    let tabs = layout::tab_bar(
        "opal-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        Tab::ALL.iter().position(|entry| *entry == tab).unwrap_or(0),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.opal.set_tab(Tab::ALL[index], cx));
        },
    );
    v_flex()
        .id("opal-section")
        .size_full()
        .min_h_0()
        .gap(PANEL_GAP)
        .child(div().id("opal-header").flex_none().child(tabs))
        .child(match tab {
            Tab::Current => current_pane(state, &opal, cx),
            Tab::Optimizer => optimizer::optimizer_pane(state, window, cx),
            Tab::Manual => simulate::pane(state, window, cx),
        })
        .into_any_element()
}

/// A tab's body in its own vertical scroll area.
fn pane(id: &'static str, body: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id(id)
        .into_any_element()
}

/// The current state pane: the overview, the villagers, the terms and the tree.
fn current_pane(state: &AppState, opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    pane(
        "opal-pane",
        v_flex()
            .id("opal-overview")
            .gap(PANEL_GAP)
            .child(overview::current_state(opal))
            .child(overview::villagers(state, opal, cx))
            .child(bonuses::terms(opal, cx))
            .child(bonuses::formula(state)),
    )
}

/// A save without `Holes` has no villagers: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    v_flex()
        .id("opal-section")
        .size_full()
        .min_h_0()
        .child(
            div().id("opal-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No villager state"))
                            .description(EmptyDescription::new().child(
                                "The opal optimizer reads the five villagers, their levels and their \
                                 invested opals from a save. Import one to see the EXP rates and every \
                                 bonus term.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("opal-import")
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
mod tests {
    use super::*;

    #[test]
    fn the_tab_ids_match_the_strip() {
        assert_eq!(
            Tab::ALL.map(Tab::id),
            ["opal-tab-current", "opal-tab-optimizer", "opal-tab-manual"]
        );
    }
}
