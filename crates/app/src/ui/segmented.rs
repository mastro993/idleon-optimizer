//! The app's segmented single choice: a kit `ToggleGroup` drawn the way
//! `docs/ui-kit/design.md` describes it.
//!
//! The kit fills a checked toggle with the hover color and leaves its label
//! in the foreground, so in the Console theme a checked segment barely stands
//! out. No theme key changes that, so the control styles its segments here: a
//! sunken track, the checked segment on surface with a strong hairline and
//! foreground text, the others in muted text.

use gpui::{
    App, ElementId, IntoElement, RenderOnce, SharedString, Styled as _, Window, px,
    transparent_black,
};
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};

/// Space between the track's edge and its segments.
const TRACK_INSET: f32 = 2.0;

/// A small segmented single choice under `id`, one segment per
/// `(value, segment id, label)` with `current` checked.
///
/// A click on the checked segment keeps it: the kit clears every flag then,
/// and `on_change` is not called.
pub(crate) fn segmented<V, I, L>(
    id: impl Into<ElementId>,
    segments: impl IntoIterator<Item = (V, I, L)>,
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> Segmented
where
    V: Copy + PartialEq + 'static,
    I: Into<ElementId>,
    L: Into<SharedString>,
{
    let mut values = Vec::new();
    let segments = segments
        .into_iter()
        .map(|(value, id, label)| {
            values.push(value);
            Segment {
                id: id.into(),
                label: label.into(),
                checked: value == current,
            }
        })
        .collect();
    Segmented {
        id: id.into(),
        segments,
        on_select: Box::new(move |ix, window, cx| on_change(values[ix], window, cx)),
    }
}

/// What a segment pick runs, given the segment's index.
type OnSelect = Box<dyn Fn(usize, &mut Window, &mut App)>;

/// A segmented control built by [`segmented`].
#[derive(IntoElement)]
pub(crate) struct Segmented {
    id: ElementId,
    segments: Vec<Segment>,
    on_select: OnSelect,
}

struct Segment {
    id: ElementId,
    label: SharedString,
    checked: bool,
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let radius = theme.radius;
        let segment_radius = (radius - px(TRACK_INSET)).max(px(0.0));
        let (surface, strong, foreground, muted) = (
            theme.group_box,
            theme.input,
            theme.foreground,
            theme.muted_foreground,
        );
        let on_select = self.on_select;
        ToggleGroup::new(self.id)
            .segmented()
            .single()
            .ghost()
            .small()
            .flex_none()
            .p(px(TRACK_INSET))
            .rounded(radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.tab_bar_segmented)
            .children(self.segments.into_iter().map(|segment| {
                // Every segment keeps a 1 px border so a pick does not shift
                // the labels; only the checked one draws it.
                let toggle = Toggle::new(segment.id)
                    .label(segment.label)
                    .checked(segment.checked)
                    .rounded(segment_radius)
                    .border_1();
                if segment.checked {
                    toggle
                        .bg(surface)
                        .border_color(strong)
                        .text_color(foreground)
                } else {
                    toggle.border_color(transparent_black()).text_color(muted)
                }
            }))
            .on_click(move |checked, window, cx| {
                if let Some(ix) = checked.iter().position(|checked| *checked) {
                    on_select(ix, window, cx);
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AppContext as _, Context, InteractiveElement as _, Modifiers, ParentElement as _, Render,
        TestAppContext, VisualTestContext, div, point,
    };

    /// Three segments with the middle one checked, counting the picks.
    struct Sample {
        current: usize,
        picks: usize,
    }

    impl Render for Sample {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let entity = cx.entity();
            // A row, so the wrapper is as wide as the track.
            div()
                .p_8()
                .flex()
                .child(div().debug_selector(|| "track".into()).child(segmented(
                    "sample",
                    (0..3).map(|ix| (ix, ElementId::from(("segment", ix)), format!("S{ix}"))),
                    self.current,
                    move |ix, _, cx| {
                        entity.update(cx, |sample, cx| {
                            sample.current = ix;
                            sample.picks += 1;
                            cx.notify();
                        })
                    },
                )))
        }
    }

    #[gpui::test]
    fn the_checked_segment_stands_out_and_a_pick_moves_it(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::theme::install(cx);
        });
        let ((window, _), sample) = cx
            .update(|cx| {
                let mut sample = None;
                let window = gpui_kit::open_window(gpui::WindowOptions::default(), cx, |_, cx| {
                    let entity = cx.new(|_| Sample {
                        current: 1,
                        picks: 0,
                    });
                    sample = Some(entity.clone());
                    entity
                })?;
                anyhow::Ok((window, sample.expect("the window built its view")))
            })
            .expect("test window");
        let cx = VisualTestContext::from_window(window, cx).into_mut();
        cx.run_until_parked();

        let (track, surface, strong) = cx.update(|_, cx| {
            let theme = cx.theme();
            (theme.tab_bar_segmented, theme.group_box, theme.input)
        });
        let quads = cx.update(|window, _| window.painted_quads());
        // A fill and its border paint as separate quads.
        let filled = |fill| {
            quads
                .iter()
                .any(|quad| quad.background.as_solid() == Some(fill))
        };
        assert!(filled(track), "no sunken track");
        assert!(filled(surface), "no checked segment on surface");
        assert!(
            quads
                .iter()
                .any(|quad| quad.border_color == strong && quad.border_widths.left.0 > 0.),
            "no strong hairline around the checked segment"
        );

        // The last segment ends just inside the track's right edge.
        let bounds = cx.debug_bounds("track").expect("track drawn");
        let last = point(
            bounds.right() - gpui::px(8.),
            bounds.origin.y + bounds.size.height / 2.,
        );
        cx.simulate_click(last, Modifiers::none());
        cx.run_until_parked();
        let state = |cx: &mut VisualTestContext| {
            cx.update(|_, cx| {
                let sample = sample.read(cx);
                (sample.current, sample.picks)
            })
        };
        assert_eq!(state(cx), (2, 1));

        // A click on the checked segment keeps it and picks nothing.
        cx.simulate_click(last, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(state(cx), (2, 1));
    }
}
