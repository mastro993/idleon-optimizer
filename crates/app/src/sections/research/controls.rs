//! The segmented choice the Research panes share, on the kit `ToggleGroup`.

use gpui::{App, ElementId, Window};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};

/// A small segmented single choice under `id`, one segment per
/// `(value, label)` with `current` checked.
///
/// Each segment keeps Ely's `segment-<n>` id under the group's, so the
/// harness still clicks `<id>.segment-<n>`. A click on the checked segment
/// keeps it: the kit clears every flag then, and `on_change` is not called.
pub(crate) fn segmented<V: Copy + PartialEq + 'static>(
    id: impl Into<ElementId>,
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
            Toggle::new(("segment", ix))
                .label(*label)
                .checked(*value == current)
        }))
        .on_click(move |checked, window, cx| {
            if let Some(ix) = checked.iter().position(|checked| *checked) {
                on_change(values[ix], window, cx);
            }
        })
}
