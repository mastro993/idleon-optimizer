//! Images resolved through the upstream asset manifest.
//!
//! Every reference goes through [`Manifest::artwork`], so a missing install or a
//! deleted file draws a labelled placeholder in the same box instead of a
//! broken image. Sections use this for sprites; the planned pages use it for a
//! small preview of their upstream art.

use crate::assets::{Artwork, Manifest};
use ely_gpui_component::primitives::{Icon, IconName};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ElementId, IntoElement, ObjectFit, ParentElement, Pixels, Role, Styled, div,
    img, px,
};

/// An image at `size`, or a labelled placeholder with the same footprint.
pub fn asset_image(
    id: impl Into<ElementId>,
    manifest: &Manifest,
    reference: &str,
    size: Pixels,
    cx: &App,
) -> AnyElement {
    let id = id.into();
    match manifest.artwork(reference) {
        Artwork::Ready(path) => img(path)
            .id(id)
            .role(Role::Image)
            .aria_label(reference.to_string())
            .w(size)
            .h(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Artwork::Missing => {
            let colors = &cx.theme().colors;
            let name = reference.rsplit('/').next().unwrap_or(reference);
            div()
                .id(id)
                .role(Role::Image)
                .aria_label(format!("Missing asset {reference}"))
                .flex()
                .flex_col()
                .flex_none()
                .items_center()
                .justify_center()
                .gap_1()
                .w(size)
                .h(size)
                .p_1()
                .rounded(px(6.0))
                .border_1()
                .border_color(colors.border)
                .text_color(colors.fg_muted)
                .child(Icon::new(IconName::ImageOff))
                .child(
                    div()
                        .max_w_full()
                        .truncate()
                        .text_size(px(9.0))
                        .line_height(px(11.0))
                        .child(name.to_string()),
                )
                .into_any_element()
        }
    }
}
