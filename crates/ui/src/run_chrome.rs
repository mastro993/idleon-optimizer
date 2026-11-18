//! Shared run chrome: progress with cancel, the cancelled notice and the stale marker.
//!
//! Every section that starts a background job renders these, so progress, cancel
//! and the stale marker read and behave the same everywhere.

use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::tag::Tag;
use idleon_common::Progress as RunProgress;

use crate::widgets::{Caption, Card, CardHeader, Tone};

/// The run in flight: its latest line, a progress bar and the cancel action.
///
/// `id` prefixes the element ids (`<id>-progress`, `<id>-progress-bar`,
/// `<id>-cancel`), so two sections can run at once without colliding.
pub fn progress_card(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    progress: RunProgress,
    on_cancel: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let fraction = progress.fraction.clamp(0.0, 1.0);
    let summary = format!("{:.0}% — {}", fraction * 100.0, progress.message);
    Card::new()
        .header(CardHeader::new(title).description(description))
        .child(
            div()
                .id(format!("{id}-progress"))
                .role(Role::Label)
                .aria_label(summary)
                .flex()
                .flex_col()
                .gap_1()
                .child(Progress::new(format!("{id}-progress-bar")).value(fraction * 100.0))
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
            div().flex().child(
                Button::new(format!("{id}-cancel"))
                    .label("Cancel")
                    .icon(IconName::X)
                    .small()
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
        .child(
            Tag::new()
                .with_variant(Tone::Warning.into())
                .outline()
                .small()
                .child("Stale"),
        )
}

/// The banner above a stale result: its numbers are from the previous inputs.
pub fn stale_banner(id: &'static str) -> impl IntoElement {
    Alert::new(
        id,
        "The inputs changed after this run; the values below are from the previous inputs. Run again to refresh them.",
    )
    .with_variant(Tone::Warning.into())
    .banner()
}

/// The notice a cancelled run leaves; `kept` says whether a previous result stayed.
pub fn cancelled_banner(id: &'static str, kept: bool) -> impl IntoElement {
    Alert::new(
        id,
        if kept {
            "Run cancelled; the previous result is unchanged."
        } else {
            "Run cancelled; nothing was applied."
        },
    )
    .with_variant(Tone::Info.into())
    .banner()
}
