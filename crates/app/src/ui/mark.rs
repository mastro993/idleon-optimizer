//! A section's activity mark on its sidebar item (D5): a progress ring while a
//! run is in flight, an accent or red dot once one finished or failed while
//! another place was open.
//!
//! The kit's `ProgressCircle` sizes its stroke from its width (15%, at most
//! 5 px) and draws its track as the arc color at 20%, so at the rail's 12 px
//! its stroke is thinner than D5's. The ring here is drawn at D5's strokes,
//! `primary` on `input` (ADR 0005). The dots are plain discs: the kit's dot
//! `Badge` pins itself over another element at its own size and offset.

use crate::state::{Mark, Shown};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Hsla, IntoElement, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    Window, canvas, div, point, px,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, ThemeStyled as _};
use idleon_ui::widgets::Tone;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The ring's width and stroke at a row's end.
const RING: (f32, f32) = (14.0, 2.0);
/// The ring's width and stroke on a rail icon's corner.
const RAIL_RING: (f32, f32) = (12.0, 2.5);
/// A finished or failed dot's width.
const DOT: f32 = 8.0;

/// One section's mark, sized for a row's end or a rail icon's corner.
#[derive(IntoElement)]
pub(crate) struct ActivityMark {
    id: SharedString,
    shown: Shown,
    rail: bool,
}

impl ActivityMark {
    pub(crate) fn new(id: impl Into<SharedString>, shown: Shown, rail: bool) -> Self {
        Self {
            id: id.into(),
            shown,
            rail,
        }
    }
}

impl RenderOnce for ActivityMark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (role, mark) = match self.shown {
            Shown::Running { fraction, .. } => {
                let (side, stroke) = if self.rail { RAIL_RING } else { RING };
                (Role::ProgressIndicator, ring(fraction, side, stroke, cx))
            }
            Shown::Mark(Mark::Finished) => (Role::Status, dot(Tone::Accent, cx)),
            Shown::Mark(Mark::Failed) => (Role::Status, dot(Tone::Danger, cx)),
        };
        let label = self.shown.label();
        // At the row's end the mark keeps the ring's width either way, so the
        // title truncates at the same place, and names its runs on hover; in
        // the rail the row's tooltip does.
        div()
            .id(self.id)
            .role(role)
            .aria_label(label.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .when(!self.rail, |slot| {
                slot.size(px(RING.0))
                    .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
            })
            .child(mark)
    }
}

/// A finished or failed dot in `tone`.
fn dot(tone: Tone, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .size(px(DOT))
        .rounded_full_style(cx)
        .bg(tone.color(cx))
        .into_any_element()
}

/// A `side` px ring filled clockwise from the top to `fraction`.
fn ring(fraction: f32, side: f32, stroke: f32, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let (track, fill) = (theme.input, theme.primary);
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let center = bounds.center();
            let radius = (bounds.size.width.min(bounds.size.height) - px(stroke)) / 2.0;
            arc(center, radius, -FRAC_PI_2, TAU, stroke, track, window);
            if fraction > 0.0 {
                arc(
                    center,
                    radius,
                    -FRAC_PI_2,
                    TAU * fraction,
                    stroke,
                    fill,
                    window,
                );
            }
        },
    )
    .size(px(side))
    .flex_none()
    .into_any_element()
}

/// Stroke the arc of `sweep` radians from `from`, as a polyline fine enough to
/// read as a curve at these sizes.
fn arc(
    center: Point<Pixels>,
    radius: Pixels,
    from: f32,
    sweep: f32,
    stroke: f32,
    color: Hsla,
    window: &mut Window,
) {
    let steps = ((32.0 * sweep / TAU).ceil() as usize).max(2);
    let mut path = PathBuilder::stroke(px(stroke));
    for step in 0..=steps {
        let angle = from + sweep * step as f32 / steps as f32;
        let at = point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if step == 0 {
            path.move_to(at);
        } else {
            path.line_to(at);
        }
    }
    match path.build() {
        Ok(path) => window.paint_path(path, color),
        Err(error) => log::error!("activity ring: an arc failed to build: {error:#}"),
    }
}
