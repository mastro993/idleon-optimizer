//! Images resolved through the game asset manifest.
//!
//! Every reference goes through [`Manifest::artwork`], so a missing install or a
//! deleted file draws a stand-in in the same box instead of a broken image:
//! a labelled placeholder for section sprites, the section's emoji for its
//! sidebar and dashboard icon.

use crate::assets::{Artwork, Manifest};
use crate::sections::Section;
use ely_gpui_component::primitives::{Icon, IconName};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ElementId, IntoElement, ObjectFit, ParentElement, Pixels, Role, Styled, div,
    img, px,
};

/// A section's game sprite in a `size` box. A missing sprite draws the
/// section's emoji in the same box, so deleting a file never moves the layout.
pub(crate) fn section_icon(section: &Section, manifest: &Manifest, size: Pixels) -> AnyElement {
    let slot = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size);
    match manifest.artwork(section.art) {
        Artwork::Ready(path) => slot
            .debug_selector(|| format!("{}-art", section.id))
            .child(img(path).size_full().object_fit(ObjectFit::Contain)),
        Artwork::Missing => slot
            .debug_selector(|| format!("{}-emoji", section.id))
            .text_size(size * 0.75)
            .child(section.icon),
    }
    .into_any_element()
}

/// An image at `size`, or a labelled placeholder with the same footprint.
pub(crate) fn asset_image(
    id: impl Into<ElementId>,
    manifest: &Manifest,
    reference: &str,
    size: Pixels,
    cx: &App,
) -> AnyElement {
    asset_image_sized(id, manifest, reference, size, size, cx)
}

/// The same image with independent width and height, for the non-square
/// game sprites (the Jelly Operator cells are 36×109 and 184×183).
pub(crate) fn asset_image_sized(
    id: impl Into<ElementId>,
    manifest: &Manifest,
    reference: &str,
    width: Pixels,
    height: Pixels,
    cx: &App,
) -> AnyElement {
    let id = id.into();
    match manifest.artwork(reference) {
        Artwork::Ready(path) => img(path)
            .id(id)
            .role(Role::Image)
            .aria_label(reference.to_string())
            .w(width)
            .h(height)
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
                .w(width)
                .h(height)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::DEFAULT_DIR;
    use crate::sections;
    use gpui::{Render, Window, size};
    use std::path::Path;

    /// Fishing drawn with the committed art, Jar with no manifest at all.
    struct Icons {
        committed: Manifest,
    }

    impl Render for Icons {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let section = |id| sections::find(id).expect("registered section");
            div()
                .flex()
                .items_start()
                .child(section_icon(section("fishing"), &self.committed, px(20.0)))
                .child(section_icon(section("jar"), &Manifest::default(), px(20.0)))
        }
    }

    #[test]
    fn a_missing_sprite_falls_back_to_the_emoji_in_the_same_box() {
        let mut cx = gpui::TestAppContext::single();
        cx.update(ely_gpui_component::init_for_tests);
        let committed = Manifest::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(DEFAULT_DIR),
        );
        let (_, cx) = cx.add_window_view(|_, _| Icons { committed });
        cx.run_until_parked();

        let box_of = |cx: &mut gpui::VisualTestContext, selector| {
            cx.debug_bounds(selector).map(|bounds| bounds.size)
        };
        let slot = Some(size(px(20.0), px(20.0)));
        assert_eq!(box_of(cx, "fishing-art"), slot);
        assert_eq!(box_of(cx, "fishing-emoji"), None);
        assert_eq!(box_of(cx, "jar-emoji"), slot);
        assert_eq!(box_of(cx, "jar-art"), None);
    }
}
