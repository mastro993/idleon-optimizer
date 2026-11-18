//! The window: shell, dashboard, shared section chrome and the automation bridge.

pub(crate) mod assets;
#[cfg(feature = "automation")]
pub(crate) mod bridge;
pub(crate) mod dashboard;
pub(crate) mod export;
pub(crate) mod header;
pub(crate) mod import;
pub(crate) mod layout;
pub(crate) mod mark;
pub(crate) mod section;
pub(crate) mod shell;
pub(crate) mod sidebar;
pub(crate) mod theme;
pub(crate) mod views;

use crate::state::AppState;
use ely_gpui_component::forms::TextInput;
use gpui::{
    Action, App, AppContext as _, Context, Entity, IntoElement, KeyBinding,
    KeyBindingContextPredicate, Render, Window,
};
use gpui_kit::component::input::InputState;
use idleon_ui::formula::FormulaTree;
use std::rc::Rc;

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::shell(self, window, cx)
    }
}

/// A key binding in `context`. `secondary-` resolves against the host OS (Cmd on
/// macOS, Ctrl elsewhere), which `KeyBinding::new` refuses since gpui 0.3.10.
pub(crate) fn key_binding<A: Action>(
    keystrokes: &str,
    action: A,
    context: &str,
    cx: &App,
) -> KeyBinding {
    let predicate = KeyBindingContextPredicate::parse(context).expect("a valid key context");
    KeyBinding::load(
        keystrokes,
        Box::new(action),
        Some(Rc::new(predicate)),
        false,
        None,
        cx,
    )
    .expect("a valid keystroke")
}

/// A formula tree whose element ids start with `id`; its owner replaces the
/// root each render.
pub(crate) fn formula_tree(
    id: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FormulaTree> {
    cx.new(|cx| FormulaTree::new(id, window, cx))
}

/// A one-line text field with its placeholder and accessible label.
pub(crate) fn text_input(
    placeholder: &'static str,
    label: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TextInput> {
    cx.new(|cx| {
        TextInput::new(window, cx)
            .placeholder(placeholder)
            .label(label)
    })
}

/// A one-line kit text field showing `placeholder` while empty. The render
/// line names it with `Input::aria_label`.
pub(crate) fn input_state(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}
