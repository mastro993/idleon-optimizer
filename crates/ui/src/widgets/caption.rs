//! Small supporting text: units, footnotes, the line under a figure.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement, Styled, Window,
    div, relative,
};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _};

use super::TEXT_CAPTION;

/// A caption: 11 px in the subtle text color. Style it like any element to
/// override the color or size.
#[derive(IntoElement)]
pub struct Caption {
    text: SharedString,
    style: StyleRefinement,
}

impl Caption {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Caption {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Caption {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(TEXT_CAPTION)
            .line_height(relative(1.45))
            .text_color(cx.theme().table_head_foreground)
            .refine_style(&self.style)
            .child(self.text)
    }
}
