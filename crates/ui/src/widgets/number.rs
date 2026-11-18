//! Number fields: the kit `NumberInput` over an `InputState` whose number
//! lives in section state.

use gpui::{App, AppContext as _, Entity, Focusable as _, Window};
use gpui_kit::component::input::InputState;

/// The state of a kit `NumberInput` whose arrows and Up/Down keys step by
/// `step` and clamp to `min..=max`. Each step emits `InputEvent::Change`.
pub fn number_state(
    min: f64,
    max: f64,
    step: f64,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).min(min).max(max).step(step))
}

/// The number `field` holds; `None` while it is blank or half typed.
pub fn number_value(field: &InputState) -> Option<f64> {
    field
        .unmask_value()
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Show `value` with `precision` decimals in `field`, unless the field
/// already holds that number or is being typed in. Call it from render: it
/// writes only when the section's number changed elsewhere (an import, a
/// config, a reset), and writing emits no `InputEvent::Change`.
pub fn show_number(
    field: &Entity<InputState>,
    value: f64,
    precision: usize,
    window: &mut Window,
    cx: &mut App,
) {
    if field.focus_handle(cx).is_focused(window) || number_value(field.read(cx)) == Some(value) {
        return;
    }
    let text = format!("{value:.precision$}");
    field.update(cx, |input, cx| input.set_value(text, window, cx));
}

#[cfg(test)]
mod tests {
    use super::{number_state, number_value, show_number};
    use gpui::{Context, Entity, IntoElement, Render, Subscription, TestAppContext, Window};
    use gpui_kit::component::input::{InputEvent, InputState, NumberInput};

    /// A view owning one number the field mirrors, as a section state does.
    struct Count {
        value: f64,
        field: Entity<InputState>,
        edits: usize,
        _subscription: Subscription,
    }

    impl Render for Count {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            show_number(&self.field, self.value, 0, window, cx);
            NumberInput::new(&self.field)
        }
    }

    fn count(cx: &mut TestAppContext) -> (Entity<Count>, &mut gpui::VisualTestContext) {
        cx.update(gpui_kit::init);
        cx.add_window_view(|window, cx| {
            let field = number_state(0.0, 10.0, 1.0, window, cx);
            let subscription = cx.subscribe(&field, |count: &mut Count, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    count.value = value;
                    count.edits += 1;
                    cx.notify();
                }
            });
            Count {
                value: 3.0,
                field,
                edits: 0,
                _subscription: subscription,
            }
        })
    }

    #[gpui::test]
    fn the_field_shows_the_number_and_reports_typing(cx: &mut TestAppContext) {
        let (view, cx) = count(cx);
        let field = view.read_with(cx, |view, _| view.field.clone());
        assert_eq!(field.read_with(cx, |field, _| field.value()), "3");
        // Showing the number is not an edit.
        assert_eq!(view.read_with(cx, |view, _| view.edits), 0);

        cx.update(|window, cx| field.update(cx, |field, cx| field.focus(window, cx)));
        cx.simulate_input("7");
        assert_eq!(view.read_with(cx, |view, _| view.value), 37.0);
    }

    #[gpui::test]
    fn a_number_changed_elsewhere_reaches_the_field_once_it_loses_focus(cx: &mut TestAppContext) {
        let (view, cx) = count(cx);
        let field = view.read_with(cx, |view, _| view.field.clone());
        cx.update(|window, cx| field.update(cx, |field, cx| field.focus(window, cx)));
        view.update(cx, |view, cx| {
            view.value = 5.0;
            cx.notify();
        });
        cx.run_until_parked();
        // Typing is never overwritten.
        assert_eq!(field.read_with(cx, |field, _| field.value()), "3");

        cx.update(|window, cx| window.blur(cx));
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(field.read_with(cx, |field, _| field.value()), "5");
        assert_eq!(view.read_with(cx, |view, _| view.edits), 0);
    }

    #[gpui::test]
    fn blank_and_half_typed_fields_hold_no_number(cx: &mut TestAppContext) {
        let (_, cx) = count(cx);
        let holds = |text: &'static str, cx: &mut gpui::VisualTestContext| {
            cx.update(|window, cx| {
                let field = number_state(0.0, 10.0, 1.0, window, cx);
                field.update(cx, |field, cx| field.set_value(text, window, cx));
                number_value(field.read(cx))
            })
        };
        assert_eq!(holds("", cx), None);
        assert_eq!(holds("-", cx), None);
        assert_eq!(holds(" 4 ", cx), Some(4.0));
    }
}
