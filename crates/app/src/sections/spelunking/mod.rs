//! The Spelunking Simulator: expedition controls, party, elixir belt and depth
//! estimates.
//!
//! Ported from `spelunking-calc.html` and `js/ui/spelunking.js`. SP-01 covers the
//! controls, the party and elixir belt, and the Depth limits tab; the run
//! simulator, amber policies and the optimizer stay on the roadmap as SP-02 to
//! SP-04.
//!
//! The four account multipliers upstream derives from its whole stat context are
//! user inputs here (see [`idleon_core::spelunking::Account`]); every other
//! control and every depth estimate comes from the imported save.

mod actions;
mod belt;
mod chart;
mod controls;
mod depth;
mod party;
mod state;

pub use state::State;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::{Banner, EmptyState};
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};

/// The section: the controls, the party and belt, the account inputs and the
/// depth estimates in one scrolling page (the upstream page's shared toolbar
/// above its Depth limits tab).
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(save) = state.spelunking_save.as_ref() else {
        return empty(cx);
    };
    let inputs = save.inputs(&state.spelunking.account);
    let breakpoint = Breakpoint::of(window);
    let config_error = state.spelunking.config(&inputs).err();
    let mut page = div()
        .id("spelunking-page")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls::card(state, &inputs, cx));
    if let Some(error) = config_error {
        page = page.child(
            Banner::new(
                "spelunk-error",
                Severity::Danger,
                format!("This expedition cannot run: {error}"),
            )
            .into_any_element(),
        );
    }
    let page = page
        .child(layout::split_pane(
            "spelunking-prep",
            breakpoint,
            party::card(state, &inputs, cx),
            belt::card(state, &inputs, cx),
            px(440.0),
        ))
        .child(controls::account_card(state, &inputs, cx))
        .child(depth::card(state, &inputs, cx));
    div()
        .id("spelunking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("spelunking-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(page),
        )
        .into_any_element()
}

/// A labelled control in the toolbars.
pub(crate) fn field(label: &'static str, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(130.0))
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// A save without the Spelunking rows has no expedition state.
fn empty(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("spelunking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("spelunking-empty", IconName::Map, "No Spelunking state")
                .body(
                    "The simulator reads the tunnel records, saved stamina, shop upgrades and \
                     elixir belt from a save. Import one to choose a party and estimate depths.",
                )
                .action(
                    Button::new("spelunking-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
