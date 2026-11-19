//! The segmented choice the Research panes share, on the app's
//! [`segmented`](crate::ui::segmented::segmented) control.

use crate::ui::segmented::Segmented;
use gpui::{App, ElementId, SharedString, Window};

/// A small segmented single choice under `id`, one segment per
/// `(value, label)` with `current` checked.
///
/// Segment `n` is `<id>.segment-<n>`, the id the bridge gave the segments
/// once two controls shared a pane, which the harness clicks.
pub(crate) fn segmented<V: Copy + PartialEq + 'static>(
    id: &'static str,
    segments: &[(V, &'static str)],
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> Segmented {
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
) -> Segmented {
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
) -> Segmented {
    crate::ui::segmented::segmented(
        id,
        segments
            .iter()
            .enumerate()
            .map(|(ix, (value, label))| (*value, segment_id(ix), *label)),
        current,
        on_change,
    )
}
