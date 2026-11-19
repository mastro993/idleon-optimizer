//! Headline figures: one labelled value, and the strip that holds a row of them.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ElementId, FontWeight, IntoElement, ParentElement, Pixels, RenderOnce, Role,
    SharedString, StyleRefinement, Styled, Window, div, px,
};
use gpui_kit::component::{ActiveTheme as _, Sizable, Size, StyledExt as _, h_flex, v_flex};

use super::{Caption, PANEL_PADDING, TEXT_BODY, TEXT_CAPTION, TEXT_KPI, Tone};

/// A labelled figure: label, value in the mono face, an optional delta and a
/// caption under it.
///
/// The value arrives formatted (`idleon_ui::format`), so the figure reads the
/// same as the tables around it. The element carries `id` and an accessible
/// label of the label and the value, which the harness reads.
#[derive(IntoElement)]
pub struct Statistic {
    id: ElementId,
    label: SharedString,
    value: SharedString,
    prefix: Option<SharedString>,
    suffix: Option<SharedString>,
    tone: Option<Tone>,
    delta: Option<(SharedString, Tone)>,
    caption: Option<SharedString>,
    size: Size,
}

impl Statistic {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            prefix: None,
            suffix: None,
            tone: None,
            delta: None,
            caption: None,
            size: Size::Medium,
        }
    }

    /// Before the value, as a currency sign.
    pub fn prefix(mut self, text: impl Into<SharedString>) -> Self {
        self.prefix = Some(text.into());
        self
    }

    /// After the value, as a unit.
    pub fn suffix(mut self, text: impl Into<SharedString>) -> Self {
        self.suffix = Some(text.into());
        self
    }

    /// Colors the value; it stays the foreground color otherwise.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// How the value moved, such as `+38.6%`, in the tone that says whether it is good.
    pub fn delta(mut self, text: impl Into<SharedString>, tone: Tone) -> Self {
        self.delta = Some((text.into(), tone));
        self
    }

    /// A line of context under the value.
    pub fn caption(mut self, text: impl Into<SharedString>) -> Self {
        self.caption = Some(text.into());
        self
    }

    fn value_size(&self) -> Pixels {
        match self.size {
            Size::XSmall => TEXT_BODY,
            Size::Small => px(15.),
            Size::Large => px(24.),
            Size::Medium | Size::Size(_) => TEXT_KPI,
        }
    }
}

impl Sizable for Statistic {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Statistic {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let value_size = self.value_size();
        let summary = format!(
            "{}: {}{}{}",
            self.label,
            self.prefix.as_deref().unwrap_or(""),
            self.value,
            self.suffix
                .as_deref()
                .map(|suffix| format!(" {suffix}"))
                .unwrap_or_default(),
        );
        let unit = |text: SharedString| {
            div()
                .text_size(TEXT_BODY)
                .text_color(theme.muted_foreground)
                .child(text)
        };
        v_flex()
            .id(self.id)
            .role(Role::Label)
            .aria_label(summary)
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .text_size(TEXT_CAPTION)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(self.label),
            )
            .child(
                // `min_w_0` lets a long value wrap inside its cell instead of
                // running into the next one on a narrow window.
                h_flex()
                    .min_w_0()
                    .items_baseline()
                    .gap_1()
                    .children(self.prefix.map(unit))
                    .child(
                        div()
                            .min_w_0()
                            .font_family(theme.mono_font_family.clone())
                            .text_size(value_size)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.tone.map_or(theme.foreground, |tone| tone.color(cx)))
                            .child(self.value),
                    )
                    .children(self.suffix.map(unit))
                    .when_some(self.delta, |row, (text, tone)| {
                        row.child(
                            div()
                                .ml_1()
                                .font_family(theme.mono_font_family.clone())
                                .text_size(TEXT_CAPTION)
                                .text_color(tone.color(cx))
                                .child(text),
                        )
                    }),
            )
            .children(self.caption.map(Caption::new))
    }
}

/// A row of figures in one bordered panel, split by vertical hairlines.
///
/// The Console design draws headline numbers this way instead of one card per
/// number; a single figure is a strip of one.
#[derive(IntoElement, Default)]
pub struct KpiStrip {
    style: StyleRefinement,
    cells: Vec<AnyElement>,
}

impl KpiStrip {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for KpiStrip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for KpiStrip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.cells.extend(elements);
    }
}

impl RenderOnce for KpiStrip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .items_stretch()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.group_box)
            .refine_style(&self.style)
            .children(self.cells.into_iter().enumerate().map(|(index, cell)| {
                div()
                    .flex_1()
                    .min_w_0()
                    .p(PANEL_PADDING)
                    .when(index > 0, |cell| {
                        cell.border_l_1().border_color(theme.border)
                    })
                    .child(cell)
            }))
    }
}
