//! The search field: the kit `Input` with a magnifier and a clear button.

use gpui::Entity;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Icon, IconName};

/// A search field over `state`: a magnifier before the text, and a clear
/// button once there is text. Name it with `aria_label`.
pub fn search_input(state: &Entity<InputState>) -> Input {
    Input::new(state)
        .prefix(Icon::new(IconName::Search))
        .cleanable(true)
}

#[cfg(test)]
mod tests {
    use super::search_input;
    use gpui::{AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, Window};
    use gpui_kit::component::input::InputState;

    struct Field(Entity<InputState>);

    impl Render for Field {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            search_input(&self.0).aria_label("Find")
        }
    }

    #[gpui::test]
    fn typing_fills_the_search_field(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (field, cx) = cx.add_window_view(|window, cx| {
            Field(cx.new(|cx| InputState::new(window, cx).placeholder("Search…")))
        });
        let state = field.read_with(cx, |field, _| field.0.clone());
        cx.update(|window, cx| state.update(cx, |state, cx| state.focus(window, cx)));
        cx.simulate_input("w5");
        assert_eq!(state.read_with(cx, |state, _| state.value()), "w5");
    }
}
