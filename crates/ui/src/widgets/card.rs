//! A flat panel with its title row inside the surface.
//!
//! The kit's `GroupBox` puts its title above the surface and its footer under
//! it; a Console panel keeps both inside, split from the body by hairlines.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div,
};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};

use super::{PANEL_GAP, PANEL_PADDING, TEXT_BODY, TEXT_CAPTION};

/// A card's title row: title, a line under it, trailing actions.
#[derive(IntoElement)]
pub struct CardHeader {
    title: SharedString,
    description: Option<SharedString>,
    actions: Vec<AnyElement>,
}

impl CardHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            actions: Vec::new(),
        }
    }

    /// A line under the title.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// An action at the end of the row; actions keep the order they are added in.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for CardHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .items_start()
            .justify_between()
            .gap_3()
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(TEXT_BODY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(self.title),
                    )
                    .when_some(self.description, |heading, text| {
                        heading.child(
                            div()
                                .text_size(TEXT_CAPTION)
                                .text_color(theme.muted_foreground)
                                .child(text),
                        )
                    }),
            )
            .when(!self.actions.is_empty(), |row| {
                row.child(h_flex().flex_none().gap_1().children(self.actions))
            })
    }
}

/// A panel: optional header, body, optional footer. Hairline border, no shadow.
#[derive(IntoElement)]
pub struct Card {
    style: StyleRefinement,
    header: Option<CardHeader>,
    footer: Option<AnyElement>,
    body: Vec<AnyElement>,
}

impl Card {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            header: None,
            footer: None,
            body: Vec::new(),
        }
    }

    pub fn header(mut self, header: CardHeader) -> Self {
        self.header = Some(header);
        self
    }

    /// A row under the body, its content aligned to the end.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.group_box)
            .refine_style(&self.style)
            .when_some(self.header, |card, header| {
                card.child(div().px(PANEL_PADDING).pt(PANEL_PADDING).child(header))
            })
            .child(v_flex().p(PANEL_PADDING).gap(PANEL_GAP).children(self.body))
            .when_some(self.footer, |card, footer| {
                card.child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .px(PANEL_PADDING)
                        .py_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(footer),
                )
            })
    }
}
