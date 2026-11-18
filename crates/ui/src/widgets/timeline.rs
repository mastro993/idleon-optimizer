//! A vertical log of events: a dot on a hairline per event, its title, time and body.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, px,
};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};

use super::{TEXT_BODY, TEXT_CAPTION, Tone};

const DOT: f32 = 7.;
/// Room between the hairline and the item text.
const INDENT: f32 = 12.;

/// The events, oldest first.
#[derive(IntoElement, Default)]
pub struct Timeline {
    items: Vec<TimelineItem>,
}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, item: TimelineItem) -> Self {
        self.items.push(item);
        self
    }
}

impl RenderOnce for Timeline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .gap_3()
            .ml(px(DOT / 2.))
            .pl(px(INDENT))
            .border_l_1()
            .border_color(cx.theme().border)
            .children(self.items)
    }
}

/// One event: a title, when it happened, a tone for its dot and a body.
#[derive(IntoElement)]
pub struct TimelineItem {
    title: SharedString,
    time: Option<SharedString>,
    tone: Tone,
    body: Vec<AnyElement>,
}

impl TimelineItem {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            time: None,
            tone: Tone::default(),
            body: Vec::new(),
        }
    }

    /// When it happened, already formatted.
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl ParentElement for TimelineItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for TimelineItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .relative()
            .gap_0p5()
            .child(
                // Centered on the timeline's hairline, level with the title.
                div()
                    .absolute()
                    .left(px(-(INDENT + DOT / 2. + 0.5)))
                    .top(px(5.))
                    .size(px(DOT))
                    .rounded_full()
                    .bg(self.tone.color(cx)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .text_size(TEXT_BODY)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(self.title),
                    )
                    .when_some(self.time, |row, time| {
                        row.child(
                            div()
                                .flex_none()
                                .font_family(theme.mono_font_family.clone())
                                .text_size(TEXT_CAPTION)
                                .text_color(theme.table_head_foreground)
                                .child(time),
                        )
                    }),
            )
            .when(!self.body.is_empty(), |item| {
                item.child(
                    div()
                        .text_size(TEXT_BODY)
                        .text_color(theme.muted_foreground)
                        .children(self.body),
                )
            })
    }
}
