//! The upstream "Am I shadow banned?" page: one large verdict from `OptLacc[26]`.
//!
//! `shadow.html` reads the multiplayer restriction flag and shows YES for any
//! finite nonzero value, NO for zero and UNKNOWN when the save has no readable
//! flag. The shared header already names the save status; the line under the
//! verdict says which flag the loaded save carried.

use crate::state::{AppState, SaveStatus};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, Window, div, px,
};
use idleon_import::ShadowVerdict;

/// The shadow-ban page: the verdict and the flag it read.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = &cx.theme().colors;
    let verdict = state.shadow.verdict();
    let tone = match verdict {
        ShadowVerdict::Yes => colors.danger,
        ShadowVerdict::No => colors.success,
        ShadowVerdict::Unknown => colors.fg_muted,
    };
    let detail = detail(state.save_status(), state.shadow.flag);
    // Fill the page below the shared header and the card's own header and padding.
    let answer_height = (f32::from(window.viewport_size().height) - 200.0).max(240.0);

    Card::new()
        .header(
            CardHeader::new("Verdict").description(
                "Read from OptLacc[26]; a save without a readable flag stays UNKNOWN.",
            ),
        )
        .child(
            div()
                .id("shadow-answer")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .min_h(px(answer_height))
                .py(px(48.0))
                .child(
                    div()
                        .id("shadow-verdict")
                        .role(Role::Label)
                        .aria_label(format!("Shadow ban verdict: {}", verdict.label()))
                        .text_size(px(144.0))
                        .font_weight(FontWeight::BLACK)
                        .text_color(tone)
                        .child(verdict.label()),
                )
                .child(
                    div()
                        .id("shadow-detail")
                        .role(Role::Label)
                        .aria_label(detail.clone())
                        .text_size(px(13.0))
                        .text_color(colors.fg_muted)
                        .child(detail),
                ),
        )
        .into_any_element()
}

/// The save status and the flag behind the verdict.
fn detail(status: SaveStatus, flag: Option<f64>) -> String {
    match (status, flag) {
        (SaveStatus::Loaded | SaveStatus::Partial, Some(flag)) => {
            format!("{} · OptLacc[26] = {}.", status.label(), number(flag))
        }
        (SaveStatus::Loaded | SaveStatus::Partial, None) => format!(
            "{} · no readable OptLacc[26], so the answer stays UNKNOWN.",
            status.label()
        ),
        (SaveStatus::Error, _) => {
            "The last import failed and no save is loaded; the answer stays UNKNOWN.".to_string()
        }
        (SaveStatus::Empty, _) => {
            "No save imported yet; the answer stays UNKNOWN until one is.".to_string()
        }
    }
}

/// A flag as the game writes it: whole numbers without a trailing zero.
fn number(flag: f64) -> String {
    if flag.fract() == 0.0 {
        format!("{flag:.0}")
    } else {
        flag.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detail_names_the_save_status_and_the_flag() {
        assert_eq!(
            detail(SaveStatus::Loaded, Some(0.0)),
            "Save imported · OptLacc[26] = 0."
        );
        assert_eq!(
            detail(SaveStatus::Partial, Some(1.5)),
            "Save imported with warnings · OptLacc[26] = 1.5."
        );
        assert!(
            detail(SaveStatus::Loaded, None).contains("no readable OptLacc[26]"),
            "a loaded save without the flag says so"
        );
        assert!(detail(SaveStatus::Empty, None).contains("No save imported yet"));
        assert!(detail(SaveStatus::Error, None).contains("last import failed"));
    }

    #[test]
    fn whole_flags_drop_the_trailing_zero() {
        assert_eq!(number(0.0), "0");
        assert_eq!(number(-2.0), "-2");
        assert_eq!(number(1.25), "1.25");
    }
}
