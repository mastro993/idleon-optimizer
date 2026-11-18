//! The controls the Research panes share: the segmented choice on the kit
//! `ToggleGroup`, and the number field with Ely's stepper buttons.

use gpui::{App, ElementId, Entity, SharedString, Window};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{
    Button, ButtonVariants as _, Toggle, ToggleGroup, ToggleVariants as _,
};
use gpui_kit::component::input::{InputState, NumberInput};
use std::rc::Rc;

/// A small segmented single choice under `id`, one segment per
/// `(value, label)` with `current` checked.
///
/// Segment `n` is `<id>.segment-<n>`, the id the bridge gave Ely's segments
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

/// [`segmented`] whose segments keep Ely's bare `segment-<n>` ids, for the
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

/// A kit `NumberInput` over `field` whose −/+ buttons keep the Ely stepper's
/// `<id>-less` and `<id>-more` ids, which the harnesses click.
///
/// A press moves `value` by `step`, rounded to `decimals`, through `set`: the
/// section setter, which clamps and stores it. The field catches up through
/// `show_number` on the next frame, as after any other change.
pub(crate) fn stepper(
    id: &str,
    field: &Entity<InputState>,
    (value, step, decimals): (f64, f64, usize),
    set: impl Fn(f64, &mut Window, &mut App) + 'static,
) -> NumberInput {
    let set = Rc::new(set);
    let scale = 10_f64.powi(decimals as i32);
    let button = |suffix: &str, icon: IconName, next: f64| {
        let set = set.clone();
        let next = (next * scale).round() / scale;
        Button::new(SharedString::from(format!("{id}-{suffix}")))
            .icon(icon)
            .ghost()
            .xsmall()
            .on_click(move |_event, window, cx| set(next, window, cx))
    };
    NumberInput::new(field)
        .stepper(false)
        .prefix(button("less", IconName::Minus, value - step))
        .suffix(button("more", IconName::Plus, value + step))
}
