//! An inline notice inside a page: a status dot, a title and a body.
//!
//! The kit's `Alert` tints the whole box and takes its message as text; a
//! Console callout is a sunken row whose body can hold any element.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, px,
};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};

use super::{PANEL_PADDING, TEXT_BODY, Tone};

const DOT: f32 = 6.;
const DOT_GAP: f32 = 8.;

/// A callout: the title in the tone's color after a dot, the body muted under it.
#[derive(IntoElement)]
pub struct Callout {
    tone: Tone,
    title: SharedString,
    style: StyleRefinement,
    body: Vec<AnyElement>,
}

impl Callout {
    pub fn new(tone: Tone, title: impl Into<SharedString>) -> Self {
        Self {
            tone,
            title: title.into(),
            style: StyleRefinement::default(),
            body: Vec::new(),
        }
    }
}

impl Styled for Callout {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Callout {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Callout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let tone = self.tone.color(cx);
        v_flex()
            .gap_1()
            .px(PANEL_PADDING)
            .py_2()
            .rounded(theme.radius)
            // The sunken surface: the theme gives it to the sidebar.
            .bg(theme.sidebar)
            .text_size(TEXT_BODY)
            .refine_style(&self.style)
            .child(
                h_flex()
                    .gap(px(DOT_GAP))
                    .child(div().flex_none().size(px(DOT)).rounded_full().bg(tone))
                    .child(
                        div()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tone)
                            .child(self.title),
                    ),
            )
            .when(!self.body.is_empty(), |callout| {
                callout.child(
                    div()
                        .pl(px(DOT + DOT_GAP))
                        .text_color(theme.muted_foreground)
                        .children(self.body),
                )
            })
    }
}
