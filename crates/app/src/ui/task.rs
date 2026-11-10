//! Shared run chrome: progress with cancel, the cancelled notice and the stale marker.
//!
//! Every section that starts a background job renders these, so progress, cancel
//! and the stale marker read and behave the same everywhere.

use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::ControlSize;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_core::model::Progress;

/// The run in flight: its latest line, a progress bar and the cancel action.
///
/// `id` prefixes the element ids (`<id>-progress`, `<id>-progress-bar`,
/// `<id>-cancel`), so two sections can run at once without colliding.
pub fn progress_card(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    progress: Progress,
    on_cancel: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let fraction = progress.fraction.clamp(0.0, 1.0);
    let summary = format!("{:.0}% — {}", fraction * 100.0, progress.message);
    Card::new()
        .header(CardHeader::new(title).description(description))
        .child(
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id(format!("{id}-progress"))
                        .role(Role::Label)
                        .aria_label(summary)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(ProgressBar::new(format!("{id}-progress-bar"), fraction))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .gap_2()
                                .child(Caption::new(progress.message))
                                .child(Caption::new(format!("{:.0}%", fraction * 100.0))),
                        ),
                )
                .child(
                    Button::new(format!("{id}-cancel"), "Cancel")
                        .size(ControlSize::Sm)
                        .icon(IconName::X)
                        .on_click(move |_event, window, cx| on_cancel(window, cx)),
                ),
        )
        .into_any_element()
}

/// The badge a result carries once its inputs changed.
pub fn stale_badge(id: &'static str) -> impl IntoElement {
    div()
        .id(format!("{id}-badge"))
        .role(Role::Label)
        .aria_label("Stale: the inputs changed after this run")
        .child(Badge::new("Stale").tone(Tone::Warning))
}

/// The banner above a stale result: its numbers are from the previous inputs.
pub fn stale_banner(id: &'static str) -> impl IntoElement {
    Banner::new(
        id,
        Severity::Warning,
        "The inputs changed after this run; the values below are from the previous inputs. Run again to refresh them.",
    )
}

/// The notice a cancelled run leaves; `kept` says whether a previous result stayed.
pub fn cancelled_banner(id: &'static str, kept: bool) -> impl IntoElement {
    Banner::new(
        id,
        Severity::Info,
        if kept {
            "Run cancelled; the previous result is unchanged."
        } else {
            "Run cancelled; nothing was applied."
        },
    )
}
