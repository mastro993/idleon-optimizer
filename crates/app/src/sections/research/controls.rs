//! The segmented choice the Research panes share, on the kit `ToggleGroup`.

use gpui::{App, ElementId, SharedString, Window};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};

/// A small segmented single choice under `id`, one segment per
/// `(value, label)` with `current` checked.
///
/// Segment `n` is `<id>.segment-<n>`, the id the bridge gave the segments
/// once two controls shared a pane, which the harness clicks. A click on the
/// checked segment keeps it: the kit clears every flag then, and `on_change`
/// is not called.
pub(crate) fn segmented<V: Copy + PartialEq + 'static>(
    id: &'static str,
    segments: &[(V, &'static str)],
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> ToggleGroup {
    group(
        id,
        |ix| SharedString::from(format!("{id}.segment-{ix}")).into(),
        segments,
        current,
        on_change,
    )
}

/// [`segmented`] whose segments keep the bare `segment-<n>` ids, for the
/// one control alone on its pane (the Decision Tree's mode switch), which the
/// harness clicks by those ids.
pub(crate) fn segmented_bare<V: Copy + PartialEq + 'static>(
    id: &'static str,
    segments: &[(V, &'static str)],
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> ToggleGroup {
    group(
        id,
        |ix| ("segment", ix).into(),
        segments,
        current,
        on_change,
    )
}

fn group<V: Copy + PartialEq + 'static>(
    id: &'static str,
    segment_id: impl Fn(usize) -> ElementId,
    segments: &[(V, &'static str)],
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> ToggleGroup {
    let values: Vec<V> = segments.iter().map(|(value, _)| *value).collect();
    ToggleGroup::new(id)
        .segmented()
        .single()
        .outline()
        .small()
        .children(segments.iter().enumerate().map(|(ix, (value, label))| {
            Toggle::new(segment_id(ix))
                .label(*label)
                .checked(*value == current)
        }))
        .on_click(move |checked, window, cx| {
            if let Some(ix) = checked.iter().position(|checked| *checked) {
                on_change(values[ix], window, cx);
            }
        })
}
