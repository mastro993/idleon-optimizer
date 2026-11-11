//! The Cglunko section: the save-derived overview and its drop-rate breakdown.
//!
//! CL-01 builds the Overview tab of `cglunko-calc.html`: the summary cards and
//! the DR formula tree. The upgrade list and optimizer (CL-02) and the stone
//! focus (CL-03) arrive later and add their own tabs to a strip that never
//! offers a page with nothing behind it, the way the Jar calculator grew.

mod overview;

use crate::state::AppState;
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::primitives::Severity;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

/// The section: the missing-data warning and the overview.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let mut page = div()
        .id("cglunko-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3();
    // The overview is the save alone, so a save without the Cove rows can only
    // read zero; say why instead of pretending the account is fresh.
    if state.save_loaded && !state.cglunko.saved {
        page = page.child(Banner::new(
            "cglunko-missing",
            Severity::Warning,
            "This save carries no Cglunko data: OptLacc[630..669] is empty, so every value below reads zero.",
        ));
    }
    page.child(overview::overview_pane(state, window, cx))
        .into_any_element()
}
